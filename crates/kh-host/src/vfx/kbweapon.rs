//! (AUDIT-B F-SHIP) THE FORM WEAPONS' OWN BONES AS EFFECT ANCHORS. KH3 gives each weapon actor of a form (its units, its
//! transform units) and each style pawn (the Kraken) an anim instance of its own, which plays the row of the logical name
//! Sora plays; the notifies of that clip attach their particles to THAT mesh's bones and sockets (`SQEX_ATTACH_OBJECT_OWNER`
//! of the weapon's own notify), and the Keyblade trail of a form weapon (`ead_key_trail0` -> the weapon's common
//! `key_trail0`) runs between ITS `vfx_top` / `vfx_bottom`. The core names such a place
//! `kb:<model>:<mount>:<bone>[:abs][:free][:fo<seconds>]` (kh-core combat/forms/ship.rs `wnotify` rows):
//! - `<model>` a model of the keyblades pack (`build/sora_pack/keyblades/<model>/`: skeleton.json, sockets.json, model.json
//!   and its clips anims/<sequence>.glb, the asset agent's pack),
//! - `<mount>` the socket of Sora's the weapon stands on (R_wep5_skt ...: the part's own socket, R_wep_skt for a unit
//!   with none, as P5 draws it: kh-er-render keyblade.rs `place_part`), or `root` = his mesh component (a pawn standing
//!   where he stands: `form_pawn` with his place and yaw),
//! - `<bone>` a bone or a socket (sockets.json `weapon_sockets`) of the weapon's skeleton, empty = its component root,
//! - `abs` bAbsoluteRotation, `free` ToAttach false (left in the world where it starts), `fo<s>` the FADEOUT end's time.
//!
//! The weapon's pose is its row of the sequence Sora plays now at his clip's time (without one, (DRILL-IDLE) its anim
//! blueprint's default clip `ResetPose` on its own clock, KH3's UTresAnimInstance 0x14056ECE0; its reference pose only
//! without that), as the renderer draws it (`model_pose`).

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

use glam::{Mat3, Vec3};
use kh_anim::formweapon::{DefaultClock, DefaultPlay, WeaponRig};
use kh_vfx::Xform;

/// A parsed `kb:` anchor.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct KbAnchor<'a> {
    pub model: &'a str,
    pub mount: &'a str,
    pub bone: &'a str,
    pub absolute: bool,
    pub free: bool,
    pub fade_out: Option<f32>,
    /// (AUDIT-B W-VFX) `to<socket>`: the attach row's EndSocketName (a TypeDataSQEX_Thunder beam's end, SetThunderAttachBones)
    pub to: Option<&'a str>,
}

/// `kb:<model>:<mount>:<bone>[:flags]` -> its parts (None: not such a name).
pub(super) fn parse(name: &str) -> Option<KbAnchor<'_>> {
    let rest = name.strip_prefix("kb:")?;
    let mut parts = rest.split(':');
    let (model, mount, bone) = (parts.next()?, parts.next()?, parts.next().unwrap_or(""));
    let mut a = KbAnchor { model, mount, bone, absolute: false, free: false, fade_out: None, to: None };
    for flag in parts {
        match flag {
            "abs" => a.absolute = true,
            "free" => a.free = true,
            // (AUDIT-B W-VFX) the beam's end socket (kept apart from the fade-out flag)
            f if f.starts_with("to") => a.to = f.strip_prefix("to").filter(|t| !t.is_empty()),
            f => a.fade_out = f.strip_prefix("fo").and_then(|s| s.parse().ok()),
        }
    }
    Some(a)
}

/// The same anchor on another bone of the same weapon (the trail's `vfx_bottom` beside its `vfx_top`).
pub(super) fn with_bone(name: &str, bone: &str) -> Option<String> {
    let a = parse(name)?;
    Some(format!("kb:{}:{}:{bone}", a.model, a.mount))
}

struct Model {
    dir: PathBuf,
    /// sequence -> clip file
    clips: Vec<(String, PathBuf)>,
    /// socket -> (bone, local location, local rotation (pitch, yaw, roll))
    sockets: HashMap<String, (String, Vec3, Vec3)>,
    /// sequence -> its rig (None: not readable)
    rigs: HashMap<String, Option<WeaponRig>>,
    /// (AUDIT-B F-HAMMER) (logical name, sequence) of its clips (model.json `logical_names`)
    logical: Vec<(String, String)>,
    /// (DRILL-IDLE) the sequence of its default clip (model.json `default_anim`), how it plays (PACK/keyblades/
    /// anim_instances.json) and its clock
    default: Option<String>,
    default_play: DefaultPlay,
    clock: DefaultClock,
}

/// The weapon models read so far (lazily, from `<pack>/keyblades/<model>/`).
pub(super) struct KbRigs {
    root: PathBuf,
    models: RefCell<HashMap<String, Option<Model>>>,
}

fn v3(v: Option<&serde_json::Value>) -> Vec3 {
    let a = |i: usize| v.and_then(|v| v.get(i)).and_then(serde_json::Value::as_f64).unwrap_or(0.0) as f32;
    Vec3::new(a(0), a(1), a(2))
}

impl KbRigs {
    /// `pack` = the pack folder (`build/sora_pack`).
    pub(super) fn new(pack: PathBuf) -> KbRigs {
        KbRigs { root: pack.join("keyblades"), models: RefCell::new(HashMap::new()) }
    }

    fn read(&self, model: &str) -> Option<Model> {
        let dir = self.root.join(model);
        let text = std::fs::read_to_string(dir.join("model.json")).ok()?;
        let json: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
        let clips = json["clips"].as_array().map(|a| a.iter().filter_map(|c| Some((c["sequence"].as_str()?.to_string(), dir.join(c["file"].as_str()?)))).collect()).unwrap_or_default();
        let sockets = std::fs::read_to_string(dir.join("sockets.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(t.trim_start_matches('\u{feff}')).ok())
            .and_then(|j| j.get("weapon_sockets").and_then(serde_json::Value::as_object).map(|o| o.iter().map(|(k, s)| (k.clone(), (s["bone"].as_str().unwrap_or("").to_string(), v3(s.get("local_location")), v3(s.get("local_rotation"))))).collect()))
            .unwrap_or_default();
        // (AUDIT-B F-HAMMER) the clips' logical names: a weapon whose own sequence differs from Sora's (Clock Drill's w_so113
        // plays L411 while he plays 6411, both M_6411) plays its row of the logical name he plays
        let logical: Vec<(String, String)> = json["clips"].as_array().into_iter().flatten().flat_map(|c| c["logical_names"].as_array().into_iter().flatten().filter_map(|n| Some((n.as_str()?.to_string(), c["sequence"].as_str()?.to_string())))).collect();
        // (DRILL-IDLE) the default clip: its logical name -> its sequence; its loop / check-same bits beside the manifest
        let instance = std::fs::read_to_string(self.root.join("anim_instances.json")).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(t.trim_start_matches('\u{feff}')).ok()).map(|j| j["models"][model].clone()).unwrap_or(serde_json::Value::Null);
        let default_name = instance["default_anim"].as_str().or_else(|| json["default_anim"].as_str()).filter(|n| !n.is_empty() && *n != "None").map(str::to_string);
        let default = default_name.and_then(|n| logical.iter().find(|(l, _)| *l == n).map(|(_, s)| s.clone()));
        let default_play = DefaultPlay { looping: instance["default_anim_loop"].as_bool().unwrap_or(false), check_same: instance["default_anim_check_same"].as_bool().unwrap_or(false) };
        Some(Model { dir, clips, sockets, rigs: HashMap::new(), logical, default, default_play, clock: DefaultClock::default() })
    }

    /// Where `bone` (a bone or a socket; "" = the root) of `model` is in the WEAPON'S component frame while Sora plays
    /// `sequence` at `time` (its row of that sequence, else its reference pose). None: no such model / bone.
    pub(super) fn local(&self, model: &str, bone: &str, sequence: &str, time: f32) -> Option<Xform> {
        self.local_logical(model, bone, sequence, None, time)
    }

    /// (FORMWEP-FIX) Whether `model` plays a row of its OWN while Sora plays `sequence` (its row of that sequence, of
    /// `M_<sequence>`, or of the blend space's logical name), not its default clip or its reference pose: [`KbRigs::local_logical`]'s
    /// rule, as the renderer's `rig_of`.
    pub(super) fn plays_own(&self, model: &str, sequence: &str, logical: Option<&str>) -> bool {
        let mut models = self.models.borrow_mut();
        let Some(m) = models.entry(model.to_string()).or_insert_with(|| self.read(model)).as_ref() else { return false };
        let own = own_row(m, sequence, logical);
        m.clips.iter().any(|(s, _)| *s == own)
    }

    /// (AUDIT-B W-DATA) [`KbRigs::local`], and while Sora plays one of the form's BLEND-SPACE rows (`logical` = its logical
    /// name, kh-core `forms::weapon_logical`: Twin Yo-Yos' MagicFire4 -> "MagicFire") the weapon's own clip of that logical name
    /// when it has no row of the sample's sequence: KH3's weapon anim instance plays its row of the name Sora's state asked for.
    pub(super) fn local_logical(&self, model: &str, bone: &str, sequence: &str, logical: Option<&str>, time: f32) -> Option<Xform> {
        let mut models = self.models.borrow_mut();
        let m = models.entry(model.to_string()).or_insert_with(|| self.read(model)).as_mut()?;
        let skeleton = m.dir.join("skeleton.json");
        let own = own_row(m, sequence, logical);
        // (DRILL-IDLE) without its own row: its default clip on the default's clock (else, without one, its reference pose)
        let has_own = m.clips.iter().any(|(s, _)| *s == own);
        let default_time = m.default.is_some().then(|| m.clock.advance(sequence, time, has_own, m.default_play)).flatten();
        let fallback = default_time.and(m.default.as_ref()).and_then(|d| m.clips.iter().find(|(s, _)| s == d));
        let wanted = m.clips.iter().find(|(s, _)| *s == own).or(fallback).or_else(|| m.clips.first()).cloned();
        let (seq, file) = wanted?;
        let playing = seq == own;
        let as_default = !playing && default_time.is_some() && m.default.as_deref() == Some(seq.as_str());
        let play = m.default_play;
        let rig = m.rigs.entry(seq).or_insert_with(|| WeaponRig::load(&skeleton, &file).ok()).as_ref()?;
        let pose = match default_time {
            _ if playing => rig.component(time.clamp(0.0, rig.length())),
            Some(clock) if as_default => rig.component(DefaultClock::sample_time(clock, rig.length(), play)),
            _ => rig.reference(),
        };
        let socket = m.sockets.get(bone).or_else(|| m.sockets.iter().find(|(k, _)| k.eq_ignore_ascii_case(bone)).map(|(_, v)| v)); // (AUDIT-B F-HAMMER) FName
        let (bone_name, relative) = match socket {
            Some((b, loc, rot)) => (b.as_str(), Some(Xform { position: *loc, rotation: kh_vfx::rotator(rot.x, rot.y, rot.z), scale: Vec3::ONE })),
            None => (bone, None),
        };
        let at = if bone_name.is_empty() {
            Xform { position: Vec3::ZERO, rotation: Mat3::IDENTITY, scale: Vec3::ONE }
        } else {
            // (AUDIT-B F-HAMMER) UE's FName compares without case (w_so051's StyleChange3 rows name `W_so050`, its bone is `w_so050`)
            let index = rig.skeleton.index_of(bone_name).or_else(|| rig.skeleton.bones.iter().position(|b| b.name.eq_ignore_ascii_case(bone_name)))?;
            let t = pose.get(index)?;
            Xform { position: t.translation, rotation: Mat3::from_quat(t.rotation), scale: t.scale }
        };
        Some(match relative {
            Some(r) => at.then(&r),
            None => at,
        })
    }
}

/// The sequence of `m`'s own row while Sora plays `sequence`: its row of that sequence; else (AUDIT-B F-HAMMER) its row of the
/// logical name M_<his sequence> (the renderer's `rig_of` rule); else (AUDIT-B W-DATA), with none of those, its row of the
/// blend space's logical name. (FORMWEP-FIX: cut out of `local_logical`, shared with `plays_own`.)
fn own_row(m: &Model, sequence: &str, logical: Option<&str>) -> String {
    let own = m.logical.iter().find(|(l, _)| l.strip_prefix("M_") == Some(sequence)).map_or(sequence, |(_, s)| s.as_str()).to_string();
    match logical {
        Some(l) if !m.clips.iter().any(|(s, _)| *s == own) => m.logical.iter().find(|(n, _)| n == l).map_or(own, |(_, s)| s.clone()),
        _ => own,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchors_parse_with_their_flags() {
        let a = parse("kb:w_so091:R_wep_skt:center:abs:free").unwrap();
        assert_eq!((a.model, a.mount, a.bone, a.absolute, a.free, a.fade_out), ("w_so091", "R_wep_skt", "center", true, true, None));
        let b = parse("kb:w_so093:R_wep6_skt:vfx_top:fo0.5").unwrap();
        assert_eq!((b.bone, b.fade_out), ("vfx_top", Some(0.5)));
        assert_eq!(parse("kb:w_so094:root:").unwrap().bone, "");
        // (AUDIT-B W-VFX) the drill rope's end socket, beside a fade-out
        let c = parse("kb:w_so053:R_ude2_wep1_skt:himo15:fo0.25:tohimo16").unwrap();
        assert_eq!((c.bone, c.fade_out, c.to), ("himo15", Some(0.25), Some("himo16")));
        assert!(parse("R_wep_skt").is_none());
        assert_eq!(with_bone("kb:w_so092:R_wep5_skt:vfx_top", "vfx_bottom").as_deref(), Some("kb:w_so092:R_wep5_skt:vfx_bottom"));
    }

    /// (DRILL-IDLE) an effect anchored on Drill Punch's drill head while Sora idles (a sequence w_so053 has no row of) sits
    /// where the renderer draws the head: its ResetPose's folded rope, at the gauntlet, not the bind pose's 1.5 m out
    #[test]
    fn a_row_less_play_anchors_on_the_default_clip() {
        let pack = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../build/sora_pack"));
        if !pack.join("keyblades").join("anim_instances.json").is_file() || !pack.join("keyblades").join("w_so053").join("model.json").is_file() {
            eprintln!("SKIP: no pack");
            return;
        }
        let rigs = KbRigs::new(pack);
        let base = rigs.local("w_so053", "dorill_base", "6001", 0.2).unwrap();
        let head = rigs.local("w_so053", "dorill_all", "6001", 0.2).unwrap();
        let thrown = rigs.local("w_so053", "dorill_all", "6421", 0.9).unwrap();
        let near = head.position.distance(base.position);
        assert!(near < 10.0, "the head is on the gauntlet in idle: {near}");
        assert!(thrown.position.distance(base.position) > near, "its own 6421 still throws it");
    }

    /// (AUDIT-B W-DATA) Twin Yo-Yos' yo-yo under Sora's MagicFire4 blend: with the heaviest sample 4321U (a clip it has no row
    /// of) it still plays its own 4321 "MagicFire" once the logical name is given; without it, its default clip (DRILL-IDLE; was its reference pose). Skipped
    /// without the pack.
    #[test]
    fn a_blend_space_play_poses_the_weapon_by_its_logical_name() {
        let pack = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../build/sora_pack"));
        if !pack.join("keyblades").join("w_so064").join("model.json").is_file() {
            eprintln!("SKIP: no pack");
            return;
        }
        let rigs = KbRigs::new(pack);
        let own = rigs.local("w_so064", "vfx_himo08", "4321", 0.35).unwrap();
        let by_name = rigs.local_logical("w_so064", "vfx_himo08", "4321U", Some("MagicFire"), 0.35).unwrap();
        let reference = rigs.local("w_so064", "vfx_himo08", "4321U", 0.35).unwrap();
        assert!(own.position.distance(by_name.position) < 1.0e-3, "{:?} vs {:?}", own.position, by_name.position);
        assert!(own.position.distance(reference.position) > 1.0, "the clip moves the string's middle ({:?} vs the reference {:?})", own.position, reference.position);
        // a sequence the weapon has keeps its own row whatever the logical name says
        let kept = rigs.local_logical("w_so064", "vfx_himo08", "4321", Some("RunLoopB"), 0.35).unwrap();
        assert!(own.position.distance(kept.position) < 1.0e-3);
    }
}
