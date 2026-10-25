//! `PACK/materials/materials.json` as per-slot records, for a host that fills and uploads the
//! uniform block itself (the Elden Ring renderer). The Bevy test app keeps its own library around
//! Bevy's asset types and shares the pieces (`OwnerCb`, `Khs`, `BoneParam`, `Rig`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use glam::{Mat4, Vec3, Vec4};
use serde_json::Value;

use crate::{four, BoneParam, BoneValue, Khs, KhsState, OwnerCb, Params, Rig, DISCERNMENT, ENV_SIZE};

/// (IMPL_ER59) A slot's texture slots: the harness's 14 (MAX_TEXTURES) + 4 an outfit's material may bind more
/// (crates/kh-er-render er_char.wgsl KH_TEX_EXTRA).
pub const SLOT_TEXTURES_MAX: usize = crate::MAX_TEXTURES + 4;

/// A texture a slot binds.
#[derive(Clone, Debug, PartialEq)]
pub struct TextureRef {
    /// Relative to `PACK/materials`.
    pub file: String,
    pub srgb: bool,
    /// `false`: the cooked texture has no mip chain (a colour table: none must be made).
    pub mips: bool,
}

/// One material slot of a model.
#[derive(Clone, Debug)]
pub struct Slot {
    /// "sora" or "kingdom_key".
    pub mesh: String,
    /// The glTF material name of the primitive (`p_ex001_mat0`).
    pub slot: String,
    pub owner: String,
    /// The shader definition `KH_C<id>`.
    pub owner_id: u16,
    /// The material instance's name (`p_ex001_mat0_Inst`): what the mesh's tables name.
    pub instance_name: String,
    pub masked: bool,
    pub translucent: bool,
    pub two_sided: bool,
    pub subsurface: bool,
    /// By texture slot of the shader (`None`: the slot is not bound: white).
    pub textures: Vec<Option<TextureRef>>,
    /// (IMPL_ER23B) The material parameter of each texture slot (`FormChangeTex0` ...; "" = not a parameter).
    pub texture_names: Vec<String>,
    /// The third UV set of the slot's primitive (8 bytes a vertex), relative to `PACK/materials`.
    pub uv2: Option<String>,
    pub scalars: HashMap<String, f32>,
    pub vectors: HashMap<String, [f32; 4]>,
}

impl Slot {
    /// The shader variant's name, as the offline compiler names its files.
    pub fn variant(&self) -> String {
        variant_name(self.owner_id, self.masked, self.translucent, self.subsurface)
    }
}

/// `c<id>` + `m` masked + `t` translucent + `s` subsurface.
pub fn variant_name(owner_id: u16, masked: bool, translucent: bool, subsurface: bool) -> String {
    format!("c{owner_id}{}{}{}", if masked { "m" } else { "" }, if translucent { "t" } else { "" }, if subsurface { "s" } else { "" })
}

/// The per-object rows of the uniform block (UE's Primitive uniform buffer, as far as the
/// materials read it), game space, cm.
#[derive(Clone, Copy, Debug)]
pub struct ObjectRows {
    pub w2l: [Vec4; 4],
    pub obj: Vec4,
    pub bounds: Vec4,
    pub actor: Vec4,
    pub bmin: Vec4,
    pub bmax: Vec4,
}

impl ObjectRows {
    /// `local_to_world`: the mesh component's LocalToWorld; `bounds`: the cooked mesh bounds
    /// (origin, box extent, sphere radius; component space).
    pub fn new(local_to_world: &kh_anim::Trs, bounds: (Vec3, Vec3, f32)) -> ObjectRows {
        let matrix = Mat4::from_scale_rotation_translation(local_to_world.scale, local_to_world.rotation, local_to_world.translation);
        let inverse = matrix.inverse();
        // local = wx * row0 + wy * row1 + wz * row2 + row3.
        let (origin, extent, radius) = bounds;
        ObjectRows {
            w2l: [inverse.x_axis.truncate().extend(0.0), inverse.y_axis.truncate().extend(0.0), inverse.z_axis.truncate().extend(0.0), inverse.w_axis.truncate().extend(1.0)],
            obj: local_to_world.transform_point(origin).extend(radius),
            bounds: extent.extend(1.0),
            actor: local_to_world.translation.extend(1.0),
            bmin: (origin - extent).extend(0.0),
            bmax: (origin + extent).extend(0.0),
        }
    }
}

/// (IMPL_ER23B) THE WEAR FORMS (`wear_forms` of materials.json = `UTresWearformSet` of the pawn's equipment component):
/// per form, for each of Sora's material instances it names, the image and the "point" texture. The equipment
/// component (0x1417915A0 start, 0x141790C80 tick) puts the new form's textures into `FormChangeTex1` /
/// `FormChangePointTex1`, runs `FormChangeSlide` 0 -> 1 over the given time and at the end makes them
/// `FormChangeTex0` / `FormChangePointTex0` with the slide back at 0. THE TEST APP'S RULE (kh-app char_material.rs
/// `update`), here for a host that binds textures itself: [`Library::step_wear`].
#[derive(Clone, Debug, Default)]
pub struct WearForms {
    /// form -> (material instance, image, point).
    pub forms: HashMap<u8, Vec<(String, Option<TextureRef>, Option<TextureRef>)>>,
    /// The form whose textures are in `FormChangeTex0`, the one sliding in, the slide and its length.
    pub worn: u8,
    pub target: u8,
    pub slide: f32,
    pub length: f32,
    /// (ER69) form -> (material instance, texture parameter, texture): replaced while that form is on (an outfit whose
    /// clothes have no form costume: wear_extra.json, tools/sora_outfit_forms.js), the replaced ones kept to put back.
    pub direct: HashMap<u8, Vec<(String, String, TextureRef)>>,
    pub direct_on: u8,
    /// (ER69) The plain recolour instead of the styled look where an outfit has both (form + 100 = the plain one).
    pub plain: bool,
    pub direct_saved: Vec<(usize, usize, Option<TextureRef>)>,
}

/// The slide's length when a form is put on (`pawn.vt+0x1498(form, 1.0)`) and when it ends
/// (`pawn.vt+0x13E0(1, 0, 0, 0.2)`) [decompiled C; kh-app char_material.rs has the same two].
pub const WEAR_FORM_ON_SECONDS: f32 = 1.0;
pub const WEAR_FORM_OFF_SECONDS: f32 = 0.2;
/// The mesh the wear forms belong to.
pub const WEAR_MESH: &str = "sora";

/// A texture slot of a material slot gets another texture (the host binds it).
#[derive(Clone, Debug, PartialEq)]
pub struct WearChange {
    /// Index into `Library::slots`.
    pub slot: usize,
    /// The shader's texture slot.
    pub texture: usize,
    pub file: TextureRef,
}

pub struct Library {
    /// `PACK/materials`.
    pub dir: PathBuf,
    pub generator: String,
    owners: HashMap<String, OwnerCb>,
    pub khs: Khs,
    /// Sora's bone pose -> material parameter rows.
    pub bone_table: Vec<BoneParam>,
    /// mesh -> the cooked bounds (origin, box extent, sphere radius; component space, cm).
    bounds: HashMap<String, (Vec3, Vec3, f32)>,
    pub slots: Vec<Slot>,
    /// (IMPL_ER23B) The costume change.
    pub wear: WearForms,
}

impl Library {
    /// (IMPL_ER23B) Every texture a wear form may bind (a host loads them with the slots' own).
    pub fn wear_textures(&self) -> Vec<TextureRef> {
        let mut out: Vec<TextureRef> = Vec::new();
        let direct: Vec<Option<TextureRef>> = self.wear.direct.values().flatten().map(|(_, _, t)| Some(t.clone())).collect();
        for t in self.wear.forms.values().flatten().flat_map(|(_, image, point)| [image, point]).chain(direct.iter()).flatten() {
            if !out.iter().any(|o| o.file == t.file) {
                out.push(t.clone());
            }
        }
        out
    }

    /// (IMPL_ER23B) ONE FRAME OF THE COSTUME CHANGE: `wanted` = the wear form the pawn has applied
    /// (`kh_core` `combat.form.wear_form`: 0 NORMAL, 5 Second Form), `dt` = the frame's game time. Writes
    /// `FormChangeSlide` on the slots that have the parameter and returns the texture bindings that changed
    /// (also written into `Slot::textures`). A form the pack has no textures for changes nothing.
    pub fn step_wear(&mut self, wanted: u8, dt: f32) -> Vec<WearChange> {
        let mut changes = Vec::new();
        // (ER69) the direct replacements: put on / taken off at once when the form changes
        let alt = wanted.saturating_add(100);
        let direct_wanted = if wanted != 0 && self.wear.plain && self.wear.direct.contains_key(&alt) { alt } else if self.wear.direct.contains_key(&wanted) { wanted } else { 0 };
        if direct_wanted != self.wear.direct_on {
            for (slot, texture, before) in std::mem::take(&mut self.wear.direct_saved) {
                if let Some(t) = self.slots.get_mut(slot).and_then(|s| s.textures.get_mut(texture)) {
                    *t = before.clone();
                    if let Some(file) = before {
                        changes.push(WearChange { slot, texture, file });
                    }
                }
            }
            for (material, parameter, file) in self.wear.direct.get(&direct_wanted).cloned().unwrap_or_default() {
                for (k, slot) in self.slots.iter_mut().enumerate().filter(|(_, s)| s.mesh == WEAR_MESH && s.instance_name == material) {
                    let Some(texture) = slot.texture_names.iter().position(|n| *n == parameter) else { continue };
                    let Some(t) = slot.textures.get_mut(texture) else { continue };
                    self.wear.direct_saved.push((k, texture, t.clone()));
                    *t = Some(file.clone());
                    changes.push(WearChange { slot: k, texture, file: file.clone() });
                }
            }
            self.wear.direct_on = direct_wanted;
        }
        if self.wear.forms.is_empty() {
            return changes;
        }
        let (slots, wear) = (&mut self.slots, &mut self.wear);
        let mut put = |form: u8, tex: &str, point: &str, changes: &mut Vec<WearChange>| {
            for (material, image, point_file) in wear.forms.get(&form).into_iter().flatten() {
                for (k, slot) in slots.iter_mut().enumerate().filter(|(_, s)| s.mesh == WEAR_MESH && s.instance_name == *material) {
                    for (parameter, file) in [(tex, image), (point, point_file)] {
                        let (Some(file), Some(texture)) = (file, slot.texture_names.iter().position(|n| n == parameter)) else { continue };
                        if slot.textures.get(texture).is_some_and(|t| t.as_ref() != Some(file)) {
                            slot.textures[texture] = Some(file.clone());
                            changes.push(WearChange { slot: k, texture, file: file.clone() });
                        }
                    }
                }
            }
        };
        if wanted != wear.target && wear.forms.contains_key(&wanted) {
            // A slide still running is finished at once (its textures become the worn ones).
            if wear.target != wear.worn {
                let target = wear.target;
                put(target, "FormChangeTex0", "FormChangePointTex0", &mut changes);
                wear.worn = target;
            }
            wear.target = wanted;
            wear.slide = 0.0;
            wear.length = if wanted == 0 { WEAR_FORM_OFF_SECONDS } else { WEAR_FORM_ON_SECONDS };
            put(wanted, "FormChangeTex1", "FormChangePointTex1", &mut changes);
        }
        let mut slide = 0.0;
        if wear.target != wear.worn {
            // [D] a linear ramp (the interpolator object of the equipment component was not read).
            wear.slide += dt.max(0.0) / wear.length.max(1.0e-3);
            if wear.slide >= 1.0 {
                let target = wear.target;
                put(target, "FormChangeTex0", "FormChangePointTex0", &mut changes);
                wear.worn = target;
                wear.slide = 0.0;
            }
            slide = wear.slide;
        }
        for slot in self.slots.iter_mut().filter(|s| s.mesh == WEAR_MESH && s.texture_names.iter().any(|n| n == "FormChangeTex1")) {
            match slot.scalars.get_mut("FormChangeSlide") {
                Some(s) => *s = slide,
                None => drop(slot.scalars.insert("FormChangeSlide".to_string(), slide)),
            }
        }
        changes
    }

    pub fn load(pack_dir: &Path) -> Result<Library, String> {
        let dir = pack_dir.join("materials");
        let path = dir.join("materials.json");
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let json: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut library = Library::from_json(dir.clone(), &json)?;
        // (ER69) an outfit's wear_extra.json (tools/sora_outfit_forms.js): more wear forms, and direct replacements
        let extra = dir.join("wear_extra.json");
        if extra.is_file() {
            let text = std::fs::read_to_string(&extra).map_err(|e| format!("{}: {e}", extra.display()))?;
            let json: Value = serde_json::from_str(text.trim_start_matches('﻿')).map_err(|e| format!("{}: {e}", extra.display()))?;
            library.add_wear_extra(&json);
        }
        Ok(library)
    }

    /// (ER69) Wear forms in the schema of materials.json's (taken where the library has none of that form) and the
    /// direct replacements of `wear_extra.json`.
    pub fn add_wear_extra(&mut self, json: &Value) {
        let texture = |t: &Value| Some(TextureRef { file: t.get("file")?.as_str()?.to_string(), srgb: t.get("srgb").and_then(Value::as_bool).unwrap_or(false), mips: t.get("mips").and_then(Value::as_u64).unwrap_or(1) > 1 });
        for (form, f) in json.get("forms").and_then(Value::as_object).into_iter().flatten() {
            let Ok(form) = form.parse::<u8>() else { continue };
            if self.wear.forms.contains_key(&form) {
                continue;
            }
            self.wear.forms.insert(form, f.get("materials").and_then(Value::as_array).into_iter().flatten().map(|m| (m.get("material").and_then(Value::as_str).unwrap_or("").to_string(), m.get("image").and_then(texture), m.get("point").and_then(texture))).collect());
        }
        for (form, list) in json.get("direct").and_then(Value::as_object).into_iter().flatten() {
            let Ok(form) = form.parse::<u8>() else { continue };
            let rows = list.as_array().into_iter().flatten().filter_map(|d| Some((d.get("material")?.as_str()?.to_string(), d.get("parameter")?.as_str()?.to_string(), texture(d)?))).collect();
            self.wear.direct.insert(form, rows);
        }
    }

    pub fn from_json(dir: PathBuf, json: &Value) -> Result<Library, String> {
        if !json.get("format").and_then(Value::as_str).is_some_and(|f| f.starts_with("kh3-sora-materials/")) {
            return Err("materials.json: format is not kh3-sora-materials/1 (node tools/sora_material_pack.js)".to_string());
        }
        let mut owners = HashMap::new();
        for (name, owner) in json.get("owners").and_then(Value::as_object).into_iter().flatten() {
            owners.insert(name.clone(), OwnerCb::parse(owner));
        }
        let v3 = |v: &Value| Vec3::new(v[0].as_f64().unwrap_or(0.0) as f32, v[1].as_f64().unwrap_or(0.0) as f32, v[2].as_f64().unwrap_or(0.0) as f32);
        let (mut bounds, mut bone_table) = (HashMap::new(), Vec::new());
        for (mesh, facts) in json.get("mesh_facts").and_then(Value::as_object).into_iter().flatten() {
            if let Some(b) = facts.get("bounds").filter(|b| b.is_object()) {
                bounds.insert(mesh.clone(), (v3(&b["origin"]), v3(&b["extent"]), b["radius"].as_f64().unwrap_or(100.0) as f32));
            }
            if mesh == "sora" {
                bone_table = BoneParam::parse_table(facts);
            }
        }
        let mut slots = Vec::new();
        for (slot, record) in json.get("materials").and_then(Value::as_object).into_iter().flatten() {
            let text = |k: &str| record.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            let (mesh, owner_name) = (text("mesh"), text("owner"));
            let Some(owner) = json.get("owners").and_then(|o| o.get(&owner_name)) else { continue };
            let ok = |v: Option<&Value>| v.and_then(|b| b.get("ok")).and_then(Value::as_bool) == Some(true);
            if !ok(owner.get("base")) {
                continue;
            }
            let blend = text("blend_mode");
            let mut textures: Vec<Option<TextureRef>> = Vec::new();
            for t in record.get("textures").and_then(Value::as_array).into_iter().flatten().take(SLOT_TEXTURES_MAX) {
                textures.push(t.get("file").and_then(Value::as_str).map(|file| TextureRef { file: file.to_string(), srgb: t.get("srgb").and_then(Value::as_bool).unwrap_or(false), mips: t.get("mips").and_then(Value::as_u64).unwrap_or(1) > 1 }));
            }
            slots.push(Slot {
                texture_names: owner.get("slots").and_then(Value::as_array).into_iter().flatten().take(SLOT_TEXTURES_MAX).map(|sl| sl.get("parameter").and_then(Value::as_str).unwrap_or("").to_string()).collect(),
                uv2: json.get("uv2").and_then(|u| u.get(format!("{mesh}:{slot}"))).and_then(|u| u.get("file")).and_then(Value::as_str).map(str::to_string),
                mesh,
                slot: slot.clone(),
                owner_id: owner.get("id").and_then(Value::as_u64).unwrap_or(0) as u16,
                instance_name: record.get("asset").and_then(Value::as_str).and_then(|a| a.rsplit('.').next()).unwrap_or("").to_string(),
                masked: blend == "BLEND_Masked" && ok(owner.get("mask")),
                translucent: blend == "BLEND_Translucent",
                two_sided: record.get("two_sided").and_then(Value::as_bool).unwrap_or(false),
                subsurface: owner.get("lighting_model").and_then(Value::as_str) == Some("MSM_Subsurface"),
                textures,
                scalars: record.get("scalars").and_then(Value::as_object).into_iter().flatten().filter_map(|(n, v)| Some((n.clone(), v.as_f64()? as f32))).collect(),
                vectors: record.get("vectors").and_then(Value::as_object).into_iter().flatten().map(|(n, v)| (n.clone(), four(v))).collect(),
                owner: owner_name,
            });
        }
        // (IMPL_ER59) UE's parameter names are FNames: compared WITHOUT case. KH2 Sora's table writes `atama_wp` where
        // its eye materials read `atama_WP`: the table's names take the materials' spelling.
        let mut spelled: HashMap<String, String> = HashMap::new();
        for owner in json.get("owners").and_then(Value::as_object).into_iter().flat_map(|o| o.values()) {
            for kind in ["scalars", "vectors"] {
                for name in owner.get("parameters").and_then(|p| p.get(kind)).and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
                    spelled.entry(name.to_ascii_lowercase()).or_insert_with(|| name.to_string());
                }
            }
        }
        for row in &mut bone_table {
            if let Some(name) = spelled.get(&row.parameter.to_ascii_lowercase()) {
                row.parameter.clone_from(name);
            }
        }
        let mut wear = WearForms::default();
        for (form, f) in json.get("wear_forms").and_then(Value::as_object).into_iter().flatten() {
            let Ok(form) = form.parse::<u8>() else { continue };
            let texture = |m: &Value, k: &str| {
                let t = m.get(k)?;
                Some(TextureRef { file: t.get("file")?.as_str()?.to_string(), srgb: t.get("srgb").and_then(Value::as_bool).unwrap_or(false), mips: t.get("mips").and_then(Value::as_u64).unwrap_or(1) > 1 })
            };
            wear.forms.insert(form, f.get("materials").and_then(Value::as_array).into_iter().flatten().map(|m| (m.get("material").and_then(Value::as_str).unwrap_or("").to_string(), texture(m, "image"), texture(m, "point"))).collect());
        }
        Ok(Library { dir, generator: json.get("generator").and_then(Value::as_str).unwrap_or("?").to_string(), owners, khs: Khs::parse(json), bone_table, bounds, slots, wear })
    }

    /// (XW) Takes in another library's slots, shader owners and mesh bounds (the XS form weapons' translated materials,
    /// PACK/xsi/weapons/materials: their own owner ids, meshes `xsi:<model>`). Its texture and third-UV files are made
    /// absolute (they lie in that library's folder; a host joins this library's folder with a file name, which an absolute
    /// path replaces); its owners are keyed `<owner_prefix><name>` so that they never replace one of this library's. Slots of
    /// a mesh this library has already are left out. Returns the number of slots taken.
    pub fn merge(&mut self, other: Library, owner_prefix: &str) -> usize {
        let dir = other.dir.clone();
        let abs = |file: &str| if Path::new(file).is_absolute() { file.to_string() } else { dir.join(file).to_string_lossy().to_string() };
        let have: Vec<String> = self.slots.iter().map(|s| s.mesh.clone()).collect();
        for (name, owner) in other.owners {
            self.owners.insert(format!("{owner_prefix}{name}"), owner);
        }
        for (mesh, b) in other.bounds {
            self.bounds.entry(mesh).or_insert(b);
        }
        let mut taken = 0;
        for mut slot in other.slots.into_iter().filter(|s| !have.contains(&s.mesh)) {
            for t in slot.textures.iter_mut().flatten() {
                t.file = abs(&t.file);
            }
            slot.uv2 = slot.uv2.as_deref().map(abs);
            slot.owner = format!("{owner_prefix}{}", slot.owner);
            self.slots.push(slot);
            taken += 1;
        }
        taken
    }

    /// The cooked bounds of a mesh (a mesh the pack has none for: a box about Sora's size).
    pub fn bounds_of(&self, mesh: &str) -> (Vec3, Vec3, f32) {
        self.bounds.get(mesh).copied().unwrap_or((Vec3::new(0.0, 0.0, 88.0), Vec3::new(75.0, 40.0, 88.0), 122.0))
    }

    /// Writes the frame's bone-driven parameters onto the slots of a mesh they name.
    pub fn feed(&mut self, mesh: &str, values: &[BoneValue]) {
        for slot in self.slots.iter_mut().filter(|s| s.mesh == mesh) {
            for (name, value, only) in values {
                if only.is_empty() || only.iter().any(|m| *m == slot.instance_name) {
                    // (every frame: no new key strings once the names are in)
                    match slot.vectors.get_mut(name) {
                        Some(v) => *v = *value,
                        None => drop(slot.vectors.insert(name.clone(), *value)),
                    }
                    match slot.scalars.get_mut(name) {
                        Some(s) => *s = value[0],
                        None => drop(slot.scalars.insert(name.clone(), value[0])),
                    }
                }
            }
        }
    }

    /// A scalar parameter by name on every slot of a mesh ("" = all).
    pub fn set_scalar(&mut self, mesh: &str, name: &str, value: f32) {
        for slot in self.slots.iter_mut().filter(|s| mesh.is_empty() || s.mesh == mesh) {
            match slot.scalars.get_mut(name) {
                Some(s) => *s = value,
                None => drop(slot.scalars.insert(name.to_string(), value)),
            }
        }
    }

    /// The uniform block of a slot for this frame. `exposure` multiplies the colour in front of
    /// the host's tone curve; `debug` is KH_CHAR_DEBUG's channel (0 = the picture).
    #[allow(clippy::too_many_arguments)]
    pub fn params(&self, slot: &Slot, object: &ObjectRows, rig: &Rig, khs: Option<&KhsState>, clock: f32, exposure: f32, debug: f32) -> Params {
        let mut p = Params::default();
        if let Some(owner) = self.owners.get(&slot.owner) {
            owner.eval(&slot.scalars, &slot.vectors, clock, &mut p.m);
        }
        (p.w2l, p.obj, p.bounds, p.actor, p.bmin, p.bmax) = (object.w2l, object.obj, object.bounds, object.actor, object.bmin, object.bmax);
        p.sh = rig.sh();
        p.light_dir = Vec4::new(0.0, 0.0, 1.0, if rig.own_lights() { 1.0 } else { 0.0 });
        (p.spot_pos, p.spot_dir, p.spot_cone) = rig.spot_rows();
        (p.point_pos, p.point_col) = rig.point_rows(); // (game over)
        p.light_color = rig.key.extend(if rig.shadows { 1.0 } else { 0.0 });
        // The cube's last mip (ComputeReflectionCaptureMipFromRoughness's CubemapMaxMip) and its scale.
        p.env = Vec4::new(ENV_SIZE.trailing_zeros() as f32, 1.0, 0.0, 0.0);
        // The two collections of the post-process material (the first is also the character
        // materials' own collection), the discernment mask, ScreenSpaceAO (STAND-IN: 1).
        match khs {
            Some(state) => {
                p.c0 = state.c0;
                p.k0 = state.k0;
                p.khs = Vec4::new(if state.on { 1.0 } else { 0.0 }, DISCERNMENT, 1.0, 0.0);
            }
            None => p.khs = Vec4::ZERO,
        }
        p.misc = Vec4::new(debug, clock, 0.0, 1.0);
        let (key_of_level, ambient_of_level) = (rig.key_of_level && !rig.own_lights(), rig.ambient_of_level && !rig.own_lights());
        p.look = Vec4::new(exposure, if key_of_level { 1.0 } else { 0.0 }, if ambient_of_level { 1.0 } else { 0.0 }, 0.0);
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Quat;

    #[test]
    fn the_object_rows_invert_the_component() {
        // A component turned a quarter about Z and moved: WorldToLocal takes a world point back.
        let l2w = kh_anim::Trs { translation: Vec3::new(100.0, 200.0, 50.0), rotation: Quat::from_rotation_z(std::f32::consts::FRAC_PI_2), scale: Vec3::ONE };
        let rows = ObjectRows::new(&l2w, (Vec3::new(0.0, 0.0, 88.0), Vec3::new(75.0, 40.0, 88.0), 122.0));
        let local = Vec3::new(10.0, 20.0, 30.0);
        let world = l2w.transform_point(local);
        let back = rows.w2l[0].truncate() * world.x + rows.w2l[1].truncate() * world.y + rows.w2l[2].truncate() * world.z + rows.w2l[3].truncate();
        assert!((back - local).length() < 1.0e-3, "{back:?}");
        assert!((rows.obj.truncate() - Vec3::new(100.0, 200.0, 138.0)).length() < 1.0e-3 && rows.obj.w == 122.0);
        assert_eq!((rows.bmin.truncate(), rows.bmax.truncate()), (Vec3::new(-75.0, -40.0, 0.0), Vec3::new(75.0, 40.0, 176.0)));
        assert_eq!(rows.actor, Vec4::new(100.0, 200.0, 50.0, 1.0));
    }

    /// (XW) The XS form weapons' library merged into Sora's: its slots with absolute files and prefixed owners whose
    /// constant buffer still evaluates; Sora's slots unchanged; a second merge takes nothing (skipped where the files are
    /// not on this machine)
    #[test]
    fn the_form_weapons_library_merges_with_its_own_owners() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let xsi = [root.join("../../../pack/xsi/weapons"), root.join("../../build/sora_tmp/er/xw/pack/xsi/weapons")].into_iter().find(|d| d.join("materials").join("materials.json").is_file());
        let main = [root.join("../../build/sora_pack"), root.join("../../../../../../sora_pack")].into_iter().find(|d| d.join("materials").join("materials.json").is_file());
        let (Some(xsi), Some(main)) = (xsi, main) else { return };
        let mut lib = Library::load(&main).unwrap();
        let before: Vec<(String, String)> = lib.slots.iter().map(|s| (s.mesh.clone(), s.slot.clone())).collect();
        let n = lib.merge(Library::load(&xsi).unwrap(), "xsi:");
        assert_eq!(n, 8);
        assert_eq!(lib.slots[..before.len()].iter().map(|s| (s.mesh.clone(), s.slot.clone())).collect::<Vec<_>>(), before);
        let hammer = lib.slots.iter().find(|s| s.slot == "w_so052_mat0").unwrap();
        assert_eq!((hammer.mesh.as_str(), hammer.owner.as_str(), hammer.owner_id), ("xsi:w_so052", "xsi:w_so050_mat0_Inst", 400));
        assert!(hammer.textures.iter().flatten().all(|t| Path::new(&t.file).is_absolute() && Path::new(&t.file).is_file()));
        let rows = ObjectRows::new(&kh_anim::Trs::IDENTITY, lib.bounds_of("xsi:w_so052"));
        let p = lib.params(hammer, &rows, &crate::stand_in::rig(), None, 0.0, 1.0, 0.0);
        assert!(p.m.iter().any(|v| *v != Vec4::ZERO), "the owner's constant buffer evaluates");
        assert_ne!(lib.bounds_of("xsi:w_so052"), lib.bounds_of("no such mesh"));
        assert_eq!(lib.merge(Library::load(&xsi).unwrap(), "xsi:"), 0);
    }

    /// (IMPL_ER23B) The costume change: Second Form's textures slide in over a second, become the worn ones, and the
    /// normal clothes come back in a fifth of a second when the form ends.
    #[test]
    fn the_wear_form_slides_in_and_out() {
        let json: Value = serde_json::from_str(
            r#"{"format":"kh3-sora-materials/1","generator":"t",
            "owners":{"O":{"id":7,"lighting_model":"MSM_Subsurface","base":{"ok":true},"mask":{"ok":true},
                "slots":[{"parameter":"Normal"},{"parameter":"FormChangeTex0"},{"parameter":"FormChangeTex1"},{"parameter":"FormChangePointTex0"},{"parameter":"FormChangePointTex1"}],
                "vec":[],"sca":[{"s":"FormChangeSlide","d":0.0}]},
              "P":{"id":8,"lighting_model":"MSM_Subsurface","base":{"ok":true},"mask":{"ok":true},"slots":[{"parameter":"Normal"}],"vec":[],"sca":[]}},
            "materials":{
              "cloth":{"mesh":"sora","owner":"O","asset":"/Game/X/cloth_Inst.cloth_Inst","blend_mode":"BLEND_Masked","scalars":{"FormChangeSlide":0},"vectors":{},
                "textures":[{"file":"textures/n.png","mips":9},{"file":"textures/d0.png","srgb":true,"mips":9},{"file":"textures/d0.png","srgb":true,"mips":9},{"file":"textures/f0.png","srgb":true,"mips":9},{"file":"textures/f0.png","srgb":true,"mips":9}]},
              "skin":{"mesh":"sora","owner":"P","asset":"/Game/X/skin_Inst.skin_Inst","blend_mode":"BLEND_Masked","scalars":{},"vectors":{},"textures":[{"file":"textures/n.png","mips":9}]}},
            "wear_forms":{
              "0":{"name":"Normal","materials":[{"material":"cloth_Inst","image":{"file":"textures/d0.png","srgb":true,"mips":9},"point":{"file":"textures/f0.png","srgb":true,"mips":9}}]},
              "5":{"name":"Limit","materials":[{"material":"cloth_Inst","image":{"file":"textures/d5.png","srgb":true,"mips":9},"point":{"file":"textures/f5.png","srgb":true,"mips":9}}]}},
            "mesh_facts":{}}"#,
        )
        .unwrap();
        let mut lib = Library::from_json(PathBuf::from("x"), &json).unwrap();
        let cloth = lib.slots.iter().position(|s| s.slot == "cloth").unwrap();
        let skin = lib.slots.iter().position(|s| s.slot == "skin").unwrap();
        let file = |lib: &Library, texture: usize| lib.slots[cloth].textures[texture].as_ref().unwrap().file.clone();
        let slide = |lib: &Library| lib.slots[cloth].scalars["FormChangeSlide"];
        // every texture of every form is asked for once
        let mut all: Vec<String> = lib.wear_textures().into_iter().map(|t| t.file).collect();
        all.sort();
        assert_eq!(all, ["textures/d0.png", "textures/d5.png", "textures/f0.png", "textures/f5.png"]);
        // nothing happens while he wears his normal clothes
        assert!(lib.step_wear(0, 1.0 / 60.0).is_empty() && slide(&lib) == 0.0);
        // SECOND FORM IS PUT ON: its textures go into the "1" slots at once, the slide starts
        let changes = lib.step_wear(5, 1.0 / 60.0);
        assert_eq!(changes.iter().map(|c| (c.slot, c.texture, c.file.file.as_str())).collect::<Vec<_>>(), [(cloth, 2, "textures/d5.png"), (cloth, 4, "textures/f5.png")]);
        assert!((slide(&lib) - 1.0 / 60.0).abs() < 1.0e-5 && file(&lib, 1) == "textures/d0.png");
        // half a second later: half way, no new binding
        for _ in 0..29 {
            assert!(lib.step_wear(5, 1.0 / 60.0).is_empty());
        }
        assert!((slide(&lib) - 0.5).abs() < 1.0e-3);
        // after the second: the new textures are the worn ones, the slide is back at 0
        let mut end = Vec::new();
        for _ in 0..31 {
            end.extend(lib.step_wear(5, 1.0 / 60.0));
        }
        assert_eq!(end.iter().map(|c| (c.texture, c.file.file.as_str())).collect::<Vec<_>>(), [(1, "textures/d5.png"), (3, "textures/f5.png")]);
        assert!(slide(&lib) == 0.0 && lib.wear.worn == 5 && file(&lib, 1) == "textures/d5.png");
        // a material without the parameter is never touched
        assert!(!lib.slots[skin].scalars.contains_key("FormChangeSlide"));
        // THE FORM ENDS: the normal clothes slide back in 0.2 s
        let back = lib.step_wear(0, 1.0 / 60.0);
        assert_eq!(back.iter().map(|c| (c.texture, c.file.file.as_str())).collect::<Vec<_>>(), [(2, "textures/d0.png"), (4, "textures/f0.png")]);
        let mut frames = 1;
        while lib.wear.worn != 0 {
            lib.step_wear(0, 1.0 / 60.0);
            frames += 1;
            assert!(frames < 100);
        }
        assert!((11..=13).contains(&frames), "{frames}");
        assert!(slide(&lib) == 0.0 && file(&lib, 1) == "textures/d0.png" && file(&lib, 3) == "textures/f0.png");
        // a form the pack has no textures for changes nothing
        assert!(lib.step_wear(9, 1.0 / 60.0).is_empty() && lib.wear.target == 0);
    }

    /// (ER69) An outfit without a form costume: its base colour is replaced while the form is on, put back after.
    #[test]
    fn a_direct_form_texture_comes_and_goes() {
        let json: Value = serde_json::from_str(
            r#"{"format":"kh3-sora-materials/1","generator":"t",
            "owners":{"O":{"id":7,"lighting_model":"MSM_Subsurface","base":{"ok":true},"mask":{"ok":true},"slots":[{"parameter":"NormalTex"},{"parameter":"BaseColorTex"}],"vec":[],"sca":[]}},
            "materials":{"cloth":{"mesh":"sora","owner":"O","asset":"/Game/X/cloth_Inst.cloth_Inst","blend_mode":"BLEND_Masked","scalars":{},"vectors":{},
                "textures":[{"file":"textures/n.png","mips":9},{"file":"textures/d.png","srgb":true,"mips":9}]}},"mesh_facts":{}}"#,
        )
        .unwrap();
        let mut lib = Library::from_json(PathBuf::from("x"), &json).unwrap();
        lib.add_wear_extra(&serde_json::from_str(r#"{"direct":{"5":[{"material":"cloth_Inst","parameter":"BaseColorTex","file":"textures/d_f5.png","srgb":true,"mips":9}]}}"#).unwrap());
        assert!(lib.wear_textures().iter().any(|t| t.file == "textures/d_f5.png"));
        assert!(lib.step_wear(0, 1.0 / 60.0).is_empty());
        let on = lib.step_wear(5, 1.0 / 60.0);
        assert_eq!(on.iter().map(|c| (c.texture, c.file.file.as_str())).collect::<Vec<_>>(), [(1, "textures/d_f5.png")]);
        assert!(lib.step_wear(5, 1.0 / 60.0).is_empty());
        let off = lib.step_wear(0, 1.0 / 60.0);
        assert_eq!(off.iter().map(|c| (c.texture, c.file.file.as_str())).collect::<Vec<_>>(), [(1, "textures/d.png")]);
        assert_eq!(lib.slots[0].textures[1].as_ref().unwrap().file, "textures/d.png");
    }

    #[test]
    fn a_slot_is_read_and_filled() {
        let json: Value = serde_json::from_str(
            r#"{"format":"kh3-sora-materials/1","generator":"t",
            "owners":{"O":{"id":7,"lighting_model":"MSM_Subsurface","base":{"ok":true},"mask":{"ok":true},
                "vec":[{"v":"Tint","d":[1,2,3,4]}],"sca":[{"s":"Fade","d":0.5}]}},
            "materials":{"slot_a":{"mesh":"sora","owner":"O","asset":"/Game/X/slot_a_Inst.slot_a_Inst","blend_mode":"BLEND_Masked","two_sided":true,
                "scalars":{"Fade":0.25},"vectors":{},"textures":[{"file":"textures/a.png","srgb":true,"mips":11},{"file":null},{"file":"textures/t.png","mips":1}]}},
            "uv2":{"sora:slot_a":{"file":"uv2_sora_0.bin"}},
            "mesh_facts":{"sora":{"bounds":{"origin":[0,0,88],"extent":[75,40,88],"radius":122},"bone_params":[]}}}"#,
        )
        .unwrap();
        let mut lib = Library::from_json(PathBuf::from("x"), &json).unwrap();
        let slot = lib.slots[0].clone();
        assert_eq!((slot.variant().as_str(), slot.instance_name.as_str(), slot.two_sided, slot.uv2.as_deref()), ("c7ms", "slot_a_Inst", true, Some("uv2_sora_0.bin")));
        assert_eq!(slot.textures, vec![Some(TextureRef { file: "textures/a.png".to_string(), srgb: true, mips: true }), None, Some(TextureRef { file: "textures/t.png".to_string(), srgb: false, mips: false })]);
        let object = ObjectRows::new(&kh_anim::Trs::IDENTITY, lib.bounds_of("sora"));
        let rig = crate::stand_in::rig();
        let p = lib.params(&slot, &object, &rig, None, 2.0, 1.0, 0.0);
        // The vector row, then the scalar row; the instance's value wins over the compiled default.
        assert_eq!((p.m[0], p.m[1].x), (Vec4::new(1.0, 2.0, 3.0, 4.0), 0.25));
        // A rig without spots: the host's key light; no in-shader curve; KHS off without a state.
        assert_eq!((p.light_dir.w, p.misc, p.khs.x, p.look.y), (0.0, Vec4::new(0.0, 2.0, 0.0, 1.0), 0.0, 0.0));
        lib.set_scalar("sora", "Fade", 1.0);
        assert_eq!(lib.params(&lib.slots[0], &object, &rig, None, 0.0, 1.0, 0.0).m[1].x, 1.0);
    }
}
