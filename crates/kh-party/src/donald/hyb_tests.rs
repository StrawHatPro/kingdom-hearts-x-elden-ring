//! (HYB) The hybrids' look (donald/hybrid_look.rs): it starts at the cast clip's release (never before), its bodies follow
//! their actors and dissipate when they end, Thermal Shock lands as its comets meet, the Light hybrids never use a Cure
//! effect, every effect is packed and drawable, every beat has a sound. A child of magic_tests (its rig).

use super::*;
use super::super::hybrid_look;

fn ready(rig: &mut Rig, a: Element, b: Element) {
    rig.give(DonAbility::DualAttunement);
    rig.attune(a, 3.0);
    rig.attune(b, 3.0);
    rig.party.status_mut(rig.don).unwrap().mp = 1000;
}

fn hyb_spawns(rig: &Rig) -> Vec<(u32, &'static str)> {
    rig.effects.spawned.iter().filter(|e| e.1.starts_with("vd_hyb_")).map(|e| (e.0, e.1)).collect()
}

#[test]
fn nothing_of_a_hybrid_appears_before_the_release() {
    for (a, b) in super::super::spells::hybrid_pairs() {
        let mut rig = Rig::new(&[Vec3::new(700.0, 0.0, 0.0), Vec3::new(850.0, 240.0, 0.0), Vec3::new(850.0, -240.0, 0.0)]);
        ready(&mut rig, a, b);
        let spell = SpellRef::Hybrid(a, b);
        assert!(rig.begin(spell, e1()));
        let shot = rig.party.member(rig.don).and_then(|p| p.don_cast.run).map_or(0.5, |r| r.shot_at);
        rig.secs((shot - 0.05).max(0.0));
        assert!(hyb_spawns(&rig).is_empty(), "{spell:?}: its look began before the release");
        rig.secs(1.5);
        let after = hyb_spawns(&rig).len() + rig.effects.spawned.iter().filter(|e| e.1.starts_with("vm_") || e.1.starts_with("vp_") || e.1.starts_with("vd_don_")).count();
        assert!(after > 0, "{spell:?}: nothing after the release");
    }
}

#[test]
fn the_light_hybrids_never_use_cure_or_heal_effects() {
    for (a, b) in super::super::spells::hybrid_pairs().filter(|p| p.0 == Element::Light || p.1 == Element::Light) {
        let mut rig = Rig::new(&[Vec3::new(700.0, 0.0, 0.0), Vec3::new(850.0, 240.0, 0.0)]);
        ready(&mut rig, a, b);
        rig.cast(SpellRef::Hybrid(a, b), e1(), 7.0);
        for e in &rig.effects.spawned {
            assert!(!e.1.contains("_cur_") && !e.1.contains("cure") && !e.1.contains("_wcu_"), "{a:?}+{b:?} spawned the Cure effect {}", e.1);
        }
    }
}

#[test]
fn a_moving_field_carries_its_body_and_dissipates_where_it_ends() {
    // Tempest drifts forward: its body is placed along its path, then stopped, and its end burst plays there
    let mut rig = Rig::new(&[Vec3::new(700.0, 0.0, 0.0)]);
    ready(&mut rig, Element::Thunder, Element::Aero);
    rig.cast(SpellRef::Hybrid(Element::Thunder, Element::Aero), e1(), 1.0);
    let body = rig.effects.spawned.iter().find(|e| e.1 == "vd_hyb_tp_body").map(|e| e.0).expect("Tempest's body");
    let path: Vec<Vec3> = rig.effects.placed.iter().filter(|p| p.0 == body).map(|p| p.1).collect();
    assert!(path.len() > 10 && path.last().unwrap().x > path[0].x + 200.0, "the storm front's body follows it forward: {:?}", path.first());
    rig.secs(8.0);
    assert!(rig.effects.stopped.contains(&body), "stopped when the field ends");
    assert_eq!(rig.spawned("vd_hyb_tp_end"), 1, "its dissipation");
    assert_eq!(rig.party.don_fx.hyb.busy(), 0, "nothing left running");
}

#[test]
fn thermal_shock_lands_as_its_comets_meet() {
    let mut rig = Rig::new(&[Vec3::new(700.0, 0.0, 0.0)]);
    ready(&mut rig, Element::Fire, Element::Blizzard);
    assert!(rig.begin(SpellRef::Hybrid(Element::Fire, Element::Blizzard), e1()));
    let shot = rig.party.member(rig.don).and_then(|p| p.don_cast.run).map_or(0.5, |r| r.shot_at);
    rig.secs(shot + 0.05);
    assert_eq!(rig.spawned("vd_hyb_ts_fire"), 1);
    assert_eq!(rig.spawned("vd_hyb_ts_ice"), 1);
    assert!(rig.dummies.hits.is_empty(), "no hit before the comets meet");
    assert_eq!(rig.spawned("vd_hyb_ts_boom"), 0);
    rig.secs(hybrid_look::THERMAL_TRAVEL + 0.05);
    assert!(!rig.dummies.hits.is_empty(), "the hit lands as they meet");
    assert_eq!(rig.spawned("vd_hyb_ts_boom"), 1, "the collision");
}

#[test]
fn divine_spark_draws_a_bolt_for_every_leap() {
    let mut rig = Rig::new(&[Vec3::new(700.0, 0.0, 0.0), Vec3::new(900.0, 250.0, 0.0), Vec3::new(900.0, -250.0, 0.0)]);
    ready(&mut rig, Element::Thunder, Element::Light);
    rig.cast(SpellRef::Hybrid(Element::Thunder, Element::Light), e1(), 2.0);
    let hits = rig.spawned("vd_hyb_ds_hit");
    assert!(hits >= 3, "a holy burst on every enemy it leaps to: {hits}");
    assert_eq!(rig.spawned("vd_hyb_arc_gold"), hits, "and a golden bolt for every leap");
}

#[test]
fn glacier_raises_its_wall_segment_by_segment_and_breaks_it_when_it_melts() {
    let mut rig = Rig::new(&[Vec3::new(700.0, 0.0, 0.0)]);
    ready(&mut rig, Element::Blizzard, Element::Water);
    rig.cast(SpellRef::Hybrid(Element::Blizzard, Element::Water), e1(), 0.6);
    let segs = rig.spawned("vd_hyb_gl_seg");
    assert!(segs >= 4, "segments along the line: {segs}");
    let d = data();
    let life = SpellRef::Hybrid(Element::Blizzard, Element::Water).row(&d).unwrap().duration;
    rig.secs(life);
    assert_eq!(rig.spawned("vd_hyb_gl_break"), segs, "every segment breaks when the wall melts");
}

#[test]
fn the_arc_is_laid_along_its_line() {
    let (rot, scale) = hybrid_look::arc_transform(Vec3::new(0.0, 500.0, 0.0));
    assert!((rot.y - 90.0).abs() < 1e-3 && (rot.x - 90.0).abs() < 1e-3, "{rot:?}");
    assert!((scale.z - 0.5).abs() < 1e-4);
    let (rot, _) = hybrid_look::arc_transform(Vec3::new(300.0, 0.0, 300.0));
    assert!((rot.x - 135.0).abs() < 1e-3 && rot.y.abs() < 1e-3, "rising 45 degrees: {rot:?}");
}

/// Every effect the look spawns is in the pack given (KH_HYB_PACK, else build/play/donpack) and every material it uses has a
/// shading row in the table the test app reads (the XSI root's, else the party one).
#[test]
fn hyb_effects_are_packed_and_drawable() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let pack = std::env::var("KH_HYB_PACK").map(std::path::PathBuf::from).unwrap_or_else(|_| root.join("build").join("play").join("donpack"));
    if !pack.join("party").join("vfx").join("effects").is_dir() {
        eprintln!("no pack at {}: skipped", pack.display());
        return;
    }
    let table = ["xsi/vfx/shaders/shading.json", "party/vfx/shaders/shading.json"].iter().map(|t| pack.join(t)).find(|p| p.is_file()).unwrap();
    let shading: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(table).unwrap()).unwrap();
    let mut missing = Vec::new();
    for n in hybrid_look::all_names() {
        let f = [pack.join("vfx/effects").join(format!("{n}.json")), pack.join("party/vfx/effects").join(format!("{n}.json"))].into_iter().find(|p| p.is_file());
        let Some(f) = f else {
            missing.push(format!("{n}: no effect file"));
            continue;
        };
        let e: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        for m in e["files"]["materials"].as_array().into_iter().flatten().filter_map(|m| m.as_str()) {
            let id = m.trim_start_matches("materials/").trim_end_matches(".json");
            if shading.get("materials").and_then(|t| t.get(id)).is_none() {
                missing.push(format!("{n}: material {id} has no shading row"));
            }
        }
    }
    assert!(missing.is_empty(), "{missing:#?}");
}
