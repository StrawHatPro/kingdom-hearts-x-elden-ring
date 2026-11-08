//! (AUDIT-B W-VFX) `TypeDataSQEX_Thunder` emitters (kh-vfx def.rs `BeamDef`, sim.rs `draw_beams`): a Beam2 between the two
//! points the host resolves on the mesh the effect hangs on (KH3 0x144250690), on the real Drill Punch rope segment
//! vw_so053_bam_lin0 (Favorite Deputy's FINISH_DR, w_so053 M_6421 / M_6422 / M_6423 on himo01..16) when the pack is on disk.

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

#[test]
fn the_drill_rope_segment_is_one_tapered_beam_between_its_two_sockets() {
    let Some(def) = pack_def("vw_so053_bam_lin0") else {
        eprintln!("SKIP: build/sora_pack not on disk");
        return;
    };
    let b = def.emitters[0].beam.clone().expect("vw_so053_bam_lin0 is a TypeDataSQEX_Thunder emitter");
    // the pack's values: ATTACH_BONE both ends, no bone list, 3 interpolation points, 1 beam, 1 sheet, PEBTM_Full taper
    // 0.5 x 0.75, BeamSource / BeamTarget strengths 0 (a straight segment), no noise
    assert_eq!((b.start, b.end, b.bones.is_none(), b.interpolation, b.max_beams, b.sheets, b.noise.is_some()), (kh_vfx::BeamPoint::AttachBone, kh_vfx::BeamPoint::AttachBone, true, 3, 1, 1, false)); // (AUDIT-B W-VFX2) noise: Option
    assert!(def.unknown().is_empty(), "everything of it is evaluated: {:?}", def.unknown());
    let mut world = World::new();
    world.register(def);
    let id = world.spawn("vw_so053_bam_lin0", Xform::at(Vec3::new(0.0, 0.0, 100.0)), SpawnOptions::default()).unwrap();
    let asks = world.beam_asks(id);
    assert_eq!(asks, [kh_vfx::BeamAsk { emitter: 0, bones: None }], "it asks for the attach row's socket pair");
    // himo15 -> himo16, 20 cm apart along +Y, seen from -X
    let (from, to) = (Vec3::new(0.0, 0.0, 100.0), Vec3::new(0.0, 20.0, 100.0));
    let camera = Camera::DEFAULT;
    // nothing to draw without the host's ends (KH3: both names must be set, 0x144250690)
    world.step(DT);
    assert!(drawn(&world, &camera).ribbons.is_empty(), "no ends: no beam");
    for _ in 0..6 {
        world.set_beam_ends(id, 0, vec![(from, to)]);
        world.step(DT);
    }
    let list = drawn(&world, &camera);
    assert_eq!(list.ribbons.len(), 1, "one beam, one sheet");
    let r = &list.ribbons[0];
    assert_eq!(r.points.len(), 4, "source + 3 interpolation points");
    // the Hermite with zero strengths: on the segment, eased (h01 at 1/3, 2/3, 1)
    for (p, t) in r.points.iter().zip([0.0f32, 7.0 / 27.0, 20.0 / 27.0, 1.0]) {
        let mid = (p.a + p.b) * 0.5;
        assert!(mid.distance(from.lerp(to, t)) < 1.0e-3, "point at {t}: {mid:?}");
        // half width = Size.X 2 x taper 0.5 x 0.75, facing the camera (across the beam and the view)
        assert!(((p.a - p.b).length() * 0.5 - 0.75).abs() < 1.0e-4, "half width {}", (p.a - p.b).length() * 0.5);
        let across = (p.a - p.b).normalize();
        assert!(across.dot(Vec3::Y).abs() < 1.0e-4 && across.dot((mid - camera.position).normalize()).abs() < 1.0e-4, "across the beam and the view: {across:?}");
    }
    let us: Vec<f32> = r.points.iter().map(|p| p.u).collect();
    assert_eq!(us, [0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0], "U = k / N along the beam");
    // StartColor (40, 5, 1), alpha 1 in the loop (AlphaScaleOverLife by emitter time)
    assert_eq!(r.points[0].color, [40.0, 5.0, 1.0, 1.0]);
    // the ends follow the sockets
    world.set_beam_ends(id, 0, vec![(from, from + Vec3::Z * 30.0)]);
    world.step(DT);
    let r = &drawn(&world, &camera).ribbons[0];
    assert!(((r.points[3].a + r.points[3].b) * 0.5).distance(from + Vec3::Z * 30.0) < 1.0e-3);
    // LOOPEND (the notify's end): the loop ends, the alpha curve runs out at the emitter's 0.39 s and it is gone
    world.stop(id, false);
    let mut alive = 0;
    for _ in 0..60 {
        world.set_beam_ends(id, 0, vec![(from, to)]);
        world.step(DT);
        if !drawn(&world, &camera).ribbons.is_empty() {
            alive += 1;
        }
    }
    assert!((5..=25).contains(&alive), "plays out its 0.19 -> 0.39 s tail after the loop end: {alive} steps");
    assert!(!world.is_alive(id) || drawn(&world, &camera).ribbons.is_empty());
}

#[test]
fn every_thunder_emitter_of_the_pack_is_a_beam_with_its_rule() {
    // the 13 effects of the pack with ParticleModuleTypeDataSQEX_Thunder (both effect folders)
    let names = [
        ("VW_SO021_CHG1", false, false),
        ("VW_SO022_CHG4", false, false),
        ("vw_so030_naka", false, true),
        ("vw_so030_top", false, true),
        ("vw_so030_und", false, true),
        ("vw_so053_bam_lin0", false, false),
        ("vw_so062_chg_thd07", false, true),
        ("vw_so064_bem_lin00", true, false),
        ("vw_so064_bem_lin03", true, false),
        ("vw_so064_gte_bem_lin01", true, false),
        ("vw_so073_fin_beam0", false, false),
        ("vw_so172_yoy_lin00", true, false),
        ("vw_so172_yoy_lin01", true, false),
    ];
    let mut seen = 0;
    for (name, bone_list, noise) in names {
        let Some(def) = pack_def(name) else { continue };
        seen += 1;
        let beams: Vec<_> = def.emitters.iter().filter_map(|e| e.beam.as_ref()).collect();
        assert!(!beams.is_empty(), "{name} has a thunder emitter");
        for b in beams {
            assert_eq!(b.bones.is_some(), bone_list, "{name}: bone list");
            assert_eq!(b.noise.is_some(), noise, "{name}: noise"); // (AUDIT-B W-VFX2) noise: Option
        }
        let mut world = World::new();
        world.register(def);
        let id = world.spawn(name, Xform::at(Vec3::ZERO), SpawnOptions::default()).unwrap();
        // Ever After's ring beams (MY_MODULE) ask for nothing; every other one asks for its attach points
        assert_eq!(world.beam_asks(id).is_empty(), name == "vw_so073_fin_beam0", "{name}");
    }
    eprintln!("{seen} of 13 thunder effects checked");
}
