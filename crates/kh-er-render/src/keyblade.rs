//! (P5 WEAPON DRAW, documentation/KH3_WEAPONS_PLAN.md 1.3 / 4 row P5) THE KEYBLADE IN SORA'S HAND, WHICHEVER HE HAS
//! EQUIPPED, AND THE WEAPON SIDE OF HIS FORM CHANGES, from the keyblades pack root (`PACK/keyblades`,
//! tools/sora_keyblades_pack.js; documentation/KH3_WEAPON_ASSETS.md):
//!
//! - THE TABLE ([`table`]): `keyblades/manifest.json` (`kh3-sora-keyblades/1`): per keyblade (`w_soNNN`, its item keys
//!   `WEP_KEYBLADE_SO_NN`) its base model, every model it uses (forms, transform units, finisher pawns) and its forms (the
//!   weapons on Sora's sockets with the socket's own turn, the transform units, the accompany pawns); per model its glb,
//!   skeleton and clips (by the logical names its anim set gives: the weapon plays the row of the name Sora plays).
//! - THE `kb:` LIBRARY ([`add_slots`]): the models' translated materials (`keyblades/materials/materials.json`, owner ids
//!   500+, meshes `kb:<model>`) merged into Sora's library like the XS form weapons' (`Library::merge`, owners keyed
//!   `kb:<owner>`); ONLY the models wanted now (below) are taken, so a load reads their textures and nothing else of the
//!   pack's 1.2 GB. Their shaders are `keyblades/materials/er/<variant><set>.ps.dxil` ([`shader_dir`]), built for Sora's
//!   vertex layout (kh-shaderc's normal mode with the main pack's world post-process block spliced in:
//!   build/sora_tmp/weapons/P5/tools/splice_kb_wgsl.js + kb_shader_jobs.js, `--rebuild`), with every set the renderer may
//!   pick ("", _pl, _bl, _bli, _hy2, _gb2, _hy3, _gb3, the Mimic Tear's _gb3_mm / _gb3_mf).
//! - WHAT IS LOADED ([`set_equipped`], the host every frame): (KEYBLADE-SWAP) EVERY SLOTTED KEYBLADE WITH ALL OF ITS MODELS
//!   (the one in his hand and the other two: a D-pad switch between them changes nothing that is loaded, so it loads nothing
//!   and is drawn at once; its forms can start at once). Only an equip change (another keyblade put into a slot) changes that
//!   set: a KEYBLADE-ONLY reload on the renderer's worker (gfx.rs `Job::Load`): Sora's textures, meshes and pipeline states
//!   are taken over from the drawn assets (shared, never a second copy), only the keyblade models' own meshes, textures,
//!   `kb:` slots and their pipeline states are made; the drawn assets stay until then and are RETIRED to the GPU fence
//!   (outfit.rs `retire`: released once the GPU has passed the frame that last used them, off the game's thread). (It was the
//!   hand's keyblade whole + the others' base models: every D-pad switch reloaded all of Sora's ~300 MB, 1.3-2.7 s, and the
//!   hand-over stalled the game's frame.) The Kingdom Key is the main pack's (`weapon/kingdom_key.glb`, its tuned slot):
//!   never taken from here.
//! - WHAT IS DRAWN ([`hand`] / [`role`]): his weapon (`FrameInput::weapon`: R_wep_skt, the host's keychain pose of THAT
//!   keyblade's own skeleton, its opacity) is drawn with the current keyblade's base model; the Kingdom Key's model is not
//!   drawn meanwhile. A keyblade not loaded yet is not drawn (the reload is under way; a failed load draws the Kingdom
//!   Key). Form weapon parts arrive as `FrameInput::forms` entries named by their mesh (`kb:<model>`).
//! - FORM WEAPONS ([`draw_form_weapon_parts`], [`form_pawn`]): THE HOOK FOR P3 AND THE F-PACKAGES. A form's weapons on
//!   their sockets (Crystal Snow's form 07: four Blizzard Blades on the forearms and shins; Double Form: Oathkeeper in the
//!   right hand, Oblivion in the left), its transform unit (the "Deform" model, on R_wep_skt when the data names no
//!   socket), each posed by its OWN skeleton and the clip of the logical name Sora plays (else its anim blueprint's
//!   default clip `ResetPose`, looped as KH3 has it: (DRILL-IDLE) [`model_pose`]; only without one its reference pose), and a
//!   finisher pawn at a ground point. The host hides his hand's keyblade while a form draws its own weapons.
//!
//! Nothing of this runs without `PACK/keyblades/manifest.json`: the Kingdom Key is drawn as before.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use glam::{Quat, Vec3};
use kh_anim::formweapon::{DefaultClock, DefaultPlay, WeaponRig};
use kh_anim::{Skeleton, Trs};
use kh_look::library::{Library, Slot};
use serde_json::Value;

use crate::xsiweapon::FormWeapon;

/// The mesh-name prefix of a keyblade model's slots and model (and of its `FormWeapon::model`).
pub const PREFIX: &str = "kb:";
/// The Kingdom Key's id: drawn from the main pack, never from the keyblades root.
pub const KINGDOM_KEY: &str = "w_so010";

/// One weapon of a form on Sora: the unit (`RWeapon`, `LWeapon`, `Deform`), the part (`RIGHT_HAND` ...), the model, Sora's
/// bone it hangs on and the socket relative to that bone (None: the data names no socket; it is put on R_wep_skt).
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub unit: String,
    pub part: String,
    pub model: String,
    pub bone: Option<String>,
    pub socket: Option<Trs>,
    /// (FORMWEP-FIX) a TRANSFORM unit the equipment attaches with NO socket (`Deform@RIGHT_HAND:None`, Nano Arms'
    /// `None@RIGHT_HAND:None`: UE's AttachToComponent with NAME_None is Sora's mesh component itself, 0x14177E4E0): its
    /// own clips are authored in his mesh space ([`place_part`])
    pub on_mesh: bool,
}

/// One form of a keyblade (`TRES_WEAPON_FORM_07` ...) at one grow level: its wear form, weapons, transform units, pawns.
#[derive(Clone, Debug, PartialEq)]
pub struct Form {
    pub form: String,
    pub grow_level: String,
    pub wear_form: String,
    pub weapons: Vec<Part>,
    pub transform: Vec<Part>,
    /// (command, model): the accompany pawns (finishers)
    pub pawns: Vec<(String, String)>,
    pub anim_sets: Vec<String>,
}

/// One keyblade of the manifest.
#[derive(Clone, Debug, PartialEq)]
pub struct Keyblade {
    /// `w_soNNN`
    pub id: String,
    pub name: String,
    /// its item keys (`WEP_KEYBLADE_SO_06`)
    pub keys: Vec<String>,
    pub base: String,
    pub models: Vec<String>,
    pub forms: Vec<Form>,
    pub dlc: bool,
}

/// One model of the root: its mesh name (`kb:<model>`), folder, glb, skeleton (component-space reference pose from it)
/// and clips by logical name.
#[derive(Clone, Debug)]
pub struct KbModel {
    pub model: String,
    pub mesh: String,
    pub dir: PathBuf,
    pub glb: PathBuf,
    pub skeleton: Option<Arc<Skeleton>>,
    pub clips: Vec<(String, PathBuf)>,
    /// (DRILL-IDLE) its anim blueprint's `m_DefaultAnimName` (`ResetPose`; None: no default) and how it plays
    /// (kh_anim::formweapon::DefaultPlay: PACK/keyblades/anim_instances.json)
    pub default_anim: Option<String>,
    pub default_play: DefaultPlay,
}

/// The keyblades root as read.
#[derive(Clone, Debug, Default)]
pub struct Table {
    /// `PACK/keyblades` (absolute)
    pub dir: PathBuf,
    pub keyblades: Vec<Keyblade>,
    pub models: Vec<KbModel>,
}

fn text(v: &Value) -> String {
    v.as_str().unwrap_or("").to_string()
}

/// (DRILL-IDLE) The weapons' anim-instance file of the keyblades root (tools/sora_keyblades_pack.js phase `anim_instances`).
pub const ANIM_INSTANCES: &str = "anim_instances.json";

/// (DRILL-IDLE) A model's default clip: the anim-instance row (`default_anim`, `default_anim_loop`,
/// `default_anim_check_same`), else the manifest's `default_anim` (played once, restarted at each call).
fn default_of(manifest: &Value, instance: &Value) -> (Option<String>, DefaultPlay) {
    let name = instance["default_anim"].as_str().or_else(|| manifest["default_anim"].as_str()).filter(|n| !n.is_empty() && *n != "None").map(str::to_string);
    let play = DefaultPlay { looping: instance["default_anim_loop"].as_bool().unwrap_or(false), check_same: instance["default_anim_check_same"].as_bool().unwrap_or(false) };
    (name, play)
}

fn part_of(w: &Value) -> Option<Part> {
    let model = w["model"].as_str()?.to_string();
    let bone = w["bone"].as_str().map(str::to_string);
    let socket = w["socket_rotation_ue"].is_object().then(|| crate::xsiweapon::socket_trs(&w["socket_location_ue"], &w["socket_rotation_ue"], &w["socket_scale_ue"]));
    Some(Part { unit: text(&w["unit"]), part: text(&w["part"]), model, bone, socket, on_mesh: false })
}

/// (FORMWEP-FIX) A form's TRANSFORM unit: [`part_of`], on his mesh component when the data names neither bone nor socket.
fn transform_part_of(w: &Value) -> Option<Part> {
    part_of(w).map(|p| Part { on_mesh: p.bone.is_none() && p.socket.is_none(), ..p })
}

impl Table {
    /// `PACK/keyblades/manifest.json`; None without it.
    pub fn read(pack: &Path) -> Option<Table> {
        let dir = std::path::absolute(pack.join("keyblades")).unwrap_or_else(|_| pack.join("keyblades"));
        let json: Value = std::fs::read_to_string(dir.join("manifest.json")).ok().and_then(|t| serde_json::from_str(t.trim_start_matches('\u{feff}')).ok())?;
        Some(Table::from_json(&dir, &json))
    }

    pub fn from_json(dir: &Path, json: &Value) -> Table {
        let mut t = Table { dir: dir.to_path_buf(), ..Default::default() };
        for (id, k) in json["keyblades"].as_object().into_iter().flatten() {
            let Some(base) = k["base_model"].as_str() else { continue };
            let forms = k["forms"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|f| Form {
                    form: text(&f["form"]),
                    grow_level: text(&f["grow_level"]),
                    wear_form: text(&f["wear_form"]),
                    weapons: f["weapons"].as_array().into_iter().flatten().filter_map(part_of).collect(),
                    transform: f["transform"].as_array().into_iter().flatten().filter_map(transform_part_of).collect(),
                    pawns: f["accompany_pawns"].as_array().into_iter().flatten().filter_map(|p| Some((text(&p["command"]), p["model"].as_str()?.to_string()))).collect(),
                    anim_sets: f["sora_anim_sets"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect(),
                })
                .collect();
            t.keyblades.push(Keyblade {
                id: id.clone(),
                name: text(&k["name"]),
                keys: k["items"].as_array().into_iter().flatten().filter_map(|i| i["key"].as_str()).map(str::to_string).collect(),
                base: base.to_string(),
                models: k["models"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect(),
                forms,
                dlc: k["dlc"].as_bool().unwrap_or(false),
            });
        }
        // (DRILL-IDLE) the weapons' anim instances (their anim blueprints' default-clip fields), beside the manifest
        let instances: Value = std::fs::read_to_string(dir.join(ANIM_INSTANCES)).ok().and_then(|t| serde_json::from_str(t.trim_start_matches('\u{feff}')).ok()).unwrap_or(Value::Null);
        for (name, m) in json["models"].as_object().into_iter().flatten() {
            let (default_anim, default_play) = default_of(m, &instances["models"][name.as_str()]);
            let sub = m["dir"].as_str().unwrap_or(name);
            let mdir = dir.join(sub);
            let skeleton = m["skeleton"].as_str().and_then(|s| std::fs::read_to_string(dir.join(s)).ok()).and_then(|text| kh_anim::json::Json::parse(text.trim_start_matches('\u{feff}')).ok()).and_then(|j| Skeleton::from_manifest(&j).ok()).map(Arc::new);
            let clips = m["clips"].as_object().into_iter().flatten().filter_map(|(logical, f)| Some((logical.clone(), dir.join(f.as_str()?)))).collect();
            let glb = m["glb"].as_str().map_or_else(|| mdir.join("weapon.glb"), |g| dir.join(g));
            t.models.push(KbModel { model: name.clone(), mesh: format!("{PREFIX}{name}"), dir: mdir, glb, skeleton, clips, default_anim, default_play });
        }
        t
    }

    /// A keyblade by its id (`w_so080`) or one of its item keys (`WEP_KEYBLADE_SO_06`).
    pub fn keyblade(&self, id_or_key: &str) -> Option<&Keyblade> {
        self.keyblades.iter().find(|k| k.id.eq_ignore_ascii_case(id_or_key) || k.keys.iter().any(|x| x.eq_ignore_ascii_case(id_or_key)))
    }

    pub fn model(&self, name: &str) -> Option<&KbModel> {
        let name = name.strip_prefix(PREFIX).unwrap_or(name);
        self.models.iter().find(|m| m.model == name)
    }

    /// THE MODELS TO HAVE LOADED for this equipment: (KEYBLADE-SWAP) every model of every slotted keyblade (the one in his
    /// hand and the others alike: a D-pad switch between them keeps this set), never the Kingdom Key's own (the main
    /// pack's); sorted, each once. The order of `current` and `slots` does not matter.
    pub fn wanted_models(&self, current: &str, slots: &[&str]) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for k in std::iter::once(current).chain(slots.iter().copied()).filter_map(|s| self.keyblade(s)) {
            out.extend(std::iter::once(k.base.clone()).chain(k.models.iter().cloned()));
        }
        out.retain(|m| m != KINGDOM_KEY && self.model(m).is_some_and(|x| x.glb.is_file()));
        out.sort();
        out.dedup();
        out
    }

    /// A form's entry (`TRES_WEAPON_FORM_07`, also `07`) at a grow level (`TRES_WEAPON_GROW_LVL02` or `2`; empty: the
    /// first entry of that form).
    pub fn form(&self, keyblade: &str, form: &str, grow: &str) -> Option<&Form> {
        let k = self.keyblade(keyblade)?;
        let want = |f: &Form| f.form.eq_ignore_ascii_case(form) || f.form.rsplit('_').next().is_some_and(|n| n.eq_ignore_ascii_case(form));
        let grow_ok = |f: &Form| grow.is_empty() || f.grow_level.eq_ignore_ascii_case(grow) || f.grow_level.trim_start_matches("TRES_WEAPON_GROW_LVL").trim_start_matches('0') == grow.trim_start_matches('0');
        k.forms.iter().find(|f| want(f) && grow_ok(f)).or_else(|| k.forms.iter().find(|f| want(f)))
    }
}

static TABLE: OnceLock<Option<Table>> = OnceLock::new();

/// The keyblades root of the pack the renderer was installed with (read once).
pub fn table() -> Option<&'static Table> {
    TABLE
        .get_or_init(|| {
            let pack = crate::SHARED.lock().ok().and_then(|g| g.as_ref().map(|s| s.pack.clone()))?;
            let t = Table::read(&pack);
            match &t {
                Some(t) => crate::rlog!("render: (P5) the keyblades root {}: {} keyblades, {} models (loaded per equipped keyblade)", t.dir.display(), t.keyblades.len(), t.models.len()),
                None => crate::rlog!("render: (P5) no keyblades root (PACK/keyblades/manifest.json): the Kingdom Key only"),
            }
            t
        })
        .as_ref()
}

// ---------------------------------------------------------------------------------------------------- what is loaded

#[derive(Default)]
struct State {
    /// the keyblade in his hand (its id) and the other slots' ids, as the host said
    current: String,
    slots: Vec<String>,
    /// the models wanted for that, the models in the drawn assets, the sets that failed to load
    wanted: Vec<String>,
    loaded: Vec<String>,
    failed: Vec<Vec<String>>,
    told: String,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> R {
    let mut g = STATE.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(State::default))
}

/// THE HOST'S EQUIPMENT (every frame is fine): the keyblade in his hand and the other slotted keyblades, each by id
/// (`w_so080`) or item key (`WEP_KEYBLADE_SO_06`); "" / unknown = the Kingdom Key.
pub fn set_equipped(current: &str, slots: &[&str]) {
    let Some(t) = table() else { return };
    let id = |s: &str| t.keyblade(s).map_or_else(|| KINGDOM_KEY.to_string(), |k| k.id.clone());
    let current = id(current);
    let slots: Vec<String> = slots.iter().filter(|s| !s.is_empty()).map(|s| id(s)).filter(|s| *s != current).collect();
    with_state(|st| {
        if st.current == current && st.slots == slots {
            return;
        }
        // (KEYBLADE-SWAP) a D-pad switch keeps the set of slotted keyblades: the wanted models stay as they are (no file look-up
        // on the game's thread, nothing to load); only another set (an equip change) is looked up again
        let set = |c: &str, s: &[String]| {
            let mut v: Vec<String> = std::iter::once(c.to_string()).chain(s.iter().cloned()).collect();
            v.sort();
            v
        };
        if st.current.is_empty() || set(&st.current, &st.slots) != set(&current, &slots) {
            let refs: Vec<&str> = slots.iter().map(String::as_str).collect();
            st.wanted = t.wanted_models(&current, &refs);
        }
        if st.told != current {
            crate::rlog!("render: (P5) keyblade in his hand: {current} ({}); slots {:?}; models wanted {:?}", t.keyblade(&current).map_or("?", |k| k.name.as_str()), slots, st.wanted);
            st.told.clone_from(&current);
        }
        st.current = current;
        st.slots = slots;
    });
}

/// A reload is due: other models are wanted than the drawn assets have, and that set has not failed before.
pub fn reload_wanted() -> bool {
    with_state(|st| st.wanted != st.loaded && !st.failed.contains(&st.wanted))
}

/// The worker starts a load: the models it is for.
pub fn begin_load() -> Vec<String> {
    with_state(|st| if st.failed.contains(&st.wanted) { st.loaded.clone() } else { st.wanted.clone() })
}

/// The load for `models` ended: on success they are the drawn ones; a failure is remembered (the drawn assets stay).
pub fn end_load(models: &[String], ok: bool) {
    with_state(|st| {
        if ok {
            st.loaded = models.to_vec();
        } else if !st.failed.iter().any(|f| f == models) {
            st.failed.push(models.to_vec());
        }
    });
}

/// The models in the drawn assets.
pub fn loaded() -> Vec<String> {
    with_state(|st| st.loaded.clone())
}

/// Appends the wanted models' slots (`models`, from [`begin_load`]) to Sora's library (after the XS form weapons').
/// Returns the number of slots taken.
pub fn add_slots(library: &mut Library, models: &[String]) -> usize {
    match table() {
        Some(t) if !models.is_empty() => add_slots_with(t, library, models),
        _ => 0,
    }
}

/// [`add_slots`] over a given table.
pub fn add_slots_with(t: &Table, library: &mut Library, models: &[String]) -> usize {
    let lib = t.dir.join("materials");
    let json = std::fs::read_to_string(lib.join("materials.json")).map_err(|e| e.to_string()).and_then(|text| serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}')).map_err(|e| e.to_string()));
    match json.and_then(|j| Library::from_json(lib.clone(), &j)) {
        Ok(mut own) => {
            let meshes: Vec<String> = models.iter().map(|m| format!("{PREFIX}{m}")).collect();
            own.slots.retain(|s| meshes.contains(&s.mesh));
            let n = library.merge(own, PREFIX);
            crate::rlog!("render: (P5) the keyblades' own materials: {n} slot(s) of {} model(s) from {}", models.len(), lib.display());
            n
        }
        Err(e) => {
            crate::rlog!("render: (P5) the keyblades' materials ({}) not read ({e}): no keyblade but the Kingdom Key is drawn", lib.display());
            0
        }
    }
}

/// The folder of a keyblade slot's compiled shaders (a merged `kb:` owner): `PACK/keyblades/materials/er`, no suffix.
pub fn shader_dir(slot: &Slot) -> Option<PathBuf> {
    (slot.mesh.starts_with(PREFIX) && slot.owner.starts_with(PREFIX)).then(|| table().map(|t| t.dir.join("materials").join("er"))).flatten()
}

/// The keyblade models whose slots the library has (the models the assets load).
pub fn models_in(library: &Library) -> Vec<&'static KbModel> {
    table().map(|t| t.models.iter().filter(|m| library.slots.iter().any(|s| s.mesh == m.mesh)).collect()).unwrap_or_default()
}

/// A model's reference pose relative to its component (its skeleton's bind pose; empty without a skeleton).
pub fn reference(model: &KbModel) -> Vec<Trs> {
    model.skeleton.as_ref().map(|s| kh_anim::pose::component_space(s, &s.reference_pose())).unwrap_or_default()
}

/// THE KEYCHAIN of a keyblade model (its own SQEX_BonamikAsset on its own skeleton: `<model>/bonamik.json`, written by
/// tools/sora_weapon_pack.js --keyblades, and `<model>/skeleton.json`), for the host to simulate as the Kingdom Key's
/// (kh_anim::Keychain); the pose it returns is in THIS model's bone order (what `FrameInput::weapon.component` must be
/// while this keyblade is in his hand). None: no such model, or it has no keychain (it then hangs rigid).
pub fn keychain(model: &str) -> Option<Result<kh_anim::Keychain, String>> {
    keychain_in(table()?, model)
}

/// (KEYCHAIN-DUAL) [`keychain`] from a given table (the offline tests' [`Table::read`]).
pub fn keychain_in(t: &Table, model: &str) -> Option<Result<kh_anim::Keychain, String>> {
    let m = t.model(model)?;
    let read = |f: &str| -> Result<kh_anim::json::Json, String> {
        let path = m.dir.join(f);
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        kh_anim::json::Json::parse(text.trim_start_matches('\u{feff}')).map_err(|e| format!("{}: {e}", path.display()))
    };
    if !m.dir.join("bonamik.json").is_file() {
        return None;
    }
    Some(read("skeleton.json").and_then(|s| read("bonamik.json").and_then(|b| kh_anim::Keychain::new(&s, &b))))
}

/// A keyblade model's skeleton sockets (`<model>/sockets.json`: the blade's trail points vfx_top / vfx_bottom ... relative
/// to its bone `wep`, the format of `PACK/vfx/host.json` `weapon_sockets`), for the effect side.
/// (KEYBLADE-SWAP) Read once per model: a D-pad switch reads no file on the game's thread.
pub fn sockets_json(model: &str) -> Option<Value> {
    static READ: Mutex<Option<HashMap<String, Option<Value>>>> = Mutex::new(None);
    let mut g = READ.lock().unwrap_or_else(|e| e.into_inner());
    g.get_or_insert_with(HashMap::new)
        .entry(model.to_string())
        .or_insert_with(|| {
            let m = table()?.model(model)?;
            let text = std::fs::read_to_string(m.dir.join("sockets.json")).ok()?;
            serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}')).ok().map(|v| v["weapon_sockets"].clone())
        })
        .clone()
}

// ---------------------------------------------------------------------------------------------------- what is drawn

/// Which model is his hand's keyblade this frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hand {
    /// the main pack's Kingdom Key (`kingdom_key`)
    KingdomKey,
    /// a keyblade model of the root (`kb:<model>`)
    Model(String),
    /// the keyblade's models are being loaded: none is drawn
    Loading,
}

/// The hand's keyblade of this frame (from the host's equipment and what is loaded).
pub fn hand() -> Hand {
    let Some(t) = table() else { return Hand::KingdomKey };
    with_state(|st| match t.keyblade(&st.current) {
        Some(k) if k.base != KINGDOM_KEY => {
            if st.loaded.contains(&k.base) {
                Hand::Model(format!("{PREFIX}{}", k.base))
            } else if st.failed.contains(&st.wanted) {
                Hand::KingdomKey
            } else {
                Hand::Loading
            }
        }
        _ => Hand::KingdomKey,
    })
}

/// How a model of the assets is drawn this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// at `FrameInput::weapon` (his hand's keyblade)
    Hand,
    /// at a `FrameInput::forms` entry (an XS form weapon, a keyblade's form part)
    Form,
    /// not drawn
    Skip,
}

/// The role of a model (by its mesh name; not "sora") this frame.
pub fn role(mesh: &str, hand: &Hand, forms: &[FormWeapon]) -> Role {
    if mesh.starts_with(crate::xsiweapon::PREFIX) {
        return Role::Form;
    }
    if let Some(_rest) = mesh.strip_prefix(PREFIX) {
        if forms.iter().any(|f| f.model == mesh) {
            return Role::Form;
        }
        return if *hand == Hand::Model(mesh.to_string()) { Role::Hand } else { Role::Skip };
    }
    match hand {
        Hand::KingdomKey => Role::Hand,
        _ => Role::Skip,
    }
}

/// The form of this frame a model is drawn at: an XS form weapon by its bare model name (`xsi:<model>` mesh), a keyblade
/// part by its mesh name; `inst` = a routed fragment's own entry.
pub fn form_for<'a>(mesh: &str, inst: Option<usize>, forms: &'a [FormWeapon]) -> Option<&'a FormWeapon> {
    inst.and_then(|i| forms.get(i)).or_else(|| forms.iter().find(|f| mesh.strip_prefix(crate::xsiweapon::PREFIX) == Some(f.model.as_str()) || (mesh.starts_with(PREFIX) && f.model == mesh)))
}

/// The mesh names whose `OpacityFade` follows his weapon's this frame (the hand's keyblade).
pub fn hand_mesh(hand: &Hand) -> Option<&str> {
    match hand {
        Hand::KingdomKey => Some("kingdom_key"),
        Hand::Model(m) => Some(m.as_str()),
        Hand::Loading => None,
    }
}

/// One line for the renderer's status.
pub fn status() -> String {
    let hand = hand();
    with_state(|st| format!("(P5) keyblade {} ({hand:?}), loaded {:?}{}", st.current, st.loaded, if st.wanted != st.loaded { format!(", wanted {:?}", st.wanted) } else { String::new() }))
}

// ---------------------------------------------------------------------------------------------------- form weapons

/// What a form's weapons are drawn from this frame (see [`draw_form_weapon_parts`]).
pub struct FormPose<'a> {
    /// Sora's bone index by name, his component-space pose (the drawn one) and where his component stands (GAME space)
    pub index_of: &'a dyn Fn(&str) -> Option<usize>,
    pub component: &'a [Trs],
    pub to_world: &'a Trs,
    /// the logical name of the clip Sora plays (`M_7419`, `Attack1` ...; the weapon plays its row of that name, else its
    /// reference pose) and its time (s)
    pub anim: Option<&'a str>,
    pub time: f32,
    /// the socket of a part whose data names none (R_wep_skt on R_buki: the host's `weapon_socket`)
    pub hand_socket: &'a Trs,
}

/// (model, logical clip name) -> its rig (None: not readable).
type RigCache = HashMap<(String, String), Option<Arc<WeaponRig>>>;

static RIGS: Mutex<Option<RigCache>> = Mutex::new(None);

/// A model's rig for a clip of that logical name (cached; None without the skeleton or the clip).
fn rig_of(model: &KbModel, logical: &str) -> Option<Arc<WeaponRig>> {
    // (F-NANO) ... or by its sequence (the file's stem: the core plays a form set's clip as the row of its sequence, 8051)
    let (_, file) = model.clips.iter().find(|(name, _)| name == logical || name.strip_prefix("M_") == Some(logical) || logical.strip_prefix("M_") == Some(name.as_str())).or_else(|| model.clips.iter().find(|(_, file)| file.file_stem().is_some_and(|s| s == logical)))?;
    let mut g = RIGS.lock().unwrap_or_else(|e| e.into_inner());
    g.get_or_insert_with(HashMap::new)
        .entry((model.model.clone(), logical.to_string()))
        .or_insert_with(|| {
            let skeleton = (**model.skeleton.as_ref()?).clone();
            let name = file.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            match kh_anim::Clip::load(file, &name) {
                Ok(clip) => Some(Arc::new(WeaponRig::new(skeleton, clip))),
                Err(e) => {
                    crate::rlog!("render: (P5) {} clip {logical} not read ({e}): its reference pose", model.model);
                    None
                }
            }
        })
        .clone()
}

/// A model's component-space pose for Sora's clip (its own row of that name at `time`, else its reference pose).
/// (DRILL-IDLE) ... else its anim blueprint's DEFAULT clip (KH3's UTresAnimInstance 0x14056ECE0: `ResetPose`, looped when
/// `m_bDefaultAnimAlwaysLoopPlay`, on its own clock: [`DefaultClock`]), and only without one its reference pose. Sora's
/// idle / walk / run / jump / guard rows have no row in most form weapons' sets, so this is what they show between moves.
pub fn model_pose(model: &KbModel, anim: Option<&str>, time: f32) -> Vec<Trs> {
    let own = anim.and_then(|a| rig_of(model, a));
    let clock = default_clock(model, anim.unwrap_or(""), time, own.is_some());
    match (own, clock) {
        (Some(rig), _) => rig.component(time),
        (None, Some((rig, t))) => rig.component(t),
        (None, None) => reference(model),
    }
}

/// (DRILL-IDLE) A model's pose under one clip of its own (a thrown part's throw clip), else its reference pose; it does not
/// move the model's default-clip clock (Sora's play does).
fn clip_pose(model: &KbModel, clip: &str, time: f32) -> Vec<Trs> {
    rig_of(model, clip).map_or_else(|| reference(model), |rig| rig.component(time))
}

/// (DRILL-IDLE) The default clips' clocks, by model.
static DEFAULT_CLOCKS: Mutex<Option<HashMap<String, DefaultClock>>> = Mutex::new(None);

/// (DRILL-IDLE) The default clip of `model` and its time while Sora plays `anim` at `time` (`own_row`: the weapon has its
/// own row of it); None when it plays its own row or has no default clip.
fn default_clock(model: &KbModel, anim: &str, time: f32, own_row: bool) -> Option<(Arc<WeaponRig>, f32)> {
    let rig = model.default_anim.as_deref().and_then(|d| rig_of(model, d))?;
    let clock = {
        let mut g = DEFAULT_CLOCKS.lock().unwrap_or_else(|e| e.into_inner());
        g.get_or_insert_with(HashMap::new).entry(model.model.clone()).or_default().advance(anim, time, own_row, model.default_play)?
    };
    let t = DefaultClock::sample_time(clock, rig.length(), model.default_play);
    Some((rig, t))
}

/// (KEYCHAIN-DUAL) A model's clips' Bonamik notify states: per clip its names (sequence, logical names) and its states (class,
/// begin s, end s, properties as JSON text).
type ClipNotes = Vec<(Vec<String>, Vec<(String, f32, f32, String)>)>;

/// (KEYCHAIN-DUAL) The clips' notes by model (`<model>/model.json`, read once).
static NOTES: Mutex<Option<HashMap<String, Arc<ClipNotes>>>> = Mutex::new(None);

/// (KEYCHAIN-DUAL) THE BONAMIK NOTIFY STATES OF A PART'S OWN CLIP active at `time` (KH3: the weapon mesh's anim instance runs
/// them on its own KBD component; Storm Flag's banner w_so093: F420's CharaWind, F418's Bonamik_Weight): its clip of Sora's
/// logical name `anim` (the row [`model_pose`] plays, `rig_of`'s rule), each (a key naming the state, its class, its
/// properties as JSON text), for the host's keychain of that part (`kh_anim::Keychain::own_notifies`). None for another clip.
pub fn bonamik_notifies(t: &Table, model: &str, anim: Option<&str>, time: f32) -> Vec<(u64, String, String)> {
    let (Some(anim), Some(m)) = (anim, t.model(model)) else { return Vec::new() };
    let notes = {
        let mut g = NOTES.lock().unwrap_or_else(|e| e.into_inner());
        g.get_or_insert_with(HashMap::new)
            .entry(model.to_string())
            .or_insert_with(|| {
                let json: Value = std::fs::read_to_string(m.dir.join("model.json")).ok().and_then(|s| serde_json::from_str(s.trim_start_matches('\u{feff}')).ok()).unwrap_or(Value::Null);
                Arc::new(
                    json["clips"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|c| {
                            let names = std::iter::once(&c["sequence"]).chain(c["logical_names"].as_array().into_iter().flatten()).filter_map(Value::as_str).map(str::to_string).collect();
                            let states = c["notifies"].as_array().into_iter().flatten().filter(|n| n["class"].as_str().is_some_and(|k| k.starts_with("TresAnimNotifyState_Bonamik_"))).map(|n| (n["class"].as_str().unwrap_or_default().to_string(), n["trigger_time"].as_f64().unwrap_or(0.0) as f32, n["end_trigger_time"].as_f64().unwrap_or(0.0) as f32, n["properties"].to_string())).collect();
                            (names, states)
                        })
                        .collect(),
                )
            })
            .clone()
    };
    let named = |name: &str| name == anim || name.strip_prefix("M_") == Some(anim) || anim.strip_prefix("M_") == Some(name);
    let mut out = Vec::new();
    for (names, states) in notes.iter().filter(|(names, _)| names.iter().any(|n| named(n))) {
        for (i, (class, begin, end, props)) in states.iter().enumerate() {
            if time >= *begin && time < *end {
                // FNV-1a of the model, the clip and the state's index: the same state keeps its key from step to step
                let key = format!("{model}/{}/{i}", names.first().map_or("", String::as_str)).bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3));
                out.push((key, class.clone(), props.clone()));
            }
        }
    }
    out
}

/// The parts of a form as drawn: its weapons on their sockets, then (`transform`) its transform units.
pub fn form_parts(keyblade: &str, form: &str, grow: &str, transform: bool) -> Vec<Part> {
    let Some(f) = table().and_then(|t| t.form(keyblade, form, grow)) else { return Vec::new() };
    f.weapons.iter().chain(f.transform.iter().filter(|_| transform)).cloned().collect()
}

/// THE HOOK FOR P3 / THE F-PACKAGES: the weapon parts of `keyblade`'s form `form` (grow level `grow`, "" = its first)
/// placed on Sora for this frame, as `FrameInput::forms` entries (mesh `kb:<model>`; the renderer draws them with their
/// own materials, skinned by their own skeletons). `transform` adds the form's transform units (the "Deform" model of a
/// form change). Parts whose model is not loaded (not the keyblade in his hand) are left out, and so are parts of a bone
/// Sora's skeleton lacks. The host hides his hand's keyblade (`FrameInput::weapon` opacity 0) while it draws these.
pub fn draw_form_weapon_parts(keyblade: &str, form: &str, grow: &str, transform: bool, pose: &FormPose<'_>) -> Vec<FormWeapon> {
    let Some(t) = table() else { return Vec::new() };
    let loaded = loaded();
    form_parts(keyblade, form, grow, transform)
        .iter()
        .filter(|p| loaded.contains(&p.model))
        .filter_map(|p| place_part(t, p, pose))
        .collect()
}

/// (AUDIT-B W-CORE) The units KH3's equipment holds for a form (UTresEquipmentComponent 0x141791120): with `transform` its
/// transform units ALONE when the form's set has any (m_TransformSetData.Num >= 1), else its own weapons; each equip replaces
/// the units before it. kh-core publishes which (`combat::form::FormEquip`).
pub fn equipped_parts(keyblade: &str, form: &str, grow: &str, transform: bool) -> Vec<Part> {
    let Some(f) = table().and_then(|t| t.form(keyblade, form, grow)) else { return Vec::new() };
    if transform && !f.transform.is_empty() { f.transform.clone() } else { f.weapons.clone() }
}

/// (AUDIT-B W-CORE) [`equipped_parts`] placed on Sora (as [`draw_form_weapon_parts`]).
pub fn draw_equipped_parts(keyblade: &str, form: &str, grow: &str, transform: bool, pose: &FormPose<'_>) -> Vec<FormWeapon> {
    let Some(t) = table() else { return Vec::new() };
    let loaded = loaded();
    equipped_parts(keyblade, form, grow, transform).iter().filter(|p| loaded.contains(&p.model)).filter_map(|p| place_part(t, p, pose)).collect()
}

/// (FORM-GRIP) THE MESH COMPONENT'S OWN TURN INSIDE ITS WEAPON ACTOR: KH3's weapon blueprint `TresWeaponMesh0`
/// RelativeRotation (pitch, yaw, roll), for the blueprints that set one (every other weapon blueprint of the forms sets none).
/// KH3 attaches the weapon ACTOR to Sora's socket and its mesh component turned by this inside it, so the mesh is
/// mesh turn, then socket, then bone. The two shield forms' weapons (Hero's Origin w_so032 / w_so033 / w_so034,
/// w_so032_wep .. w_so034_wep; Grand Chef's Frying Pan w_so142 / w_so143 / w_so144, w_so142_Wep .. w_so144_Wep) turn theirs
/// yaw 270, and their socket R_Wep3_skt is (pitch 90, roll -90): without the mesh turn the shield sat sideways and back on
/// his hand (the user 2026-11-09: "shield is held wrong"). With it, each shield is on KH3's own transform unit at the end of
/// its form change (w_so031 / w_so141's StyleChange11 / 19, authored in his mesh space: tests/formwep_grip.rs).
/// assets_converted/packages/KINGDOM HEARTS III/Content/Blueprints/Player/p_ex001/<model>_wep.uasset.json.
pub const WEAPON_MESH_TURNS: &[(&str, [f32; 3])] = &[
    ("w_so032", [0.0, 270.0, 0.0]),
    ("w_so033", [0.0, 270.0, 0.0]),
    ("w_so034", [0.0, 270.0, 0.0]),
    ("w_so142", [0.0, 270.0, 0.0]),
    ("w_so143", [0.0, 270.0, 0.0]),
    ("w_so144", [0.0, 270.0, 0.0]),
];

/// (FORM-GRIP) The mesh turn of a weapon part's model inside its actor (identity for every model without one).
pub fn weapon_mesh_turn(model: &str) -> Trs {
    let model = model.strip_prefix(PREFIX).unwrap_or(model);
    WEAPON_MESH_TURNS.iter().find(|(m, _)| *m == model).map_or(Trs::IDENTITY, |(_, [pitch, yaw, roll])| Trs { translation: Vec3::ZERO, rotation: kh_anim::blend::Rotator { pitch: *pitch, yaw: *yaw, roll: *roll }.quat(), scale: Vec3::ONE })
}

/// One part on Sora (see [`draw_form_weapon_parts`]).
pub fn place_part(t: &Table, p: &Part, pose: &FormPose<'_>) -> Option<FormWeapon> {
    let model = t.model(&p.model)?;
    let bone_name = p.bone.as_deref().unwrap_or("R_buki");
    let bone = pose.component.get((pose.index_of)(bone_name)?)?;
    // (F-NANO) A part whose own clips are authored in Sora's MESH space (Nano Arms' w_so172: its R_wep follows his R_buki
    // in mesh space frame by frame, tools/sora_form_nano.js) stands at his mesh origin while it plays one of them.
    // (AUDIT-B W-HANDOVER) ... only the unit the equipment attaches with NO socket (the transform unit `None@RIGHT_HAND:None`:
    // UE's AttachToComponent with NAME_None is the component itself). The weapon unit is on R_wep_skt (0x14177E4E0); its
    // clips' TresAnimNotifyState_ChangeAttachParent windows put it on his kh_root (= his mesh origin) and the hosts apply
    // them (kh-core placement::part_parent / clip_parent, `part_on_bone`). After a window that closes before its clip ends
    // (8415 at 1.1667 of 1.7333 s, 8483 at 0.6666 of 0.7 s) KH3 is back on that socket (0x141785E90), so this is the socket.
    // (FORMWEP-FIX, the user 2026-11-07: "keyblade transformations look broken like this when the keyblade transforms into
    // another weapon", then "only broken during their transformation cinematic") EVERY such transform unit, not only Nano
    // Arms': w_so051's StyleChange3 (3401) ends with its bone w_so052 EXACTLY on the Hyper Hammer's R_wep4_skt on R_buki4 when
    // it stands at his mesh origin (0.0 cm; on his hand it was 1.7 m off: the hammer assembled beside him, the user's
    // screenshot), and so do the other units' StyleChange clips with their forms' weapons (tests/formwep_grip.rs). Hung on his
    // hand it carried his hand's whole transform on top of a pose that already holds it.
    if p.on_mesh && pose.anim.and_then(|a| rig_of(model, a)).is_some() {
        return Some(FormWeapon { model: model.mesh.clone(), to_world: weapon_mesh_turn(&p.model).then(pose.to_world), component: model_pose(model, pose.anim, pose.time) });
    }
    let socket = p.socket.unwrap_or(*pose.hand_socket);
    // (FORM-GRIP) its mesh component's turn inside the actor, then the socket, the bone, his component
    Some(FormWeapon { model: model.mesh.clone(), to_world: weapon_mesh_turn(&p.model).then(&socket).then(bone).then(pose.to_world), component: model_pose(model, pose.anim, pose.time) })
}

/// A finisher / accompany pawn of a form (`model`, "" = the form's first) standing at `ground` (GAME space, cm) turned
/// `yaw` (the core's convention: its mesh component -90 as a character's), its own clip of Sora's logical name at `time`.
pub fn form_pawn(keyblade: &str, form: &str, model: &str, ground: Vec3, yaw: f32, anim: Option<&str>, time: f32) -> Option<FormWeapon> {
    let t = table()?;
    let f = t.form(keyblade, form, "")?;
    // (AUDIT-A SHIELD) or a pawn the form's pawn spawns in turn (Pegasus's Chariot w_so035: the manifest lists it among the
    // form's projectile-set models, not its accompany pawns), when the table has it
    let name = if model.is_empty() { f.pawns.first().map(|p| p.1.as_str())? } else { f.pawns.iter().find(|p| p.1 == model).map(|p| p.1.as_str()).or_else(|| t.model(model).map(|_| model))? };
    if !loaded().iter().any(|m| m == name) {
        return None;
    }
    let m = t.model(name)?;
    let to_world = Trs { translation: ground, rotation: Quat::from_rotation_z((yaw - 90.0).to_radians()), scale: Vec3::ONE };
    Some(FormWeapon { model: m.mesh.clone(), to_world, component: model_pose(m, anim, time) })
}

/// (AUDIT-A PLACEMENT) THE PER-PART SOCKET OVERRIDE: the drawn parts of `model` (`kb:<model>` entries of `forms`) hung on
/// another bone of Sora's instead of their own socket (KH3's TresAnimNotifyState_ChangeAttachParent of the weapon's own
/// clip: kh-core `combat::forms::placement::part_parent`), `bone` being that parent's component-space transform (a bone,
/// or a socket on one: kh_root is kh_sk x its socket turn) and
/// `to_world` his component's; their own clip's pose is kept. Returns how many were moved.
pub fn part_on_bone(forms: &mut [FormWeapon], model: &str, bone: &Trs, to_world: &Trs) -> usize {
    let mesh = format!("{PREFIX}{model}");
    let mut n = 0;
    // (FORM-GRIP) the actor goes on that bone: its mesh turned inside it as on its socket
    let turn = weapon_mesh_turn(model);
    for f in forms.iter_mut().filter(|f| f.model == mesh) {
        f.to_world = turn.then(bone).then(to_world);
        n += 1;
    }
    n
}

/// (AUDIT-A PLACEMENT) A THROWN PART (KH3's weapon state 5 FreeMove after TresAnimNotify_ThrowWeapon: kh-core
/// `combat::forms::placement::ThrownWeapon`): the drawn parts of `model` detached from Sora, at `to_world` (GAME space: the
/// weapon actor's place and turn), posed by the weapon's own throw clip (`clip`, its logical name) at `time`. Returns how many
/// were moved.
pub fn part_thrown(forms: &mut [FormWeapon], model: &str, to_world: Trs, clip: &str, time: f32) -> usize {
    let mesh = format!("{PREFIX}{model}");
    let pose = table().and_then(|t| t.model(model)).map(|m| clip_pose(m, clip, time)); // (DRILL-IDLE) its own clip, not Sora's play
    let mut n = 0;
    // (FORM-GRIP) the actor's place and turn: its mesh turned inside it
    let turn = weapon_mesh_turn(model);
    for f in forms.iter_mut().filter(|f| f.model == mesh) {
        f.to_world = turn.then(&to_world);
        if let Some(p) = &pose {
            f.component.clone_from(p);
        }
        n += 1;
    }
    n
}

/// (VFX-XLATE) A loaded model of the table (a projectile's own mesh: the Rocket Fist w_so173) standing at `ground` (GAME
/// space, cm) turned `yaw`, in its reference pose: the effects harness's picture of it with its translated materials.
pub fn model_at(name: &str, ground: Vec3, yaw: f32) -> Option<FormWeapon> {
    let t = table()?;
    if !loaded().iter().any(|m| m == name) {
        return None;
    }
    let m = t.model(name)?;
    let to_world = Trs { translation: ground, rotation: Quat::from_rotation_z((yaw - 90.0).to_radians()), scale: Vec3::ONE };
    Some(FormWeapon { model: m.mesh.clone(), to_world, component: model_pose(m, None, 0.0) })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack() -> Option<PathBuf> {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack");
        p.join("keyblades").join("manifest.json").is_file().then_some(p)
    }

    /// the manifest: 17 item keyblades and the four without an item row, every form part's model in the table, the
    /// wanted set of an equipment (KEYBLADE-SWAP: every slotted keyblade whole, never the Kingdom Key; a D-pad switch keeps it)
    #[test]
    fn the_table_names_every_keyblade_and_its_parts() {
        let Some(pack) = pack() else { return };
        let t = Table::read(&pack).unwrap();
        assert!(t.keyblades.iter().filter(|k| !k.keys.is_empty()).count() >= 17);
        for k in &t.keyblades {
            for f in &k.forms {
                for p in f.weapons.iter().chain(&f.transform) {
                    assert!(t.model(&p.model).is_some(), "{} {}: {}", k.id, f.form, p.model);
                }
            }
        }
        assert_eq!(t.keyblade("WEP_KEYBLADE_SO_06").map(|k| k.id.as_str()), Some("w_so080"));
        let wanted = t.wanted_models("WEP_KEYBLADE_SO_06", &["w_so010", "WEP_KEYBLADE_SO_12"]);
        assert!(wanted.contains(&"w_so080".to_string()) && wanted.contains(&"w_so084".to_string()) && wanted.contains(&"w_so200".to_string()));
        // (KEYBLADE-SWAP) the slotted Oathkeeper's Double Form part (Oblivion) is loaded too: switching to it loads nothing
        assert!(!wanted.contains(&"w_so010".to_string()) && wanted.contains(&"w_so210".to_string()), "{wanted:?}");
        // a D-pad switch (another of the same slots in his hand) wants the same models
        assert_eq!(wanted, t.wanted_models("WEP_KEYBLADE_SO_12", &["WEP_KEYBLADE_SO_06", "w_so010"]));
        // Crystal Snow's form 07: four Blizzard Blades on the forearms and shins, each with its socket's own turn
        let parts = form_parts_with(&t, "w_so080", "07");
        assert_eq!(parts.iter().map(|p| format!("{}@{}", p.model, p.bone.clone().unwrap_or_default())).collect::<Vec<_>>(), ["w_so084@R_ude2_buki2", "w_so085@L_ude2_buki2", "w_so086@R_ashi2", "w_so087@L_ashi2"]);
        // Double Form: Oathkeeper right, Oblivion left
        let double = t.form("w_so200", "TRES_WEAPON_FORM_18", "").unwrap();
        assert_eq!(double.weapons.iter().map(|p| p.model.as_str()).collect::<Vec<_>>(), ["w_so200", "w_so210"]);
    }

    /// (DRILL-IDLE) THE USER'S REPORT (2026-11-05, Drill Punch idle: "floating and detatched"): while Sora plays a clip the
    /// drill w_so053 has no row of (his idle), it shows its anim blueprint's ResetPose looped (KH3 0x14056ECE0), not its bind
    /// pose: the rope folded (the drill head on the gauntlet, not 1.5 m out) and the head spinning; a clip of its own still
    /// wins. Every form weapon with a default clip gets the same. Skipped without the pack.
    #[test]
    fn a_form_weapon_without_a_row_plays_its_default_clip() {
        let Some(pack) = pack() else { return };
        let t = Table::read(&pack).unwrap();
        let Some(drill) = t.model("w_so053") else { return };
        if !t.dir.join(ANIM_INSTANCES).is_file() {
            eprintln!("SKIP: no {ANIM_INSTANCES}");
            return;
        }
        assert_eq!((drill.default_anim.as_deref(), drill.default_play), (Some("ResetPose"), DefaultPlay { looping: true, check_same: true }));
        let s = drill.skeleton.as_ref().unwrap();
        let (base, head) = (s.index_of("dorill_base").unwrap(), s.index_of("dorill_all").unwrap());
        let gap = |p: &[Trs]| p[head].translation.distance(p[base].translation);
        let bind = reference(drill);
        let idle = model_pose(drill, Some("DRILL-IDLE-test-6001"), 0.10);
        assert!(gap(&idle) < 0.05 * gap(&bind), "the rope is folded: {} vs the bind pose's {}", gap(&idle), gap(&bind));
        // the head spins on the default's own clock (one turn per 0.567 s)
        let later = model_pose(drill, Some("DRILL-IDLE-test-6001"), 0.24);
        assert!(idle[head].rotation.dot(later[head].rotation).abs() < 0.9, "the drill turns");
        assert!(gap(&later) < 0.05 * gap(&bind));
        // a clip of its own: its own row (M_6421 throws the warhead out on the rope)
        let own = model_pose(drill, Some("6421"), 0.9);
        let row = clip_pose(drill, "6421", 0.9);
        assert!(own.iter().zip(&row).all(|(a, b)| a.translation.distance(b.translation) < 1.0e-4));
        // every model with a default clip finds it
        for m in t.models.iter().filter(|m| m.default_anim.is_some() && m.skeleton.is_some()) {
            if m.clips.iter().any(|(n, _)| Some(n.as_str()) == m.default_anim.as_deref()) {
                assert!(rig_of(m, m.default_anim.as_deref().unwrap()).is_some(), "{}", m.model);
            }
        }
    }

    fn form_parts_with(t: &Table, k: &str, f: &str) -> Vec<Part> {
        t.form(k, f, "").map(|f| f.weapons.clone()).unwrap_or_default()
    }

    /// the drawn role of each model: the hand's keyblade at the weapon, a form's part at its form entry, the rest not
    #[test]
    fn each_model_takes_its_role() {
        let form = |m: &str| FormWeapon { model: m.to_string(), to_world: Trs::IDENTITY, component: Vec::new() };
        let kb = Hand::Model("kb:w_so080".into());
        assert_eq!(role("kingdom_key", &kb, &[]), Role::Skip);
        assert_eq!(role("kingdom_key", &Hand::KingdomKey, &[]), Role::Hand);
        assert_eq!(role("kb:w_so080", &kb, &[]), Role::Hand);
        assert_eq!(role("kb:w_so084", &kb, &[]), Role::Skip);
        assert_eq!(role("kb:w_so084", &kb, &[form("kb:w_so084")]), Role::Form);
        assert_eq!(role("kb:w_so080", &Hand::Loading, &[]), Role::Skip);
        assert_eq!(role("kingdom_key", &Hand::Loading, &[]), Role::Skip);
        assert_eq!(role("xsi:w_so052", &Hand::KingdomKey, &[]), Role::Form);
        // an XS form weapon is found by its bare model name, a keyblade part by its mesh
        let forms = [form("w_so052"), form("kb:w_so084")];
        assert_eq!(form_for("xsi:w_so052", None, &forms).map(|f| f.model.as_str()), Some("w_so052"));
        assert_eq!(form_for("kb:w_so084", None, &forms).map(|f| f.model.as_str()), Some("kb:w_so084"));
        assert!(form_for("kb:w_so085", None, &forms).is_none());
    }

    /// the `kb:` library takes only the wanted models' slots; each slot has every set of shaders in materials/er when the
    /// folder is built (skipped otherwise)
    #[test]
    fn the_library_takes_only_the_wanted_models() {
        let Some(pack) = pack() else { return };
        let t = Table::read(&pack).unwrap();
        let mut lib = Library::load(&pack).unwrap();
        let before = lib.slots.len();
        let n = add_slots_with(&t, &mut lib, &["w_so080".to_string(), "w_so084".to_string()]);
        assert!(n >= 2 && lib.slots.len() == before + n);
        assert!(lib.slots[before..].iter().all(|s| (s.mesh == "kb:w_so080" || s.mesh == "kb:w_so084") && s.owner.starts_with(PREFIX) && s.owner_id >= 500));
        let er = t.dir.join("materials").join("er");
        if !er.join(format!("{}_hy3.ps.dxil", lib.slots[before].variant())).is_file() {
            return;
        }
        for s in &lib.slots[before..] {
            for set in ["", "_pl", "_bl", "_bli", "_hy3", "_gb3"] {
                assert!(er.join(format!("{}{set}.ps.dxil", s.variant())).is_file(), "{} {set}", s.variant());
            }
        }
        // (KEYBLADE-SWAP) every slotted keyblade is loaded whole now, so any model can be in the library with any other: each
        // slot of every model has every set (one without would switch his whole library's set: gfx.rs read_shaders)
        let mut all = Library::load(&pack).unwrap();
        let start = all.slots.len();
        let models: Vec<String> = t.models.iter().filter(|m| m.glb.is_file()).map(|m| m.model.clone()).collect();
        add_slots_with(&t, &mut all, &models);
        assert!(all.slots.len() > start);
        for s in &all.slots[start..] {
            for set in ["", "_pl", "_bl", "_bli", "_hy3", "_gb3"] {
                assert!(er.join(format!("{}{set}.ps.dxil", s.variant())).is_file(), "{} {} {set}", s.mesh, s.variant());
            }
        }
    }
}
