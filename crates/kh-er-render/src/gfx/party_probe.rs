//! (ER party I, the coordinator's request after ER86) EACH FRIEND'S OWN STEADY LIGHT PROBE. Before, Donald and Goofy were
//! lit by Sora's rig: the game's ground measured around SORA'S feet (light.rs probes, ER86's world probe), so a friend
//! standing in shade beside Sora in the sun was lit as the sun.
//!
//! HOW (feature `party` only; Sora's own path is untouched):
//! - The game-data texture gets `ROWS` more rows (light.rs rows 0..7 stay as they are): row 8 = Donald, row 9 = Goofy.
//! - The probe pass draws the SAME probe shader once more per friend, into his row, with HIS frame copy bound (gfx/party.rs
//!   `prepare` writes his feet's pixel and the probe mode into its `er.probe` row; `er.blob` and `er.caps` are already his:
//!   the world probe's taps are around HIS feet and hidden-by-body is tested against HIS capsules).
//! - At the read-back (`read`) each row is parsed as Sora's probe row is (light.rs `parse`, stable.rs `parse_taps`) and
//!   kept with its own TapMemory (ER86's world-direction memory) and its own `stable::Filter` (ER86's easing by HIS speed).
//! - `steady` gives his light for this frame: the world values (sun, sky, exposure, fog: the same for everyone) are
//!   Sora's steady ones; the probe values are his own, eased. `prepare` turns it into his rig (light.rs `rig`, the same
//!   gains as Sora's) and writes his frame copy's light rows; his materials take his rig.
//! - No reading of his yet (the first frames, his feet never in the picture, the game light off): `None` = Sora's light,
//!   as before. `r.party_probes = 0` (kh-er) = Sora's light always.

use std::sync::Mutex;
use std::time::Instant;

use glam::{Mat4, Vec3, Vec4};

use crate::light::{GameLight, HEIGHT, ROW_PROBE, WIDTH};
use crate::stable::{self, Filter, TapMemory};

/// The party's rows after light.rs's: one per drawn KH character of the party (RSF2: given out in the order they are first
/// drawn, any draw id; before, rows 8 / 9 were Donald's and Goofy's by name and any other character had no probe of his own).
pub const ROWS: usize = 4;

/// (RSF2) which draw id each party row belongs to (kept once given; `reset` clears it)
static SLOTS: Mutex<Vec<u8>> = Mutex::new(Vec::new());

/// `r.party_probes` (crate::party::probes).
pub fn enabled() -> bool {
    crate::party::probes()
}

/// The game-data texture row of a party draw: the row given to that draw id (RSF2: the first free one when it is first drawn,
/// whoever he is); None for the team props (no body) and when every row is taken.
pub fn row_of(chara: u8) -> Option<usize> {
    if chara == crate::party::TEAM || chara == 0 {
        return None;
    }
    let mut slots = SLOTS.lock().unwrap_or_else(|e| e.into_inner());
    let k = match slots.iter().position(|c| *c == chara) {
        Some(k) => k,
        None if slots.len() < ROWS => {
            slots.push(chara);
            slots.len() - 1
        }
        None => return None,
    };
    Some(HEIGHT + k)
}

/// (RSF2) The draw id of party row `k` (0-based after light.rs's rows), when given.
fn chara_of_row(k: usize) -> Option<u8> {
    SLOTS.lock().ok().and_then(|s| s.get(k).copied())
}

/// (RSF2, the offline harness only: it has no game-data read-back) a friend's probe reading as the read-back gives it.
pub fn set_test(chara: u8, light: GameLight) {
    with_one(chara, |o| o.raw = Some(light));
}

/// His probe constant (`er.probe`) as gfx.rs makes Sora's: his feet's pixel (kept off the picture's edges), the screen
/// spread (`w = 1`), or the world probe's radius (`w = 2`) when `world` is Some(radius cm); zero when his feet are behind
/// the camera. `viewport` = (width, height), `tan_y` = the vertical half-angle's tangent.
pub fn probe_at(clip: &Mat4, feet: Vec3, viewport: (f32, f32), tan_y: f32, world: Option<f32>) -> Vec4 {
    let f = *clip * feet.extend(1.0);
    if !(f.w > 0.3) || !f.is_finite() {
        return Vec4::ZERO;
    }
    let (w, h) = viewport;
    let x = ((f.x / f.w * 0.5 + 0.5) * w).clamp(0.04 * w, 0.96 * w);
    let y = ((0.5 - f.y / f.w * 0.5) * h).clamp(0.04 * h, 0.96 * h);
    match world {
        Some(r) => Vec4::new(x, y, r.clamp(20.0, 400.0), 2.0),
        None => Vec4::new(x, y, (0.35 / (f.w * tan_y.max(1.0e-4)) * h * 0.5).clamp(2.0, 200.0), 1.0),
    }
}

/// One party row of the read-back as a frame of light.rs's layout (that row in Sora's probe row, the rest zero), so that
/// light.rs `parse` and stable.rs `parse_taps` read it as they read his.
pub fn as_probe_frame(party_rows: &[f32], k: usize) -> Vec<f32> {
    let mut words = vec![0.0f32; WIDTH * HEIGHT * 4];
    if let Some(row) = party_rows.get(k * WIDTH * 4..(k + 1) * WIDTH * 4) {
        words[ROW_PROBE * WIDTH * 4..(ROW_PROBE + 1) * WIDTH * 4].copy_from_slice(row);
    }
    words
}

/// The probe values of `mine` over the world values of `base`.
pub fn with_probes(base: &GameLight, mine: &GameLight) -> GameLight {
    let mut out = *base;
    (out.scene_px, out.direct_px, out.albedo_px, out.mask, out.taps) = (mine.scene_px, mine.direct_px, mine.albedo_px, mine.mask, mine.taps);
    (out.ring, out.ring_at) = (mine.ring, mine.ring_at);
    out
}

/// One friend's probe state.
#[derive(Default)]
struct One {
    chara: u8,
    /// The latest read-back of his row (probe values only meaningful).
    raw: Option<GameLight>,
    memory: TapMemory,
    memory_at: Option<Instant>,
    filter: Filter,
    at: Option<Instant>,
    /// His feet when his probe was last drawn (the memory's movement rule).
    feet: Option<Vec3>,
    /// ER86 off: the old easing's last light.
    old: Option<GameLight>,
}

static STATE: Mutex<Vec<One>> = Mutex::new(Vec::new());

fn with_one<R>(chara: u8, f: impl FnOnce(&mut One) -> R) -> R {
    let mut v = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let k = match v.iter().position(|o| o.chara == chara) {
        Some(k) => k,
        None => {
            v.push(One { chara, ..One::default() });
            v.len() - 1
        }
    };
    f(&mut v[k])
}

/// (gfx.rs, the read-back) The party rows of that frame (`ROWS` rows of `WIDTH` texels, floats). `size` = the scene
/// target's size (light.rs `parse`).
pub fn read(party_rows: &[f32], size: (f32, f32)) {
    for k in 0..ROWS {
        // (RSF2) the row's own character (any draw id), not Donald / Goofy by name
        let Some(chara) = chara_of_row(k) else { continue };
        let words = as_probe_frame(party_rows, k);
        let mine = crate::light::parse(&words, size);
        let taps = stable::parse_taps(&words);
        with_one(chara, |o| {
            let dt = o.memory_at.map_or(0.0, |t| t.elapsed().as_secs_f32());
            o.memory_at = Some(Instant::now());
            if let Some(feet) = o.feet {
                o.memory.update(&taps, feet, dt);
            }
            o.raw = Some(mine);
        });
    }
}

/// (gfx/party.rs `prepare`) HIS STEADY LIGHT this frame: Sora's steady world values (`base`) with his own probes, eased
/// by ER86's rules (`stable::Filter`, his TapMemory) or the old easing with `r.light_stable = 0`. `None` = no reading of
/// his (the caller lights him with Sora's light).
pub fn steady(chara: u8, base: &GameLight, feet: Vec3) -> Option<GameLight> {
    if !enabled() {
        return None;
    }
    let s = stable::settings();
    with_one(chara, |o| {
        o.feet = Some(feet);
        let dt = o.at.map_or(0.0, |t| t.elapsed().as_secs_f32());
        o.at = Some(Instant::now());
        let raw = o.raw?;
        let fresh = s.probe_world && o.memory_at.is_some_and(|t| t.elapsed().as_secs_f32() < 0.5);
        let mine = with_probes(base, &if fresh { o.memory.apply(&raw) } else { raw });
        let eased = if s.on {
            o.filter.step(Some(&mine), Some(feet), dt, &s)?
        } else {
            let e = stable::ease_old(o.old.as_ref(), mine);
            o.old = Some(e);
            e
        };
        // (the world values stay Sora's steady ones: the filter only eases his probes)
        let out = with_probes(base, &eased);
        out.probed().then_some(out)
    })
}

/// Forget every friend's readings (the renderer's shutdown / a device change).
pub fn reset() {
    if let Ok(mut v) = STATE.lock() {
        v.clear();
    }
    if let Ok(mut s) = SLOTS.lock() {
        s.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(words: &mut [f32], row: usize, x: usize, v: [f32; 4]) {
        words[(row * WIDTH + x) * 4..][..4].copy_from_slice(&v);
    }

    #[test]
    fn erpi_a_friends_row_is_read_as_soras_probe_row() {
        // two party rows: Donald in shade (row 0), Goofy in the sun (row 1)
        let mut rows = vec![0.0f32; ROWS * WIDTH * 4];
        for (k, (direct, mask)) in [(0.01f32, 0.0f32), (0.2, 1.0)].into_iter().enumerate() {
            put(&mut rows, k, 1, [direct, direct, direct, 9.0]);
            put(&mut rows, k, 2, [1.0, 1.0, 0.0, mask]);
            put(&mut rows, k, 3, [direct * 1.2, direct * 1.2, direct * 1.2, 9.0]);
            put(&mut rows, k, 4, [0.4, 0.4, 0.4, 9.0]);
        }
        let donald = crate::light::parse(&as_probe_frame(&rows, 0), (1.0, 1.0));
        let goofy = crate::light::parse(&as_probe_frame(&rows, 1), (1.0, 1.0));
        assert!(donald.probed() && goofy.probed());
        assert_eq!((donald.mask.w, goofy.mask.w), (0.0, 1.0));
        assert!(goofy.direct_px.x > donald.direct_px.x * 10.0);
        // the world values are the base's, the probes his
        let base = GameLight { sun: Some((Vec3::Y, Vec3::ONE)), exposure: 2.0, scene: true, ..GameLight::default() };
        let mine = with_probes(&base, &donald);
        assert_eq!((mine.sun, mine.exposure, mine.mask.w, mine.direct_px), (base.sun, 2.0, 0.0, donald.direct_px));
        // his rig: in shade (mask 0) his key is the shadow colour's share; in the sun the full key
        let sunny = GameLight { shadow_color: Vec3::splat(1.0), ..with_probes(&base, &goofy) };
        let shady = GameLight { shadow_color: Vec3::splat(1.0), ..with_probes(&base, &donald) };
        let (rs, rh) = (crate::light::rig(&sunny, Vec3::Y, 1.0, 1.0), crate::light::rig(&shady, Vec3::Y, 1.0, 1.0));
        assert!(rs.key.x * rs.out_scale > rh.key.x * rh.out_scale * 5.0, "sun {:?} shade {:?}", rs.key * rs.out_scale, rh.key * rh.out_scale);
    }

    #[test]
    fn erpi_his_probe_constant_is_soras_rule_at_his_feet() {
        let clip = Mat4::perspective_rh(1.0, 16.0 / 9.0, 0.3, 1000.0) * Mat4::look_at_rh(Vec3::new(0.0, -500.0, 150.0), Vec3::ZERO, Vec3::Z);
        let a = probe_at(&clip, Vec3::ZERO, (1920.0, 1080.0), 0.5, None);
        assert_eq!(a.w, 1.0);
        assert!((a.x - 960.0).abs() < 2.0 && (a.y - 540.0).abs() < 2.0 && (2.0..=200.0).contains(&a.z), "{a:?}");
        let w = probe_at(&clip, Vec3::new(100.0, 0.0, 0.0), (1920.0, 1080.0), 0.5, Some(65.0));
        assert_eq!((w.z, w.w), (65.0, 2.0));
        assert!(w.x > a.x);
        // behind the camera: off
        assert_eq!(probe_at(&clip, Vec3::new(0.0, -2000.0, 0.0), (1920.0, 1080.0), 0.5, None), Vec4::ZERO);
        // (RSF2) a row per drawn character, whoever he is, kept once given; none for the team props
        let (d, g, x) = (row_of(1), row_of(2), row_of(9));
        for r in [d, g, x] {
            assert!(r.is_some_and(|r| (HEIGHT..HEIGHT + ROWS).contains(&r)), "{r:?}");
        }
        assert!(d != g && g != x && d != x);
        assert_eq!((row_of(1), row_of(2), row_of(9), row_of(crate::party::TEAM)), (d, g, x, None));
    }

    #[test]
    fn erpi_steady_eases_his_own_probes_and_keeps_the_world_values() {
        crate::party::set_probes(true);
        let mut rows = vec![0.0f32; ROWS * WIDTH * 4];
        // (RSF2) the row his draw was given
        let k = row_of(1).expect("a row") - HEIGHT;
        put(&mut rows, k, 1, [0.2, 0.2, 0.2, 9.0]);
        put(&mut rows, k, 2, [1.0, 1.0, 0.0, 1.0]);
        put(&mut rows, k, 3, [0.25, 0.25, 0.25, 9.0]);
        put(&mut rows, k, 4, [0.4, 0.4, 0.4, 9.0]);
        let base = GameLight { sun: Some((Vec3::Y, Vec3::ONE)), exposure: 3.0, scene: true, ..GameLight::default() };
        // no reading yet: Sora's light
        assert!(steady(1, &base, Vec3::ZERO).is_none());
        read(&rows, (1.0, 1.0));
        let got = steady(1, &base, Vec3::ZERO).expect("his own light");
        assert_eq!((got.sun, got.exposure), (base.sun, 3.0));
        assert!((got.mask.w - 1.0).abs() < 1.0e-6 && got.direct_px.x > 0.19);
        // switched off: Sora's light
        crate::party::set_probes(false);
        assert!(steady(1, &base, Vec3::ZERO).is_none());
        crate::party::set_probes(true);
    }
}
