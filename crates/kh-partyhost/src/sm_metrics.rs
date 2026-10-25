//! (SM) THE FRIENDS' SMOOTHNESS METRICS (test-only; shared by kh-partyhost `sm_tests` and kh-er's party harness by
//! `#[path]`). Input: one member's DRAWN transform per rendered frame (display time, position, yaw, the two feet) and
//! his simulated position per fixed step. Output: one `Report` row per member and run.
//!
//! - jerk: the drawn position's third derivative over the display times (m/s^3; RMS and 95th percentile), plus the
//!   same on the fixed steps (`sim_jerk`: the simulation's own smoothness, whatever the drawing does);
//! - frame-to-frame error: |drawn displacement of the frame - the smoothed path's displacement over the same display
//!   interval| (cm; RMS / max). The smoothed path = the step positions through a Gaussian (sigma 0.1 s), read one step
//!   late (the interpolated drawing's constant latency, invisible). Also against the leader's drawn place (`rel_*`:
//!   what the camera, which follows the leader, shows);
//! - yaw snaps: drawn frames turning faster than 400 deg/s (KH3's facing turns at 360) or more than 15 deg in one frame;
//!   steps turning more than 400 deg/s (`sim_snaps`; those under a start / turn clip, whose own root turns him, apart);
//! - foot sliding: a foot on the ground (its bone within CONTACT cm of its lowest height over the run, in both frames): its world
//!   speed / the body's speed, while he moves over 100 cm/s (median / 90th percentile, percent);
//! - zig-zag: the drawn path's sideways offset from its slow smoothing (sigma 0.25 s), RMS (cm) and its sign changes of
//!   more than 1.5 cm per 10 s;
//! - velocity flips: drawn frames whose velocity points back against the last frame's (both over 30 cm/s); speed swings
//!   of more than 40 cm/s on the steps per 10 s (`speed_osc`); height pops: a frame's drawn height off the chord of its two
//!   neighbours (cm; max, and frames over 2 cm).

use glam::Vec3;

/// A foot bone within this of its lowest height over the run (its 5th percentile above the actor's origin, cm) stands on
/// the ground (the bones sit at different heights in Donald's and Goofy's feet).
pub const CONTACT: f32 = 4.0;

#[derive(Clone, Copy, Debug, Default)]
pub struct DrawnSample {
    /// Display time (s).
    pub t: f64,
    pub pos: Vec3,
    pub yaw: f32,
    /// L / R foot bone in the world (None: no pose).
    pub feet: Option<[Vec3; 2]>,
    /// The leader's drawn feet this frame (the camera follows him).
    pub leader: Vec3,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SimSample {
    pub t: f64,
    pub pos: Vec3,
    pub yaw: f32,
    pub leader: Vec3,
    /// The yaw is a clip's own root turn this step (a start / turn clip: KH3's animation turns him).
    pub clip_turn: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Report {
    pub jerk_rms: f32,
    pub jerk_p95: f32,
    pub sim_jerk_rms: f32,
    pub err_rms: f32,
    pub err_max: f32,
    pub rel_err_rms: f32,
    pub rel_err_max: f32,
    pub yaw_snaps: u32,
    pub yaw_rate_max: f32,
    pub sim_snaps: u32,
    pub sim_clip_snaps: u32,
    pub slide_median: f32,
    pub slide_p90: f32,
    pub zig_rms: f32,
    pub zig_flips: f32,
    pub vel_flips: u32,
    pub speed_osc: f32,
    pub height_pop: f32,
    pub height_pops: u32,
    pub frames: usize,
    pub secs: f32,
}

fn wrap(a: f32) -> f32 {
    (a + 540.0).rem_euclid(360.0) - 180.0
}

fn pct(v: &mut [f32], p: f32) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    v[((v.len() - 1) as f32 * p).round() as usize]
}

fn rms(v: &[f32]) -> f32 {
    if v.is_empty() {
        0.0
    } else {
        (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt()
    }
}

/// A Gaussian-smoothed copy of evenly spaced points (sigma in samples).
fn smooth(p: &[Vec3], sigma: f32) -> Vec<Vec3> {
    let r = (3.0 * sigma).ceil() as isize;
    let w: Vec<f32> = (-r..=r).map(|k| (-(k as f32).powi(2) / (2.0 * sigma * sigma)).exp()).collect();
    (0..p.len() as isize)
        .map(|i| {
            let (mut s, mut n) = (Vec3::ZERO, 0.0);
            for (j, k) in (-r..=r).enumerate() {
                let x = i + k;
                if x >= 0 && (x as usize) < p.len() {
                    s += p[x as usize] * w[j];
                    n += w[j];
                }
            }
            s / n
        })
        .collect()
}

/// The smoothed path at time `t` (linear between the step samples).
fn at(ts: &[f64], p: &[Vec3], t: f64) -> Vec3 {
    if p.is_empty() {
        return Vec3::ZERO;
    }
    if t <= ts[0] {
        return p[0];
    }
    let i = ts.partition_point(|x| *x <= t);
    if i >= ts.len() {
        return *p.last().unwrap();
    }
    let f = ((t - ts[i - 1]) / (ts[i] - ts[i - 1]).max(1e-9)) as f32;
    p[i - 1].lerp(p[i], f)
}

/// One member's report. `step` = the fixed step (s); `skip` = seconds left out at the start (settling).
pub fn report(drawn: &[DrawnSample], sim: &[SimSample], step: f32) -> Report {
    let mut r = Report { frames: drawn.len(), ..Report::default() };
    if drawn.len() < 4 || sim.len() < 8 {
        return r;
    }
    r.secs = (drawn.last().unwrap().t - drawn[0].t) as f32;
    let ts: Vec<f64> = sim.iter().map(|s| s.t).collect();
    let sp: Vec<Vec3> = sim.iter().map(|s| s.pos).collect();
    let rel: Vec<Vec3> = sim.iter().map(|s| s.pos - s.leader).collect();
    let ref_p = smooth(&sp, 0.1 / step);
    let ref_rel = smooth(&rel, 0.1 / step);
    let slow = smooth(&sp, 0.25 / step);
    let lat = step as f64;
    // frame-to-frame error, jerk, yaw, flips, pops, slide
    let (mut err, mut rel_err, mut jerk, mut slide) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    // each foot's ground height above the origin
    let ground: Vec<f32> = (0..2).map(|i| { let mut h: Vec<f32> = drawn.iter().filter_map(|d| d.feet.map(|f| f[i].z - d.pos.z)).collect(); pct(&mut h, 0.05) + CONTACT }).collect();
    let mut v_prev: Option<(Vec3, f64)> = None;
    let mut a_prev: Option<(Vec3, f64)> = None;
    for k in 1..drawn.len() {
        let (a, b) = (drawn[k - 1], drawn[k]);
        let dt = (b.t - a.t) as f32;
        if dt <= 0.0 {
            continue;
        }
        let d_ref = at(&ts, &ref_p, b.t - lat) - at(&ts, &ref_p, a.t - lat);
        let e = (b.pos - a.pos) - d_ref;
        err.push(e.length());
        let d_rel = at(&ts, &ref_rel, b.t - lat) - at(&ts, &ref_rel, a.t - lat);
        rel_err.push(((b.pos - b.leader) - (a.pos - a.leader) - d_rel).length());
        let v = (b.pos - a.pos) / dt;
        let tm = (a.t + b.t) * 0.5;
        if let Some((vp, tp)) = v_prev {
            let h = Vec3::new(v.x, v.y, 0.0);
            let hp = Vec3::new(vp.x, vp.y, 0.0);
            if h.length() > 30.0 && hp.length() > 30.0 && h.dot(hp) < 0.0 {
                r.vel_flips += 1;
            }
            let acc = (v - vp) / ((tm - tp) as f32).max(1e-4);
            let ta = (tm + tp) * 0.5;
            if let Some((ap, tap)) = a_prev {
                jerk.push(((acc - ap) / ((ta - tap) as f32).max(1e-4)).length() / 100.0);
            }
            a_prev = Some((acc, ta));
        }
        v_prev = Some((v, tm));
        let rate = wrap(b.yaw - a.yaw).abs() / dt;
        r.yaw_rate_max = r.yaw_rate_max.max(rate);
        if rate > 400.0 || wrap(b.yaw - a.yaw).abs() > 15.0 {
            r.yaw_snaps += 1;
        }
        if let (Some(fa), Some(fb)) = (a.feet, b.feet) {
            let body = Vec3::new(v.x, v.y, 0.0).length();
            if body > 100.0 {
                // a foot on the ground in both frames (within 6 cm of the actor's origin height): its world speed
                for i in 0..2 {
                    if fa[i].z - a.pos.z < ground[i] && fb[i].z - b.pos.z < ground[i] {
                        let s = Vec3::new(fb[i].x - fa[i].x, fb[i].y - fa[i].y, 0.0).length() / dt;
                        slide.push(100.0 * s / body);
                    }
                }
            }
        }
    }
    // height pops: a frame's drawn height off the chord of its neighbours (a ramp or a slope: 0; a stair popped up at once:
    // about half the stair)
    for k in 1..drawn.len() - 1 {
        let (a, b, c) = (drawn[k - 1], drawn[k], drawn[k + 1]);
        let span = (c.t - a.t) as f32;
        if span <= 0.0 {
            continue;
        }
        let f = (b.t - a.t) as f32 / span;
        let off = (b.pos.z - (a.pos.z + (c.pos.z - a.pos.z) * f)).abs();
        r.height_pop = r.height_pop.max(off);
        if off > 2.0 {
            r.height_pops += 1;
        }
    }
    r.err_rms = rms(&err);
    r.err_max = err.iter().copied().fold(0.0, f32::max);
    r.rel_err_rms = rms(&rel_err);
    r.rel_err_max = rel_err.iter().copied().fold(0.0, f32::max);
    r.jerk_rms = rms(&jerk);
    r.jerk_p95 = pct(&mut jerk, 0.95);
    r.slide_median = pct(&mut slide.clone(), 0.5);
    r.slide_p90 = pct(&mut slide, 0.9);
    // the steps: jerk, yaw snaps, speed swings
    let mut sj = Vec::new();
    for k in 3..sim.len() {
        let j = sp[k] - sp[k - 1] * 3.0 + sp[k - 2] * 3.0 - sp[k - 3];
        sj.push(j.length() / step.powi(3) / 100.0);
    }
    r.sim_jerk_rms = rms(&sj);
    for k in 1..sim.len() {
        if wrap(sim[k].yaw - sim[k - 1].yaw).abs() / step > 400.0 {
            if sim[k].clip_turn {
                r.sim_clip_snaps += 1;
            } else {
                r.sim_snaps += 1;
            }
        }
    }
    let speed: Vec<f32> = (1..sim.len()).map(|k| Vec3::new(sp[k].x - sp[k - 1].x, sp[k].y - sp[k - 1].y, 0.0).length() / step).collect();
    let mut swings = 0u32;
    let (mut ext, mut dir) = (speed.first().copied().unwrap_or(0.0), 0i8);
    for &s in &speed {
        match dir {
            1 if s > ext => ext = s,
            -1 if s < ext => ext = s,
            1 | -1 if (s - ext).abs() > 40.0 => {
                swings += 1;
                dir = -dir;
                ext = s;
            }
            0 if (s - ext).abs() > 40.0 => {
                dir = if s > ext { 1 } else { -1 };
                ext = s;
            }
            _ => {}
        }
    }
    let secs = (sim.last().unwrap().t - sim[0].t) as f32;
    r.speed_osc = swings as f32 * 10.0 / secs.max(1e-3);
    // zig-zag: the sideways offset from the slow path, its sign changes over 1.5 cm
    let mut lat_off = Vec::new();
    for k in 1..sim.len() - 1 {
        let tan = Vec3::new(slow[k + 1].x - slow[k - 1].x, slow[k + 1].y - slow[k - 1].y, 0.0);
        if tan.length() / (2.0 * step) < 50.0 {
            continue;
        }
        let n = Vec3::new(-tan.y, tan.x, 0.0).normalize();
        lat_off.push((sp[k] - slow[k]).dot(n));
    }
    r.zig_rms = rms(&lat_off);
    let (mut flips, mut sign, mut peak) = (0u32, 0.0f32, 0.0f32);
    for &l in &lat_off {
        if l.abs() > 1.5 && l.signum() != sign {
            if sign != 0.0 && peak > 1.5 {
                flips += 1;
            }
            sign = l.signum();
            peak = 0.0;
        }
        peak = peak.max(l.abs());
    }
    r.zig_flips = flips as f32 * 10.0 / secs.max(1e-3);
    r
}

pub const HEADER: &str = "jerk rms/p95 (m/s3) | sim jerk | f2f err rms/max (cm) | rel-to-leader err rms/max | yaw snaps (max deg/s) | sim snaps (+ in turn clips) | foot slide med/p90 % | zigzag rms cm, flips/10s | vel flips | speed osc /10s | height pop max cm (frames > 2 cm)";

pub fn line(r: &Report) -> String {
    format!(
        "{:7.1}/{:7.1} | {:6.1} | {:5.2}/{:5.1} | {:5.2}/{:5.1} | {:4} ({:5.0}) | {:4} (+{:3}) | {:5.1}/{:5.1} | {:5.2}, {:5.1} | {:4} | {:5.1} | {:5.2} ({:3})",
        r.jerk_rms, r.jerk_p95, r.sim_jerk_rms, r.err_rms, r.err_max, r.rel_err_rms, r.rel_err_max, r.yaw_snaps, r.yaw_rate_max, r.sim_snaps, r.sim_clip_snaps, r.slide_median, r.slide_p90, r.zig_rms, r.zig_flips, r.vel_flips, r.speed_osc, r.height_pop, r.height_pops
    )
}
