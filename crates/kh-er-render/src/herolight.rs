//! (HEROLIGHT-KB, 2026-11-05) THE HERO LIGHT ADAPTS TO THE WEAPON THAT CARRIES IT. The user: "the light effect for the hero
//! light in dark places needs to adapt to all the keyblades", and "for the hero light, it should just be whoever is party
//! leader, not dependent on Sora". State: build/sora_tmp/weapons/HEROLIGHT-KB/STATE.md.
//!
//! THE TABLE is `sheets/port/sora-herolight.json` (tools/sora_herolight_sheet.js, traced from KH3's data: every weapon
//! blueprint's `m_AppearEffect`, that effect's emitters, the weapon's own skeleton sockets): one row per weapon model a
//! carrier can hold (Sora's 17 Keyblades with every form / grow-level weapon of theirs, Donald's 14 staffs, Goofy's 14
//! shields). Each row gives
//! - the steady GLOW: a copy of that weapon's appear effect with only its glow emitters, its clock held (`glow_name`);
//! - the SPARKLES: a copy with only its spawning emitters, restarted at `hero.sparkle_s` (`spark_name`);
//! - the character SOCKET both are started at (as KH3's Draw starts the appear effect);
//! - the LINE the light sits on (the weapon's vfx_bottom -> vfx_top, else ATK_00 -> ATK_01), at the Kingdom Key's fraction
//!   of its own line (`hero.blade_cm` on 14.2 .. 79 cm): the same place relative to every blade;
//! - the TINT of the light: the weapon's glow colour relative to the Kingdom Key's (1 for it: its look is unchanged).
//!
//! THE CARRIER (the party leader, whoever: kh-er herolight.rs) is told here once a step ([`set_carrier`]): the light pass takes
//! its tint and Sora's blade point follows its line (gfx.rs). [`Driver`] is the host side every host shares (kh-er, the effects
//! harness): when to start / stop the glow and the sparkles, and a carrier or weapon change (a D-pad switch, a form, a leader
//! swap) ends the old ones AT ONCE before the new ones start (no doubled or stuck effects).

use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use glam::Vec3;
use serde_json::Value;

use crate::pointlight::{KeybladeGlow, LightSettings};

/// The table as built into the renderer (the sheet is the data; the pack holds the effects it names).
const SHEET: &str = include_str!("../../../sheets/port/sora-herolight.json");

/// The Kingdom Key's model: the reference row (its tint is 1, its line the one `hero.blade_cm` is measured on).
pub const KINGDOM_KEY: &str = "w_so010";

/// One weapon of the table.
#[derive(Clone, Debug, PartialEq)]
pub struct HeroWeapon {
    /// The weapon model (`w_so080`, `w_so033`, `w_do010`, `w_gu140` ...).
    pub model: String,
    /// `sora`, `donald` or `goofy`.
    pub owner: String,
    /// The item keys of the weapons that use this model (Sora: the Keyblades whose base or form weapon it is).
    pub keys: Vec<String>,
    /// The pack's file of the weapon's appear effect (the effect host's name for it) and the root it is in.
    pub effect: String,
    pub root: String,
    /// The steady glow's emitters and its held clock (seconds); the sparkles' emitters.
    pub glow: Vec<String>,
    pub hold: f32,
    pub spark: Vec<String>,
    /// The names the host spawns: `<effect>#hl_glow`, `<effect>#hl_spark`.
    pub glow_name: String,
    pub spark_name: String,
    /// The character socket the weapon hangs on (`R_wep_skt`, a form unit's `R_wep1_skt`, Goofy's `R_wep`).
    pub socket: String,
    /// The weapon's bone the line is on and its two ends (bone-local cm, UE), with the two sockets' names.
    pub line_bone: String,
    pub line: Option<(Vec3, Vec3)>,
    pub line_sockets: Option<(String, String)>,
    /// The light's colour relative to the Kingdom Key's (luma 1).
    pub tint: Vec3,
}

fn vec3(v: &Value) -> Option<Vec3> {
    let a = v.as_array()?;
    (a.len() >= 3).then(|| Vec3::new(a[0].as_f64().unwrap_or(0.0) as f32, a[1].as_f64().unwrap_or(0.0) as f32, a[2].as_f64().unwrap_or(0.0) as f32))
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default()
}

/// The table read from a sheet's text (rows by its columns); rows that do not parse are left out.
pub fn parse(text: &str) -> Vec<HeroWeapon> {
    let Ok(json) = serde_json::from_str::<Value>(text) else { return Vec::new() };
    let columns: Vec<&str> = json["columns"].as_array().map(|c| c.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    let at = |row: &Value, name: &str| columns.iter().position(|c| *c == name).map_or(Value::Null, |i| row[i].clone());
    let mut out = Vec::new();
    for row in json["rows"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
        let text = |name: &str| at(row, name).as_str().unwrap_or("").to_string();
        let (model, effect) = (text("model"), text("effect"));
        if model.is_empty() || effect.is_empty() {
            continue;
        }
        let sockets = text("line_sockets");
        let line = vec3(&at(row, "line_from")).zip(vec3(&at(row, "line_to")));
        out.push(HeroWeapon {
            glow_name: format!("{effect}#hl_glow"),
            spark_name: format!("{effect}#hl_spark"),
            owner: text("owner"),
            keys: strings(&at(row, "keyblades")),
            root: text("root"),
            glow: strings(&at(row, "glow")),
            hold: at(row, "hold").as_f64().unwrap_or(0.0) as f32,
            spark: strings(&at(row, "spark")),
            socket: text("socket"),
            line_bone: text("line_bone"),
            line,
            line_sockets: sockets.split_once('>').map(|(a, b)| (a.to_string(), b.to_string())),
            tint: vec3(&at(row, "tint")).filter(|t| t.is_finite() && t.min_element() >= 0.0).unwrap_or(Vec3::ONE),
            model,
            effect,
        });
    }
    out
}

/// The built-in table.
pub fn table() -> &'static [HeroWeapon] {
    static TABLE: OnceLock<Vec<HeroWeapon>> = OnceLock::new();
    TABLE.get_or_init(|| parse(SHEET))
}

/// The row of a weapon model (any case; a `kb:` mesh name is taken too).
pub fn weapon(model: &str) -> Option<&'static HeroWeapon> {
    let model = model.strip_prefix(crate::keyblade::PREFIX).unwrap_or(model);
    table().iter().find(|w| w.model.eq_ignore_ascii_case(model))
}

/// The Kingdom Key's row.
pub fn kingdom_key() -> Option<&'static HeroWeapon> {
    weapon(KINGDOM_KEY)
}

impl HeroWeapon {
    /// The two copies of its appear effect a host defines (`EffectHost::define_variant`): (name, base, kept emitters, held
    /// clock). An empty list is no copy (that part of the effect is not started).
    pub fn variants(&self) -> Vec<(&str, &str, Vec<String>, Option<f32>)> {
        let mut out = Vec::new();
        if !self.glow.is_empty() {
            // (HEROLIGHT-KB final pass) with the emitters that GENERATE its copies (sora-herolight-copies `parents`: Shooting Star's
            // Key0 is made by "+*"; without its parent the copy was never made: no glow at all on 11 weapons)
            let mut keep = self.glow.clone();
            for p in copies().iter().filter(|c| c.model == self.model && self.glow.contains(&c.emitter)).flat_map(|c| c.parents.iter()) {
                if !keep.contains(p) {
                    keep.push(p.clone());
                }
            }
            // (HEROLIGHT-KB final pass) held no earlier than its copies show (sora-herolight-copies `hold_min`: their emitter's own
            // timeline start; Grand Chef's / Ultima Weapon's copies start at 0.2 s and were held at 0.15 s: never made)
            let hold = copies().iter().filter(|c| c.model == self.model && self.glow.contains(&c.emitter)).map(|c| c.hold_min).fold(self.hold, f32::max);
            out.push((self.glow_name.as_str(), self.effect.as_str(), keep, Some(hold.max(0.0))));
        }
        if !self.spark.is_empty() {
            out.push((self.spark_name.as_str(), self.effect.as_str(), self.spark.clone(), None));
        }
        out
    }

    /// The light's place on its line, bone-local (cm): at fraction `f` from the first socket to the second (no line: the bone).
    pub fn line_point(&self, f: f32) -> Vec3 {
        self.line.map_or(Vec3::ZERO, |(a, b)| a.lerp(b, f))
    }
}

/// Where the Kingdom Key's light sits on its own line: `hero.blade_cm` (55) between its vfx_bottom (14.2) and vfx_top (79).
pub fn blade_fraction(s: &LightSettings) -> f32 {
    match kingdom_key().and_then(|k| k.line) {
        Some((a, b)) if (b.x - a.x).abs() > 1.0 => ((s.hero_blade - a.x) / (b.x - a.x)).clamp(0.0, 1.0),
        _ => 0.63,
    }
}

/// The distance along +X of the weapon's bone (cm) of the light on a Keyblade in his hand (`shadowcaps::blade_point`): the
/// carrier's line at the Kingdom Key's fraction; the Kingdom Key (and a weapon without a line): `hero.blade_cm` as before.
pub fn hand_along(row: Option<&HeroWeapon>, s: &LightSettings) -> f32 {
    match row {
        Some(w) if w.model != KINGDOM_KEY && w.line.is_some() => w.line_point(blade_fraction(s)).x,
        _ => s.hero_blade,
    }
}

/// SORA'S BLADE POINT of the frame (gfx.rs, the light pass's `blade`): game space and the weapon's opacity. The carrier's weapon
/// is a FORM weapon of this frame (`kb:<model>` in `forms`): the point on its own line, on its bone (its pose, or its
/// skeleton's reference pose when it is drawn rigid), fully drawn; else the hand's Keyblade (`weapon`, as IMPL_ER44: along
/// +X of its bone `wep`, at the carrier's line or `hero.blade_cm`). None: nothing drawn.
pub fn sora_blade(weapon: Option<&crate::Weapon>, forms: &[crate::FormWeapon], s: &LightSettings) -> Option<(Vec3, f32)> {
    let row = carrier();
    if let Some(w) = row.filter(|w| w.line.is_some()) {
        let mesh = format!("{}{}", crate::keyblade::PREFIX, w.model);
        if let Some(form) = forms.iter().find(|f| f.model == mesh) {
            let model = crate::keyblade::table().and_then(|t| t.model(&w.model));
            let bone = model.and_then(|m| m.skeleton.as_ref()).and_then(|k| k.index_of(&w.line_bone));
            let reference = model.map(crate::keyblade::reference).unwrap_or_default();
            let at = bone.and_then(|b| form.component.get(b).or(reference.get(b))).copied().filter(|t| t.translation.is_finite() && t.rotation.is_finite());
            if let Some(at) = at {
                let p = at.then(&form.to_world).transform_point(w.line_point(blade_fraction(s)));
                return p.is_finite().then_some((p, 1.0));
            }
        }
    }
    weapon.filter(|w| w.opacity > 0.0).and_then(|w| crate::shadowcaps::blade_point(w, hand_along(row, s)).map(|p| (p, w.opacity)))
}

/// The hand weapon's MODEL frame in game space (cm): where its mesh's model space (the bind pose, as the pack's glb and the
/// effects' copies of it are authored) is drawn. The mesh is skinned to its bone `wep` (index 1): a vertex goes
/// `to_world x pose(wep) x bind(wep)^-1`; `bind` = the model's own reference of `wep` (None: the half turn about X every
/// Keyblade's skeleton has, `shadowcaps`), the pose = `component[1]` (none: the reference, i.e. the component itself).
pub fn hand_frame(weapon: &crate::Weapon, bind: Option<kh_anim::Trs>) -> kh_anim::Trs {
    model_frame(&weapon.to_world, weapon.component.get(1).copied(), bind)
}

/// A skinned weapon's model frame: `component x pose(wep) x bind(wep)^-1` (no pose: the bind, i.e. the component itself; no
/// bind: the half turn about X every Keyblade skeleton has).
pub fn model_frame(component: &kh_anim::Trs, pose: Option<kh_anim::Trs>, bind: Option<kh_anim::Trs>) -> kh_anim::Trs {
    let bind = bind.unwrap_or(kh_anim::Trs { rotation: glam::Quat::from_xyzw(1.0, 0.0, 0.0, 0.0), ..kh_anim::Trs::IDENTITY });
    let pose = pose.filter(|t| t.translation.is_finite() && t.rotation.is_finite()).unwrap_or(bind);
    kh_anim::Trs::IDENTITY.relative_to(&bind).then(&pose).then(component)
}

/// The reference (bind) transform of a keyblade model's bone `wep` (component space), from the pack's skeleton.
pub fn wep_bind(model: &str) -> Option<kh_anim::Trs> {
    let m = crate::keyblade::table()?.model(model)?;
    let index = m.skeleton.as_ref()?.index_of("wep")?;
    crate::keyblade::reference(m).get(index).copied()
}

// ---------------------------------------------------------------------------------------------------- the copies, locked
//
// (HEROLIGHT-KB final pass, the user: "do a final pass on every keyblade's hero light, all of them have graphical errors") The
// glow's weapon copy was left where the particle system put it: on the character socket's frame (2.5 cm off the drawn Kingdom
// Key, whose bone `wep` the keychain pose moves), with the emitter's own local-space offsets (Oblivion 10 cm), at the size of
// its appear animation at the held moment (Ever After's 42 cm short at the tip), turned by an authored roll its glb does not
// need (Wheel of Fate). Now every copy of `sheets/port/sora-herolight-copies.json` (tools/sora_herolight_copies.js: its mesh,
// its turn, its fit against the weapon's own extents within 1 cm) is DRAWN ON THE WEAPON AS DRAWN THAT FRAME while the hero
// light is on: its model frame (component x pose(wep) x bind(wep)^-1) x the copy's turn; a skeletal copy at the weapon's
// component. Whatever the particle system made of the copy's place, it sits exactly on the blade.

/// The copies' table as built into the renderer.
const COPIES: &str = include_str!("../../../sheets/port/sora-herolight-copies.json");
/// (HEROLIGHT-KB, DEPLOY4 flicker) KH3's own depth pull of each copy (tools/sora_herolight_depth.js).
const DEPTH: &str = include_str!("../../../sheets/port/sora-herolight-depth.json");

/// One weapon copy of a glow.
#[derive(Clone, Debug, PartialEq)]
pub struct WeaponCopy {
    /// The weapon it copies (its row's model).
    pub model: String,
    /// The pack's mesh name (lower case, the glb's stem; a skeletal copy: the skeletal mesh's base name `w_so032_0`).
    pub mesh: String,
    pub skeletal: bool,
    /// Mesh space -> the weapon's model space.
    pub turn: glam::Quat,
    /// The glow emitter that is the copy, and the emitters that generate it (kept in the glow copy with it).
    pub emitter: String,
    pub parents: Vec<String>,
    /// The earliest held moment that shows it (seconds).
    pub hold_min: f32,
    /// (DEPLOY4, the user: "the light overlay on the keyblade flickers with the actual keyblade model") cm the copy is drawn
    /// TOWARD THE EYE, its screen picture unchanged: KH3's own `SQEX_DepthOffset` of a mesh copy (2.5 / 5 / 10), the
    /// `Camera_Offset` material scalar of a skeletal copy (sheets/port/sora-herolight-depth.json). Without it the copy,
    /// exactly on the weapon, z-fought it.
    pub depth: f32,
}

/// UE's FRotator (roll about X, pitch about Y, yaw about Z; applied roll, pitch, yaw), degrees, as a rotation.
pub fn rotator(roll: f32, pitch: f32, yaw: f32) -> glam::Quat {
    glam::Quat::from_rotation_z(yaw.to_radians()) * glam::Quat::from_rotation_y(pitch.to_radians()) * glam::Quat::from_rotation_x(roll.to_radians())
}

/// The copies' table from a sheet's text.
pub fn parse_copies(text: &str) -> Vec<WeaponCopy> {
    let Ok(json) = serde_json::from_str::<Value>(text) else { return Vec::new() };
    let columns: Vec<&str> = json["columns"].as_array().map(|c| c.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    let at = |row: &Value, name: &str| columns.iter().position(|c| *c == name).map_or(Value::Null, |i| row[i].clone());
    json["rows"].as_array().map(Vec::as_slice).unwrap_or(&[]).iter().filter_map(|row| {
        let turn = vec3(&at(row, "turn")).unwrap_or(Vec3::ZERO);
        Some(WeaponCopy { model: at(row, "model").as_str()?.to_string(), mesh: at(row, "mesh").as_str()?.to_ascii_lowercase(), skeletal: at(row, "kind").as_str() == Some("skel"), turn: rotator(turn.x, turn.y, turn.z), emitter: at(row, "emitter").as_str()?.to_string(), parents: strings(&at(row, "parents")), hold_min: at(row, "hold_min").as_f64().unwrap_or(0.0) as f32, depth: 0.0 })
    }).collect()
}

pub fn copies() -> &'static [WeaponCopy] {
    static TABLE: OnceLock<Vec<WeaponCopy>> = OnceLock::new();
    TABLE.get_or_init(|| with_depth(parse_copies(COPIES), DEPTH))
}

/// The copies with their depth pull from the depth sheet's text (rows `model`, `emitter`, `offset_cm`; none: KH3's 2.5 cm).
pub fn with_depth(mut copies: Vec<WeaponCopy>, text: &str) -> Vec<WeaponCopy> {
    let json = serde_json::from_str::<Value>(text).unwrap_or_default();
    let columns: Vec<&str> = json["columns"].as_array().map(|c| c.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    let at = |row: &Value, name: &str| columns.iter().position(|c| *c == name).map_or(Value::Null, |i| row[i].clone());
    let rows = json["rows"].as_array().cloned().unwrap_or_default();
    for c in &mut copies {
        c.depth = rows.iter().find(|r| at(r, "model").as_str() == Some(c.model.as_str()) && at(r, "emitter").as_str() == Some(c.emitter.as_str())).and_then(|r| at(r, "offset_cm").as_f64()).map_or(DEFAULT_DEPTH_CM, |v| v as f32);
    }
    copies
}

/// (offline comparison only: the effects harness's `--no-depth-pull`) the copies' depth pull off, as DEPLOY4 drew them.
static NO_PULL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_depth_pull(on: bool) {
    NO_PULL.store(!on, std::sync::atomic::Ordering::Relaxed);
}

/// KH3's commonest copy depth pull (cm): the Kingdom Key's `SQEX_DepthOffset`, the Counter Shield's `Camera_Offset`.
pub const DEFAULT_DEPTH_CM: f32 = 2.5;

/// KH3's depth pull (kh-vfx `depth_pull`) as a transform: everything slides along its eye ray toward `eye` by `offset` cm at
/// `at` (scaled about the eye: its picture stays where and as big as it was; only what it hides changes). Identity when the
/// offset would reach the eye.
pub fn depth_pull(eye: Vec3, at: Vec3, offset: f32) -> glam::Affine3A {
    let d = (at - eye).length();
    if offset <= 0.0 || !d.is_finite() || d <= offset + 1.0 {
        return glam::Affine3A::IDENTITY;
    }
    let k = (d - offset) / d;
    glam::Affine3A::from_translation(eye) * glam::Affine3A::from_scale(Vec3::splat(k)) * glam::Affine3A::from_translation(-eye)
}

/// A mesh name of the effects' lists as the copies' table names it (the root's namespace `kbfx:` / `party:` and a clash
/// suffix `__...` taken off, lower case).
fn bare_mesh(name: &str) -> String {
    let name = name.rsplit(':').next().unwrap_or(name);
    name.split("__").next().unwrap_or(name).to_ascii_lowercase()
}

/// One weapon drawn this frame: its model, its MODEL frame and its component.
#[derive(Clone, Debug, PartialEq)]
pub struct DrawnWeapon {
    pub model: String,
    pub frame: kh_anim::Trs,
    pub component: kh_anim::Trs,
}

static DRAWN: Mutex<Option<(Vec<DrawnWeapon>, Instant)>> = Mutex::new(None);

/// The weapons a frame draws: the hand's Keyblade (`weapon`: the model in his hand, `keyblade::hand`) and every keyblade-pack
/// form weapon (`forms`: `kb:<model>`), each with its model frame (its bone `wep` from its own skeleton's bind).
pub fn drawn_weapons(weapon: Option<&crate::Weapon>, hand_model: Option<&str>, forms: &[crate::FormWeapon]) -> Vec<DrawnWeapon> {
    let mut out = Vec::new();
    if let (Some(w), Some(model)) = (weapon.filter(|w| w.opacity > 0.0), hand_model) {
        out.push(DrawnWeapon { model: model.to_string(), frame: hand_frame(w, wep_bind(model)), component: w.to_world });
    }
    for f in forms {
        let Some(model) = f.model.strip_prefix(crate::keyblade::PREFIX) else { continue };
        let index = crate::keyblade::table().and_then(|t| t.model(model)).and_then(|m| m.skeleton.as_ref()).and_then(|s| s.index_of("wep"));
        let pose = index.and_then(|i| f.component.get(i).copied());
        out.push(DrawnWeapon { model: model.to_string(), frame: model_frame(&f.to_world, pose, wep_bind(model)), component: f.to_world });
    }
    out
}

/// THE RENDERER, once a frame (lib.rs `submit`): the weapons it draws this frame (the copies are locked onto them).
pub fn note_drawn(input: &crate::FrameInput) {
    note_drawn_weapons(drawn_weapons(input.weapon.as_ref(), hand_model().as_deref(), &input.forms));
}

/// The same with the weapons given (a host that prepares its effects before it submits the frame: the effects harness).
pub fn note_drawn_weapons(drawn: Vec<DrawnWeapon>) {
    if let Ok(mut g) = DRAWN.lock() {
        *g = Some((drawn, Instant::now()));
    }
}

/// The model of the Keyblade in his hand (`keyblade::hand`; None while its models load).
pub fn hand_model() -> Option<String> {
    match crate::keyblade::hand() {
        crate::keyblade::Hand::KingdomKey => Some(KINGDOM_KEY.to_string()),
        crate::keyblade::Hand::Model(m) => Some(m.strip_prefix(crate::keyblade::PREFIX).unwrap_or(&m).to_string()),
        crate::keyblade::Hand::Loading => None,
    }
}

fn drawn_now() -> Vec<DrawnWeapon> {
    DRAWN.lock().ok().and_then(|g| g.as_ref().filter(|(_, at)| at.elapsed().as_millis() < 250).map(|(d, _)| d.clone())).unwrap_or_default()
}

/// Where a copy of the effects' mesh `mesh` is drawn: on the drawn weapon it copies (its model frame x its turn) while the hero
/// light is on; `transform` (the particle system's) otherwise.
pub fn lock_mesh(mesh: &str, transform: glam::Affine3A, eye: Vec3) -> glam::Affine3A {
    lock_in(&drawn_now(), carrier().is_some(), mesh, false, eye).unwrap_or(transform)
}

/// The same for a skeletal copy (`SkelInstance::mesh`): at the weapon's component.
pub fn lock_skel(mesh: &str, transform: glam::Affine3A, eye: Vec3) -> glam::Affine3A {
    lock_in(&drawn_now(), carrier().is_some(), mesh, true, eye).unwrap_or(transform)
}

/// The rule of `lock_mesh` / `lock_skel` on given drawn weapons: the copy's row whose weapon is drawn (several rows may share a
/// mesh: the drawn one), its frame x its turn, pulled toward `eye` by the copy's own KH3 depth offset (so it never shares the
/// weapon's depth). None: not a copy, the light off, or its weapon not drawn.
pub fn lock_in(drawn: &[DrawnWeapon], lit: bool, mesh: &str, skeletal: bool, eye: Vec3) -> Option<glam::Affine3A> {
    if !lit || mesh.is_empty() {
        return None;
    }
    let mesh = bare_mesh(mesh);
    let copy = copies().iter().filter(|c| c.skeletal == skeletal && c.mesh == mesh).find_map(|c| drawn.iter().find(|d| d.model.eq_ignore_ascii_case(&c.model)).map(|d| (c, d)));
    let (copy, weapon) = copy?;
    let base = if skeletal { weapon.component } else { weapon.frame };
    let pull = if NO_PULL.load(std::sync::atomic::Ordering::Relaxed) { 0.0 } else { copy.depth };
    let m = depth_pull(eye, base.translation, pull) * glam::Affine3A::from_scale_rotation_translation(base.scale, base.rotation, base.translation) * glam::Affine3A::from_quat(copy.turn);
    m.is_finite().then_some(m)
}

// ---------------------------------------------------------------------------------------------------- the carrier

/// The carrier's weapon this step (its model) and when it was told.
static CARRIER: Mutex<Option<(String, Instant)>> = Mutex::new(None);

/// THE HOST, once a step: the weapon model of whoever carries the hero light (None: nobody, the light is off).
pub fn set_carrier(model: Option<&str>) {
    if let Ok(mut g) = CARRIER.lock() {
        let was = g.as_ref().map(|(m, _)| m.clone());
        if was.as_deref() != model {
            crate::rlog!("render: (HEROLIGHT-KB) the hero light's weapon: {}", model.map_or("none".to_string(), |m| format!("{m} ({})", weapon(m).map_or("NOT IN THE TABLE: the Kingdom Key's look".to_string(), |w| format!("{} glow {:?}, {} sparkle emitter(s), tint {:?}", w.effect, w.glow, w.spark.len(), w.tint)))));
        }
        *g = model.map(|m| (m.to_string(), Instant::now()));
    }
}

/// The carrier's row (it lapses half a second after the host stops telling it).
pub fn carrier() -> Option<&'static HeroWeapon> {
    let g = CARRIER.lock().ok()?;
    g.as_ref().filter(|(_, at)| at.elapsed().as_millis() < 500).and_then(|(m, _)| weapon(m))
}

/// The light's tint this frame (1: no carrier told, or a weapon not in the table).
pub fn tint_now() -> Vec3 {
    carrier().map_or(Vec3::ONE, |w| w.tint)
}

// ---------------------------------------------------------------------------------------------------- the host's driver

/// What a host does for the hero light's effects in one step.
#[derive(Clone, Debug, PartialEq)]
pub enum Act {
    /// Start `name` at the carrier's `socket`: the steady glow (persistent, its handle kept) or a sparkle (it ends by itself;
    /// its handle kept until the next one, to end it at once on a change).
    Start { name: &'static str, socket: &'static str, glow: bool },
    /// Stop the glow / the latest sparkle: softly (it plays out as authored) or at once.
    Stop { glow: bool, immediate: bool },
}

/// THE HOST SIDE every host shares. `who`: the carrier's id (the host's own: Sora 0, Donald 1, Goofy 2).
#[derive(Clone, Debug, Default)]
pub struct Driver {
    glow: KeybladeGlow,
    on: Option<(u8, &'static HeroWeapon)>,
}

impl Driver {
    /// One step: `lit` = the light is on and the carrier's weapon fully drawn; `carrier` = who carries it and with what.
    pub fn step(&mut self, dt: f32, lit: bool, carrier: Option<(u8, &'static HeroWeapon)>, s: &LightSettings) -> Vec<Act> {
        let target = carrier.filter(|_| lit);
        let mut acts = Vec::new();
        let same = |a: &(u8, &HeroWeapon), b: &(u8, &HeroWeapon)| a.0 == b.0 && a.1.model == b.1.model;
        if let (Some(was), Some(now)) = (self.on, target) {
            if !same(&was, &now) {
                // a change of carrier or weapon while lit: the old glow and sparkle end at once, the new ones start now
                if self.glow.glowing {
                    acts.push(Act::Stop { glow: true, immediate: true });
                }
                acts.push(Act::Stop { glow: false, immediate: true });
                self.glow = KeybladeGlow::default();
            }
        }
        let step = self.glow.step(dt, target.is_some(), s.hero_glow, s.hero_sparkle);
        if step.stop_glow {
            acts.push(Act::Stop { glow: true, immediate: false });
        }
        if let Some((_, w)) = target {
            if step.start_glow && !w.glow.is_empty() {
                acts.push(Act::Start { name: w.glow_name.as_str(), socket: w.socket.as_str(), glow: true });
            }
            if step.sparkle {
                // hero.glow 0: IMPL_ER44's whole appear effect at the interval (of this weapon)
                let name = if s.hero_glow { (!w.spark.is_empty()).then_some(w.spark_name.as_str()) } else { Some(w.effect.as_str()) };
                if let Some(name) = name {
                    acts.push(Act::Start { name, socket: w.socket.as_str(), glow: false });
                }
            }
        }
        self.on = target;
        acts
    }

    /// Who carries it now (None: off).
    pub fn on(&self) -> Option<(u8, &'static HeroWeapon)> {
        self.on
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every row names an effect of the pack with its emitters; every Keyblade Sora can equip (the 17 item keys of
    /// sheets/port/sora-keyblades) and every party weapon has its row; the Kingdom Key's row is the reference.
    #[test]
    fn every_weapon_resolves_to_a_real_effect_and_socket() {
        let t = table();
        assert_eq!(t.len(), 76, "Sora's 48 weapon models (17 Keyblades and their form weapons), Donald's 14, Goofy's 14");
        let kk = kingdom_key().expect("the Kingdom Key row");
        assert_eq!((kk.effect.as_str(), kk.socket.as_str(), kk.tint), ("VW_SO010_APP0", "R_wep_skt", Vec3::ONE));
        assert_eq!(kk.glow, ["Key", "glw_I"], "IMPL_ER53's look-chosen glow emitters come out of the rule");
        // the 17 equippable Keyblades (their item keys, as the core's catalogue has them)
        let keys: Vec<String> = {
            let json: Value = serde_json::from_str(include_str!("../../../sheets/port/sora-keyblades.json")).unwrap();
            let cols: Vec<&str> = json["columns"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
            let (kind, key) = (cols.iter().position(|c| *c == "kind").unwrap(), cols.iter().position(|c| *c == "keyblade").unwrap());
            json["rows"].as_array().unwrap().iter().filter(|r| r[kind] == "keyblade").map(|r| r[key].as_str().unwrap().to_string()).collect()
        };
        assert_eq!(keys.len(), 17);
        for k in &keys {
            assert!(t.iter().any(|w| w.owner == "sora" && w.keys.contains(k)), "{k} has no weapon row");
        }
        for owner in ["donald", "goofy"] {
            assert_eq!(t.iter().filter(|w| w.owner == owner).count(), 14, "{owner}'s weapons");
        }
        // the pack: each row's effect file and its kept emitters (the pack is the user's live one: read, never written)
        let pack = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack");
        if !pack.join("vfx").exists() {
            return;
        }
        for w in t {
            let file = pack.join(&w.root).join("effects").join(format!("{}.json", w.effect));
            let text = std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("{}: {} ({e})", w.model, file.display()));
            let fx: Value = serde_json::from_str(&text).unwrap();
            let names: Vec<&str> = fx["emitters"].as_array().unwrap().iter().filter_map(|e| e["name"].as_str()).collect();
            for e in w.glow.iter().chain(&w.spark) {
                assert!(names.contains(&e.as_str()), "{}: no emitter {e} in {}", w.model, w.effect);
            }
            assert!(!w.glow.is_empty() || !w.spark.is_empty(), "{}: neither a glow nor sparkles", w.model);
            assert!(!w.socket.is_empty() && w.line.is_some(), "{}: socket / line", w.model);
            // the line's sockets are the weapon's own
            let (a, b) = w.line_sockets.clone().unwrap();
            let sockets = if w.owner == "sora" { pack.join("keyblades").join(&w.model).join("sockets.json") } else { pack.join("party/weapons").join(&w.model).join("manifest.json") };
            let s = std::fs::read_to_string(&sockets).unwrap();
            assert!(s.contains(&format!("\"{a}\"")) && s.contains(&format!("\"{b}\"")), "{}: {a} / {b} not in {}", w.model, sockets.display());
        }
    }

    #[test]
    fn the_kingdom_key_keeps_its_place_and_every_blade_gets_the_same_fraction() {
        let s = LightSettings::default();
        let f = blade_fraction(&s);
        assert!((f - (55.0 - 14.154) / (79.0368 - 14.154)).abs() < 1e-3, "{f}");
        assert_eq!(hand_along(kingdom_key(), &s), 55.0);
        // Ultima Weapon (vfx_bottom 10, vfx_top 133.5): the same fraction of its longer blade
        let ultima = weapon("w_so190").unwrap();
        assert!((hand_along(Some(ultima), &s) - (10.0 + f * 123.5)).abs() < 1e-2);
        assert!(weapon("kb:w_so190").is_some() && weapon("W_SO190").is_some());
    }

    #[test]
    fn a_switch_ends_the_old_glow_at_once_and_starts_the_new_one() {
        let s = LightSettings::default();
        let (kk, crystal) = (kingdom_key().unwrap(), weapon("w_so080").unwrap());
        let dt = 1.0 / 60.0;
        let mut d = Driver::default();
        let first = d.step(dt, true, Some((0, kk)), &s);
        assert_eq!(first, [Act::Start { name: "VW_SO010_APP0#hl_glow", socket: "R_wep_skt", glow: true }, Act::Start { name: "VW_SO010_APP0#hl_spark", socket: "R_wep_skt", glow: false }]);
        for _ in 0..30 {
            assert!(d.step(dt, true, Some((0, kk)), &s).is_empty(), "the glow is held, nothing else for 0.5 s");
        }
        // the D-pad switch to Crystal Snow while lit
        let swap = d.step(dt, true, Some((0, crystal)), &s);
        assert_eq!(swap[..2], [Act::Stop { glow: true, immediate: true }, Act::Stop { glow: false, immediate: true }]);
        assert!(swap.contains(&Act::Start { name: "vw_so080_app0#hl_glow", socket: "R_wep_skt", glow: true }));
        assert_eq!(swap.iter().filter(|a| matches!(a, Act::Start { glow: true, .. })).count(), 1, "one glow, not two");
        // the leader swap to Donald: Sora's ends at once, Donald's staff starts at his socket
        let donald = weapon("w_do010").unwrap();
        let lead = d.step(dt, true, Some((1, donald)), &s);
        assert!(lead.contains(&Act::Stop { glow: true, immediate: true }) && lead.contains(&Act::Start { name: "vw_don_app00#hl_glow", socket: "R_wep_skt", glow: true }));
        // the light goes off: the glow stops softly, nothing starts
        assert_eq!(d.step(dt, false, Some((1, donald)), &s), [Act::Stop { glow: true, immediate: false }]);
        assert!(d.step(dt, false, Some((1, donald)), &s).is_empty() && d.on().is_none());
    }

    /// (HEROLIGHT-KB final pass) the user's shots: the glow copy detached beside him, shifted along the blade. Every copy of the
    /// table is drawn exactly on its weapon's model frame (within 0.1 mm at the grip, 80 cm up the blade and 10 cm off it),
    /// whatever its particle put it; with the light off or the weapon not drawn it is left alone.
    #[test]
    fn every_glow_copy_is_drawn_exactly_on_its_weapon() {
        assert_eq!(copies().len(), 50, "22 mesh copies and 28 skeletal ones (sheets/port/sora-herolight-copies.json)");
        let ever_after = copies().iter().find(|c| c.model == "w_so070").unwrap();
        assert!(ever_after.turn.dot(rotator(90.0, 0.0, 0.0)).abs() > 0.9999, "Ever After's copy is stored a quarter turn about X (fitted in the effects' mesh space)");
        let component = kh_anim::Trs { translation: Vec3::new(1000.0, -2000.0, 150.0), rotation: glam::Quat::from_rotation_z(0.7) * glam::Quat::from_rotation_x(1.1), scale: Vec3::ONE };
        // the keychain pose moves the bone wep 2.5 cm off its bind (the Kingdom Key's measured offset): the frame follows it
        let bind = kh_anim::Trs { rotation: glam::Quat::from_xyzw(1.0, 0.0, 0.0, 0.0), ..kh_anim::Trs::IDENTITY };
        let pose = kh_anim::Trs { translation: Vec3::new(0.0, 2.5, 0.0), ..bind };
        let frame = model_frame(&component, Some(pose), Some(bind));
        assert!((frame.transform_point(Vec3::ZERO) - component.transform_point(Vec3::new(0.0, 2.5, 0.0))).length() < 1e-3);
        assert_eq!(model_frame(&component, None, Some(bind)), kh_anim::Trs::IDENTITY.relative_to(&bind).then(&bind).then(&component));
        // on the weapon's own eye rays (the same picture), each point its KH3 depth pull nearer the eye
        let eye = component.translation + Vec3::new(-300.0, 120.0, 90.0);
        for c in copies() {
            let drawn = [DrawnWeapon { model: c.model.clone(), frame, component }];
            let m = lock_in(&drawn, true, &format!("kbfx:{}", c.mesh), c.skeletal, eye).unwrap_or_else(|| panic!("{} {} not locked", c.model, c.mesh));
            let base = if c.skeletal { component } else { frame };
            // (copies sharing one mesh, Dead of Night's two, are told apart by nothing in a draw list: the first row's pull serves both)
            let depth = copies().iter().find(|o| o.model == c.model && o.mesh == c.mesh && o.skeletal == c.skeletal).map_or(c.depth, |o| o.depth);
            let k = 1.0 - depth / (base.translation - eye).length();
            for a in [Vec3::ZERO, Vec3::X * 80.0, Vec3::Y * 10.0, Vec3::Z * 10.0] {
                let want = eye + (base.transform_point(a) - eye) * k;
                let off = (m.transform_point3(c.turn.inverse() * a) - want).length();
                assert!(off < 0.01, "{} {}: {off} cm", c.model, c.mesh);
            }
            assert!(c.depth >= 2.5, "{} {}: KH3's pull {} cm", c.model, c.mesh, c.depth);
            assert!(lock_in(&drawn, false, &c.mesh, c.skeletal, eye).is_none(), "the light off: the particle's own place");
            assert!(lock_in(&[], true, &c.mesh, c.skeletal, eye).is_none(), "its weapon not drawn: left alone");
        }
        assert!(lock_in(&[DrawnWeapon { model: "w_so010".into(), frame, component }], true, "kbfx:jmd_cir_32x1_00", false, eye).is_none(), "not a copy");
    }

    /// (DEPLOY4, the user: "the light overlay on the keyblade flickers with the actual keyblade model") DEPTH SEPARATION over a frame
    /// sweep: the camera orbits the held blade from 1 m to 80 m (and still), and for every copy and every sample point of the
    /// blade the copy's reversed-Z depth (near / view z, as Elden Ring's buffer) is NEARER than the weapon's at the same pixel by at
    /// least 64 float steps of that depth: the depth test can never trade them (no z-fight), at any distance.
    #[test]
    fn a_glow_copy_never_shares_its_weapons_depth_over_a_frame_sweep() {
        let near = 10.0; // cm: a near plane closer than the game's
        let component = kh_anim::Trs { translation: Vec3::new(0.0, 0.0, 120.0), rotation: glam::Quat::from_rotation_y(-0.6), scale: Vec3::ONE };
        let frame = model_frame(&component, None, None);
        let mut worst = f32::MAX;
        for step in 0..240 {
            let t = step as f32 / 240.0;
            let distance = 100.0 * (8000.0f32 / 100.0).powf(t); // 1 m .. 80 m
            let yaw = t * 12.0;
            let eye = component.translation + Vec3::new(yaw.cos() * distance, yaw.sin() * distance, 30.0 + 0.2 * distance);
            for c in copies() {
                let drawn = [DrawnWeapon { model: c.model.clone(), frame, component }];
                let m = lock_in(&drawn, true, &c.mesh, c.skeletal, eye).unwrap();
                let base = if c.skeletal { component } else { frame };
                for a in [Vec3::ZERO, Vec3::X * 40.0, Vec3::X * 120.0, Vec3::new(60.0, 15.0, -5.0)] {
                    let (copy, weapon) = (m.transform_point3(c.turn.inverse() * a), base.transform_point(a));
                    let forward = (component.translation - eye).normalize();
                    let (zc, zw) = ((copy - eye).dot(forward), (weapon - eye).dot(forward));
                    if zw <= 2.0 * near + 10.0 {
                        continue; // behind or at the near plane: not drawn
                    }
                    let (dc, dw) = (near / zc, near / zw);
                    let steps = (dc - dw) / (dw * f32::EPSILON);
                    worst = worst.min(steps);
                    assert!(dc > dw && steps >= 64.0, "{} {} at {distance:.0} cm: copy depth {dc:e}, weapon {dw:e} ({steps:.0} float steps)", c.model, c.mesh);
                }
            }
        }
        assert!(worst >= 64.0, "{worst}");
    }

    #[test]
    fn the_tint_follows_the_weapons_glow_and_the_kingdom_key_stays_white() {
        assert_eq!(kingdom_key().unwrap().tint, Vec3::ONE);
        // Shooting Star's blue glow (Key0 StartColor 1.75, 5, 10), Hunny Spout's honey (Key ColorScale 3, 2.35, 0.63)
        let star = weapon("w_so020").unwrap().tint;
        assert!(star.z > star.x * 3.0, "{star:?}");
        let hunny = weapon("w_so040").unwrap().tint;
        assert!(hunny.x > hunny.z * 3.0, "{hunny:?}");
    }
}
