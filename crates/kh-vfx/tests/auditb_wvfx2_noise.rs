//! (AUDIT-B W-VFX2) Beam noise (`ParticleModuleBeamNoise`: kh-vfx def.rs `BeamNoiseDef`, sim.rs `BeamNoise` /
//! `noise_strip`, KH3's spawn 0x144251710, update 0x144253960, payload 0x144254110 and the noise fills 0x144315F40 /
//! 0x144318D30) and the Beam2 spawn count (tick 0x144252AC0), on the real effects when the pack is on disk:
//! Blizzard Claws' transformation `vw_so062_chg_thd07`, Hero's Origin's `vw_so030_naka / top / und`, Twin Yo-Yos'
//! `vw_so064_bem_lin00` (bAlwaysOn, 16 links).

use glam::Vec3;
use kh_vfx::{Camera, DrawList, EffectDef, SpawnOptions, World, Xform};

const DT: f32 = 1.0 / 60.0;

fn pack_def(name: &str) -> Option<EffectDef> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../build/sora_pack/keyblades/vfx/effects/{name}.json"));
    let text = std::fs::read_to_string(path).ok()?;
    Some(EffectDef::parse(&text).expect("the pack's effect parses"))
}

fn drawn(world: &World, camera: &Camera) -> DrawList {
    let mut list = DrawList::default();
    world.draw(1.0, camera, &mut list);
    list
}

/// The distance of `p` from the segment a..b.
fn off_segment(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

#[test]
fn blizzard_claws_thunder_is_a_jittering_noise_polyline_between_its_sockets() {
    let Some(def) = pack_def("vw_so062_chg_thd07") else {
        eprintln!("SKIP: build/sora_pack not on disk");
        return;
    };
    let b = def.emitters[0].beam.clone().expect("a thunder emitter");
    let nd = b.noise.clone().expect("its BeamNoise module (bLowFreq_Enabled)");
    // the pack's values: Frequency 30 (LowRange 1), NoiseRange +-3 cm on every axis, tessellation 1, tension 0.5,
    // NoiseTangentStrength 250, NoiseLockTime 0 (new points every tick), no smooth / target noise / distance rule
    assert_eq!((nd.frequency, nd.low_range, nd.tessellation, nd.smooth, nd.target_noise, nd.frequency_distance), (30, 1, 1, false, false, 0.0));
    assert_eq!((nd.tension, nd.lock_time), (0.5, 0.0));
    assert_eq!((b.interpolation, b.sheets, b.max_beams, b.always_on, b.use_source, b.use_target), (30, 2, 1, true, true, true));
    assert!(def.unknown().is_empty(), "everything of it is evaluated: {:?}", def.unknown());
    let mut world = World::new();
    world.register(def);
    let id = world.spawn("vw_so062_chg_thd07", Xform::at(Vec3::new(0.0, 0.0, 100.0)), SpawnOptions::default()).unwrap();
    let (from, to) = (Vec3::new(0.0, 0.0, 100.0), Vec3::new(0.0, 40.0, 100.0));
    let camera = Camera::DEFAULT;
    let mut shapes = Vec::new();
    for step in 0..8 {
        world.set_beam_ends(id, 0, vec![(from, to)]);
        world.step(DT);
        let list = drawn(&world, &camera);
        assert_eq!(list.ribbons.len(), 2, "step {step}: one beam, two sheets");
        for r in &list.ribbons {
            // the first pair at the source, Steps = Freq (1..=30) noise points, the last segment's 1 point at the target
            let steps = r.points.len() - 2;
            assert!((1..=30).contains(&steps), "Freq in [LowRange, Frequency]: {steps}");
            let mids: Vec<Vec3> = r.points.iter().map(|p| (p.a + p.b) * 0.5).collect();
            assert!(mids[0].distance(from) < 1.0e-3 && mids[mids.len() - 1].distance(to) < 1.0e-3, "from the source to the target");
            for m in &mids {
                // tessellation 1: the Hermite only at t = 1, i.e. the noise points themselves (the 250 cm tangents never show)
                assert!(off_segment(*m, from, to) <= 3.0 * 3f32.sqrt() + 1.0e-3, "within the +-3 cm noise of the line: {m:?}");
            }
            for p in &r.points {
                // half width = Size.X 5 x taper (TaperFactor 0.5 x TaperScale 0.75)
                assert!(((p.a - p.b).length() * 0.5 - 1.875).abs() < 1.0e-3, "half width {}", (p.a - p.b).length() * 0.5);
                assert_eq!(p.color[0..3], [0.8 * 10.0, 5.0 * 10.0, 10.0 * 10.0], "StartColor x ColorScaleOverLife 10");
            }
            // U = 1 / (Frequency x Tess) per point, from 0
            assert!(r.points.windows(2).all(|w| (w[1].u - w[0].u - 1.0 / 30.0).abs() < 1.0e-5));
        }
        // the two sheets: the same centre line, the second turned 90 degrees about it
        let (r0, r1) = (&list.ribbons[0], &list.ribbons[1]);
        assert_eq!(r0.points.len(), r1.points.len());
        for (p, q) in r0.points.iter().zip(&r1.points).skip(1) {
            assert!(((p.a + p.b) * 0.5).distance((q.a + q.b) * 0.5) < 1.0e-3);
            assert!((p.a - p.b).normalize().dot((q.a - q.b).normalize()).abs() < 1.0e-3, "pi x 1 / 2 about Right");
        }
        shapes.push(r0.points.iter().map(|p| (p.a + p.b) * 0.5).collect::<Vec<_>>());
    }
    // NoiseLockTime 0: the points are drawn again every tick, so the shape changes from step to step
    assert!(shapes.windows(2).all(|w| w[0] != w[1]), "it jitters every step");
}

#[test]
fn heros_origin_strands_are_noise_beams_across_their_sockets() {
    for (name, beams, range) in [("vw_so030_naka", 3, 1.5f32), ("vw_so030_top", 2, 1.0), ("vw_so030_und", 2, 0.5)] {
        let Some(def) = pack_def(name) else {
            eprintln!("SKIP: build/sora_pack not on disk");
            return;
        };
        let nd = def.emitters[0].beam.as_ref().and_then(|b| b.noise.clone()).expect("noise");
        // FrequencyDistance 0.1 cm (Steps = Freq on any visible beam), tessellation 0 (the replay's 1), no rate, no burst
        assert_eq!((nd.frequency_distance, nd.replay_tessellation()), (0.1, 1), "{name}");
        let mut world = World::new();
        world.register(def);
        // the component turned 90 degrees about Z: the noise (x 0, y / z +-range in its space) lies in world X / Z
        let mut at = Xform::at(Vec3::ZERO);
        at.rotation = glam::Mat3::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let id = world.spawn(name, at, SpawnOptions::default()).unwrap();
        let (from, to) = (Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 30.0, 0.0));
        let camera = Camera::DEFAULT;
        let mut counts = Vec::new();
        for _ in 0..10 {
            world.set_beam_ends(id, 0, vec![(from, to)]);
            world.step(DT);
            counts.push(drawn(&world, &camera).ribbons.len() / 2);
        }
        // the spawn rate curve (40 -> 0 per second): an empty Beam2 emitter spawns at least one beam at once, then the
        // rate adds beams up to MaxBeamCount
        assert_eq!(counts[0], 1, "{name}: {counts:?}");
        assert!(counts.windows(2).all(|w| w[0] <= w[1]) && counts.iter().max() == Some(&beams), "{name}: {counts:?}");
        for r in &drawn(&world, &camera).ribbons {
            for p in r.points.iter().skip(1) {
                let m = (p.a + p.b) * 0.5;
                // the noise is in the component's space: its x (0) is world -Y, so the strand stays on the line's
                // length and wanders across it by up to +-range in world X and Z
                assert!(m.x.abs() <= range + 1.0e-3 && m.z.abs() <= range + 1.0e-3, "{name}: {m:?}");
            }
        }
    }
}

#[test]
fn an_always_on_beam_emitter_spawns_every_link_at_once() {
    let Some(def) = pack_def("vw_so064_bem_lin00") else {
        eprintln!("SKIP: build/sora_pack not on disk");
        return;
    };
    let b = def.emitters[0].beam.clone().unwrap();
    assert_eq!((b.always_on, b.max_beams, b.bones.as_ref().map(Vec::len)), (true, 16, Some(17)));
    let mut world = World::new();
    world.register(def);
    let id = world.spawn("vw_so064_bem_lin00", Xform::at(Vec3::ZERO), SpawnOptions::default()).unwrap();
    let links: Vec<(Vec3, Vec3)> = (0..16).map(|i| (Vec3::new(0.0, i as f32 * 5.0, 0.0), Vec3::new(0.0, (i + 1) as f32 * 5.0, 0.0))).collect();
    world.set_beam_ends(id, 0, links.clone());
    world.step(DT);
    let list = drawn(&world, &Camera::DEFAULT);
    // bAlwaysOn: the burst is MaxBeamCount, so all 16 links of the string are there from the first tick
    assert_eq!(list.ribbons.len(), 16, "every link at once");
    for (r, (s, t)) in list.ribbons.iter().zip(&links) {
        let (a, z) = (r.points[0], r.points[r.points.len() - 1]);
        assert!(((a.a + a.b) * 0.5).distance(*s) < 1.0e-3 && ((z.a + z.b) * 0.5).distance(*t) < 1.0e-3);
    }
}
