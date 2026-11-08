//! (AUDIT-B W-HANDOVER) KH3's `ParticleModuleSQEX_Raycast` (def.rs `RaycastDef`: slot 117 0x144375FC0 per particle, slot 114
//! 0x14434E070 per child of `SQEX_Parent::Update` 0x14437A3B0, the cast 0x14435FCB0, the spawn rule 0x14434A6E0) on the
//! REAL ground-crack effects of the hammer forms and Nano Arms, against a host's `RayWorld`: flat dry ground, water, a
//! slope, a step down, and no host at all (the flat ground of `World::set_ground`).
//! - Nano Arms' giant stamp `vw_so172_jwr080` (8413) and fall `vw_so172_jwr050` (8420), live pack;
//! - Hyper Hammer's Giant Stamp / Drill Punch's finish crack `vw_so052_jwr080` (M_3416, FINISH_DR 6423) and Boom Hammer's /
//!   Clock Drill's `vw_so112_jwr080` (staged by this audit: `AUDIT-B/W-HANDOVER/stage/kbvfx`, not in the live pack yet);
//! - the drills' and hammers' cracks `vw_so052_jwr030`, `vw_so053_jwr_f080`, `vw_so112_jwr050` (live pack).
//!
//! Every one: the crack generator `zwr_gen` (FORCE_SPAWN_CHECK_MAT, bIsNoHitSetLocation, the waters 32 .. 39 NOT equal)
//! creates its crack children unless the 60 cm ray down meets water; the water pillar `ps_miz_hashira` (ONHIT, equal) only
//! over water. bIsNoHitSetLocation keeps every child at the generator's place: a slope or a step changes nothing of where the
//! crack's decal box is or how it is turned (the renderer's projection lays it on the ground under it).

use glam::{Affine3A, Vec3};
use kh_vfx::{Camera, DrawList, EffectDef, RayHit, RayWorld, SpawnOptions, World, Xform};
use std::fmt::Write as _;
use std::sync::Arc;

const DT: f32 = 1.0 / 60.0;

fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn def(name: &str) -> Option<EffectDef> {
    let live = root().join(format!("build/sora_pack/keyblades/vfx/effects/{name}.json"));
    let staged = root().join(format!("build/sora_tmp/weapons/AUDIT-B/W-HANDOVER/stage/kbvfx/effects/{name}.json"));
    let text = std::fs::read_to_string(live).or_else(|_| std::fs::read_to_string(staged)).ok()?;
    Some(EffectDef::parse(&text).expect("the effect parses"))
}

/// A level for the rays: a ground height (and its normal) per (x, y), water or not.
struct Level {
    height: fn(Vec3) -> Option<(f32, Vec3)>,
    surface: u8,
    asked: std::sync::Mutex<Vec<(Vec3, Vec3)>>,
}

impl RayWorld for Level {
    fn trace(&self, start: Vec3, end: Vec3) -> Option<RayHit> {
        self.asked.lock().unwrap().push((start, end));
        // march the ray (the test levels are height fields)
        let n = 200;
        for i in 0..=n {
            let p = start + (end - start) * (i as f32 / n as f32);
            if let Some((z, normal)) = (self.height)(p) {
                if p.z <= z {
                    return Some(RayHit { point: Vec3::new(p.x, p.y, z), normal, surface: self.surface });
                }
            }
        }
        None
    }
}

fn flat(_: Vec3) -> Option<(f32, Vec3)> {
    Some((0.0, Vec3::Z))
}
/// z = 0.4 x (22 degrees), through the origin
fn slope(p: Vec3) -> Option<(f32, Vec3)> {
    Some((0.4 * p.x, Vec3::new(-0.4, 0.0, 1.0).normalize()))
}
/// a step: the floor at z 0 for x < 360, 2 m lower beyond
fn step(p: Vec3) -> Option<(f32, Vec3)> {
    Some((if p.x < 360.0 { 0.0 } else { -200.0 }, Vec3::Z))
}

#[derive(Debug, Default)]
struct Seen {
    cracks: Vec<String>,
    pillar: Vec<String>,
    decals: Vec<Affine3A>,
    asked: usize,
}

/// The effect played at `at` (yaw 0) on `level` (None: the flat ground of set_ground(0)); 0.4 s of it.
fn play(name: &str, at: Vec3, level: Option<Level>, log: &mut String) -> Option<Seen> {
    let d = def(name)?;
    let gen: Vec<usize> = d.emitters.iter().enumerate().filter(|(_, e)| e.name == "zwr_gen").map(|(i, _)| i).collect();
    let pil: Vec<usize> = d.emitters.iter().enumerate().filter(|(_, e)| e.name.starts_with("ps_miz_")).map(|(i, _)| i).collect();
    let mut world = World::new();
    world.register(d.clone());
    let level = level.map(Arc::new);
    match &level {
        Some(l) => world.set_rays(Some(l.clone() as Arc<dyn RayWorld>)),
        None => world.set_ground(Some(0.0)),
    }
    let id = world.spawn(name, Xform::at(at), SpawnOptions::default())?;
    let mut seen = Seen::default();
    for _ in 0..24 {
        world.step(DT);
        for e in world.probe(id) {
            let Some(p) = e.parent else { continue };
            let parent = world.probe(id).get(p).map(|x| x.name.clone()).unwrap_or_default();
            let slot = if parent == "zwr_gen" { &mut seen.cracks } else if parent.starts_with("ps_miz_") { &mut seen.pillar } else { continue };
            if !slot.contains(&e.name) {
                slot.push(e.name.clone());
            }
        }
        let mut list = DrawList::default();
        world.draw(1.0, &Camera::DEFAULT, &mut list);
        if seen.decals.is_empty() && !list.decals.is_empty() {
            seen.decals = list.decals.iter().map(|x| x.transform).collect();
        }
    }
    seen.asked = level.as_ref().map_or(0, |l| l.asked.lock().unwrap().len());
    let _ = writeln!(log, "{name} at ({:.0}, {:.0}, {:.0}): crack generators {gen:?} -> children {:?}; water pillars {pil:?} -> children {:?}; first decals {}; rays asked {}", at.x, at.y, at.z, seen.cracks, seen.pillar, seen.decals.len(), seen.asked);
    Some(seen)
}

fn level(height: fn(Vec3) -> Option<(f32, Vec3)>, surface: u8) -> Option<Level> {
    Some(Level { height, surface, asked: std::sync::Mutex::new(Vec::new()) })
}

const CRACKS: [&str; 7] = ["vw_so172_jwr080", "vw_so172_jwr050", "vw_so052_jwr080", "vw_so112_jwr080", "vw_so052_jwr030", "vw_so053_jwr_f080", "vw_so112_jwr050"];

#[test]
fn the_cracks_play_on_dry_ground_and_the_pillars_only_over_water() {
    let mut log = String::new();
    let at = Vec3::new(370.0, 0.0, 0.0); // vfx_root + the notifies' LocationOffset (370, 0, 0): on the floor ahead of him
    let mut played = 0;
    for name in CRACKS {
        let Some(d) = def(name) else {
            eprintln!("SKIP {name}: not on disk");
            continue;
        };
        let rc = d.emitters.iter().find(|e| e.name == "zwr_gen").and_then(|e| e.raycast.clone());
        if let Some(rc) = &rc {
            // the data: FORCE_SPAWN_CHECK_MAT, the place kept, 60 cm down, the waters, not equal
            assert_eq!((rc.theme, rc.keep_place, rc.surface_equal, rc.end_offset.z), (2, true, false, -60.0), "{name}");
            assert_eq!(rc.surfaces, [32, 33, 34, 35, 36, 37, 39], "{name}");
        }
        let has_pillar = d.emitters.iter().any(|e| e.name.starts_with("ps_miz_") && e.raycast.as_ref().is_some_and(|r| r.theme == 0 && r.surface_equal));
        for (what, l) in [("dry ground (host rays)", level(flat, 0)), ("dry ground (set_ground, no host)", None)] {
            let s = play(name, at, l, &mut log).unwrap();
            assert!(rc.is_none() || !s.cracks.is_empty(), "{name} on {what}: the crack children are created: {s:?}");
            assert!(s.pillar.is_empty(), "{name} on {what}: NO water pillar on dry ground: {s:?}");
        }
        let s = play(name, at, level(flat, 32), &mut log).unwrap();
        assert!(s.cracks.is_empty(), "{name} over water: no crack: {s:?}");
        assert_eq!(!s.pillar.is_empty(), has_pillar, "{name} over water: its water pillar: {s:?}");
        played += 1;
    }
    std::fs::create_dir_all(root().join("build/sora_tmp/weapons/AUDIT-B/W-HANDOVER/traces")).ok();
    std::fs::write(root().join("build/sora_tmp/weapons/AUDIT-B/W-HANDOVER/traces/raycast_dry_water.log"), &log).ok();
    eprintln!("{log}");
    assert!(played == 0 || played >= 5, "the pack's crack effects ran: {played}");
}

#[test]
fn a_slope_or_a_step_leaves_the_crack_where_kh3_puts_it() {
    let mut log = String::new();
    for name in ["vw_so172_jwr080", "vw_so052_jwr080", "vw_so053_jwr_f080"] {
        if def(name).is_none() {
            eprintln!("SKIP {name}");
            continue;
        }
        // the reference: flat dry ground; the generator on the floor at x 370
        let flat_s = play(name, Vec3::new(370.0, 0.0, 0.0), level(flat, 0), &mut log).unwrap();
        // a slope through the same place (z = 0.4 x: the generator 148 cm up the slope, ON it)
        let slope_s = play(name, Vec3::new(370.0, 0.0, 148.0), level(slope, 0), &mut log).unwrap();
        assert_eq!(flat_s.cracks, slope_s.cracks, "{name}: the same children on the slope");
        assert!(slope_s.pillar.is_empty());
        // the decal boxes: only moved by the spawn place (148 cm up), never turned to the slope (no bIsHitNormalInfluenced)
        assert_eq!(flat_s.decals.len(), slope_s.decals.len(), "{name}");
        for (a, b) in flat_s.decals.iter().zip(&slope_s.decals) {
            assert!((Vec3::from(b.translation) - Vec3::from(a.translation) - Vec3::new(0.0, 0.0, 148.0)).length() < 0.5, "{name}: {a:?} {b:?}");
            assert!((b.matrix3 - a.matrix3).abs_diff_eq(glam::Mat3A::ZERO, 1.0e-3), "{name}: the box is not turned");
        }
        // a step: the floor 2 m under the generator (nothing within its 60 cm ray): FORCE_SPAWN_CHECK_MAT with no hit plays
        // where it is (bIsNoHitSetLocation); no water pillar (ONHIT)
        let step_s = play(name, Vec3::new(370.0, 0.0, 0.0), level(step, 0), &mut log).unwrap();
        assert_eq!(flat_s.cracks, step_s.cracks, "{name}: over the step");
        assert!(step_s.pillar.is_empty());
        for (a, b) in flat_s.decals.iter().zip(&step_s.decals) {
            assert!((Vec3::from(b.translation) - Vec3::from(a.translation)).length() < 0.5, "{name}: not dropped to the lower floor");
        }
        assert!(step_s.asked > 0, "the host was asked");
    }
    std::fs::create_dir_all(root().join("build/sora_tmp/weapons/AUDIT-B/W-HANDOVER/traces")).ok();
    std::fs::write(root().join("build/sora_tmp/weapons/AUDIT-B/W-HANDOVER/traces/raycast_slope_step.log"), &log).ok();
    eprintln!("{log}");
}

/// The rule of 0x14434A6E0 / 0x14435FCB0 on its own (every theme, the place, the offset).
#[test]
fn kh3s_spawn_rule() {
    use kh_vfx::RaycastDef;
    let base = RaycastDef { end_offset: Vec3::new(0.0, 0.0, -100.0), world: false, offset: Vec3::new(0.0, 0.0, 5.0), force: false, theme: 0, keep_place: false, size: Vec3::ZERO, normal_influenced: false, surfaces: Vec::new(), surface_equal: false };
    let dry = RayHit { point: Vec3::new(0.0, 0.0, -40.0), normal: Vec3::Z, surface: 2 };
    let wet = RayHit { surface: 33, ..dry };
    let waters = vec![32, 33, 34, 35, 36, 37, 39];
    // ONHIT
    assert!(!base.plays(None) && base.plays(Some(&dry)));
    let eq = RaycastDef { surfaces: waters.clone(), surface_equal: true, ..base.clone() };
    assert!(eq.plays(Some(&wet)) && !eq.plays(Some(&dry)) && !eq.plays(None));
    let ne = RaycastDef { surfaces: waters.clone(), ..base.clone() };
    assert!(!ne.plays(Some(&wet)) && ne.plays(Some(&dry)));
    // FORCE_SPAWN
    let force = RaycastDef { theme: 1, force: true, ..base.clone() };
    assert!(force.plays(None) && force.plays(Some(&wet)));
    // FORCE_SPAWN_CHECK_MAT
    let cm = RaycastDef { theme: 2, surfaces: waters, ..base.clone() };
    assert!(!cm.plays(None), "no hit: only with bIsNoHitSetLocation");
    assert!(RaycastDef { keep_place: true, ..cm.clone() }.plays(None));
    assert!(cm.plays(Some(&dry)) && !cm.plays(Some(&wet)));
    assert!(!RaycastDef { surface_equal: true, ..cm.clone() }.plays(Some(&dry)));
    // the place: the hit + RaycastSize / 2 + the turned offset; no hit: the ray's END; bIsNoHitSetLocation: unchanged
    let start = Vec3::new(10.0, 0.0, 0.0);
    let dir = Vec3::new(0.0, 0.0, -100.0);
    assert_eq!(base.place(start, dir, glam::Mat3::IDENTITY, Some(&dry)).0, Vec3::new(0.0, 0.0, -35.0));
    assert_eq!(RaycastDef { size: Vec3::splat(10.0), ..base.clone() }.place(start, dir, glam::Mat3::IDENTITY, Some(&dry)).0, Vec3::new(5.0, 5.0, -30.0));
    assert_eq!(base.place(start, dir, glam::Mat3::IDENTITY, None).0, Vec3::new(10.0, 0.0, -100.0));
    assert_eq!(RaycastDef { keep_place: true, ..base.clone() }.place(start, dir, glam::Mat3::IDENTITY, Some(&dry)).0, start);
    // bIsHitNormalInfluenced: the offset along the hit's normal, which is given back
    let tilted = RayHit { normal: Vec3::new(1.0, 0.0, 1.0).normalize(), ..dry };
    let (p, n) = RaycastDef { normal_influenced: true, ..base.clone() }.place(start, dir, glam::Mat3::IDENTITY, Some(&tilted));
    assert!((p - (dry.point + tilted.normal * 5.0)).length() < 1.0e-4 && (n - tilted.normal).length() < 1.0e-6);
}
