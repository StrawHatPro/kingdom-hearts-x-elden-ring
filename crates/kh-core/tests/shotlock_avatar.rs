//! (SHOT-C avatar) Ghost Horde (Favorite Deputy, kind 8) and Phantom Rush (Classic Tone, kind 25): update 0x1413B5E30
//! (magic/shotlocks/avatar.rs). Trace per kind: lock-on -> fire -> M_1454 -> the avatars (one per locked target: the
//! first 0.133 s after the clip, then every 0.3 s; the world slowed to 0.1) -> 0.5 s -> M_1456 and the rush (one
//! projectile per lock at its target, SF_AVATARRUSH / SF_TRAVATARRUSH) -> ShootFlowEnd -> back to normal.

mod shotlocks_common;

use kh_core::{AnimId, EventKind, StateId, TargetId};
use shotlocks_common::{dummy, level99_with, seconds, Bench};

struct Seen {
    m1454_end: usize,
    avatars: Vec<usize>,
    slowed: bool,
    rush: usize,
    end_clip: usize,
    ended: usize,
    classes: Vec<&'static str>,
}

fn run(key: &str, want_kind: u8, max_locks: i32, row: &str, class: &str) {
    let mut b = Bench::new(level99_with(key), &[dummy(1, 800.0, 0.0), dummy(2, 800.0, 150.0)]);
    let kind = b.aim_until(6, 300);
    assert_eq!(kind, want_kind, "the base kind of {key}");
    assert_eq!(b.sora.aim().max_locks, max_locks, "TresShootFlowData Param0");
    let n = b.sora.aim().total as usize;
    let targets = b.sora.aim().locks.len();
    b.fire();
    let mut s = Seen { m1454_end: 0, avatars: Vec::new(), slowed: false, rush: 0, end_clip: 0, ended: 0, classes: Vec::new() };
    let mut last_avatars = 0;
    // (SHOT-FIX) the attack camera after each avatar (mode 2 on its target, anchored), off at sub 5; the rush spawned at
    // once (one projectile per lock, each at its target) and fired lock by lock.
    let locked: Vec<TargetId> = b.sora.aim().locks.iter().map(|l| l.0).collect();
    let (mut cam_events, mut cam_on_each_avatar, mut cam_off_at_rush) = (0, true, false);
    let mut rush_spawned_at_once = None;
    let mut rush_rides = false;
    let mut hit_steps: Vec<usize> = Vec::new();
    for k in 1..900 {
        let o = b.step(&[]);
        let count = b.sora.magic().shot_avatars.len();
        cam_events += o.events.iter().filter(|e| matches!(e.kind, EventKind::AttackCamera { mode: 2, .. })).count();
        if s.m1454_end == 0 && o.sub_step == 1 {
            s.m1454_end = k;
        }
        if count > last_avatars {
            s.avatars.push(k);
            let cam = b.sora.combat().attack_cam;
            cam_on_each_avatar &= cam.mode == 2 && locked.contains(&cam.target) && cam.anchor.is_some();
        }
        last_avatars = count;
        if count > 0 && b.sora.time_dilation(2) < 0.2 {
            s.slowed = true;
        }
        if s.rush == 0 && o.anim.name == AnimId::M1456 {
            s.rush = k;
            cam_off_at_rush = b.sora.combat().attack_cam.mode == 0;
            let views: Vec<_> = b.sora.magic().projectile_views().collect();
            rush_spawned_at_once = Some((views.len(), views.iter().all(|v| b.host.targets.iter().any(|t| (t.center - v.position).length() < 1.0))));
            // the target walks on: the waiting projectiles ride on it (attached, KeepWorldTransform)
            for t in b.host.targets.iter_mut().filter(|t| t.id == TargetId(2)) {
                t.position.x += 60.0;
                t.center.x += 60.0;
            }
        }
        if s.rush > 0 && k == s.rush + 1 {
            let c = b.host.targets.iter().find(|t| t.id == TargetId(2)).unwrap().center;
            rush_rides = b.sora.magic().projectile_views().any(|v| (v.position - c).length() < 1.0);
        }
        for _ in hit_steps.len()..b.hits().len() {
            hit_steps.push(k);
        }
        if s.end_clip == 0 && o.anim.name == AnimId::ShootFlowEnd {
            s.end_clip = k;
        }
        for p in b.sora.magic().projectile_views() {
            if !s.classes.contains(&p.class) {
                s.classes.push(p.class);
            }
        }
        if o.state != StateId::Shootflow {
            s.ended = k;
            break;
        }
    }
    assert!(s.ended > 0, "the state ended");
    // M_1454 (1.7667 s: sub 1 from its end), then one avatar per locked target: 0.1333 s after, then every 0.3 s.
    assert!((seconds(s.m1454_end) - 1.7667).abs() < 0.05, "M_1454 ended at {}", seconds(s.m1454_end));
    assert_eq!(s.avatars.len(), targets, "one avatar per locked target");
    assert!((seconds(s.avatars[0] - s.m1454_end) - 0.1333).abs() < 0.05, "first avatar {} after the clip", seconds(s.avatars[0] - s.m1454_end));
    for w in s.avatars.windows(2) {
        assert!((seconds(w[1] - w[0]) - 0.3).abs() < 0.05, "avatar interval {}", seconds(w[1] - w[0]));
    }
    assert!(s.slowed, "the world slowed to 0.1 while the avatars run");
    // The last avatar arrives 0.1667 s after its spawn; 0.5 s later the rush (M_1456).
    let last = *s.avatars.last().unwrap();
    assert!((seconds(s.rush - last) - (0.1667 + 0.5)).abs() < 0.08, "rush {} after the last avatar", seconds(s.rush - last));
    assert!(b.sora.magic().shot_avatars.is_empty(), "the avatars are off");
    assert!((seconds(s.end_clip - s.rush) - 1.7).abs() < 0.05, "M_1456 lasted {}", seconds(s.end_clip - s.rush));
    assert!((seconds(s.ended - s.end_clip) - 0.8).abs() < 0.05, "ShootFlowEnd lasted {}", seconds(s.ended - s.end_clip));
    assert_eq!(s.classes, vec![class], "the rush projectile");
    // (SHOT-FIX) the camera calls of 0x1413BD290 / sub 5, and the rush of 0x1413BF420: all spawned together, delayed fire.
    assert!(cam_on_each_avatar && cam_events == targets, "attack camera mode 2 on each avatar's target ({cam_events} events)");
    assert!(cam_off_at_rush, "the attack camera off at sub 5");
    assert_eq!(rush_spawned_at_once, Some((n, true)), "every rush projectile spawned at once, at its target");
    assert!(rush_rides, "a waiting rush projectile moves with its target");
    for k in s.ended + 1..s.ended + 121 {
        b.step(&[]);
        for _ in hit_steps.len()..b.hits().len() {
            hit_steps.push(k);
        }
    }
    let first = hit_steps.iter().copied().min().unwrap();
    assert_eq!(hit_steps.iter().filter(|&&k| k == first).count(), targets, "the first lock of each target fires at once (delay 0)");
    assert!(hit_steps.iter().any(|&k| k > first), "the other locks fire later (0.033333 s x the lock's index)");
    let hits = b.hits();
    assert_eq!(hits.len(), n, "one rush hit per lock");
    assert!(hits.iter().all(|h| h.attack_name == row && h.flags == 0xFD | (want_kind as u32) << 8), "{:?}", hits.iter().map(|h| (h.attack_name, h.flags)).collect::<Vec<_>>());
    assert!(hits.iter().any(|h| h.target == TargetId(1)) && hits.iter().any(|h| h.target == TargetId(2)));
    b.assert_back_to_normal();
}

#[test]
fn ghost_horde_kind_8() {
    run("WEP_KEYBLADE_SO_03", 8, 12, "SF_AVATARRUSH", "TresProjTS_AvatarRush_C");
}

#[test]
fn phantom_rush_kind_25() {
    run("WEP_KEYBLADE_SO_11", 25, 16, "SF_TRAVATARRUSH", "TresProjTR_AvatarRush_C");
}
