//! Reading the Sora pack (`build/sora_pack/`, written by `tools/sora_pack.js`) into plain arrays,
//! without any engine's glTF types: the skeleton from `manifest.json`, the clips from the JSON
//! chunk and the binary chunk of each `anims/<sequence>.glb`.
//!
//! The pack is in glTF space (metres, +Y up, right-handed, the character faces +Z); everything
//! here is converted to KH3's own space on the way in (the manifest's `conventions` inverted):
//!
//! | quantity | pack -> UE |
//! |---|---|
//! | position | (x, y, z) m -> (x, z, y) x 100 cm |
//! | rotation quaternion | (x, y, z, w) -> (-x, -z, -y, w) |
//! | scale | (x, y, z) -> (x, z, y) |
//!
//! UE space here is the MESH COMPONENT's: centimetres, Z up, left-handed, the character faces +Y
//! (the pawn turns the mesh component by -90 degrees of yaw).

use std::collections::BTreeMap;
use std::path::Path;

use glam::{Quat, Vec3};

use crate::json::Json;
use crate::Trs;

/// A pack position (metres, Y up) as a UE one (cm, Z up).
pub fn position_to_ue(p: Vec3) -> Vec3 {
    Vec3::new(p.x, p.z, p.y) * 100.0
}

/// A pack rotation as a UE one.
pub fn rotation_to_ue(q: Quat) -> Quat {
    Quat::from_xyzw(-q.x, -q.z, -q.y, q.w)
}

/// A pack scale as a UE one.
pub fn scale_to_ue(s: Vec3) -> Vec3 {
    Vec3::new(s.x, s.z, s.y)
}

/// The inverse conversions, for a host that draws in the pack's (glTF) space. The three mappings
/// are their own inverses up to the unit factor.
pub fn trs_to_pack(t: &Trs) -> Trs {
    Trs { translation: Vec3::new(t.translation.x, t.translation.z, t.translation.y) / 100.0, rotation: Quat::from_xyzw(-t.rotation.x, -t.rotation.z, -t.rotation.y, t.rotation.w), scale: Vec3::new(t.scale.x, t.scale.z, t.scale.y) }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Bone {
    pub name: String,
    /// Index of the parent bone; `None` for the root.
    pub parent: Option<usize>,
    /// The mesh reference pose, local, UE space.
    pub reference: Trs,
}

/// Sora's skeleton: 274 bones in the game's order (a parent always comes before its children).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Skeleton {
    pub bones: Vec<Bone>,
    index: BTreeMap<String, usize>,
}

impl Skeleton {
    /// From the text of `manifest.json` (`skeleton.bones[]`: the `*_ue` values are used as they are).
    pub fn from_manifest(manifest: &Json) -> Result<Skeleton, String> {
        let mut bones = Vec::new();
        for (i, b) in manifest.get("skeleton").get("bones").items().iter().enumerate() {
            let name = b.get("name").as_str().ok_or("skeleton bone without a name")?.to_string();
            let parent = b.get("parent").as_i64().filter(|p| *p >= 0).map(|p| p as usize);
            if parent.is_some_and(|p| p >= i) {
                return Err(format!("bone {name}: its parent does not come before it"));
            }
            let (t, r, s) = (b.get("translation_ue").floats(), b.get("rotation_ue").floats(), b.get("scale_ue").floats());
            if t.len() != 3 || r.len() != 4 || s.len() != 3 {
                return Err(format!("bone {name}: translation_ue / rotation_ue / scale_ue missing"));
            }
            bones.push(Bone { name, parent, reference: Trs { translation: Vec3::new(t[0], t[1], t[2]), rotation: Quat::from_xyzw(r[0], r[1], r[2], r[3]), scale: Vec3::new(s[0], s[1], s[2]) } });
        }
        if bones.is_empty() {
            return Err("manifest.json has no skeleton.bones".to_string());
        }
        let index = bones.iter().enumerate().map(|(i, b)| (b.name.clone(), i)).collect();
        Ok(Skeleton { bones, index })
    }

    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.index.get(name).copied()
    }

    pub fn len(&self) -> usize {
        self.bones.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bones.is_empty()
    }

    /// The reference pose, local.
    pub fn reference_pose(&self) -> Vec<Trs> {
        self.bones.iter().map(|b| b.reference).collect()
    }
}

/// The keys of one property of one bone: times in seconds, ascending.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Channel<T> {
    pub times: Vec<f32>,
    pub values: Vec<T>,
}

impl<T: Copy> Channel<T> {
    /// The two keys around `time` and the fraction between them; `None` when there are no keys.
    /// Outside the keys the nearest key is held.
    pub fn around(&self, time: f32) -> Option<(T, T, f32)> {
        let last = self.values.len().checked_sub(1)?;
        let after = self.times.partition_point(|t| *t <= time);
        if after == 0 {
            return Some((self.values[0], self.values[0], 0.0));
        }
        if after > last {
            return Some((self.values[last], self.values[last], 0.0));
        }
        let (t0, t1) = (self.times[after - 1], self.times[after]);
        Some((self.values[after - 1], self.values[after], if t1 > t0 { (time - t0) / (t1 - t0) } else { 0.0 }))
    }
}

/// One bone's keys in a clip, UE space.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Track {
    pub bone: String,
    pub translation: Channel<Vec3>,
    pub rotation: Channel<Quat>,
    pub scale: Channel<Vec3>,
}

/// One animation sequence: `anims/<name>.glb` (or `face/<name>.glb`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Clip {
    pub name: String,
    pub tracks: Vec<Track>,
}

impl Clip {
    pub fn load(path: &Path, name: &str) -> Result<Clip, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Clip::from_glb(&bytes, name).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Reads the first animation of a binary glTF file: per node its translation, rotation and
    /// scale keys (LINEAR samplers over float accessors: what the pack tool writes).
    pub fn from_glb(bytes: &[u8], name: &str) -> Result<Clip, String> {
        let u32_at = |at: usize| bytes.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize);
        if bytes.get(0..4) != Some(b"glTF".as_slice()) {
            return Err("not a .glb file".to_string());
        }
        let json_length = u32_at(12).ok_or("truncated")?;
        let text = std::str::from_utf8(bytes.get(20..20 + json_length).ok_or("truncated")?).map_err(|e| e.to_string())?;
        let json = Json::parse(text)?;
        let bin_start = 20 + json_length + 8;
        let bin_length = u32_at(20 + json_length).ok_or("no binary chunk")?;
        let bin = bytes.get(bin_start..bin_start + bin_length).ok_or("truncated binary chunk")?;
        // An accessor's floats, `components` per element.
        let floats = |accessor: usize, components: usize| -> Result<Vec<f32>, String> {
            let a = json.get("accessors").at(accessor);
            if a.get("componentType").as_usize() != Some(5126) {
                return Err("an animation accessor is not float".to_string());
            }
            let view = json.get("bufferViews").at(a.get("bufferView").as_usize().ok_or("accessor without a buffer view")?);
            let start = view.get("byteOffset").as_usize().unwrap_or(0) + a.get("byteOffset").as_usize().unwrap_or(0);
            let stride = view.get("byteStride").as_usize().unwrap_or(components * 4);
            let count = a.get("count").as_usize().unwrap_or(0);
            let mut out = Vec::with_capacity(count * components);
            for i in 0..count {
                for c in 0..components {
                    let at = start + i * stride + c * 4;
                    let b = bin.get(at..at + 4).ok_or("accessor runs past the binary chunk")?;
                    out.push(f32::from_le_bytes([b[0], b[1], b[2], b[3]]));
                }
            }
            Ok(out)
        };
        let animation = json.get("animations").at(0);
        if animation.is_null() {
            return Err("no animation in the file".to_string());
        }
        let mut tracks: BTreeMap<usize, Track> = BTreeMap::new();
        for channel in animation.get("channels").items() {
            let target = channel.get("target");
            let (Some(node), Some(path)) = (target.get("node").as_usize(), target.get("path").as_str()) else { continue };
            let sampler = animation.get("samplers").at(channel.get("sampler").as_usize().ok_or("channel without a sampler")?);
            let times = floats(sampler.get("input").as_usize().ok_or("sampler without input")?, 1)?;
            let output = sampler.get("output").as_usize().ok_or("sampler without output")?;
            let bone = json.get("nodes").at(node).get("name").as_str().ok_or("animated node without a name")?;
            let track = tracks.entry(node).or_insert_with(|| Track { bone: bone.to_string(), ..Track::default() });
            match path {
                "translation" => track.translation = Channel { times, values: floats(output, 3)?.chunks_exact(3).map(|v| position_to_ue(Vec3::new(v[0], v[1], v[2]))).collect() },
                "scale" => track.scale = Channel { times, values: floats(output, 3)?.chunks_exact(3).map(|v| scale_to_ue(Vec3::new(v[0], v[1], v[2]))).collect() },
                "rotation" => track.rotation = Channel { times, values: floats(output, 4)?.chunks_exact(4).map(|v| rotation_to_ue(Quat::from_xyzw(v[0], v[1], v[2], v[3]).normalize())).collect() },
                _ => {}
            }
        }
        Ok(Clip { name: name.to_string(), tracks: tracks.into_values().collect() })
    }
}

/// One notify of a sequence (`items[].notifies[]` of the manifest).
#[derive(Clone, Debug, PartialEq)]
pub struct Notify {
    pub class: String,
    /// A notify state (with an end) or an instant notify.
    pub state: bool,
    /// Trigger time (link time + `TriggerTimeOffset`), seconds.
    pub trigger: f32,
    /// End trigger of a state, seconds.
    pub end: f32,
    pub properties: Json,
}

/// What the manifest says about one sequence.
#[derive(Clone, Debug, PartialEq)]
pub struct ClipInfo {
    pub length: f32,
    /// `bEnableRootMotion`: played by the pawn (never with mode bit 0x10), such a sequence has its
    /// root bone locked to the mesh reference root (spec 7, change A12).
    pub root_motion_enabled: bool,
    /// `m_InterpolationTime`: the default blend-in of a play call.
    pub interpolation_time: f32,
    pub notifies: Vec<Notify>,
}

/// The pack on disk: the manifest, the skeleton, clips by sequence name, the data files.
#[derive(Clone, Debug)]
pub struct Pack {
    pub manifest: Json,
    pub skeleton: Skeleton,
    /// The clips read at load: the stage-1 body clips (`anims/`) and the face clips (`face/`),
    /// by sequence name.
    pub clips: BTreeMap<String, Clip>,
    /// The files of the other body clips (every other row of the anim set, pack 1.5.0: several
    /// hundred), by sequence name: read on first use (`Animator`).
    pub lazy: BTreeMap<String, std::path::PathBuf>,
    pub info: BTreeMap<String, ClipInfo>,
    /// `data/legik.json` and `data/kinedriver.json`, when the pack has them.
    pub leg_ik: Option<Json>,
    pub kine_driver: Option<Json>,
    /// `data/bonamik.json` (the Bonamik asset data), when the pack has it.
    pub bonamik: Option<Json>,
    /// (party I2) `data/lookat.json` (a friend's TresAnimNode_ExLookAt + anim-instance look-at data), when the pack has it.
    pub look_at: Option<Json>,
}

impl Pack {
    /// Reads `manifest.json`, every animation item it lists (kinds `animation` and
    /// `face-animation`) and the data files the pose needs.
    pub fn load(dir: &Path) -> Result<Pack, String> {
        let text = std::fs::read_to_string(dir.join("manifest.json")).map_err(|e| format!("{}: {e}", dir.join("manifest.json").display()))?;
        let manifest = Json::parse(&text)?;
        let skeleton = Skeleton::from_manifest(&manifest)?;
        let data = |file: &str| -> Result<Option<Json>, String> {
            let path = dir.join("data").join(file);
            if !path.is_file() {
                return Ok(None);
            }
            let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            Json::parse(&text).map(Some).map_err(|e| format!("{}: {e}", path.display()))
        };
        // Sequences of the rows marked `stage1: false` (and of no stage-1 row) are not read now.
        let mut later: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        let mut now: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for row in manifest.get("logical_animations").items() {
            let extra = row.get("stage1").as_bool() == Some(false);
            let stem = |file: &str| file.rsplit('/').next().unwrap_or(file).trim_end_matches(".glb").to_string();
            let mut names: Vec<String> = row.get("samples").items().iter().filter_map(|s| s.get("animation_name").as_str().map(str::to_string)).collect();
            if let Some(file) = row.get("file").as_str() {
                names.push(stem(file));
            }
            for name in names {
                if extra {
                    later.insert(name);
                } else {
                    now.insert(name);
                }
            }
        }
        let mut pack = Pack { skeleton, clips: BTreeMap::new(), lazy: BTreeMap::new(), info: BTreeMap::new(), leg_ik: data("legik.json")?, kine_driver: data("kinedriver.json")?, bonamik: data("bonamik.json")?, look_at: data("lookat.json")?, manifest };
        let mut files: Vec<(String, String)> = Vec::new();
        for item in pack.manifest.get("items").items() {
            if !matches!(item.get("kind").as_str(), Some("animation") | Some("face-animation")) {
                continue;
            }
            let (Some(name), Some(file)) = (item.get("animation_name").as_str(), item.get("file").as_str()) else { continue };
            let properties = item.get("sequence_properties");
            let notifies = item
                .get("notifies")
                .items()
                .iter()
                .map(|n| {
                    let trigger = n.get("trigger_time").as_f32().unwrap_or(0.0);
                    let state = n.get("kind").as_str() == Some("state");
                    Notify { class: n.get("class").as_str().unwrap_or("").to_string(), state, trigger, end: if state { n.get("end_trigger_time").as_f32().unwrap_or(trigger) } else { trigger }, properties: n.get("properties").clone() }
                })
                .collect();
            pack.info.insert(
                name.to_string(),
                ClipInfo {
                    length: item.get("duration_s").as_f32().unwrap_or(0.0),
                    root_motion_enabled: properties.get("bEnableRootMotion").as_bool().unwrap_or(false),
                    interpolation_time: properties.get("m_InterpolationTime").as_f32().unwrap_or(0.0),
                    notifies,
                },
            );
            files.push((name.to_string(), file.to_string()));
        }
        for (name, file) in files {
            if later.contains(&name) && !now.contains(&name) {
                pack.lazy.insert(name, dir.join(file));
            } else {
                pack.load_clip(dir, &name, &file)?;
            }
        }
        pack.merge_overlay(dir);
        Ok(pack)
    }

    /// (XSI, 2026-10-16) THE XS ABILITIES' CLIPS (tools/xsi_pack.js): `xsi_manifest.json` next to the manifest once deployed,
    /// else the folder of the environment variable `KH_PACK_OVERLAY` (the staged build/sora_tmp/xsi/pack, for test runs before
    /// the deploy). Its animation items are read on first use, as the other non-stage-1 clips; a clip the main manifest has
    /// stays the main one's. A missing or unreadable overlay changes nothing.
    fn merge_overlay(&mut self, dir: &Path) {
        let mut roots = vec![dir.to_path_buf()];
        if let Some(extra) = std::env::var_os("KH_PACK_OVERLAY").filter(|v| !v.is_empty()) {
            roots.push(std::path::PathBuf::from(extra));
        }
        // the staged copy beside the pack (build/sora_pack -> build/sora_tmp/xsi/pack), before the deploy
        roots.push(dir.join("..").join("sora_tmp").join("xsi").join("pack"));
        for root in roots {
            let Ok(text) = std::fs::read_to_string(root.join("xsi_manifest.json")) else { continue };
            let Ok(overlay) = Json::parse(&text) else { continue };
            for item in overlay.get("items").items() {
                if item.get("kind").as_str() != Some("animation") {
                    continue;
                }
                let (Some(name), Some(file)) = (item.get("animation_name").as_str(), item.get("file").as_str()) else { continue };
                if self.info.contains_key(name) {
                    continue;
                }
                let properties = item.get("sequence_properties");
                let notifies = item
                    .get("notifies")
                    .items()
                    .iter()
                    .map(|n| {
                        let trigger = n.get("trigger_time").as_f32().unwrap_or(0.0);
                        let state = n.get("kind").as_str() == Some("state");
                        Notify { class: n.get("class").as_str().unwrap_or("").to_string(), state, trigger, end: if state { n.get("end_trigger_time").as_f32().unwrap_or(trigger) } else { trigger }, properties: n.get("properties").clone() }
                    })
                    .collect();
                self.info.insert(
                    name.to_string(),
                    ClipInfo { length: item.get("duration_s").as_f32().unwrap_or(0.0), root_motion_enabled: properties.get("bEnableRootMotion").as_bool().unwrap_or(false), interpolation_time: properties.get("m_InterpolationTime").as_f32().unwrap_or(0.0), notifies },
                );
                self.lazy.insert(name.to_string(), root.join(file));
            }
            return;
        }
    }

    pub fn load_clip(&mut self, dir: &Path, name: &str, file: &str) -> Result<(), String> {
        let clip = Clip::load(&dir.join(file), name)?;
        self.clips.insert(name.to_string(), clip);
        Ok(())
    }
}
