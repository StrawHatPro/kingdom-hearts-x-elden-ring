//! (VFX-2) THE EFFECTS' SKELETAL MESH COMPONENTS (kh_vfx `SkelInstance`: KH3's `TypeDataSQEX_SkeletalMesh`, traced in
//! kh-vfx def.rs `SkeletalDef`): Ghost Horde's afterimages (Sora's body p_ex001_0 held at clip 1316's 0.7 s, its Favorite
//! Deputy in the body's R_wep_skt), the keyblades' summon copies (vw_so0NN_app0), Mirage Staff's Sora copies ...
//!
//! KH3 makes a USkeletalMeshComponent of the asset, plays the entry's clip on it in single-node mode and gives EVERY slot
//! the emitter's (dynamic) material. Here the mesh is the pack's own skinned model of that asset (found by its source
//! mesh: `PACK/manifest.json` Sora, `PACK/weapon/manifest.json` the Kingdom Key, `PACK/keyblades/*/model.json`,
//! `PACK/party/*/manifest.json`), posed with kh_anim's sampling of the clip from the model's `anims/` (WeaponRig: the
//! keys as the game blends them) and skinned on the CPU into the effect mesh layout (`MESH_VERTEX_FLOATS`, the mesh's own
//! space, UE cm), so the material's translated mesh-particle shaders draw it as one instance. A held pose is skinned once
//! (the cache below); its vertices stay in an upload buffer the GPU reads (`gpu_mesh`). The dynamic material's parameters
//! (the emitter's SQEX_Material_* modules) are evaluated into the material's constants through the translator's
//! parameter trees (shading.json `params`, `eval_tree`).
//!
//! A component without the emitter's material (Warp Trick's yo-yo vw_so064_gte_yoyo01: Required material none) keeps its
//! mesh's own materials: it goes to the character pipeline as a form weapon (`FormWeapon`, as the routed fragments).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use glam::{Affine3A, Mat3, Mat4, Vec3};
use kh_anim::formweapon::WeaponRig;
use kh_anim::{Clip, Skeleton, Trs};
use serde_json::Value;
use windows::core::Interface;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R32_UINT;

use super::{SortKey, MESH_INSTANCE_FLOATS, MESH_VERTEX_FLOATS};
use crate::model::Model;
use crate::rlog;

/// A skinned model of the pack, its skeleton and where its clips are.
struct Asset {
    model: Model,
    skeleton: Skeleton,
    /// Per joint of the skin: its bone in the skeleton.
    joint_bones: Vec<Option<usize>>,
    /// Every primitive's indices with its base vertex added (one list for one draw).
    indices: Arc<Vec<u32>>,
    anims: PathBuf,
    /// The mesh's own sockets (a keyblade's `sockets.json`; `PACK/vfx/host.json` for Sora): name -> (bone, location, rotation).
    sockets: HashMap<String, (String, Vec3, Vec3)>,
    rigs: HashMap<String, Option<WeaponRig>>,
    reference: Vec<Trs>,
    /// The clips with `bEnableRootMotion` and their `RootMotionRootLock` (the manifest's `items`): UE locks such a clip's
    /// root in the pose (FRootMotionReset: RefPose = the reference, AnimFirstFrame = its first key, Zero = identity), as
    /// kh_anim's Animator::sample does for Sora's own clips; a skeletal mesh component's single-node play is no exception.
    locks: HashMap<String, RootLock>,
    /// (FINISH-AUDIT2) The clips of the overlay manifests (Sora's: `xsi_manifest.json`, `keyblades/sora_forms_manifest.json`,
    /// read as kh_anim's `Pack::merge_overlay` reads them): name -> file. A clip of the main manifest stays the main one's.
    /// Mirage Staff's copies (vw_so072_mrg_sora1 A411, vw_so073_mrg1 A417) play such clips: without this they stood in
    /// the reference (T) pose.
    files: HashMap<String, PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RootLock {
    RefPose,
    FirstFrame,
    Zero,
}

struct Lib {
    /// Asset base name (lower case) -> (glb, skeleton file, anims folder, sockets file).
    index: HashMap<String, (PathBuf, PathBuf, PathBuf, Option<PathBuf>)>,
    /// (FINISH-AUDIT2) Asset base name -> its overlay manifests (files relative to the first path), in kh_anim's order.
    overlays: HashMap<String, Vec<(PathBuf, PathBuf)>>,
    assets: HashMap<String, Option<Asset>>,
    /// Skinned vertices by (asset, clip, time) key, with the frame they were last asked for.
    skins: HashMap<u64, (Arc<Vec<f32>>, u64)>,
    frame: u64,
    told: std::collections::HashSet<String>,
}

static LIB: Mutex<Option<Lib>> = Mutex::new(None);

const SKIN_CACHE: usize = 24;

fn read_json(path: &Path) -> Option<Value> {
    std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(t.trim_start_matches('\u{feff}')).ok())
}

fn base(path: &str) -> String {
    kh_vfx::base_name(path).to_lowercase()
}

impl Lib {
    fn new(pack: &Path) -> Lib {
        let mut index = HashMap::new();
        let mut overlays = HashMap::new();
        // Sora (and the party members): the manifest's skeletal mesh
        let mut people = vec![(pack.join("manifest.json"), pack.join("sora.glb"), pack.join("anims"))];
        if let Ok(dir) = std::fs::read_dir(pack.join("party")) {
            for d in dir.flatten() {
                let p = d.path();
                people.push((p.join("manifest.json"), p.join("model.glb"), p.join("anims")));
            }
        }
        for (manifest, glb, anims) in people {
            if let Some(name) = read_json(&manifest).as_ref().and_then(|m| m.pointer("/identification/skeletal_mesh")).and_then(Value::as_str) {
                if glb.is_file() {
                    let host = pack.join("vfx").join("host.json");
                    if name.contains("p_ex001") {
                        // (FINISH-AUDIT2) Sora's overlay clips, as kh_anim Pack::merge_overlay: the XS abilities', then the
                        // keyblade forms' (A4xx: Mirage Staff's copies)
                        let keyblades = pack.join("keyblades");
                        overlays.entry(base(name)).or_insert_with(|| vec![(pack.to_path_buf(), pack.join("xsi_manifest.json")), (keyblades.clone(), keyblades.join("sora_forms_manifest.json"))]);
                    }
                    index.entry(base(name)).or_insert((glb, manifest, anims, (name.contains("p_ex001")).then_some(host)));
                }
            }
        }
        // the Kingdom Key: its skeleton's source asset (`w_so010_0_Skeleton`)
        let kk = pack.join("weapon").join("manifest.json");
        if let Some(name) = read_json(&kk).as_ref().and_then(|m| m.pointer("/skeleton/source_asset")).and_then(Value::as_str) {
            let name = base(name);
            index.entry(name.trim_end_matches("_skeleton").to_string()).or_insert((pack.join("weapon").join("kingdom_key.glb"), kk.clone(), pack.join("weapon").join("anims"), None));
        }
        // every keyblade / form / projectile model: model.json's source mesh
        if let Ok(dir) = std::fs::read_dir(pack.join("keyblades")) {
            for d in dir.flatten() {
                let p = d.path();
                let Some(m) = read_json(&p.join("model.json")) else { continue };
                let Some(name) = m.get("source_mesh").and_then(Value::as_str) else { continue };
                let glb = p.join(m.get("glb").and_then(Value::as_str).unwrap_or("weapon.glb"));
                let skeleton = p.join(m.get("skeleton").and_then(Value::as_str).unwrap_or("skeleton.json"));
                index.entry(base(name)).or_insert((glb, skeleton, p.join("anims"), Some(p.join("sockets.json"))));
            }
        }
        rlog!("vfx: (VFX-2) {} skeletal mesh asset(s) of the pack can be an effect's skeletal mesh component", index.len());
        Lib { index, overlays, assets: HashMap::new(), skins: HashMap::new(), frame: 0, told: Default::default() }
    }

    fn tell(&mut self, line: String) {
        if self.told.insert(line.clone()) {
            rlog!("{line}");
        }
    }

    fn asset(&mut self, name: &str) -> Option<&mut Asset> {
        let key = name.to_lowercase();
        if !self.assets.contains_key(&key) {
            let made = match self.index.get(&key).cloned() {
                Some((glb, skeleton, anims, sockets)) => load_asset(&glb, &skeleton, anims, sockets.as_deref(), self.overlays.get(&key).map_or(&[][..], Vec::as_slice)).map_err(|e| format!("vfx: (VFX-2) skeletal mesh {name}: {e}: not drawn")),
                None => Err(format!("vfx: (VFX-2) skeletal mesh {name}: no model of the pack is that asset: not drawn")),
            };
            let made = match made {
                Ok(a) => Some(a),
                Err(line) => {
                    self.tell(line);
                    None
                }
            };
            self.assets.insert(key.clone(), made);
        }
        self.assets.get_mut(&key).and_then(Option::as_mut)
    }
}

/// (FINISH-AUDIT2) A manifest's animation items (`items` with an `animation_name`); none for a missing or unreadable file.
fn animation_items(manifest: &Path) -> Vec<Value> {
    read_json(manifest).and_then(|m| m.get("items").and_then(Value::as_array).cloned()).unwrap_or_default().into_iter().filter(|i| i.get("animation_name").and_then(Value::as_str).is_some()).collect()
}

/// An item's root lock: a clip with `bEnableRootMotion` and its `RootMotionRootLock`.
fn lock_of(item: &Value) -> Option<RootLock> {
    let p = item.get("sequence_properties")?;
    (p.get("bEnableRootMotion")?.as_bool()?).then_some(())?;
    Some(match p.get("RootMotionRootLock").and_then(Value::as_str).unwrap_or("") {
        l if l.ends_with("AnimFirstFrame") => RootLock::FirstFrame,
        l if l.ends_with("Zero") => RootLock::Zero,
        _ => RootLock::RefPose,
    })
}

fn load_asset(glb: &Path, skeleton_file: &Path, anims: PathBuf, sockets: Option<&Path>, overlays: &[(PathBuf, PathBuf)]) -> Result<Asset, String> {
    let model = Model::load(glb)?;
    let text = std::fs::read_to_string(skeleton_file).map_err(|e| format!("{}: {e}", skeleton_file.display()))?;
    let skeleton = Skeleton::from_manifest(&kh_anim::json::Json::parse(text.trim_start_matches('\u{feff}'))?)?;
    let joint_bones = model.joints.iter().map(|(n, _)| skeleton.index_of(n)).collect();
    // (FINISH-AUDIT2) the main manifest's root locks, then the overlays' clips the main one does not have (kh_anim's rule)
    let main = animation_items(skeleton_file);
    let mut known: std::collections::HashSet<String> = main.iter().filter_map(|i| i.get("animation_name").and_then(Value::as_str).map(str::to_string)).collect();
    let mut locks: HashMap<String, RootLock> = main.iter().filter_map(|i| Some((i.get("animation_name")?.as_str()?.to_string(), lock_of(i)?))).collect();
    let mut files = HashMap::new();
    for (root, manifest) in overlays {
        for i in animation_items(manifest) {
            let (Some(name), Some(file)) = (i.get("animation_name").and_then(Value::as_str), i.get("file").and_then(Value::as_str)) else { continue };
            if i.get("kind").and_then(Value::as_str) != Some("animation") || !known.insert(name.to_string()) {
                continue;
            }
            if let Some(lock) = lock_of(&i) {
                locks.insert(name.to_string(), lock);
            }
            files.insert(name.to_string(), root.join(file));
        }
    }
    let mut indices = Vec::with_capacity(model.indices.len());
    for p in &model.primitives {
        let (from, to) = (p.first_index as usize, (p.first_index + p.index_count) as usize);
        indices.extend(model.indices[from..to].iter().map(|i| (*i as i64 + p.base_vertex as i64) as u32));
    }
    let mut table = HashMap::new();
    if let Some(file) = sockets.and_then(read_json) {
        // host.json `sockets` / sockets.json `weapon_sockets` (local_location / local_rotation) or `sockets`
        for (list, location, rotation) in [("sockets", "location", "rotation"), ("weapon_sockets", "local_location", "local_rotation")] {
            for (name, s) in file.get(list).and_then(Value::as_object).into_iter().flatten() {
                let v = |k: &str| s.get(k).and_then(Value::as_array).map_or(Vec3::ZERO, |a| Vec3::new(a.first().and_then(Value::as_f64).unwrap_or(0.0) as f32, a.get(1).and_then(Value::as_f64).unwrap_or(0.0) as f32, a.get(2).and_then(Value::as_f64).unwrap_or(0.0) as f32));
                table.entry(name.clone()).or_insert((s.get("bone").and_then(Value::as_str).unwrap_or("").to_string(), v(location), v(rotation)));
            }
        }
    }
    let reference = kh_anim::pose::component_space(&skeleton, &skeleton.reference_pose());
    Ok(Asset { model, skeleton, joint_bones, indices: Arc::new(indices), anims, sockets: table, rigs: HashMap::new(), reference, locks, files })
}

impl Asset {
    fn rig(&mut self, clip: &str) -> Option<&WeaponRig> {
        if !self.rigs.contains_key(clip) {
            let file = self.files.get(clip).cloned().unwrap_or_else(|| self.anims.join(format!("{clip}.glb"))); // (FINISH-AUDIT2)
            let rig = Clip::load(&file, clip).ok().map(|c| WeaponRig::new(self.skeleton.clone(), c));
            self.rigs.insert(clip.to_string(), rig);
        }
        self.rigs.get(clip).and_then(Option::as_ref)
    }

    /// The component-space pose of `clip` at `time`, its root locked when the clip has root motion (`locks`).
    fn pose(&mut self, clip: &str, time: f32) -> Option<Vec<Trs>> {
        let lock = self.locks.get(clip).copied();
        let reference = self.skeleton.bones.first().map(|b| b.reference);
        let rig = self.rig(clip)?;
        let mut local = rig.local(time);
        if let (Some(lock), Some(root)) = (lock, local.first_mut()) {
            *root = match lock {
                RootLock::RefPose => reference.unwrap_or(*root),
                RootLock::FirstFrame => rig.local(0.0).first().copied().unwrap_or(*root),
                RootLock::Zero => Trs { translation: Vec3::ZERO, rotation: glam::Quat::IDENTITY, scale: Vec3::ONE },
            };
        }
        Some(kh_anim::pose::component_space(&self.skeleton, &local))
    }

    /// A bone's (else a socket's) place in the component, as a matrix (UE cm).
    fn socket(&self, pose: &[Trs], name: &str) -> Option<Mat4> {
        if let Some(i) = self.skeleton.index_of(name) {
            return pose.get(i).map(crate::math::matrix_of);
        }
        let (bone, location, rotation) = self.sockets.get(name)?;
        let b = pose.get(self.skeleton.index_of(bone)?)?;
        let r = kh_vfx::rotator(rotation.x, rotation.y, rotation.z);
        // (kh-host vfx.rs socket_world: the bone, then the socket's place and turn in it)
        Some(crate::math::matrix_of(b) * Mat4::from_translation(*location) * Mat4::from_mat3(r))
    }

    /// The model skinned with a component-space pose, in the effect mesh layout (position, normal, tangent, two UV sets,
    /// colour), the mesh's own space in UE cm (the pack's (x, y, z) m -> (x, z, y) cm, as the effect meshes are read).
    fn skin(&self, pose: &[Trs]) -> Vec<f32> {
        let joints: Vec<Mat4> = self.joint_bones.iter().zip(&self.model.joints).map(|(b, (_, ibm))| b.and_then(|b| pose.get(b)).map_or(Mat4::IDENTITY, |t| crate::math::joint_in_pack(t) * *ibm)).collect();
        let mut out = Vec::with_capacity(self.model.vertices.len() * MESH_VERTEX_FLOATS);
        for v in &self.model.vertices {
            let (mut p, mut n, mut t) = (Vec3::ZERO, Vec3::ZERO, Vec3::ZERO);
            let mut sum = 0.0;
            for k in 0..4 {
                let w = v.weights[k];
                if w <= 0.0 {
                    continue;
                }
                let Some(m) = joints.get(v.joints[k] as usize) else { continue };
                p += m.transform_point3(Vec3::from(v.position)) * w;
                n += m.transform_vector3(Vec3::from(v.normal)) * w;
                t += m.transform_vector3(Vec3::new(v.tangent[0], v.tangent[1], v.tangent[2])) * w;
                sum += w;
            }
            if sum <= 1.0e-6 {
                (p, n, t) = (Vec3::from(v.position), Vec3::from(v.normal), Vec3::new(v.tangent[0], v.tangent[1], v.tangent[2]));
            } else {
                p /= sum;
            }
            let (n, t) = (n.normalize_or(Vec3::Y), t.normalize_or(Vec3::X));
            out.extend_from_slice(&[p.x * 100.0, p.z * 100.0, p.y * 100.0, n.x, n.z, n.y, t.x, t.z, t.y, -v.tangent[3], v.uv0[0], v.uv0[1], v.uv1[0], v.uv1[1], 1.0, 1.0, 1.0, 1.0]);
        }
        out
    }
}

/// One component made ready for the GPU (vfx.rs `Prepared::skel`).
pub(super) struct Posed {
    pub sort: SortKey,
    pub material: u32,
    pub key: u64,
    pub vertices: Arc<Vec<f32>>,
    pub indices: Arc<Vec<u32>>,
    /// The instance: three rows (x, y, z axis, origin) of the mesh space -> world, colour, dynamic parameter, clock.
    pub rows: [f32; MESH_INSTANCE_FLOATS],
    pub params: Vec<(String, [f32; 4], bool)>,
    pub center: Vec3,
    pub mirrored: bool,
}

fn key_of(mesh: &str, clip: Option<&(String, f32)>) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    mesh.to_lowercase().hash(&mut h);
    if let Some((c, t)) = clip {
        c.hash(&mut h);
        // (a 1/1000 s grid: a held pose is one key, a playing clip a new one every frame)
        ((t * 1000.0).round() as i64).hash(&mut h);
    }
    h.finish()
}

fn trs_of(m: &Affine3A) -> Trs {
    let (scale, rotation, translation) = m.to_scale_rotation_translation();
    Trs { translation, rotation, scale }
}

/// The frame's skeletal mesh components: the posed ones for the effect pipeline (material ids from `material_of`), the
/// ones that keep their own materials as form weapons for the character pipeline.
pub(super) fn prepare(list: &kh_vfx::DrawList, material_of: &mut dyn FnMut(u32, kh_vfx::Blend) -> Option<u32>, camera: Vec3) -> (Vec<Posed>, Vec<crate::xsiweapon::FormWeapon>) {
    let (mut posed, mut forms) = (Vec::new(), Vec::new());
    if list.skeletal.is_empty() {
        return (posed, forms);
    }
    let Ok(mut guard) = LIB.lock() else { return (posed, forms) };
    if guard.is_none() {
        let Some(pack) = crate::SHARED.lock().ok().and_then(|g| g.as_ref().map(|s| s.pack.clone())) else { return (posed, forms) };
        *guard = Some(Lib::new(&pack));
    }
    let Some(lib) = guard.as_mut() else { return (posed, forms) };
    lib.frame += 1;
    let frame = lib.frame;
    // per component: its world (mesh space -> world) and its pose (for the children in its sockets)
    let mut placed: Vec<Option<(Mat4, String, Vec<Trs>)>> = Vec::with_capacity(list.skeletal.len());
    for s in &list.skeletal {
        if s.mesh.is_empty() {
            lib.tell("vfx: (VFX-2) a skeletal mesh component of the owner's own mesh (bUseExtanalSkeletalMesh without a mesh): not drawn".to_string());
            placed.push(None);
            continue;
        }
        if s.owner_pose {
            lib.tell(format!("vfx: (VFX-2) skeletal mesh {}: the owner's pose copy (both external flags) is not made: its reference pose", s.mesh));
        }
        let parent_world = match &s.parent {
            Some((k, socket)) => {
                let Some(Some((world, mesh, pose))) = placed.get(*k as usize) else {
                    placed.push(None);
                    continue;
                };
                let (world, mesh, pose) = (*world, mesh.clone(), pose.clone());
                let at = lib.asset(&mesh).and_then(|a| a.socket(&pose, socket));
                if at.is_none() {
                    lib.tell(format!("vfx: (VFX-2) skeletal mesh {}: its parent {mesh} has no bone or socket {socket}: on the parent's origin", s.mesh));
                }
                Some(world * at.unwrap_or(Mat4::IDENTITY))
            }
            None => None,
        };
        let Some(asset) = lib.asset(&s.mesh) else {
            placed.push(None);
            continue;
        };
        // the clips' lengths decide the ANIM_END hand-overs and the loop: read the clips first
        let lengths: HashMap<String, f32> = s.anims.iter().filter_map(|a| a.clip.clone()).filter_map(|c| asset.rig(&c).map(|r| (c, r.length()))).collect();
        let clip = s.clip_at(&|c| lengths.get(c).copied());
        let pose = match &clip {
            Some((c, t)) => asset.pose(c, *t),
            None => None,
        };
        // (FINISH-AUDIT2) a named clip the pack has no file for is said once (it was a silent reference pose)
        let missing = clip.as_ref().filter(|_| pose.is_none()).map(|(c, _)| format!("vfx: (FINISH-AUDIT2) skeletal mesh {}: clip {c} not found: its reference pose", s.mesh));
        let pose = pose.unwrap_or_else(|| asset.reference.clone());
        if let Some(line) = missing {
            lib.tell(line);
        }
        let world = parent_world.unwrap_or_else(|| Mat4::from(crate::herolight::lock_skel(&s.mesh, s.transform, camera))); // (HEROLIGHT-KB) a glow's skeletal copy on its weapon
        placed.push(Some((world, s.mesh.clone(), pose.clone())));
        if s.material == u32::MAX {
            // its own materials: the character pipeline's form weapon (the model's folder name)
            let model = s.mesh.trim_end_matches("_0").to_string();
            forms.push(crate::xsiweapon::FormWeapon { model, to_world: trs_of(&Affine3A::from_mat4(world)), component: pose });
            continue;
        }
        let Some(material) = material_of(s.material, s.blend) else { continue };
        let key = key_of(&s.mesh, clip.as_ref());
        let vertices = match lib.skins.get_mut(&key) {
            Some((v, used)) => {
                *used = frame;
                v.clone()
            }
            None => {
                let Some(asset) = lib.asset(&s.mesh) else { continue };
                let v = Arc::new(asset.skin(&pose));
                if lib.skins.len() >= SKIN_CACHE {
                    if let Some(old) = lib.skins.iter().min_by_key(|(_, (_, used))| *used).map(|(k, _)| *k) {
                        lib.skins.remove(&old);
                    }
                }
                lib.skins.insert(key, (v.clone(), frame));
                v
            }
        };
        let Some(asset) = lib.asset(&s.mesh) else { continue };
        let m = Affine3A::from_mat4(world);
        let o = Vec3::from(m.translation);
        let mut rows = [0.0f32; MESH_INSTANCE_FLOATS];
        for k in 0..3 {
            rows[k * 4..k * 4 + 4].copy_from_slice(&[m.matrix3.x_axis[k], m.matrix3.y_axis[k], m.matrix3.z_axis[k], o[k]]);
        }
        // a skeletal mesh's material sees the vertex factory's defaults: particle colour and dynamic parameter 1
        rows[12..20].copy_from_slice(&[1.0; 8]);
        rows[20..24].copy_from_slice(&[s.time, 0.0, 0.0, 0.0]);
        let mirrored = Mat3::from(m.matrix3).determinant() < 0.0;
        posed.push(Posed { sort: (0, -o.distance(camera), 0, 0), material, key, vertices, indices: asset.indices.clone(), rows, params: s.params.clone(), center: o, mirrored });
    }
    (posed, forms)
}

/// The value of one of the translator's uniform-expression trees (tools/sora_vfx_shaders.js `bindExpr` / `evalTree`: {c},
/// {t}, {f, a}; a leaf with `p` is a parameter the dynamic material may set: `params` (name, value, vector) win over its
/// value) at `time` seconds.
pub(super) fn eval_tree(n: &Value, time: f32, params: &[(String, [f32; 4], bool)]) -> [f32; 4] {
    if let Some(name) = n.get("p").and_then(Value::as_str) {
        if let Some((_, v, vector)) = params.iter().rev().find(|(p, ..)| p == name) {
            return if *vector { *v } else { [v[0]; 4] };
        }
    }
    if let Some(c) = n.get("c").and_then(Value::as_array) {
        let f = |k: usize| c.get(k).and_then(Value::as_f64).unwrap_or(0.0) as f32;
        return [f(0), f(1), f(2), f(3)];
    }
    if n.get("t").is_some() {
        return [time; 4];
    }
    let a: Vec<[f32; 4]> = n.get("a").and_then(Value::as_array).map(|a| a.iter().map(|x| eval_tree(x, time, params)).collect()).unwrap_or_default();
    let x = a.first().copied().unwrap_or([0.0; 4]);
    let y = a.get(1).copied().unwrap_or([0.0; 4]);
    let m1 = |f: &dyn Fn(f32) -> f32| x.map(f);
    let m2 = |f: &dyn Fn(f32, f32) -> f32| [f(x[0], y[0]), f(x[1], y[1]), f(x[2], y[2]), f(x[3], y[3])];
    let int = |k: &str| n.get(k).and_then(Value::as_i64).unwrap_or(0);
    match n.get("f").and_then(Value::as_str).unwrap_or("") {
        "add" => m2(&|p, q| p + q),
        "sub" => m2(&|p, q| p - q),
        "mul" => m2(&|p, q| p * q),
        "div" => m2(&|p, q| if q.abs() < 1.0e-5 { 0.0 } else { p / q }),
        "dot" => [x[0] * y[0] + x[1] * y[1] + x[2] * y[2] + x[3] * y[3]; 4],
        "append" => {
            let k = int("n").clamp(0, 4) as usize;
            std::array::from_fn(|i| if i < k { x[i] } else { y[i - k] })
        }
        "swizzle" => {
            let idx: Vec<i64> = n.get("i").and_then(Value::as_array).map(|a| a.iter().map(|v| v.as_i64().unwrap_or(-1)).collect()).unwrap_or_default();
            let k = int("n") as usize;
            std::array::from_fn(|i| match idx.get(i) {
                Some(j) if i < k && *j >= 0 => x[(*j as usize).min(3)],
                _ => 0.0,
            })
        }
        "clamp" => {
            let z = a.get(2).copied().unwrap_or([0.0; 4]);
            std::array::from_fn(|i| x[i].max(y[i]).min(z[i]))
        }
        "min" => m2(&f32::min),
        "max" => m2(&f32::max),
        "fmod" => m2(&|p, q| if q == 0.0 { 0.0 } else { p % q }),
        "sin" => m1(&f32::sin),
        "cos" => m1(&f32::cos),
        "sqrt" => m1(&|v| v.max(0.0).sqrt()),
        "frac" => m1(&|v| v - v.floor()),
        "floor" => m1(&f32::floor),
        "ceil" => m1(&f32::ceil),
        "abs" => m1(&f32::abs),
        "saturate" => m1(&|v| v.clamp(0.0, 1.0)),
        "round" => m1(&f32::round),
        "sign" => m1(&|v| if v > 0.0 { 1.0 } else if v < 0.0 { -1.0 } else { 0.0 }),
        "truncate" => m1(&f32::trunc),
        "trig0" => m1(&f32::sin),
        "trig1" => m1(&f32::cos),
        "trig2" => m1(&|v| finite(v.tan())),
        "trig3" => m1(&|v| finite(v.asin())),
        "trig4" => m1(&|v| finite(v.acos())),
        "trig5" => m1(&f32::atan),
        "trig6" => m2(&f32::atan2),
        _ => [0.0; 4],
    }
}

fn finite(v: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        0.0
    }
}

/// The GPU buffers of the posed components: per asset its index buffer (made once), per key its skinned vertices in an
/// upload buffer of a small ring per asset (a buffer is written again only when no frame in flight can read it).
struct GpuEntry {
    vertices: ID3D12Resource,
    bytes: usize,
    key: u64,
    used: u64,
}

struct GpuLib {
    device: usize,
    indices: HashMap<usize, (ID3D12Resource, u32)>,
    ring: Vec<GpuEntry>,
    frame: u64,
    last_frame_id: usize,
}

// SAFETY: D3D12 objects are free-threaded; used under the mutex only.
unsafe impl Send for GpuLib {}

static GPU_LIB: Mutex<Option<GpuLib>> = Mutex::new(None);

/// At most this many skinned vertex buffers; one is written again only after this many frames.
const RING: usize = 16;
const IN_FLIGHT: u64 = 12;

/// The vertex and index views of a posed component for this frame (`frame_id`: the prepared frame's identity, to count
/// frames), None when its buffers cannot be had now (the ring is full of buffers in flight).
pub(super) unsafe fn gpu_mesh(device: &ID3D12Device, frame_id: usize, p: &Posed) -> Option<(D3D12_VERTEX_BUFFER_VIEW, D3D12_INDEX_BUFFER_VIEW, u32)> {
    let mut guard = GPU_LIB.lock().ok()?;
    if guard.as_ref().is_none_or(|g| g.device != device.as_raw() as usize) {
        if let Some(old) = guard.take() {
            crate::outfit::retire(Box::new((old.indices.into_values().map(|v| v.0).collect::<Vec<_>>(), old.ring.into_iter().map(|e| e.vertices).collect::<Vec<_>>())));
        }
        *guard = Some(GpuLib { device: device.as_raw() as usize, indices: HashMap::new(), ring: Vec::new(), frame: 0, last_frame_id: 0 });
    }
    let g = guard.as_mut()?;
    if g.last_frame_id != frame_id {
        g.last_frame_id = frame_id;
        g.frame += 1;
    }
    let frame = g.frame;
    let index_id = Arc::as_ptr(&p.indices) as usize;
    if let std::collections::hash_map::Entry::Vacant(slot) = g.indices.entry(index_id) {
        let bytes = std::slice::from_raw_parts(p.indices.as_ptr() as *const u8, p.indices.len() * 4);
        slot.insert((super::upload_buffer(device, bytes).ok()?, p.indices.len() as u32));
    }
    let bytes = p.vertices.len() * 4;
    let slot = match g.ring.iter().position(|e| e.key == p.key && e.bytes == bytes) {
        Some(k) => k,
        None => {
            let free = g.ring.iter().enumerate().filter(|(_, e)| e.used + IN_FLIGHT < frame && e.bytes >= bytes).min_by_key(|(_, e)| e.used).map(|(k, _)| k);
            let k = match free {
                Some(k) => k,
                None if g.ring.len() < RING => {
                    let buffer = super::committed(device, D3D12_HEAP_TYPE_UPLOAD, &super::buffer_desc(bytes as u64), D3D12_RESOURCE_STATE_GENERIC_READ).ok()?;
                    g.ring.push(GpuEntry { vertices: buffer, bytes, key: 0, used: 0 });
                    g.ring.len() - 1
                }
                None => return None,
            };
            let e = &mut g.ring[k];
            let mut at: *mut std::ffi::c_void = std::ptr::null_mut();
            e.vertices.Map(0, None, Some(&mut at)).ok()?;
            std::ptr::copy_nonoverlapping(p.vertices.as_ptr() as *const u8, at as *mut u8, bytes);
            e.vertices.Unmap(0, None);
            e.key = p.key;
            k
        }
    };
    let e = &mut g.ring[slot];
    e.used = frame;
    let (indices, count) = g.indices.get(&index_id)?;
    Some((
        D3D12_VERTEX_BUFFER_VIEW { BufferLocation: e.vertices.GetGPUVirtualAddress(), SizeInBytes: bytes as u32, StrideInBytes: (MESH_VERTEX_FLOATS * 4) as u32 },
        D3D12_INDEX_BUFFER_VIEW { BufferLocation: indices.GetGPUVirtualAddress(), SizeInBytes: count * 4, Format: DXGI_FORMAT_R32_UINT },
        *count,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_parameter_leaf_takes_the_dynamic_materials_value() {
        let tree: Value = serde_json::from_str(r#"{"f":"mul","a":[{"c":[1,1,1,1],"p":"alpha","k":"s"},{"c":[2,2,2,2]}]}"#).unwrap();
        assert_eq!(eval_tree(&tree, 0.0, &[]), [2.0; 4]);
        assert_eq!(eval_tree(&tree, 0.0, &[("alpha".to_string(), [0.25, 0.0, 0.0, 0.0], false)]), [0.5; 4]);
        let v: Value = serde_json::from_str(r#"{"f":"swizzle","i":[2,1,0,-1],"n":3,"a":[{"c":[1,1,1,1],"p":"Col_Shift","k":"v"}]}"#).unwrap();
        assert_eq!(eval_tree(&v, 0.0, &[("Col_Shift".to_string(), [0.6, 1.2, 2.0, 0.0], true)]), [2.0, 1.2, 0.6, 0.0]);
        let t: Value = serde_json::from_str(r#"{"f":"append","n":2,"a":[{"t":1},{"c":[5,6,7,8]}]}"#).unwrap();
        assert_eq!(eval_tree(&t, 3.0, &[]), [3.0, 3.0, 5.0, 6.0]);
    }

    /// The pack's Sora (skipped without it): his body p_ex001_0 is found, clip 1316 held at 0.7 s skins every vertex to a
    /// finite place within two metres of the component, and the Favorite Deputy w_so050_0 has its own skeleton.
    #[test]
    fn the_pack_has_the_ghost_horde_body_and_its_keyblade() {
        let pack = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack");
        if !pack.join("sora.glb").is_file() {
            return;
        }
        let mut lib = Lib::new(&pack);
        let body = lib.asset("p_ex001_0").expect("Sora's body");
        let pose = body.pose("1316", 0.7).expect("clip 1316");
        assert_eq!(body.locks.get("1316"), Some(&RootLock::RefPose), "1316 has root motion: its root is held");
        let r_buki = body.socket(&pose, "R_wep_skt").expect("the socket on R_buki");
        assert!(r_buki.w_axis.truncate().length() < 200.0);
        let v = body.skin(&pose);
        assert_eq!(v.len(), body.model.vertices.len() * MESH_VERTEX_FLOATS);
        for c in v.as_chunks::<MESH_VERTEX_FLOATS>().0 {
            assert!(c.iter().all(|x| x.is_finite()) && Vec3::new(c[0], c[1], c[2]).length() < 250.0, "{c:?}");
        }
        let held = body.skin(&pose);
        assert_eq!(held, v, "a held pose skins the same twice");
        assert!(lib.asset("w_so050_0").is_some(), "the Favorite Deputy");
    }

    /// (FINISH-AUDIT2) Mirage Staff's Sora copies play clips of the keyblade forms' overlay (vw_so072_mrg_sora1 A411 @ 0.42,
    /// vw_so073_mrg1 A417 @ 2.8): both are found and pose the body away from its reference (T) pose; the right hand rises
    /// in A417 (Sora holds the staff up to the orb): his right hand is far from its T-pose place.
    #[test]
    fn mirage_staff_copies_find_their_form_clips() {
        let pack = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack");
        if !pack.join("keyblades").join("sora_forms_manifest.json").is_file() {
            return;
        }
        let mut lib = Lib::new(&pack);
        let body = lib.asset("p_ex001_0").expect("Sora's body");
        let reference = body.reference.clone();
        let hand = body.skeleton.index_of("R_te").expect("the right hand R_te");
        for (clip, t) in [("A411", 0.42), ("A417", 2.8)] {
            let pose = body.pose(clip, t).unwrap_or_else(|| panic!("clip {clip} of the forms overlay"));
            let moved = pose.iter().zip(&reference).filter(|(a, b)| a.translation.distance(b.translation) > 1.0 || a.rotation.angle_between(b.rotation) > 0.05).count();
            assert!(moved > pose.len() / 4, "{clip} @ {t}: only {moved} of {} bones away from the reference pose", pose.len());
            if clip == "A417" {
                assert!(pose[hand].translation.distance(reference[hand].translation) > 30.0, "A417 @ 2.8: the right hand left its T-pose place ({:?} vs {:?})", pose[hand].translation, reference[hand].translation);
            }
        }
    }
}
