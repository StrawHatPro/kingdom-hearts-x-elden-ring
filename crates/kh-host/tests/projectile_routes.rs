//! (FINAL-FIX, FINAL 9: `sound: no route for "TresProjTS_AvatarRush_C"` x6, TresProjTR_AvatarRush_C, TresProjFZ_Burst_C,
//! se02006_032, se02008_036) EVERY PROJECTILE CLASS THE PORT SPAWNS HAS A SOUND ROUTE, through the pack and the roots kh-er
//! fx.rs merges (PACK/xsi/audio, PACK/keyblades/audio, PACK/shotlocks/audio):
//! - its pawn hit (kh-core magic/projectile.rs): the row's `hit_sound` by name when it has one, else the class
//!   (`ACTOR_PAWN_HIT`, the blueprint's m_HitEffects[0].m_PawnHitSEAsset);
//! - its map hit: always the class (`ACTOR_MAP_HIT`, m_MapHitSEAsset);
//! - every SE an actor row names is a packed, playable asset.
//!
//! A class whose blueprint has no SE is a row with none: silent by KH3's own data, never "unknown". The classes: every
//! `*class` row of the sheets kh-core's `StageDef` loads from (`Sheet::Proj` sora-magic-projectiles, `Sheet::Shotlocks`
//! sora-shotlocks). Skipped, saying so, without the pack.

use std::path::PathBuf;
use std::sync::Arc;

use glam::Vec3;
use kh_audio::{Engine, Language, Pack, SeamPlay};
use kh_core::magic::projectile::StageDef;
use kh_core::magic::tables::{Sheet, PV};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("build").join("sora_pack")
}

/// The pack as kh-er fx.rs builds it for Sora.
fn pack() -> Option<Pack> {
    let dir = root().join("audio");
    if !dir.join("manifest.json").is_file() {
        eprintln!("SKIPPED: no audio pack at {}", dir.display());
        return None;
    }
    let mut pack = Pack::load(&dir).expect("the audio pack");
    for extra in [root().join("party").join("audio"), root().join("xsi").join("audio"), root().join("party").join("n_ex028").join("audio"), root().join("keyblades").join("audio"), root().join("shotlocks").join("audio")] {
        pack.merge_party_root(&extra).expect("an audio root merges");
    }
    Some(pack)
}

/// Every projectile class of the sheets `StageDef` loads, with its row's hit sound ("" = by the class).
fn classes() -> Vec<(&'static str, &'static str)> {
    let mut out: Vec<(&'static str, &'static str)> = Vec::new();
    for (id, v) in Sheet::Shotlocks.rows() {
        if let (true, PV::S(class)) = (id.starts_with("proj.") && id.ends_with(".class"), v) {
            let def = StageDef::shot_class(class).unwrap_or_else(|| panic!("{class} loads"));
            out.push((def.class_name, def.hit_sound));
        }
    }
    for (id, v) in Sheet::Proj.rows() {
        if let (true, PV::S(class)) = (id.ends_with("class"), v) {
            if class.starts_with("TresProj") && class.ends_with("_C") && !out.iter().any(|c| c.0 == *class) {
                // the magic families' rows give no hit sound by name: the class plays its own
                out.push((class, ""));
            }
        }
    }
    out
}

#[test]
fn every_projectile_class_the_port_spawns_has_a_sound_route() {
    let Some(pack) = pack() else { return };
    let pack = Arc::new(pack);
    let mut engine = Engine::new(pack.clone(), 3, Language::En);
    let list = classes();
    assert!(list.len() > 60, "the sheets' classes: {}", list.len());
    let mut bad = Vec::new();
    for (class, hit_sound) in &list {
        let actor = pack.actor(class);
        if actor.is_none() {
            bad.push(format!("{class}: not in the actor tables (its map hit has no route)"));
        }
        if hit_sound.is_empty() {
            engine.actor_hit(None, class, false, Vec3::ZERO);
        } else {
            engine.seam_play(1, &SeamPlay { name: hit_sound, position: Vec3::ZERO, volume: 1.0, pitch: 1.0, looping: false, spatial: true, anim: None });
        }
        engine.actor_hit(None, class, true, Vec3::ZERO);
        for se in actor.into_iter().flat_map(|a| [&a.fire_se, &a.pawn_hit_se, &a.map_hit_se]).flatten() {
            if pack.asset(se).is_none() {
                bad.push(format!("{class}: its SE {se} is not packed"));
            }
        }
        if !hit_sound.is_empty() && engine.routes.by_name.get(*hit_sound).is_none_or(|p| pack.asset(p).is_none()) {
            bad.push(format!("{class}: its hit sound {hit_sound} is not a packed asset"));
        }
    }
    bad.extend(engine.unknown.iter().map(|n| format!("no route for \"{n}\"")));
    assert!(bad.is_empty(), "{} of {} classes:\n{}", bad.len(), list.len(), bad.join("\n"));
    println!("{} projectile classes: every pawn hit, map hit and SE has its route", list.len());
}

/// The five names FINAL heard nothing for, each to KH3's own asset (or KH3's own silence).
#[test]
fn finals_five_names_resolve_to_kh3s_assets() {
    let Some(pack) = pack() else { return };
    let ends = |a: &Option<String>, name: &str| a.as_deref().is_some_and(|p| p.ends_with(name));
    // Avatar Rush (Favorite Deputy / Classic Tone): the blueprints carry no SE at all; the hit effect carries its own
    for class in ["TresProjTS_AvatarRush_C", "TresProjTR_AvatarRush_C"] {
        let a = pack.actor(class).unwrap_or_else(|| panic!("{class} is a known class"));
        assert!(a.fire_se.is_none() && a.pawn_hit_se.is_none() && a.map_hit_se.is_none(), "{class}: silent by KH3's data: {a:?}");
    }
    // Frozen Pride's burst: fire se02008_035, pawn hit se02008_036 (also its row's hit sound), map hit se02008_037
    let fz = pack.actor("TresProjFZ_Burst_C").expect("TresProjFZ_Burst_C");
    assert!(ends(&fz.fire_se, "se02008_035") && ends(&fz.pawn_hit_se, "se02008_036") && ends(&fz.map_hit_se, "se02008_037"), "{fz:?}");
    // Happy Gear's shot: pawn hit se02006_032, map hit se02006_033
    let mi = pack.actor("TresProjMI_Shot_C").expect("TresProjMI_Shot_C");
    assert!(ends(&mi.pawn_hit_se, "se02006_032") && ends(&mi.map_hit_se, "se02006_033"), "{mi:?}");
    let mut engine = Engine::new(Arc::new(pack), 3, Language::En);
    for name in ["se02006_032", "se02008_036"] {
        assert!(engine.seam_play(1, &SeamPlay { name, position: Vec3::ZERO, volume: 1.0, pitch: 1.0, looping: false, spatial: true, anim: None }).is_some(), "{name} plays");
    }
    assert!(engine.unknown.is_empty(), "{:?}", engine.unknown);
}
