//! (ER90, XSI request 1; XW) THE XS MOVES' FORM WEAPONS IN THE ELDEN RING RENDERER (build/sora_tmp/party/XS_SORA_ABILITIES.md
//! 2.6, 2.7, 2.14; kh-app xsi_weapons.rs is the test app's side, the same rule): while Giant Stamp, Chandelier or Myriad
//! plays, KH3's form weapons of that move are drawn on Sora instead of the Kingdom Key.
//!
//! (XW) AS KH3 HAS THEM, from `PACK/xsi/weapons/weapons_xw.json` (build/sora_tmp/er/xw/tools/xw_form_weapons.js; read from
//! the game data: the equip data's form whose m_ReplaceAnimSets holds the clip's anim set):
//! - Giant Stamp: the Hyper Hammer `w_so052` on R_wep4_skt (bone R_buki4);
//! - Chandelier: the Blizzard Blades `w_so084` / `w_so085` on the forearms (R/L_ude2_wep2_skt) and `w_so086` / `w_so087`
//!   on the legs (R/L_ashi2_wep_skt): TRES_WEAPON_FORM_07 of AnimSet7 (ER90 / XSI drew w_so082 / w_so083, the other form);
//! - Myriad: Ultima `w_so192` on R_wep_skt (bone R_buki).
//! Each weapon is posed by its OWN skeleton and clip (kh_anim::formweapon: the Hyper Hammer's clip 3416 grows the hammer and
//! its parts; a weapon whose anim set has no row of the move holds its ResetPose), sampled at the time of Sora's clip, on
//! its socket (the socket's own turn); its slots are the weapons' OWN translated materials (`PACK/xsi/weapons/materials`,
//! tools/sora_material_pack.js outfit mode, owner ids 400+, meshes `xsi:<model>`), merged into the outfit's library, their
//! shaders `PACK/xsi/weapons/materials/er/c3xx*.ps.dxil` (gfx.rs `read_shaders` reads them for the `xsi:` slots).
//!
//! (XW-b) CHANDELIER'S CHANDELIER: `PACK/xsi/weapons/pawns_xwb.json` (build/sora_tmp/er/xwb/tools/xwb_chandelier.js): KH3's
//! StylePawn w_so088 with its own skeleton, its clip 7423 (the fall and the crash) and its own material
//! (`materials_xwb`, owner 410, shaders `materials_xwb/er`). The host hands it as one more `FormWeapon` ([`pawn_form`]:
//! where kh-core's `Sora::xs_chandelier_pawn` says it stands, its clip at its time); it is drawn as the form weapons are.
//!
//! Without weapons_xw.json: ER90's approximation as before (weapons.json + er_slots.json: copies of the Kingdom Key's slot
//! with the weapons' maps, bind pose, the Kingdom Key's socket turn). Neither: nothing changes (the Kingdom Key stays in his
//! hand). With `r.xsi_weapons = 0` (host) no form weapon is submitted.

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use glam::Vec3;
use kh_anim::formweapon::WeaponRig;
use kh_anim::Trs;
use kh_look::library::{Library, Slot, TextureRef};
use serde_json::Value;

/// The mesh-name prefix of a form weapon's slots and model.
pub const PREFIX: &str = "xsi:";

/// One form weapon of a move: the model, Sora's bone it hangs on, (XW) the socket relative to that bone and the weapon's
/// own skeleton + clip for the move.
#[derive(Clone, Debug)]
pub struct FormDef {
    pub model: String,
    pub bone: String,
    /// (XW) the socket relative to `bone` (UE rotator / location of p_ex001_0_Skeleton's SkeletalMeshSocket); None: the
    /// Kingdom Key's socket turn (ER90's table)
    pub socket: Option<Trs>,
    /// (XW) None: the bind pose (ER90's table, or the files are missing)
    pub rig: Option<Arc<WeaponRig>>,
}

impl PartialEq for FormDef {
    fn eq(&self, other: &FormDef) -> bool {
        self.model == other.model && self.bone == other.bone
    }
}

/// One form weapon model: its name, its slots' and model's mesh name (`xsi:<model>`), its glb, its skeleton (XW) and, for
/// ER90's table, its slots (glTF material -> base colour, normal map: absolute paths).
#[derive(Clone, Debug, Default)]
pub struct FormModel {
    pub model: String,
    pub mesh: String,
    pub glb: PathBuf,
    pub slots: Vec<(String, Option<String>, Option<String>)>,
    /// (XW) the weapon's skeleton: joint names -> bones, the reference pose (None: bind pose, ER90)
    pub rig: Option<Arc<WeaponRig>>,
}

/// The XSI weapon table as read: per move, its clip and weapons; the models; (XW) the weapons' own materials library.
#[derive(Clone, Debug, Default)]
pub struct Table {
    pub dir: PathBuf,
    /// (move, Sora's clip of it, weapons)
    pub moves: Vec<(String, String, Vec<FormDef>)>,
    pub models: Vec<FormModel>,
    pub neutral: Option<String>,
    /// (XW) `PACK/xsi/weapons` when it has `materials/materials.json` (the weapons' translated materials)
    pub own_materials: Option<PathBuf>,
    /// (XW-b) the material libraries to merge: (folder of its materials.json, the meshes it gives); its shaders in `<folder>/er`
    pub libs: Vec<(PathBuf, Vec<String>)>,
    /// (XW-b) the accompany pawns: (move, model)
    pub pawns: Vec<(String, String)>,
    /// (party VF-b) shards_vfb.json: the effect materials whose mesh particles the character pipeline draws (a fragment of
    /// the chandelier: w_so088_mat0_Inst) and, per effect mesh, its fragment model (`vfb_<mesh>`)
    pub shard_materials: Vec<String>,
    pub shards: Vec<(String, String)>,
}

/// (party VF-b) `fx.shards_own` (default on; read before the table is first read): the chandelier's fragments with their
/// own material through the character pipeline (shards_vfb.json); off: the effect's own draw (XW-c's stand-in).
pub static SHARDS_OWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// (party VF-b) A model of shards_vfb.json (drawn once per routed fragment instance).
pub fn is_shard_model(model: &str) -> bool {
    table().is_some_and(|t| t.shards.iter().any(|(_, m)| m == model))
}

/// (party VF-b) The fragment model a mesh particle of `material` / `mesh` (effect names, `xsi:` stripped) is drawn as.
pub fn shard_model(material: &str, mesh: &str) -> Option<&'static str> {
    let t = table()?;
    let strip = |n: &str| n.strip_prefix(PREFIX).unwrap_or(n).to_ascii_lowercase();
    let (material, mesh) = (strip(material), strip(mesh));
    if !t.shard_materials.iter().any(|m| m.eq_ignore_ascii_case(&material)) {
        return None;
    }
    t.shards.iter().find(|(m, _)| m.eq_ignore_ascii_case(&mesh)).map(|(_, model)| model.as_str())
}

/// A UE socket (location cm, rotator degrees, scale) as a transform relative to its bone.
pub fn socket_trs(location: &Value, rotation: &Value, scale: &Value) -> Trs {
    let f = |v: &Value, k: &str, d: f64| v.get(k).and_then(Value::as_f64).unwrap_or(d) as f32;
    Trs {
        translation: Vec3::new(f(location, "x", 0.0), f(location, "y", 0.0), f(location, "z", 0.0)),
        rotation: kh_anim::blend::Rotator { pitch: f(rotation, "pitch", 0.0), yaw: f(rotation, "yaw", 0.0), roll: f(rotation, "roll", 0.0) }.quat(),
        scale: Vec3::new(f(scale, "x", 1.0), f(scale, "y", 1.0), f(scale, "z", 1.0)),
    }
}

impl Table {
    /// (XW) `PACK/xsi/weapons/weapons_xw.json` first, else ER90's `weapons.json` + `er_slots.json`. None when neither is
    /// readable.
    pub fn read(pack: &Path) -> Option<Table> {
        // (absolute: the library joins its own folder with a slot's texture file, which an absolute path replaces)
        let dir = std::path::absolute(pack.join("xsi").join("weapons")).unwrap_or_else(|_| pack.join("xsi").join("weapons"));
        let read = |f: &str| std::fs::read_to_string(dir.join(f)).ok().and_then(|t| serde_json::from_str::<Value>(t.trim_start_matches('\u{feff}')).ok());
        if let Some(xw) = read("weapons_xw.json") {
            let mut t = Table::from_xw(&dir, &xw);
            // (XW-b) the chandelier pawn (pawns_xwb.json beside it)
            if let Some(p) = read("pawns_xwb.json") {
                t.add_pawns(&p);
            }
            // (party VF-b) the chandelier's fragments (shards_vfb.json beside it), unless fx.shards_own = 0
            if let Some(p) = read("shards_vfb.json").filter(|_| SHARDS_OWN.load(std::sync::atomic::Ordering::Relaxed)) {
                t.add_shards(&p);
                // (XW-d) shards_xwd.json (same format, after it): the pieces kh_vfx's fragment meshes add (hahen02 / hahen03)
                if let Some(x) = read("shards_xwd.json") {
                    t.add_shards(&x);
                }
            }
            return Some(t);
        }
        let (weapons, slots) = (read("weapons.json")?, read("er_slots.json")?);
        let abs = |rel: &str| dir.join(rel).to_string_lossy().to_string();
        let mut t = Table { dir: dir.clone(), neutral: slots["neutral"].as_str().map(abs), ..Default::default() };
        for m in weapons["moves"].as_array().into_iter().flatten() {
            let Some(name) = m["move"].as_str() else { continue };
            let mut list = Vec::new();
            for w in m["weapons"].as_array().into_iter().flatten() {
                let (Some(model), Some(bone), Some(file)) = (w["model"].as_str(), w["bone"].as_str(), w["file"].as_str()) else { continue };
                list.push(FormDef { model: model.to_string(), bone: bone.to_string(), socket: None, rig: None });
                if !t.models.iter().any(|m| m.model == model) {
                    let own: Vec<(String, Option<String>, Option<String>)> = slots["models"][model]["slots"].as_object().into_iter().flatten().map(|(mat, s)| (mat.clone(), s["base"].as_str().map(abs), s["normal"].as_str().map(abs))).collect();
                    t.models.push(FormModel { model: model.to_string(), mesh: format!("{PREFIX}{model}"), glb: dir.join(file), slots: own, rig: None });
                }
            }
            t.moves.push((name.to_string(), m["clip"].as_str().unwrap_or("").to_string(), list));
        }
        Some(t)
    }

    /// (XW) The table of weapons_xw.json: sockets, skeletons and clips, the materials library.
    fn from_xw(dir: &Path, xw: &Value) -> Table {
        let mut t = Table { dir: dir.to_path_buf(), ..Default::default() };
        t.own_materials = xw["materials"].as_str().filter(|m| dir.join(m).is_file()).map(|_| dir.to_path_buf());
        let lib_dir = xw["materials"].as_str().and_then(|m| Path::new(m).parent().map(|p| dir.join(p)));
        for m in xw["moves"].as_array().into_iter().flatten() {
            let Some(name) = m["move"].as_str() else { continue };
            let mut list = Vec::new();
            for w in m["weapons"].as_array().into_iter().flatten() {
                let (Some(model), Some(bone), Some(file)) = (w["model"].as_str(), w["bone"].as_str(), w["file"].as_str()) else { continue };
                let rig = match (w["skeleton"].as_str(), w["clip"]["file"].as_str()) {
                    (Some(s), Some(c)) => match WeaponRig::load(&dir.join(s), &dir.join(c)) {
                        Ok(r) => Some(Arc::new(r)),
                        Err(e) => {
                            crate::rlog!("render: (XW) form weapon {model}: its skeleton / clip not read ({e}): bind pose");
                            None
                        }
                    },
                    _ => None,
                };
                if !t.models.iter().any(|m| m.model == model) {
                    // (the model's skeleton: that of its first clip, the same skeleton file for every move)
                    t.models.push(FormModel { model: model.to_string(), mesh: format!("{PREFIX}{model}"), glb: dir.join(file), slots: Vec::new(), rig: rig.clone() });
                }
                list.push(FormDef { model: model.to_string(), bone: bone.to_string(), socket: Some(socket_trs(&w["socket_location_ue"], &w["socket_rotation_ue"], &w["socket_scale_ue"])), rig });
            }
            t.moves.push((name.to_string(), m["clip"].as_str().unwrap_or("").to_string(), list));
        }
        if let (Some(lib), Some(_)) = (lib_dir, &t.own_materials) {
            t.libs.push((lib, t.models.iter().map(|m| m.mesh.clone()).collect()));
        }
        t
    }

    /// (XW-b) The pawns of pawns_xwb.json: their models (skeleton + clip), their material library.
    fn add_pawns(&mut self, p: &Value) {
        let dir = self.dir.clone();
        let lib = p["materials"].as_str().filter(|m| dir.join(m).is_file()).and_then(|m| Path::new(m).parent().map(|d| dir.join(d)));
        let mut meshes = Vec::new();
        for w in p["pawns"].as_array().into_iter().flatten() {
            let (Some(mv), Some(model), Some(file)) = (w["move"].as_str(), w["model"].as_str(), w["file"].as_str()) else { continue };
            let rig = match (w["skeleton"].as_str(), w["clip"]["file"].as_str()) {
                (Some(s), Some(c)) => WeaponRig::load(&dir.join(s), &dir.join(c)).map_err(|e| crate::rlog!("render: (XW-b) pawn {model}: its skeleton / clip not read ({e})")).ok().map(Arc::new),
                _ => None,
            };
            if rig.is_none() || self.models.iter().any(|m| m.model == model) {
                continue;
            }
            self.models.push(FormModel { model: model.to_string(), mesh: format!("{PREFIX}{model}"), glb: dir.join(file), slots: Vec::new(), rig: rig.clone() });
            self.pawns.push((mv.to_string(), model.to_string()));
            meshes.push(format!("{PREFIX}{model}"));
        }
        if let (Some(lib), false) = (lib, meshes.is_empty()) {
            self.libs.push((lib, meshes));
        }
    }

    /// (party VF-b) The fragments of shards_vfb.json: static models (one joint), their material library (materials_vfb).
    fn add_shards(&mut self, p: &Value) {
        let dir = self.dir.clone();
        let Some(lib) = p["materials"].as_str().filter(|m| dir.join(m).is_file()).and_then(|m| Path::new(m).parent().map(|d| dir.join(d))) else { return };
        let mut meshes = Vec::new();
        for s in p["models"].as_array().into_iter().flatten() {
            let (Some(mesh), Some(model), Some(file)) = (s["mesh"].as_str(), s["model"].as_str(), s["file"].as_str()) else { continue };
            if !dir.join(file).is_file() || self.models.iter().any(|m| m.model == model) {
                continue;
            }
            self.models.push(FormModel { model: model.to_string(), mesh: format!("{PREFIX}{model}"), glb: dir.join(file), slots: Vec::new(), rig: None });
            self.shards.push((mesh.to_string(), model.to_string()));
            meshes.push(format!("{PREFIX}{model}"));
        }
        for r in p["routes"].as_array().into_iter().flatten().filter_map(|r| r["effect_material"].as_str()) {
            if !self.shard_materials.iter().any(|m| m.eq_ignore_ascii_case(r)) {
                self.shard_materials.push(r.to_string());
            }
        }
        if !meshes.is_empty() {
            crate::rlog!("render: (party VF-b) {} chandelier fragment model(s) drawn with their own material {} ({}); fx.shards_own = 0 switches it off", meshes.len(), self.shard_materials.join(", "), p["generator"].as_str().unwrap_or("shards_vfb.json"));
            self.libs.push((lib, meshes));
        }
    }

    pub fn weapons_of(&self, name: &str) -> &[FormDef] {
        self.moves.iter().find(|(m, ..)| m == name).map_or(&[], |(_, _, l)| l.as_slice())
    }

    /// (XW) Sora's clip of a move (`3416`, `7419`, `9419`): the form weapons' clips run on its clock.
    pub fn clip_of(&self, name: &str) -> Option<&str> {
        self.moves.iter().find(|(m, ..)| m == name).map(|(_, c, _)| c.as_str()).filter(|c| !c.is_empty())
    }
}

static TABLE: OnceLock<Option<Table>> = OnceLock::new();

/// The table of the pack the renderer was installed with (read once).
pub fn table() -> Option<&'static Table> {
    TABLE
        .get_or_init(|| {
            let pack = crate::SHARED.lock().ok().and_then(|g| g.as_ref().map(|s| s.pack.clone()))?;
            let t = Table::read(&pack);
            match &t {
                Some(t) if t.own_materials.is_some() => crate::rlog!(
                    "render: (XW) form weapons of the XS moves from {} (weapons_xw.json: their own materials, skeletons and clips): {}",
                    t.dir.display(),
                    t.moves.iter().map(|(m, c, w)| format!("{m} ({c}): {}", w.iter().map(|d| format!("{} on {}{}", d.model, d.bone, if d.rig.is_some() { "" } else { " BIND POSE" })).collect::<Vec<_>>().join(", "))).collect::<Vec<_>>().join("; ")
                ),
                Some(t) => crate::rlog!("render: (XSI) form weapons of the XS moves from {}: {}", t.dir.display(), t.models.iter().map(|m| format!("{} ({} slot(s))", m.model, m.slots.len())).collect::<Vec<_>>().join(", ")),
                None => crate::rlog!("render: (XSI) no form weapons (PACK/xsi/weapons/weapons_xw.json, or weapons.json + er_slots.json): the XS moves are drawn with the Kingdom Key"),
            }
            t
        })
        .as_ref()
}

/// A form weapon of this frame (the host's `FrameInput::forms`): the model, where its component is (GAME space) and (XW) its
/// bones relative to the component (empty: the reference / bind pose).
#[derive(Clone, Debug, PartialEq)]
pub struct FormWeapon {
    pub model: String,
    pub to_world: Trs,
    pub component: Vec<Trs>,
}

/// The form weapons of the move `name` (giant_stamp | chandelier | myriad) on Sora's pose: the component-space bone, the
/// weapon's socket (ER90's table: the Kingdom Key's socket turn `socket`, R_wep_skt relative to R_buki), the component's
/// place `to_world` (kh-app `socket_transform`, the same product); (XW) each weapon's bones from its own clip at `clip_time`
/// (the time of Sora's clip of the move). Empty without the table or for another name.
pub fn place(name: &str, index_of: &dyn Fn(&str) -> Option<usize>, component: &[Trs], to_world: &Trs, socket: &Trs, clip_time: f32) -> Vec<FormWeapon> {
    let Some(t) = table() else { return Vec::new() };
    place_with(t, name, index_of, component, to_world, socket, clip_time)
}

/// [`place`] over a given table.
pub fn place_with(t: &Table, name: &str, index_of: &dyn Fn(&str) -> Option<usize>, component: &[Trs], to_world: &Trs, socket: &Trs, clip_time: f32) -> Vec<FormWeapon> {
    t.weapons_of(name)
        .iter()
        .filter_map(|w| {
            let bone = component.get(index_of(&w.bone)?)?;
            let s = w.socket.unwrap_or(*socket);
            Some(FormWeapon { model: w.model.clone(), to_world: s.then(bone).then(to_world), component: w.rig.as_ref().map(|r| r.component(clip_time)).unwrap_or_default() })
        })
        .collect()
}

/// (XW) The time the form weapons' clips of the move `name` run at: Sora's clip time while his body slot plays the move's
/// clip (`sequence`, its first sample), else 0 (the clips' first key: the weapon as it is when the move begins).
pub fn clip_time(name: &str, sequence: Option<&str>, time: f32) -> f32 {
    match (table().and_then(|t| t.clip_of(name)), sequence) {
        (Some(clip), Some(s)) if clip == s => time,
        _ => 0.0,
    }
}

/// One slot of a form weapon (ER90's table): a copy of the Kingdom Key's slot `kk` named `mesh` / `material`, with the
/// weapon's own base colour (BaseColor / AO parameters) and normal map (Normal parameter), the neutral texture in the mask
/// slots (MaskTexture: metal / leather zones; the AO alpha), the Kingdom Key's fixed textures (the library tilings) kept.
pub fn form_slot(kk: &Slot, mesh: &str, material: &str, base: Option<&str>, normal: Option<&str>, neutral: Option<&str>) -> Slot {
    let mut s = kk.clone();
    s.mesh = mesh.to_string();
    s.slot = material.to_string();
    s.uv2 = None;
    for (k, name) in kk.texture_names.iter().enumerate() {
        let n = name.to_ascii_lowercase();
        let wanted = if n.contains("basecol") {
            base.map(|f| (f, true, true))
        } else if n.contains("normal") && !n.contains("library") && !n.is_empty() {
            normal.map(|f| (f, false, true))
        } else if n.contains("mask") || n.starts_with("ao") {
            neutral.map(|f| (f, false, false))
        } else {
            continue;
        };
        if let (Some((file, srgb, mips)), Some(slot)) = (wanted, s.textures.get_mut(k)) {
            *slot = Some(TextureRef { file: file.to_string(), srgb, mips });
        }
    }
    s
}

/// Appends the form weapons' slots to an outfit's library (after its own set is applied, before its shaders are read).
/// (XW) With the weapons' own materials (weapons_xw.json): their translated slots and owners (`Library::merge`, owners
/// keyed `xsi:<owner>`); else (ER90) copies of the Kingdom Key's slot (its variant: no new shader). Returns the models that
/// got slots.
pub fn add_slots(library: &mut Library) -> Vec<&'static FormModel> {
    let Some(t) = table() else { return Vec::new() };
    add_slots_with(t, library)
}

/// [`add_slots`] over a given table.
pub fn add_slots_with<'a>(t: &'a Table, library: &mut Library) -> Vec<&'a FormModel> {
    if t.own_materials.is_some() {
        if t.models.iter().any(|m| library.slots.iter().any(|s| s.mesh == m.mesh)) {
            return Vec::new();
        }
        // (XW-b) every library of the table: the form weapons' (materials), the chandelier pawn's (materials_xwb)
        for (lib, _) in &t.libs {
            let json = std::fs::read_to_string(lib.join("materials.json")).map_err(|e| e.to_string()).and_then(|t| serde_json::from_str::<Value>(t.trim_start_matches('﻿')).map_err(|e| e.to_string()));
            match json.and_then(|j| Library::from_json(lib.clone(), &j)) {
                Ok(own) => {
                    let n = library.merge(own, PREFIX);
                    crate::rlog!("render: (XW) the form weapons' own materials: {n} slot(s) from {}", lib.display());
                }
                Err(e) => crate::rlog!("render: (XW) the materials of {} not read ({e}): its models are not drawn", lib.display()),
            }
        }
        return t.models.iter().filter(|m| m.glb.is_file() && library.slots.iter().any(|s| s.mesh == m.mesh)).collect();
    }
    let Some(kk) = library.slots.iter().find(|s| s.mesh == "kingdom_key").cloned() else {
        crate::rlog!("render: (XSI) the library has no Kingdom Key slot: no form weapons");
        return Vec::new();
    };
    let mut models = Vec::new();
    for m in &t.models {
        if !m.glb.is_file() || m.slots.is_empty() || library.slots.iter().any(|s| s.mesh == m.mesh) {
            continue;
        }
        for (material, base, normal) in &m.slots {
            library.slots.push(form_slot(&kk, &m.mesh, material, base.as_deref(), normal.as_deref(), t.neutral.as_deref()));
        }
        models.push(m);
    }
    models
}

/// (XW) The folder of a slot's compiled shaders and the suffix its files take: a form weapon's own material (owner ids
/// 400+, `xsi:` slots of weapons_xw.json) is built once, without an outfit's own-set suffix, in
/// `PACK/xsi/weapons/materials/er`; every other slot as before (`dir`, `suffix`).
pub fn shader_dir<'a>(slot: &Slot, dir: &'a Path, suffix: &'a str) -> (PathBuf, &'a str) {
    // (XW-b) the folder of the library that gives the slot's mesh
    match table().and_then(|t| t.libs.iter().find(|(_, meshes)| meshes.contains(&slot.mesh))) {
        Some((lib, _)) if slot.mesh.starts_with(PREFIX) => (lib.join("er"), ""),
        _ => (dir.to_path_buf(), suffix),
    }
}

/// (XW-b) An accompany pawn of the move `name` (pawns_xwb.json) as one more form to draw: its actor at `ground` (GAME
/// space, cm) turned `yaw` (the core's convention; its mesh component -90 as a character's), its bones from its clip at
/// `time`. None without the table or such a pawn.
pub fn pawn_form(name: &str, ground: glam::Vec3, yaw: f32, time: f32) -> Option<FormWeapon> {
    pawn_form_with(table()?, name, ground, yaw, time)
}

/// [`pawn_form`] over a given table.
pub fn pawn_form_with(t: &Table, name: &str, ground: glam::Vec3, yaw: f32, time: f32) -> Option<FormWeapon> {
    let (_, model) = t.pawns.iter().find(|(m, _)| m == name)?;
    let rig = t.models.iter().find(|m| &m.model == model)?.rig.as_ref()?;
    let to_world = Trs { translation: ground, rotation: glam::Quat::from_rotation_z((yaw - 90.0).to_radians()), scale: Vec3::ONE };
    Some(FormWeapon { model: model.clone(), to_world, component: rig.component(time) })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kk() -> Slot {
        let t = |f: &str, srgb: bool| Some(TextureRef { file: f.to_string(), srgb, mips: true });
        Slot {
            mesh: "kingdom_key".into(),
            slot: "w_so010_mat0".into(),
            owner: "w_so010_mat0_Inst".into(),
            owner_id: 11,
            instance_name: "w_so010_mat0_Inst".into(),
            masked: true,
            translucent: false,
            two_sided: false,
            subsurface: true,
            textures: vec![t("textures/w_so010_0n00.png", false), t("textures/LibraryTiling16a_normal.png", false), t("textures/w_so010_0m00.png", false), t("textures/w_so010_0d00.png", true), t("textures/w_so010_0d00.png", false)],
            texture_names: vec!["NormalTex".into(), "".into(), "MaskTexture".into(), "BaseColorTex".into(), "AOTexture".into()],
            uv2: Some("uv2_kk.bin".into()),
            scalars: Default::default(),
            vectors: Default::default(),
        }
    }

    /// (ER90) a form weapon's slot: the Kingdom Key's material with the weapon's own maps; the library tiling kept
    #[test]
    fn a_form_slot_takes_the_weapons_maps_and_keeps_the_material() {
        let s = form_slot(&kk(), "xsi:w_so052", "w_so052_mat0", Some("D:/x/w_so052_0d00.png"), Some("D:/x/w_so052_0n00.png"), Some("D:/x/neutral.png"));
        assert_eq!((s.mesh.as_str(), s.slot.as_str(), s.owner_id, s.variant(), s.uv2.clone()), ("xsi:w_so052", "w_so052_mat0", 11, kk().variant(), None));
        let files: Vec<String> = s.textures.iter().map(|t| t.as_ref().unwrap().file.clone()).collect();
        assert_eq!(files, ["D:/x/w_so052_0n00.png", "textures/LibraryTiling16a_normal.png", "D:/x/neutral.png", "D:/x/w_so052_0d00.png", "D:/x/neutral.png"]);
        assert!(s.textures[3].as_ref().unwrap().srgb && !s.textures[0].as_ref().unwrap().srgb);
        // no base colour of its own: the Kingdom Key's stays (never white or missing)
        let s = form_slot(&kk(), "xsi:w_so082", "w_so082_mat0", None, Some("n.png"), None);
        assert_eq!(s.textures[3].as_ref().unwrap().file, "textures/w_so010_0d00.png");
        assert_eq!(s.textures[2].as_ref().unwrap().file, "textures/w_so010_0m00.png");
    }

    /// The staged XSI root with XW's files over it (a hard-link folder is not needed: the table reads one folder, so XW's
    /// test folder is XW's pack when it has the glbs, else skipped).
    /// (XW-d) a fragment model of the table (shards_vfb / shards_xwd.json)
    fn is_shard_of(t: &Table, model: &str) -> bool {
        t.shards.iter().any(|(_, m)| m == model)
    }

    fn xw_pack() -> Option<PathBuf> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        [root.join("../../../hpack"), root.join("../../build/sora_tmp/er/xw/hpack")].into_iter().find(|d| d.join("xsi").join("weapons").join("weapons_xw.json").is_file())
    }

    /// (XW) weapons_xw.json: the forms of the clips' anim sets (Chandelier's four Blizzard Blades), sockets with their own
    /// turn, every weapon with its skeleton and clip, the materials library (skipped without XW's harness pack)
    #[test]
    fn the_xw_table_names_every_weapon_with_its_rig() {
        let Some(pack) = xw_pack() else { return };
        let t = Table::read(&pack).unwrap();
        assert!(t.own_materials.is_some());
        let bones = |m: &str| t.weapons_of(m).iter().map(|w| (w.model.clone(), w.bone.clone())).collect::<Vec<_>>();
        assert_eq!(bones("giant_stamp"), [("w_so052".to_string(), "R_buki4".to_string())]);
        assert_eq!(bones("chandelier").iter().map(|(m, b)| format!("{m}@{b}")).collect::<Vec<_>>(), ["w_so084@R_ude2_buki2", "w_so085@L_ude2_buki2", "w_so086@R_ashi2", "w_so087@L_ashi2"]);
        assert_eq!(bones("myriad"), [("w_so192".to_string(), "R_buki".to_string())]);
        assert_eq!((t.clip_of("giant_stamp"), t.clip_of("chandelier"), t.clip_of("myriad")), (Some("3416"), Some("7419"), Some("9419")));
        for (_, _, list) in &t.moves {
            for w in list {
                assert!(w.rig.is_some() && w.socket.is_some(), "{}", w.model);
                // every socket is the bone's turn of roll -90 (as R_wep_skt on R_buki)
                let s = w.socket.unwrap();
                assert!(s.rotation.dot(glam::Quat::from_xyzw(std::f32::consts::FRAC_1_SQRT_2, 0.0, 0.0, std::f32::consts::FRAC_1_SQRT_2)).abs() > 0.9999, "{}", w.model);
            }
        }
        // (XW-d) and the fragment models of shards_vfb / shards_xwd.json when the pack has them
        assert_eq!(t.models.len(), 6 + t.pawns.len() + t.shards.len());
        assert!(t.models.iter().all(|m| m.glb.is_file() && (m.rig.is_some() || is_shard_of(&t, &m.model))));
        // the hammer's clip is applied: at 1.0 s of Giant Stamp its root is scaled up, the pose differs from the bind pose
        let index_of = |b: &str| ["R_buki4"].iter().position(|x| *x == b);
        let at = |time: f32| place_with(&t, "giant_stamp", &index_of, &[Trs::IDENTITY], &Trs::IDENTITY, &Trs::IDENTITY, time);
        let (rest, big) = (at(0.0), at(1.4));
        assert_eq!(rest.len(), 1);
        assert!(big[0].component[0].scale.x > 1.5 && (rest[0].component[0].scale.x - 1.0).abs() < 1.0e-3, "{} {}", big[0].component[0].scale.x, rest[0].component[0].scale.x);
    }

    /// (XW) the weapons' own materials merge into Sora's library: every primitive of every weapon glb has its slot, each
    /// slot's shader is in materials/er with all the sets the renderer may pick (skipped without XW's harness pack)
    #[test]
    fn the_own_materials_cover_every_primitive_and_set() {
        let Some(pack) = xw_pack() else { return };
        let t = Table::read(&pack).unwrap();
        let mut lib = Library::load(&pack).unwrap();
        let models = add_slots_with(&t, &mut lib);
        assert_eq!(models.len(), 6 + t.pawns.len() + t.shards.len());
        for m in models {
            let model = crate::model::Model::load(&m.glb).unwrap();
            for p in &model.primitives {
                let slot = lib.slots.iter().find(|s| s.mesh == m.mesh && s.slot == p.material).unwrap_or_else(|| panic!("{} {}", m.model, p.material));
                assert!(slot.owner_id >= 400 && slot.owner.starts_with(PREFIX));
                let dir = t.libs.iter().find(|(_, meshes)| meshes.contains(&m.mesh)).unwrap().0.join("er");
                for set in ["", "_pl", "_bl", "_bli", "_hy2", "_gb2"] {
                    assert!(dir.join(format!("{}{set}.ps.dxil", slot.variant())).is_file(), "{} {set}", slot.variant());
                }
            }
            // (XW-d) a fragment model (shards_*.json) is static: one joint, no rig
            let Some(rig) = m.rig.as_ref() else {
                assert!(is_shard_of(&t, &m.model), "{}", m.model);
                continue;
            };
            assert!(model.joints.iter().all(|(name, _)| rig.skeleton.index_of(name).is_some()), "{}: every joint is a bone of its skeleton", m.model);
        }
    }

    /// (XW-b) the chandelier pawn: in the table with its own library, placed at its ground point, its lower body falling
    /// with its clip (bodyunder from 3.93 s on); no pawn for another move
    #[test]
    fn the_chandelier_pawn_falls_with_its_own_clip() {
        let Some(pack) = xw_pack() else { return };
        let t = Table::read(&pack).unwrap();
        if t.pawns.is_empty() {
            return;
        }
        assert_eq!(t.pawns, [("chandelier".to_string(), "w_so088".to_string())]);
        assert!(t.libs.iter().any(|(lib, meshes)| meshes == &["xsi:w_so088".to_string()] && lib.join("er").join("c410ms_hy2.ps.dxil").is_file()));
        assert!(pawn_form_with(&t, "myriad", Vec3::ZERO, 0.0, 1.0).is_none());
        let ground = Vec3::new(500.0, -200.0, 10.0);
        let (hang, fallen) = (pawn_form_with(&t, "chandelier", ground, 30.0, 3.0).unwrap(), pawn_form_with(&t, "chandelier", ground, 30.0, 5.6).unwrap());
        assert_eq!((hang.model.as_str(), hang.to_world.translation), ("w_so088", ground));
        let rig = t.models.iter().find(|m| m.model == "w_so088").unwrap().rig.clone().unwrap();
        let under = rig.skeleton.index_of("bodyunder").unwrap();
        let drop = (hang.component[under].translation - fallen.component[under].translation).length();
        assert!(drop > 900.0, "the lower body falls about 10 m: {drop}");
    }

    /// (XW) THE CHECK AGAINST THE GAME'S OWN WEAPON TRACKS: the Hyper Hammer skinned by our path (weapons_xw.json's skeleton
    /// and clip through kh_anim's sampling, the renderer's joint matrices and the glb's inverse binds) has, at every time of
    /// Giant Stamp, the bounding box that plain glTF skinning of the converted game assets gives (xw/tools/ref_skin.js:
    /// checks/w_so052_3416_ref.json, ratios to the bind pose: rotation free) at each of its 94 keys (between keys the game's
    /// scale is geometric, kh_anim key_scale; the plain glTF reference is linear there). Skipped without those files.
    #[test]
    fn the_hammer_skins_as_the_games_own_tracks_say() {
        let Some(pack) = xw_pack() else { return };
        let Some(reference) = std::fs::read_to_string(pack.join("../checks/w_so052_3416_ref.json")).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) else { return };
        let t = Table::read(&pack).unwrap();
        let def = &t.weapons_of("giant_stamp")[0];
        let rig = def.rig.as_ref().unwrap();
        let model = crate::model::Model::load(&t.models.iter().find(|m| m.model == "w_so052").unwrap().glb).unwrap();
        let bones: Vec<usize> = model.joints.iter().map(|(n, _)| rig.skeleton.index_of(n).unwrap()).collect();
        let boxed = |pose: &[Trs]| {
            let mats: Vec<glam::Mat4> = bones.iter().zip(&model.joints).map(|(b, (_, ibm))| crate::math::joint_in_pack(&pose[*b]) * *ibm).collect();
            let (mut lo, mut hi) = (Vec3::splat(1.0e9), Vec3::splat(-1.0e9));
            for v in &model.vertices {
                let p = Vec3::from(v.position);
                let s: Vec3 = (0..4).filter(|k| v.weights[*k] > 0.0).map(|k| mats[v.joints[k] as usize].transform_point3(p) * v.weights[k]).sum();
                (lo, hi) = (lo.min(s), hi.max(s));
            }
            let mut e = (hi - lo).to_array();
            e.sort_by(f32::total_cmp);
            (Vec3::from(e).length(), e)
        };
        let (bind_diag, bind_e) = boxed(&rig.reference());
        let mut worst = 0.0f32;
        let rows = reference["rows"].as_array().unwrap();
        for row in rows {
            let time = row["t"].as_f64().unwrap() as f32;
            let (diag, e) = boxed(&rig.component(time));
            let d = (diag / bind_diag - row["diag_ratio"].as_f64().unwrap() as f32).abs();
            if d > 1.0e-3 {
                println!("  t {time}: ours {} game {}", diag / bind_diag, row["diag_ratio"]);
            }
            worst = worst.max(d);
            for k in 0..3 {
                worst = worst.max((e[k] / bind_e[k] - row["extent_ratios"][k].as_f64().unwrap() as f32).abs());
            }
        }
        println!("w_so052 3416: {} times, largest ratio difference {worst:.2e}", rows.len());
        assert!(worst < 2.0e-3, "largest difference of a ratio: {worst}");
    }
}
