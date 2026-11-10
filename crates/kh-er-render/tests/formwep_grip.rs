//! (FORMWEP-FIX) EVERY FORM WEAPON IN SORA'S HAND, IN THE FORM-CHANGE CINEMATIC AND AFTER IT, IN EVERY OUTFIT.
//!
//! The user (2026-11-07): "keyblade transformations look broken like this when the keyblade transforms into another weapon"
//! (the Hyper Hammer assembled ~1.7 m beside him, only its grip in his hand), then "only broken during their transformation
//! cinematic". KH3 equips a form's TRANSFORM unit alone during StyleChange (`Deform@RIGHT_HAND:None` ...: no socket = UE's
//! AttachToComponent(NAME_None) = his MESH component); its form-change clip is authored in his mesh space. The port hung it on
//! his hand (keyblade.rs `place_part`), so his hand's transform came twice. These tests measure, with the pack's own data:
//!
//! 1. every transform unit's every form-change clip: at the clip's START its shown Keyblade is on his hand socket, at its END
//!    its shown bones are on every weapon of the form it changes into (cm), placed as the renderer places it (`place_part`);
//! 2. every form row's every weapon part at every grow level is on its socket of his bone, and in every outfit on that
//!    outfit's bone (the renderer's `Retarget::weapon_to_world`), on his pose at the end of the form's change clip;
//! 3. every outfit: the unit's last cinematic frame and the weapon's first frame after it agree (no jump at the handover).
//!
//! Sora's pose is his own clip (his root motion taken out, as the hosts move his actor by it). Skipped without the pack.
//! `FORMWEP_KB_PACK=<folder>`: the keyblades of another pack folder (a staged rebuild's `<folder>/keyblades`).

use std::path::{Path, PathBuf};

use glam::Vec3;
use kh_anim::formweapon::WeaponRig;
use kh_anim::{Clip, Skeleton, Trs};
use kh_er_render::keyblade::{FormPose, Part, Table};
use kh_er_render::FormWeapon;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The pack's keyblades and Sora's skeleton (None without the pack: skipped).
fn pack() -> Option<(Table, kh_anim::Pack, PathBuf)> {
    let pack = root().join("build/sora_pack");
    let kb = std::env::var("FORMWEP_KB_PACK").map(PathBuf::from).unwrap_or_else(|_| pack.clone());
    match (Table::read(&kb), kh_anim::Pack::load(&pack)) {
        (Some(t), Ok(p)) => Some((t, p, pack)),
        _ => {
            eprintln!("SKIPPED: no pack at {}", pack.display());
            None
        }
    }
}

/// Sora's clips whose root motion the hosts take out (the pack manifest's `root_motion_enabled`).
fn root_motion_clips(pack: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(pack.join("manifest.json")).unwrap_or_default();
    let json: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).unwrap_or(serde_json::Value::Null);
    json["items"].as_array().into_iter().flatten().filter(|i| i["root_motion_enabled"].as_bool() == Some(true)).filter_map(|i| i["animation_name"].as_str().map(str::to_string)).collect()
}

/// One of Sora's clips on his skeleton (his root motion taken out when the clip has it).
struct SoraClip {
    rig: WeaponRig,
    root_motion: bool,
    kh_sk: usize,
}

impl SoraClip {
    fn load(pack: &Path, skeleton: &Skeleton, stem: &str, moving: &[String]) -> Option<SoraClip> {
        let clip = Clip::load(&pack.join("anims").join(format!("{stem}.glb")), stem).ok()?;
        Some(SoraClip { rig: WeaponRig::new(skeleton.clone(), clip), root_motion: moving.iter().any(|m| m == stem), kh_sk: skeleton.index_of("kh_sk")? })
    }

    fn length(&self) -> f32 {
        self.rig.length()
    }

    /// His component pose at `t`.
    fn pose(&self, t: f32) -> Vec<Trs> {
        let mut pose = self.rig.component(t);
        if self.root_motion {
            let delta = pose[self.kh_sk].translation - self.rig.component(0.0)[self.kh_sk].translation;
            for b in &mut pose {
                b.translation -= delta;
            }
        }
        pose
    }
}

/// R_wep_skt relative to R_buki (the hosts' `weapon_socket`: roll -90).
fn hand_socket() -> Trs {
    Trs { translation: Vec3::ZERO, rotation: kh_anim::blend::Rotator { pitch: 0.0, yaw: 0.0, roll: -90.0 }.quat(), scale: Vec3::ONE }
}

/// A part placed on Sora's pose as the renderer places it (keyblade.rs `place_part`), his component at the origin.
fn place(t: &Table, p: &Part, skeleton: &Skeleton, pose: &[Trs], anim: Option<&str>, time: f32) -> Option<FormWeapon> {
    let index_of = |b: &str| skeleton.index_of(b);
    let socket = hand_socket();
    kh_er_render::keyblade::place_part(t, p, &FormPose { index_of: &index_of, component: pose, to_world: &Trs::IDENTITY, anim, time, hand_socket: &socket })
}

/// The world places of a placed part's SHOWN bones (component scale >= 0.5) under `to_world`.
fn shown(f: &FormWeapon, to_world: &Trs) -> Vec<(usize, Vec3)> {
    f.component.iter().enumerate().filter(|(_, b)| b.scale.min_element() >= 0.5).map(|(k, b)| (k, b.then(to_world).translation)).collect()
}

fn nearest(points: &[(usize, Vec3)], p: Vec3) -> f32 {
    points.iter().map(|(_, q)| (*q - p).length()).fold(f32::INFINITY, f32::min)
}

/// One form row's change clip as its transform unit plays it.
struct Change {
    row: &'static kh_core::combat::form::KeybladeFormRow,
    keyblade: String,
    unit: Part,
    logical: String,
    stem: String,
}

/// Every form row whose form has a transform unit the equipment attaches with no socket, with that unit's clip of the row's
/// change clip (StyleChange Tick sub 0's `transform_clip`).
fn changes(t: &Table) -> Vec<Change> {
    let mut out = Vec::new();
    for row in kh_core::combat::form::KEYBLADE_FORMS {
        let Some(kb) = t.keyblade(row.keyblade) else { continue };
        let Some(form) = t.form(&kb.id, row.weapon_form, "") else { continue };
        for unit in form.transform.iter().filter(|u| u.on_mesh) {
            let Some(m) = t.model(&unit.model) else { continue };
            for (logical, file) in m.clips.iter().filter(|(l, _)| l == row.transform_clip) {
                let stem = file.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                out.push(Change { row, keyblade: kb.id.clone(), unit: unit.clone(), logical: logical.clone(), stem });
            }
        }
    }
    out
}

#[test]
fn every_transform_unit_starts_in_his_hand_and_ends_on_its_form_weapons() {
    let Some((t, p, pack)) = pack() else { return };
    let moving = root_motion_clips(&pack);
    let all = changes(&t);
    assert!(all.len() >= 18, "the form-change clips of the transform units: {} found", all.len());
    let r_buki = p.skeleton.index_of("R_buki").expect("R_buki");
    let mut faults = Vec::new();
    for c in &all {
        let Some(sora) = SoraClip::load(&pack, &p.skeleton, &c.stem, &moving) else {
            faults.push(format!("{} {}: Sora has no clip {}", c.row.id, c.logical, c.stem));
            continue;
        };
        let form = t.form(&c.keyblade, c.row.weapon_form, "").expect("the row's form");
        let base = t.keyblade(&c.keyblade).map(|k| k.base.clone()).unwrap_or_default();
        let skel = t.model(&c.unit.model).and_then(|m| m.skeleton.clone()).expect("the unit's skeleton");
        // START: the unit's Keyblade bone (when shown) on his hand socket
        let start = sora.pose(0.0);
        let hand = hand_socket().then(&start[r_buki]).translation;
        let unit0 = place(&t, &c.unit, &p.skeleton, &start, Some(&c.logical), 0.0).expect("the unit is placed");
        let kb_bone = skel.bones.iter().position(|b| b.name.eq_ignore_ascii_case(&base));
        let d0 = kb_bone.and_then(|k| shown(&unit0, &unit0.to_world).into_iter().find(|(b, _)| *b == k)).map_or(0.0, |(_, at)| (at - hand).length());
        // END: every weapon of the form has a shown bone of the unit on its drawn root
        let end_t = sora.length();
        let end = sora.pose(end_t);
        let unit1 = place(&t, &c.unit, &p.skeleton, &end, Some(&c.logical), end_t).expect("the unit is placed");
        let bones = shown(&unit1, &unit1.to_world);
        let mut line = format!("{} {} ({} on {}, {:.2} s): Keyblade at the start {d0:.1} cm", c.row.id, c.row.name, c.logical, c.unit.model, end_t);
        let mut worst = d0;
        for w in &form.weapons {
            let Some(drawn) = place(&t, w, &p.skeleton, &end, None, 0.0) else { continue };
            let d = nearest(&bones, drawn.to_world.translation);
            line += &format!(", {} at the end {d:.1} cm", w.model);
            worst = worst.max(d);
        }
        println!("{line}");
        if worst > 3.0 {
            faults.push(line);
        }
    }
    for f in &faults {
        println!("DETACHED: {f}");
    }
    assert!(faults.is_empty(), "{} form-change clip(s) with a weapon off his hand (see DETACHED lines)", faults.len());
}

/// An outfit's pose of his frame (None on KH3 Sora): its own (`Retarget::pose`) and its bones in his skeleton's order
/// (`Retarget::effect_component`).
type OutfitView = (Option<Vec<Trs>>, Option<Vec<Trs>>);

/// The pack's outfits (outfits.json) but the main one.
fn outfits(pack: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(pack.join("outfits").join("outfits.json")).unwrap_or_default();
    let json: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).unwrap_or(serde_json::Value::Null);
    json["outfits"].as_array().into_iter().flatten().filter(|o| o["main"].as_bool() != Some(true)).filter_map(|o| o["id"].as_str().map(str::to_string)).collect()
}

#[test]
fn every_form_weapon_part_is_on_its_mount_in_every_outfit_and_the_cinematic_hands_over_without_a_jump() {
    let Some((t, p, pack)) = pack() else { return };
    let moving = root_motion_clips(&pack);
    let mut rigs: Vec<(String, Option<kh_er_render::outfit::Retarget>)> = vec![("kh3".to_string(), None)];
    for id in outfits(&pack) {
        match kh_er_render::outfit::retarget_for(&pack, &id, &p.skeleton) {
            Ok(Some(r)) => rigs.push((id, Some(r))),
            other => println!("outfit {id}: no retarget ({:?})", other.err()),
        }
    }
    assert!(rigs.len() >= 3, "KH3 and at least two outfits");
    let mut faults = Vec::new();
    let mut measured = 0usize;
    for row in kh_core::combat::form::KEYBLADE_FORMS.iter().filter(|r| r.weapon_form_value > 1) {
        let Some(kb) = t.keyblade(row.keyblade) else { continue };
        // his pose at the end of the form's change clip (the new weapon in his hand), else his reference pose
        let change = t.form(&kb.id, row.weapon_form, "").and_then(|f| f.transform.iter().find(|u| u.on_mesh).cloned()).and_then(|u| {
            let (logical, file) = t.model(&u.model)?.clips.iter().find(|(l, _)| l == row.transform_clip)?.clone();
            let stem = file.file_stem()?.to_string_lossy().to_string();
            Some((u, logical, SoraClip::load(&pack, &p.skeleton, &stem, &moving)?))
        });
        let pose = change.as_ref().map_or_else(|| kh_anim::bonamik::reference_pose(&p.skeleton), |(_, _, s)| s.pose(s.length()));
        let views: Vec<OutfitView> = rigs.iter().map(|(_, r)| (r.as_ref().map(|r| r.pose(&[], &pose)), r.as_ref().map(|r| r.effect_component(&p.skeleton, &[], &pose)))).collect();
        // 2. every weapon part of every grow level on its mount (an outfit's: that outfit's bone)
        let mut seen: Vec<String> = Vec::new();
        for grow in ["1", "2", "3"] {
            let Some(form) = t.form(&kb.id, row.weapon_form, grow) else { continue };
            for w in &form.weapons {
                if seen.contains(&w.model) {
                    continue;
                }
                seen.push(w.model.clone());
                let Some(drawn) = place(&t, w, &p.skeleton, &pose, None, 0.0) else { continue };
                let bone = w.bone.as_deref().unwrap_or("R_buki");
                let Some(bi) = p.skeleton.index_of(bone) else { continue };
                let socket = w.socket.unwrap_or_else(hand_socket);
                for ((id, rig), (posed, body)) in rigs.iter().zip(&views) {
                    let (want, got) = match (rig, posed, body) {
                        (Some(r), Some(posed), Some(body)) => (socket.then(&body[bi]).translation, r.weapon_to_world(&drawn.to_world, &pose, posed, &Trs::IDENTITY).translation),
                        _ => (socket.then(&pose[bi]).translation, drawn.to_world.translation),
                    };
                    let d = (got - want).length();
                    measured += 1;
                    if d > 2.0 {
                        faults.push(format!("{} {} {} on {bone} in {id}: {d:.1} cm off its mount", row.id, row.name, w.model));
                    }
                }
            }
        }
        // 3. the handover: the unit's last cinematic frame (outfit-moved as the renderer moves it) on the form's weapons
        if let Some((u, logical, sora)) = change.as_ref() {
            let end_t = sora.length();
            let (Some(unit), Some(form)) = (place(&t, u, &p.skeleton, &pose, Some(logical), end_t), t.form(&kb.id, row.weapon_form, "")) else { continue };
            for ((id, rig), (posed, _)) in rigs.iter().zip(&views) {
                // as the renderer draws it (gfx.rs: outfit.rs `mesh_weapon_pose` on the unit's own skeleton)
                let skeleton = t.model(&u.model).and_then(|m| m.skeleton.clone()).expect("the unit's skeleton");
                let drawn_unit = match (rig, posed) {
                    (Some(r), Some(posed)) => FormWeapon { component: r.mesh_weapon_pose(&unit.to_world, &unit.component, &skeleton, &pose, posed, &Trs::IDENTITY), ..unit.clone() },
                    _ => unit.clone(),
                };
                let bones = shown(&drawn_unit, &drawn_unit.to_world);
                for w in &form.weapons {
                    let Some(drawn) = place(&t, w, &p.skeleton, &pose, None, 0.0) else { continue };
                    let after = match (rig, posed) {
                        (Some(r), Some(posed)) => r.weapon_to_world(&drawn.to_world, &pose, posed, &Trs::IDENTITY).translation,
                        _ => drawn.to_world.translation,
                    };
                    let d = nearest(&bones, after);
                    measured += 1;
                    if d > 3.0 {
                        faults.push(format!("{} {} handover of {} in {id}: the cinematic's last frame is {d:.1} cm from the weapon's first", row.id, row.name, w.model));
                    }
                }
            }
        }
    }
    println!("{measured} placements measured over {} bodies (KH3 + outfits)", rigs.len());
    for f in &faults {
        println!("DETACHED: {f}");
    }
    assert!(faults.is_empty(), "{} detached part(s) (see DETACHED lines)", faults.len());
}

// ---------------------------------------------------------------------------------------------------- (FORM-GRIP)

/// (FORM-GRIP) A placed model's skinned vertices on its shown bones (the CPU skin the renderer's vertex shader does).
fn skinned(t: &Table, f: &FormWeapon) -> Vec<Vec3> {
    let m = t.model(&f.model).expect("a model of the table");
    let model = kh_er_render::model::Model::load(&m.glb).expect("its glb");
    let sk = m.skeleton.clone().expect("its skeleton");
    let joints: Vec<Option<usize>> = model.joints.iter().map(|(n, _)| sk.index_of(n)).collect();
    let mat = |t: &Trs| glam::Mat4::from_scale_rotation_translation(t.scale, t.rotation, t.translation);
    let world = mat(&f.to_world);
    model
        .vertices
        .iter()
        .filter_map(|v| {
            let k = (0..4).max_by(|a, b| v.weights[*a].total_cmp(&v.weights[*b]))?;
            let main = joints[v.joints[k] as usize].and_then(|b| f.component.get(b))?;
            if main.scale.min_element() < 0.5 {
                return None;
            }
            let mut at = Vec3::ZERO;
            for k in (0..4).filter(|k| v.weights[*k] > 0.0) {
                let j = v.joints[k] as usize;
                let bone = joints[j].and_then(|b| f.component.get(b)).map(mat).unwrap_or(glam::Mat4::IDENTITY);
                at += v.weights[k] * (world * bone * model.joints[j].1).transform_point3(Vec3::from(v.position));
            }
            Some(at)
        })
        .collect()
}

/// (FORM-GRIP) The mean distance (cm) of `a`'s points (about 400 of them) to the nearest of `b`'s.
fn mean_nearest(a: &[Vec3], b: &[Vec3]) -> f32 {
    let picked: Vec<&Vec3> = a.iter().step_by((a.len() / 400).max(1)).collect();
    picked.iter().map(|p| b.iter().map(|q| (**p - *q).length_squared()).fold(f32::MAX, f32::min).sqrt()).sum::<f32>() / picked.len().max(1) as f32
}

/// (FORM-GRIP) THE USER (2026-11-09): "shield is held wrong" (Counter Shield standing idle: the shield sideways and back on his
/// right hand). KH3 attaches the weapon ACTOR to R_Wep3_skt (pitch 90, roll -90) and the shield's mesh component inside it is
/// turned yaw 270 (w_so032_wep .. w_so034_wep, Frying Pan's w_so142_Wep .. w_so144_Wep `TresWeaponMesh0`); the port hung the
/// mesh on the socket directly. KH3's own data says where the shield is: the transform unit (authored in his mesh space)
/// ends its form change exactly on it. Each shield's WHOLE SHAPE (its skinned vertices, not only a bone) must lie on the unit's
/// last frame, on Sora's pose of that frame. (Before: 12.0 cm for Guardian Shield, 7.6 cm for the Frying Pan.)
#[test]
fn both_shields_are_held_as_their_form_change_leaves_them() {
    let Some((t, p, pack)) = pack() else { return };
    let moving = root_motion_clips(&pack);
    let mut measured = 0;
    for c in changes(&t).iter().filter(|c| matches!(c.row.id, "form.SH" | "form.SH2")) {
        let sora = SoraClip::load(&pack, &p.skeleton, &c.stem, &moving).expect("Sora's clip of the change");
        let end_t = sora.length();
        let pose = sora.pose(end_t);
        let unit = place(&t, &c.unit, &p.skeleton, &pose, Some(&c.logical), end_t).expect("the unit is placed");
        let unit_shape = skinned(&t, &unit);
        let form = t.form(&c.keyblade, c.row.weapon_form, "").expect("the row's form");
        for w in &form.weapons {
            let drawn = place(&t, w, &p.skeleton, &pose, None, 0.0).expect("the shield is placed");
            let d = mean_nearest(&skinned(&t, &drawn), &unit_shape);
            println!("{} {}: {} on {:?} is {d:.2} cm from the unit {}'s last frame", c.row.id, c.row.name, w.model, w.bone, c.unit.model);
            assert!(d < 3.0, "{} {}: {} is {d:.1} cm off KH3's own unit at the end of its form change", c.row.id, c.row.name, w.model);
            measured += 1;
        }
    }
    assert!(measured >= 2, "Guardian Shield and the Frying Pan measured: {measured}");
}

/// (FORM-GRIP) KH3's blueprint of a form part or pawn, when the converted packages are here (None otherwise).
fn blueprint_json(blueprint: &str) -> Option<serde_json::Value> {
    let rel = blueprint.strip_prefix("/Game/")?;
    let file = root().join("assets_converted").join("packages").join("KINGDOM HEARTS III").join("Content").join(format!("{rel}.uasset.json"));
    serde_json::from_str(std::fs::read_to_string(file).ok()?.trim_start_matches('\u{feff}')).ok()
}

/// (FORM-GRIP) A component's relative (location, rotation) in a blueprint (zero when the data does not set it).
fn relative_of(bp: &serde_json::Value, component: &str) -> Option<([f64; 3], [f64; 3])> {
    let e = bp["exports"].as_array()?.iter().find(|e| e["objectName"].as_str() == Some(component))?;
    let props = e["data"]["props"].as_array()?;
    let get = |name: &str, keys: [&str; 3]| props.iter().find(|p| p["name"].as_str() == Some(name)).map_or([0.0; 3], |p| keys.map(|k| p["value"][k].as_f64().unwrap_or(0.0)));
    Some((get("RelativeLocation", ["x", "y", "z"]), get("RelativeRotation", ["pitch", "yaw", "roll"])))
}

/// (FORM-GRIP) EVERY form weapon's mesh component as KH3's blueprints set it: the weapons' `TresWeaponMesh0` turns are
/// exactly keyblade.rs `WEAPON_MESH_TURNS` (none sets a place). Skipped without the converted packages.
#[test]
fn every_form_weapon_mesh_turn_is_kh3s() {
    let Some((t, _, _)) = pack() else { return };
    let json: serde_json::Value = serde_json::from_str(std::fs::read_to_string(t.dir.join("manifest.json")).unwrap().trim_start_matches('﻿')).unwrap();
    let mut weapons: Vec<(String, String)> = Vec::new();
    for k in json["keyblades"].as_object().into_iter().flatten().map(|(_, k)| k) {
        for f in k["forms"].as_array().into_iter().flatten() {
            for u in f["weapons"].as_array().into_iter().flatten().chain(f["transform"].as_array().into_iter().flatten()) {
                if let (Some(bp), Some(m)) = (u["blueprint"].as_str(), u["model"].as_str()) {
                    weapons.push((bp.to_string(), m.to_string()));
                }
            }
        }
    }
    weapons.sort();
    weapons.dedup();
    let near = |a: [f64; 3], b: [f32; 3]| a.iter().zip(b).all(|(x, y)| (x - f64::from(y)).abs() < 1.0e-3);
    let mut read = 0;
    let mut turned = 0;
    let mut faults = Vec::new();
    for (bp, model) in &weapons {
        let Some(json) = blueprint_json(bp) else { continue };
        let Some((at, rot)) = relative_of(&json, "TresWeaponMesh0") else { continue };
        read += 1;
        let want = kh_er_render::keyblade::WEAPON_MESH_TURNS.iter().find(|(m, _)| m == model).map_or([0.0; 3], |(_, r)| *r);
        if want != [0.0; 3] {
            turned += 1;
        }
        if !near(rot, want) || at != [0.0; 3] {
            faults.push(format!("{bp} ({model}): TresWeaponMesh0 at {at:?} turned {rot:?}, the port turns {want:?}"));
        }
    }
    if read == 0 {
        eprintln!("SKIPPED: no converted blueprints");
        return;
    }
    println!("{read} form weapon blueprints read, {turned} with a mesh turn");
    assert!(faults.is_empty(), "{faults:#?}");
    assert_eq!(turned, kh_er_render::keyblade::WEAPON_MESH_TURNS.len(), "every row of the table is a form weapon of the manifest");
}
