//! (HUD2) KH3'S ENEMY (SCAN) GAUGE FED THE GAME'S OWN HP, as the Elden Ring host now feeds it (kh-er `Enemy::hp_game`):
//! damage sequences frame by frame. The green bar (stock squares, then the top bar's frame) drops at once to the new
//! value and never rises without a heal; the red part waits KH3's 800 ms (`FollowGauge.DELAY_TIME`) and then follows in
//! 33.3 ms steps of (target - start) x 2 / 10 (HudRight's FollowGauge has scale 2): 5 steps. Needs the UI pack.

use kh_ui::hud_side::{EnemyGauge, EnemyValues, HudRight, SideStrings};
use kh_ui::{Fonts, Pack};

const DT: f32 = 1.0 / 60.0;
const RED: &str = "hudRight.enmHpPack.enmHpSetAll.enmHpSet.enmHp.enmHpGgR";

struct Run {
    right: HudRight,
    gauge: EnemyGauge,
    /// (frame, stock, green, red) per frame
    trace: Vec<(usize, i32, u32, u32)>,
}

impl Run {
    fn new(pack: &mut Pack) -> Run {
        let fonts = Fonts::load(pack).unwrap();
        let side = SideStrings::load(pack);
        let right = HudRight::new(pack.movie("HudRight").unwrap(), fonts, &side);
        Run { right, gauge: EnemyGauge::default(), trace: Vec::new() }
    }

    fn frames(&mut self, key: u64, hp: i32, max: i32, n: usize) {
        for _ in 0..n {
            let values_of = |k: u64| (k == key).then_some(EnemyValues { hp, hp_max: max, large: false, alive: true });
            self.gauge.point_at(key, false, None);
            self.gauge.update(&mut self.right, DT, &values_of, &mut Vec::new());
            self.right.tick(DT);
            let (stock, green) = self.right.enemy_gauge_state();
            let red = self.right.movie.current_frame(RED).unwrap_or(0);
            self.trace.push((self.trace.len(), stock, green, red));
        }
    }

    fn last(&self) -> (i32, u32, u32) {
        let t = self.trace.last().unwrap();
        (t.1, t.2, t.3)
    }
}

fn pack() -> Option<Pack> {
    Pack::open_default().map_err(|_| eprintln!("SKIPPED: no build/sora_pack/ui")).ok()
}

/// The green bar's level: the stock squares first (a bar that empties shows the next one full).
fn level(t: &(usize, i32, u32, u32)) -> i64 {
    t.1 as i64 * 1000 + t.2 as i64
}

#[test]
fn hud2_single_hit_drops_at_once_and_the_red_follows_after_800_ms() {
    let Some(mut pack) = pack() else { return };
    let mut r = Run::new(&mut pack);
    r.frames(1, 2169, 2169, 30);
    let before = r.last();
    assert_eq!(before.1, before.2, "full: no red");
    r.frames(1, 2099, 2169, 1);
    let hit = r.last();
    assert!(hit.1 < before.1 && hit.0 == before.0, "the green drops at the hit's frame: {before:?} -> {hit:?}");
    assert_eq!(hit.2, before.2, "the red stays where the green was");
    let at = r.trace.len() - 1;
    r.frames(1, 2099, 2169, 90);
    let moved = r.trace[at..].iter().position(|t| t.3 != hit.2).expect("the red moves");
    let done = r.trace[at..].iter().position(|t| t.3 == hit.1).expect("the red reaches the green");
    assert!((47..=50).contains(&moved), "the red waits KH3's 800 ms: first move {moved} frames after the hit");
    assert!(done - moved <= 12, "then follows in about 5 steps of 33.3 ms: {} frames", done - moved);
    assert!(r.trace[at..].windows(2).all(|w| level(&w[1]) == level(&w[0])), "the green never moves while nothing changes");
}

#[test]
fn hud2_multi_hits_heal_and_bar_edges_never_raise_the_green() {
    let Some(mut pack) = pack() else { return };
    let mut r = Run::new(&mut pack);
    // a combo landing one bullet a frame across a bar's edge, a hit during the red's wait and one during its follow
    let seq: [(i32, usize); 8] = [(2169, 30), (2099, 1), (2029, 1), (1959, 1), (1959, 20), (1900, 1), (1900, 52), (1850, 70)];
    for (hp, n) in seq {
        r.frames(1, hp, 2169, n);
    }
    assert!(r.trace.windows(2).all(|w| level(&w[1]) <= level(&w[0])), "the green never rises without a heal");
    // the red is never below the green's place in the same bar, and settles on it
    let (stock, green, red) = r.last();
    assert_eq!((stock, red), (9, green), "settled: 1850 = 9 squares + 50 of 200");
    // a heal: the red is set to the green at once (FollowGauge.setGauge: a rise stops the follow)
    r.frames(1, 1700, 2169, 5);
    r.frames(1, 1800, 2169, 1);
    let (_, g, red) = r.last();
    assert_eq!(red, g, "a heal leaves no red part");
}

#[test]
fn hud2_switching_targets_starts_the_new_one_clean() {
    let Some(mut pack) = pack() else { return };
    let mut r = Run::new(&mut pack);
    r.frames(1, 1500, 2169, 30);
    r.frames(1, 1400, 2169, 10); // red waiting on target 1
    r.frames(2, 600, 900, 1); // the gauge points at target 2: setEHPMax, no red carried over
    let (stock, green, red) = r.last();
    assert_eq!((stock, red), (2, green), "the new target: its own HP, no red from the last one");
}
