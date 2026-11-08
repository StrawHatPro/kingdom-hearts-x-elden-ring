//! (BOSS-BARS, THE USER 2026-11-05: "make it so if a boss has more than 99 health bars, it actually shows the numbert of
//! health bars (no 99+) it must actually show the amount") THE ENEMY GAUGE'S "+N" COUNTER at any count. KH3 holds the
//! HP at 120 bars (`EnemyGauge::update`: unit x 120), so the counter stopped at 99, and its bitmap number has 2 digits.
//! Now: no cap; 1 - 9 and 10 - 99 are KH3's `digit1` / `digit10` layouts as authored; 100 and up = `digit10` with one
//! bitmap digit more per power of ten, the "+" and the plate's left end one digit step (20) further left each.
//! Needs the UI pack (skipped without it).

use kh_ui::hud_side::{AddStockView, EnemyGauge, EnemyValues, HudRight, SideStrings, ENEMY_HP_STOCK_MAX};
use kh_ui::{Fonts, Pack};

const DT: f32 = 1.0 / 60.0;
/// The movie's places (HudRight `McEnmHpAddSSet_60`): the "+" at `digit1` / `digit10`, the plate (`enmHpAddS.@1`).
const PLUS_DIGIT1: f32 = 25.0 - 43.0;
const PLUS_DIGIT10: f32 = 5.0 - 43.0;
const PLATE_X: f32 = 0.0;
/// One digit step of `HudRightEnmHPNum`: width 24, kerning -4.
const STEP: f32 = 20.0;

fn pack() -> Option<Pack> {
    Pack::open_default().map_err(|_| eprintln!("SKIPPED: no build/sora_pack/ui")).ok()
}

struct Boss {
    right: HudRight,
    gauge: EnemyGauge,
    max: i32,
    unit: i32,
    large: bool,
}

impl Boss {
    fn new(pack: &mut Pack, extra: i32, large: bool) -> Boss {
        let fonts = Fonts::load(pack).unwrap();
        let side = SideStrings::load(pack);
        let unit = if large { 2000 } else { 200 };
        let mut b = Boss { right: HudRight::new(pack.movie("HudRight").unwrap(), fonts, &side), gauge: EnemyGauge::default(), max: hp_for(extra, unit), unit, large };
        b.gauge.point_at(1, false, None);
        b.feed(extra, 60);
        b
    }

    /// `frames` frames at the HP that leaves `extra` full bars over the 20 squares (the top bar full).
    fn feed(&mut self, extra: i32, frames: usize) {
        let (hp, max, large) = (hp_for(extra, self.unit), self.max, self.large);
        for _ in 0..frames {
            self.gauge.update(&mut self.right, DT, &|_: u64| Some(EnemyValues { hp, hp_max: max, large, alive: true }), &mut Vec::new());
            self.right.tick(DT);
        }
    }

    fn view(&self) -> AddStockView {
        self.right.add_stock_view().expect("the counter's clips")
    }
}

fn hp_for(extra: i32, unit: i32) -> i32 {
    (ENEMY_HP_STOCK_MAX + extra + 1) * unit
}

fn decimal(n: i32) -> Vec<i32> {
    n.to_string().bytes().map(|b| (b - b'0') as i32).collect()
}

/// What KH3's layout (1 - 2 digits) or its extension (3+) must show for `n` extra bars.
fn check(v: &AddStockView, n: i32, what: &str) {
    let len = decimal(n).len() as i32;
    assert!(v.shown, "{what}: the counter is shown");
    assert_eq!(v.number, n, "{what}: the exact count, never 99 or 99+");
    assert_eq!(v.digits, decimal(n), "{what}: the digits drawn, most significant first");
    assert_eq!(v.label, if n < 10 { "digit1" } else { "digit10" }, "{what}: the movie's layout frame");
    assert_eq!(v.num_digit, len.max(2), "{what}: bitmap digits (KH3's 2 up to 99)");
    let extra = (len - 2).max(0) as f32;
    let plus = if n < 10 { PLUS_DIGIT1 } else { PLUS_DIGIT10 - STEP * extra };
    assert_eq!(v.plus_x, plus, "{what}: the '+' place");
    assert_eq!((v.plate_x, v.plate_by), (PLATE_X - STEP * extra, STEP * extra), "{what}: the plate's place and widening");
    // right-aligned, 20 apart: the last digit stays where KH3 puts it, the others go left
    let want_x: Vec<f32> = (0..len).map(|i| -STEP * (len - 1 - i) as f32).collect();
    assert_eq!(v.digit_x, want_x, "{what}: the digits' places");
}

#[test]
fn boss_bars_counts_show_exactly_with_kh3_layouts_up_to_99_and_the_extension_beyond() {
    let Some(mut pack) = pack() else { return };
    for n in [1, 9, 10, 99, 100, 150, 999, 1000, 1234, 10_000] {
        let b = Boss::new(&mut pack, n, false);
        check(&b.view(), n, &format!("+{n}"));
        assert_eq!(b.right.enemy_gauge_state().0, ENEMY_HP_STOCK_MAX + n, "+{n}: stock = 20 squares + the counter");
    }
}

#[test]
fn boss_bars_count_down_through_100_99_10_9_1_switching_layouts_cleanly() {
    let Some(mut pack) = pack() else { return };
    let mut b = Boss::new(&mut pack, 1234, false);
    check(&b.view(), 1234, "start");
    for n in (0..1234).rev() {
        b.feed(n, if [1000, 999, 100, 99, 10, 9, 1].contains(&n) { 40 } else { 1 });
        if n == 0 {
            let v = b.view();
            assert!(!v.shown, "20 bars left: no counter");
        } else {
            check(&b.view(), n, &format!("countdown at +{n}"));
        }
    }
    // a heal back over 100 (the layout extends again), then down to 1 at once
    b.feed(150, 30);
    check(&b.view(), 150, "healed to +150");
    b.feed(1, 30);
    check(&b.view(), 1, "down to +1 at once: KH3's digit1 layout exactly");
}

#[test]
fn boss_bars_no_cap_anywhere_huge_counts_and_the_large_gauge() {
    let Some(mut pack) = pack() else { return };
    // KH3's cap was 120 bars (99 over the squares): far beyond it, the whole count
    let b = Boss::new(&mut pack, 10_000_000, false);
    check(&b.view(), 10_000_000, "+10,000,000 (2,000,004,200 HP)");
    let b = Boss::new(&mut pack, 999, true);
    check(&b.view(), 999, "large gauge (2000 HP a bar) +999");
    let b = Boss::new(&mut pack, 100, true);
    check(&b.view(), 100, "large gauge +100 (KH3 capped it at 240,000 HP)");
}

/// No code path between Elden Ring's HP and the counter clamps the count: the host's gauge feed (kh-er), the HUD's
/// (kh-host) and the gauge's own split (kh-ui) carry the HP unclamped.
#[test]
fn boss_bars_no_clamp_in_the_chain_source() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let read = |p: &str| std::fs::read_to_string(root.join(p)).unwrap_or_else(|e| panic!("{p}: {e}"));
    let clamps = |line: &str| [".min(", ".clamp(", "99", "* 120"].iter().any(|k| line.contains(k));
    let er = read("kh-er/src/hud.rs");
    let feed: Vec<&str> = er.lines().filter(|l| l.contains("TargetView { center")).collect();
    assert!(!feed.is_empty() && feed.iter().all(|l| !clamps(l)), "kh-er's gauge feed: {feed:?}");
    let host = read("kh-host/src/hud.rs");
    let values: Vec<&str> = host.lines().filter(|l| l.contains("EnemyValues {")).collect();
    assert!(!values.is_empty() && values.iter().all(|l| !clamps(l)), "kh-host's EnemyValues: {values:?}");
    let ui = read("kh-ui/src/hud_side.rs");
    let update = ui.split("pub fn update(&mut self, hud: &mut HudRight").nth(1).and_then(|s| s.split("fn send_hp").next()).expect("EnemyGauge::update");
    assert!(!update.contains("* 120") && !update.contains("clamp(0, cap)"), "EnemyGauge::update holds no 120-bar cap");
    let counter = ui.split("pub fn set_add_stock_num").nth(1).and_then(|s| s.split("\n    }\n").next()).expect("set_add_stock_num");
    assert!(!counter.contains("99") && !counter.contains(".min("), "set_add_stock_num holds no cap: {counter}");
}
