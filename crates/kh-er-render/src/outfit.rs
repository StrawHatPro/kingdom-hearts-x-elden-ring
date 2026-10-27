//! (IMPL_ER59, the user: "Boc is able to change Sora's appearance: KH1, KH2, and all of Sora's world outfits in 3")
//! SORA'S OUTFIT: which of the pack's models is drawn, loaded ON DEMAND.
//!
//! The outfits are folders of the pack written by `tools/sora_outfit_pack.js` (`PACK/outfits/<id>/`: `model.glb`,
//! `manifest.json` with the outfit's own skeleton, `materials/` with its translated shaders, textures, DXIL). The host
//! names the outfit it wants ([`crate::set_outfit`]); the renderer's worker builds the new model, library, textures and
//! shaders on its own thread while the old ones are still drawn, then swaps them in; the old ones are kept until
//! the GPU has finished the frames that drew them ([`retire`], [`frame_begins`]). "kh3" (or "") = the main pack, as before.
//!
//! THE POSE (an outfit has its own skeleton: the same bone names as Sora's for most bones, other lengths and extra
//! bones): every bone the outfit shares with Sora takes his frame's local rotation AS A CHANGE FROM HIS REFERENCE
//! (rotation = his local * his reference^-1 * the outfit's reference), his translation as a change likewise, his scale
//! as a ratio; a bone Sora does not have keeps the outfit's reference relative to its parent. In the reference pose the
//! outfit is therefore exactly its bind pose. A STAND-IN for KH3's own way (each world pawn plays its own anim set,
//! retargeted in the editor): the clips here are Sora's.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use glam::Vec3;
use kh_anim::Trs;
use windows::Win32::Graphics::Direct3D12::ID3D12Fence;

/// The outfit the host wants ("" = the main pack).
static WANTED: Mutex<String> = Mutex::new(String::new());
/// The outfit whose assets are drawn (or being made) and the ones that failed to load (not tried again).
static LOADED: Mutex<String> = Mutex::new(String::new());
static FAILED: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// Old assets kept until the GPU is surely done with them: `after` = the frame fence's value the GPU must pass
/// (None until our next frame begins). (GPUSAFE) Once a fixed 3 s, which a stall longer than that outlived (the AMD
/// driver then removes the device: a "GPU crash" on an outfit change or a new game).
struct Retired {
    after: Option<(ID3D12Fence, u64)>,
    _item: Box<dyn std::any::Any + Send>,
}
static RETIRED: Mutex<Vec<Retired>> = Mutex::new(Vec::new());

/// (ER69) The outfits' own Bonamik runs (false: their extra bones keep their pose on the parent, as before).
const SIM_LIVE: bool = true;

static SIM_REPORT: Mutex<String> = Mutex::new(String::new());
/// (ER73) The largest push of the anti-clip pass since the report was last read (cm).
static UNCLIP_MAX: Mutex<f32> = Mutex::new(0.0);

/// (ER73, the user: "Toyboxes and Monsters Inc Sora's legs are fucked") THE OUTFIT'S OWN KINEDRIVER runs on its pose
/// (`kinedriver.json`, tools/sora_outfit_kdi.js), and its mesh with those bones unfolded (`model_kd.glb`) is drawn.
/// kh-er `boc.outfit_kd = 0`: off (the older pose and model.glb).
static KD_LIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// (ER79, the user: "I just want his pants to never clip into his leg") THE LEG GUARD of an outfit whose
/// `legguard.json` (tools/sora_outfit_legguard.js) is `enabled`: its simulated lower-body cloth bones are pushed out of
/// capsules fitted around its leg mesh after its Bonamik step, and the skin shader (`skin_cg.vs.dxil`, er_char.wgsl
/// KH_CLOTH_GUARD) puts any cloth vertex that still ends inside one on its surface. kh-er `boc.leg_guard = 0`: off
/// (takes effect at the next outfit load).
static GUARD_LIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub fn set_leg_guard(on: bool) {
    GUARD_LIVE.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub fn leg_guard_on() -> bool {
    GUARD_LIVE.load(std::sync::atomic::Ordering::Relaxed)
}

/// (ER79, the user's 54.jpg: "the shorts show their inside ... the hem looks hollow") AN OUTFIT'S CLOTH DRAWN TWO-SIDED:
/// the material slots its `twosided.json` lists (`enabled`) are drawn with both faces in every pass, and their back faces
/// are shaded as the cloth's inside (normal turned in, x the interior factor: er_char.wgsl KH_INTERIOR, the `_bli` set).
/// kh-er `boc.cloth_twosided = 0`: off (read when an outfit is put on).
static TWOSIDED_LIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub fn set_cloth_twosided(on: bool) {
    TWOSIDED_LIVE.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub fn cloth_twosided_on() -> bool {
    TWOSIDED_LIVE.load(std::sync::atomic::Ordering::Relaxed)
}

/// (ER79) The slot scalar that carries a two-sided cloth slot's interior factor (gfx.rs puts it in the material's khs.w).
pub const INTERIOR_SCALAR: &str = "__er79_interior";

/// (ER79) Marks the slots of `look/twosided.json` two-sided with their interior factor (nothing when off, absent or not
/// enabled). Returns how many slots it marked.
pub fn apply_cloth_twosided(look: &Path, library: &mut kh_look::library::Library) -> usize {
    if !cloth_twosided_on() {
        return 0;
    }
    let Ok(text) = std::fs::read_to_string(look.join("twosided.json")) else { return 0 };
    let json: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
    if json["enabled"].as_bool() != Some(true) {
        return 0;
    }
    let factor = json["interior"].as_f64().unwrap_or(0.35).clamp(0.01, 1.0) as f32;
    let names: Vec<&str> = json["slots"].as_array().into_iter().flatten().filter_map(|s| s.as_str()).collect();
    let mut n = 0;
    for slot in library.slots.iter_mut().filter(|s| names.contains(&s.slot.as_str()) && !s.translucent) {
        slot.two_sided = true;
        slot.scalars.insert(INTERIOR_SCALAR.to_string(), factor);
        n += 1;
    }
    n
}

/// (ER79) The palette entries the leg guard uses (er_char.wgsl KH_CLOTH_GUARD): a skin of at most this many joints.
pub const GUARD_FIRST_ENTRY: usize = 314;

/// (ER79) The leg guard's data of an outfit: its capsules and (once its skin is known) one bit per skin joint that is cloth.
#[derive(Clone, Debug, Default)]
pub struct LegGuard {
    capsules: Vec<Capsule>,
    cloth: std::collections::HashSet<String>,
    flags: [u32; 16],
    joints: usize,
}

impl LegGuard {
    /// Whether the skin joint `k` is cloth.
    pub fn is_cloth(&self, k: usize) -> bool {
        k < 512 && self.flags[k >> 5] & (1 << (k & 31)) != 0
    }
}

/// (ER73) Switches the outfits' own KineDriver (takes effect at the next outfit load).
pub fn set_kinedriver(on: bool) {
    KD_LIVE.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub fn kinedriver_on() -> bool {
    KD_LIVE.load(std::sync::atomic::Ordering::Relaxed)
}

/// (ER69) The outfit's Bonamik as of the latest frame (running / stopped, bones it moved, fallback branches met).
pub fn sim_report() -> String {
    let unclip = UNCLIP_MAX.lock().map(|mut m| std::mem::take(&mut *m)).unwrap_or(0.0);
    if unclip > 0.0 {
        return format!("{}; anti-clip pushed a bone out by up to {unclip:.1} cm", SIM_REPORT.lock().map(|s| s.clone()).unwrap_or_default());
    }
    SIM_REPORT.lock().map(|r| r.clone()).unwrap_or_default()
}

fn before_check(p: &[Trs]) -> Vec<Trs> {
    p.to_vec()
}

/// The main pack's names.
pub fn is_main(id: &str) -> bool {
    id.is_empty() || id == "kh3"
}

pub fn set_wanted(id: &str) {
    let id = if is_main(id) { String::new() } else { id.to_string() };
    if let Ok(mut w) = WANTED.lock() {
        *w = id;
    }
}

pub fn wanted() -> String {
    WANTED.lock().map(|w| w.clone()).unwrap_or_default()
}

pub fn loaded() -> String {
    LOADED.lock().map(|w| w.clone()).unwrap_or_default()
}

/// A swap is due: another outfit is wanted than the one drawn, and it has not failed before.
pub fn change_wanted() -> bool {
    let (w, l) = (wanted(), loaded());
    w != l && !FAILED.lock().map(|f| f.contains(&w)).unwrap_or(false)
}

/// The worker starts loading: the id the assets are for (`wanted()`, or the main pack when that one failed before).
pub fn begin_load() -> String {
    let w = wanted();
    if FAILED.lock().map(|f| f.contains(&w)).unwrap_or(false) {
        String::new()
    } else {
        w
    }
}

/// The load of `id` ended: on success it is the drawn one; a failure is remembered (the old assets stay).
pub fn end_load(id: &str, ok: bool) {
    if ok {
        if let Ok(mut l) = LOADED.lock() {
            *l = id.to_string();
        }
    } else if let Ok(mut f) = FAILED.lock() {
        if !f.iter().any(|x| x == id) {
            f.push(id.to_string());
        }
    }
}

/// The folder the materials (and shaders) of `id` come from: `PACK/outfits/<id>` when it has its materials, else the pack.
pub fn look_dir(pack: &Path, id: &str) -> PathBuf {
    if is_main(id) {
        return pack.to_path_buf();
    }
    let dir = pack.join("outfits").join(id);
    if dir.join("materials").join("materials.json").is_file() && dir.join("model.glb").is_file() {
        dir
    } else {
        pack.to_path_buf()
    }
}

/// The body model of `id`.
pub fn model_file(pack: &Path, id: &str) -> PathBuf {
    let look = look_dir(pack, id);
    if look == pack {
        pack.join("sora.glb")
    } else if kinedriver_on() && look.join("kinedriver.json").is_file() && look.join("model_kd.glb").is_file() {
        // (ER73) the bones its own KineDriver moves keep their skin
        look.join("model_kd.glb")
    } else {
        look.join("model.glb")
    }
}

/// The retarget of `id` onto Sora's skeleton (`None` for the main pack).
pub fn retarget_for(pack: &Path, id: &str, host: &kh_anim::Skeleton) -> Result<Option<Retarget>, String> {
    let look = look_dir(pack, id);
    if look == pack {
        return Ok(None);
    }
    let file = look.join("manifest.json");
    let text = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    let skeleton = kh_anim::Skeleton::from_manifest(&kh_anim::json::Json::parse(&text)?)?;
    let mut retarget = Retarget::new(&skeleton, host);
    // (ER69) the outfit's own secondary motion (its SQEX_BonamikAsset: bonamik.json, tools/sora_outfit_pack.js)
    let bnm = look.join("bonamik.json");
    // (ER69) NOT LIVE until every branch of the port the outfits' assets reach falls back instead of stopping
    if SIM_LIVE && bnm.is_file() {
        let text = std::fs::read_to_string(&bnm).map_err(|e| format!("{}: {e}", bnm.display()))?;
        let solver = kh_anim::Bonamik::new(&kh_anim::json::Json::parse(&text)?, &skeleton, kh_anim::BonamikOptions::default()).map_err(|e| format!("{}: {e}", bnm.display()))?;
        retarget.sim = Some(Sim(std::sync::Arc::new(Mutex::new(SimState { solver, broken: false, last: None, cached: None }))));
    }
    // (ER72, the user: "toy box hair should not move") AN OUTFIT WITH ITS OWN BONAMIK ASSET does not take Sora's SIMULATED
    // motion on a bone his asset drives and its own does not (Toy Box: its asset enables no group, so its hair is rigid,
    // as in KH3); an outfit without an asset of its own (Verum Rex) keeps his.
    if bnm.is_file() {
        let roles = |file: &Path| -> std::collections::HashMap<String, bool> {
            let text = std::fs::read_to_string(file).unwrap_or_default();
            let json: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
            json["bones"].as_array().into_iter().flatten().filter_map(|b| Some((b["name"].as_str()?.to_string(), b["bonamik"].as_str() != Some("none")))).collect()
        };
        let (his, its) = (roles(&pack.join("data").join("bonamik.json")), roles(&bnm));
        let mut rigid = 0;
        for (k, b) in skeleton.bones.iter().enumerate() {
            if his.get(&b.name) == Some(&true) && its.get(&b.name) != Some(&true) {
                retarget.bones[k].own_rest = true;
                rigid += 1;
            }
        }
        retarget.rigid = rigid;
    }
    // (ER73, the user: "will you fix all the clipping issues as well?") its anti-clip capsules (anticlip.json,
    // tools/sora_outfit_anticlip.js): simulated bones its own colliders leave free to pass through the body
    let clip = look.join("anticlip.json");
    if bnm.is_file() && clip.is_file() {
        let read = |file: &Path| -> serde_json::Value { serde_json::from_str(&std::fs::read_to_string(file).unwrap_or_default()).unwrap_or_default() };
        let (rules, roles) = (read(&clip), read(&bnm));
        let simulated: std::collections::HashSet<&str> = roles["bones"].as_array().into_iter().flatten().filter(|b| b["bonamik"].as_str() == Some("simulated_node")).filter_map(|b| b["name"].as_str()).collect();
        for rule in rules["rules"].as_array().into_iter().flatten() {
            let named: std::collections::HashSet<&str> = rule["bones"].as_array().into_iter().flatten().filter_map(|b| b.as_str()).collect();
            let bones: Vec<usize> = skeleton.bones.iter().enumerate().filter(|(_, b)| named.contains(b.name.as_str()) && simulated.contains(b.name.as_str())).map(|(k, _)| k).collect();
            let f = |c: &serde_json::Value, k: &str| c[k].as_f64().unwrap_or(0.0) as f32;
            let capsules: Vec<Capsule> = rule["capsules"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|c| Some(Capsule { a: skeleton.index_of(c["a"].as_str()?)?, b: skeleton.index_of(c["b"].as_str()?)?, t: (f(c, "t0"), f(c, "t1")), r: (f(c, "r0"), f(c, "r1")) }))
                .collect();
            if !bones.is_empty() && !capsules.is_empty() {
                retarget.clip.push((bones, capsules));
            }
        }
    }
    // (ER79) ITS LEG GUARD (legguard.json, tools/sora_outfit_legguard.js): the capsules around its leg mesh; its simulated
    // lower-body cloth bones are kept out of them after the Bonamik step (as an anti-clip rule), its cloth vertices in the
    // skin shader
    let guard_file = look.join("legguard.json");
    if leg_guard_on() && guard_file.is_file() {
        let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&guard_file).unwrap_or_default()).unwrap_or_default();
        if json["enabled"].as_bool() == Some(true) {
            let f = |c: &serde_json::Value, k: &str| c[k].as_f64().unwrap_or(0.0) as f32;
            let capsules: Vec<Capsule> = json["capsules"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|c| Some(Capsule { a: skeleton.index_of(c["a"].as_str()?)?, b: skeleton.index_of(c["b"].as_str()?)?, t: (f(c, "t0"), f(c, "t1")), r: (f(c, "r0"), f(c, "r1")) }))
                .take(8)
                .collect();
            let simulated: std::collections::HashSet<String> = if bnm.is_file() {
                let roles: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&bnm).unwrap_or_default()).unwrap_or_default();
                roles["bones"].as_array().into_iter().flatten().filter(|b| b["bonamik"].as_str() == Some("simulated_node")).filter_map(|b| b["name"].as_str().map(str::to_string)).collect()
            } else {
                Default::default()
            };
            let bones: Vec<usize> = json["bones"].as_array().into_iter().flatten().filter_map(|b| b.as_str()).filter(|b| simulated.contains(*b)).filter_map(|b| skeleton.index_of(b)).collect();
            if !capsules.is_empty() {
                if !bones.is_empty() {
                    retarget.clip.push((bones, capsules.clone()));
                }
                let cloth = json["cloth_joints"].as_array().into_iter().flatten().filter_map(|j| j.as_str().map(str::to_string)).collect();
                retarget.guard = Some(LegGuard { capsules, cloth, flags: [0; 16], joints: 0 });
            }
        }
    }
    // (ER73) ITS OWN KINEDRIVER (its knee, hip, shoulder, boot-top pieces follow ITS rules); Sora's helper bones of the
    // same name that his driver moves and its own does not keep the outfit's pose (his driver's motion is his body's)
    let kdi = look.join("kinedriver.json");
    if kinedriver_on() && kdi.is_file() {
        let text = std::fs::read_to_string(&kdi).map_err(|e| format!("{}: {e}", kdi.display()))?;
        let driver = kh_anim::kine::KineDriver::from_json(&kh_anim::json::Json::parse(&text)?).map_err(|e| format!("{}: {e}", kdi.display()))?;
        let ours: std::collections::HashSet<usize> = driver.written().iter().copied().collect();
        let his_text = std::fs::read_to_string(pack.join("data").join("kinedriver.json")).unwrap_or_default();
        let his: serde_json::Value = serde_json::from_str(&his_text).unwrap_or_default();
        for b in his["bones"].as_array().into_iter().flatten().filter(|b| b["written"].as_bool() == Some(true)) {
            if let Some(k) = b["name"].as_str().and_then(|n| retarget.index_of(n)) {
                if !ours.contains(&k) {
                    retarget.bones[k].own_rest = true;
                }
            }
        }
        retarget.kine = Some(Kine(std::sync::Arc::new(Mutex::new((driver, false)))));
    }
    // (ER83) ITS HAND CONTACT IK (KH1 only, the user: "they seem correct except for KH1"): its hands where his touch the
    // Keyblade's grip or his body (handik.rs). The body surfaces come from both skins (cached); without them: none.
    if crate::handik::OUTFITS.contains(&id) {
        let skins = crate::handik::cloud_of(&pack.join("sora.glb"), host).and_then(|his| Ok((his, crate::handik::cloud_of(&model_file(pack, id), &skeleton)?)));
        if let Ok((his, its)) = skins {
            retarget.hand_ik = crate::handik::HandIk::new(host, &skeleton, his, its).map(std::sync::Arc::new);
        }
    }
    Ok(Some(retarget))
}

/// Keeps `assets` alive until the GPU has finished every frame that may still read them ([`frame_begins`]).
pub fn retire(assets: Box<dyn std::any::Any + Send>) {
    if let Ok(mut r) = RETIRED.lock() {
        r.push(Retired { after: None, _item: assets });
    }
}

/// (GPUSAFE) Called at the start of each of our frames with the frame fence and its last signaled value: every list
/// recorded before now was executed (or dropped) by then, so what was retired since the last frame is let go of once
/// the GPU passes that value; what it has passed is let go of now. A lost device reads as done (UINT64_MAX).
pub fn frame_begins(fence: &ID3D12Fence, signaled: u64) {
    if let Ok(mut r) = RETIRED.lock() {
        for old in r.iter_mut().filter(|o| o.after.is_none()) {
            old.after = Some((fence.clone(), signaled));
        }
        // SAFETY: plain getter.
        r.retain(|o| o.after.as_ref().is_none_or(|(f, v)| unsafe { f.GetCompletedValue() } < *v));
    }
}

/// Lets go of everything retired (our frames are over: the device was waited for, or is gone).
pub fn release_all() {
    if let Ok(mut r) = RETIRED.lock() {
        r.clear();
    }
}

/// One bone of the outfit.
#[derive(Clone, Debug)]
struct RBone {
    parent: Option<usize>,
    /// Sora's bone of the same name.
    host: Option<usize>,
    reference: Trs,
    /// (ER72) his simulated bone the outfit's own data leaves still: kept at the outfit's pose on its parent.
    own_rest: bool,
}

/// (ER69) The outfit's Bonamik on its own pose: stepped once per frame (by the frame's clock), the result kept for the
/// frame's other users (skin, Keyblade, blood, materials, portrait).
type Posed = std::sync::Arc<(Vec<Trs>, Vec<Trs>)>;
struct SimState {
    solver: kh_anim::Bonamik,
    /// The solver stopped (a branch of the port that is not transcribed): the outfit's bones keep the retargeted pose.
    broken: bool,
    last: Option<f32>,
    cached: Option<(f32, Posed)>,
}
/// (ER73) An anti-clip capsule: the segment between fractions `t` of bone a -> bone b, radius `r.0` at `t.0` to `r.1` at `t.1`.
#[derive(Clone, Debug)]
struct Capsule {
    a: usize,
    b: usize,
    t: (f32, f32),
    r: (f32, f32),
}

impl Capsule {
    /// `p` pushed out of the capsule (component space, the pose `comps`).
    fn push(&self, comps: &[Trs], p: Vec3) -> Vec3 {
        let (Some(a), Some(b)) = (comps.get(self.a), comps.get(self.b)) else { return p };
        let (a, b) = (a.translation, b.translation);
        let (s, e) = (a + (b - a) * self.t.0, a + (b - a) * self.t.1);
        let d = e - s;
        let u = if d.length_squared() > 1.0e-6 { ((p - s).dot(d) / d.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
        let q = s + d * u;
        let r = self.r.0 + (self.r.1 - self.r.0) * u;
        let off = p - q;
        let l = off.length();
        if l >= r || l < 1.0e-4 {
            return p;
        }
        q + off * (r / l)
    }
}

/// (ER73) The outfit's own KineDriver (and whether it stopped: a panic or a non-finite bone, then the pose without it).
#[derive(Clone)]
struct Kine(std::sync::Arc<Mutex<(kh_anim::kine::KineDriver, bool)>>);
impl std::fmt::Debug for Kine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("KineDriver")
    }
}
#[derive(Clone)]
struct Sim(std::sync::Arc<Mutex<SimState>>);
impl std::fmt::Debug for Sim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Bonamik")
    }
}

/// (ER73) The bones the Keyblade hangs on (KH3's weapon sockets R_wep_skt / L_wep_skt sit on them).
const WEAPON_BONES: [&str; 2] = ["R_buki", "L_buki"];

/// The outfit's skeleton driven by Sora's pose.
#[derive(Clone, Debug)]
pub struct Retarget {
    /// (ER69) its own Bonamik, when it has one.
    sim: Option<Sim>,
    /// (ER73) its own KineDriver, when it has one.
    kine: Option<Kine>,
    /// (ER73) its anti-clip rules: simulated bones and the capsules they are kept out of.
    clip: Vec<(Vec<usize>, Vec<Capsule>)>,
    /// (ER79) its leg guard (legguard.json, enabled and `boc.leg_guard` on).
    pub guard: Option<LegGuard>,
    /// (ER83) its hand contact IK (KH1 only: handik.rs; `boc.grip_ik`).
    pub hand_ik: Option<std::sync::Arc<crate::handik::HandIk>>,
    /// (ER72) bones kept still although his are simulated (see `retarget_for`).
    pub rigid: usize,
    bones: Vec<RBone>,
    names: std::collections::HashMap<String, usize>,
    /// Sora's reference (local) and parents, for his pose's changes.
    host_reference: Vec<Trs>,
    host_parent: Vec<Option<usize>>,
    /// His weapon sockets and the outfit's bones of the same name.
    hands: Vec<(usize, usize)>,
    /// (the user: "for Sora's KH1 outfit, animations need to be adjusted for his skeleton as I believe it's shorter")
    /// THE BODY SCALE: the outfit's hip height (bone `koshi` in its reference, component space) over Sora's; his
    /// animated TRANSLATIONS (the hips' bob and crouch, a bone sliding) are scaled by it, so a shorter body's feet stay
    /// on the floor. 1 when either has no `koshi`. KH1 Sora (p_ex005): 88.6 / 102.3 cm = 0.866; the other outfits 1.
    pub scale: f32,
}

/// The height of a bone in a skeleton's reference pose (component space, cm).
fn reference_height(skeleton: &kh_anim::Skeleton, name: &str) -> Option<f32> {
    let mut at = skeleton.index_of(name)?;
    let mut chain = vec![at];
    while let Some(p) = skeleton.bones[at].parent {
        chain.push(p);
        at = p;
    }
    let mut t = Trs::IDENTITY;
    for k in chain.into_iter().rev() {
        t = skeleton.bones[k].reference.then(&t);
    }
    Some(t.translation.z)
}

impl Retarget {
    /// (ER79) The leg guard's cloth bits for the drawn skin (its joints in the order the vertices index them).
    pub fn with_guard_joints(mut self, joints: &[(String, glam::Mat4)]) -> Retarget {
        if let Some(g) = self.guard.as_mut() {
            g.flags = [0; 16];
            g.joints = joints.len();
            for (k, (name, _)) in joints.iter().enumerate().take(512) {
                if g.cloth.contains(name) {
                    g.flags[k >> 5] |= 1 << (k & 31);
                }
            }
        }
        self
    }

    /// (ER79) The leg guard's capsules in PACK space for the outfit's component pose `posed`: (end a, radius a, end b,
    /// radius b) each, flat ends (the skin shader's KH_CLOTH_GUARD).
    pub fn guard_capsules_pack(&self, posed: &[Trs]) -> Vec<(Vec3, f32, Vec3, f32)> {
        let Some(g) = &self.guard else { return Vec::new() };
        let pack = |p: Vec3| crate::math::joint_in_pack(&Trs { translation: p, rotation: glam::Quat::IDENTITY, scale: Vec3::ONE }).w_axis.truncate();
        g.capsules
            .iter()
            .filter_map(|c| {
                let (a, b) = (posed.get(c.a)?.translation, posed.get(c.b)?.translation);
                let (s, e) = (a + (b - a) * c.t.0, a + (b - a) * c.t.1);
                // centimetres -> pack units
                let unit = (pack(s + Vec3::X) - pack(s)).length();
                Some((pack(s), c.r.0 * unit, pack(e), c.r.1 * unit))
            })
            .collect()
    }

    /// (ER79) Writes the leg guard into a skin palette (`skin`: 16 floats of game_from_pack, then 16 a joint) for the
    /// outfit's component pose `posed`: entries 314..319 (er_char.wgsl KH_CLOTH_GUARD). Nothing when the outfit has no
    /// guard or its skin has more than 314 joints (the shader then reads no "on" mark: a joint matrix has 0 there).
    pub fn write_leg_guard(&self, skin: &mut [f32], posed: &[Trs]) {
        let Some(g) = &self.guard else { return };
        if g.joints == 0 || g.joints > GUARD_FIRST_ENTRY || skin.len() < 16 * (1 + GUARD_FIRST_ENTRY + 6) {
            return;
        }
        let caps = self.guard_capsules_pack(posed);
        let entry = |k: usize| 16 * (1 + k);
        // [314] .. [317]: two capsules each (end a + radius, end b + radius)
        for (i, (a, ra, b, rb)) in caps.iter().take(8).enumerate() {
            let at = entry(GUARD_FIRST_ENTRY + i / 2) + (i % 2) * 8;
            skin[at..at + 8].copy_from_slice(&[a.x, a.y, a.z, *ra, b.x, b.y, b.z, *rb]);
        }
        // [318]: the cloth bits
        let at = entry(GUARD_FIRST_ENTRY + 4);
        for (k, word) in g.flags.iter().enumerate() {
            skin[at + k] = f32::from_bits(*word);
        }
        // [319] column 0: (-, count, -, 1 = on)
        let at = entry(GUARD_FIRST_ENTRY + 5);
        skin[at..at + 4].copy_from_slice(&[0.0, caps.len().min(8) as f32, 0.0, 1.0]);
    }

    pub fn new(outfit: &kh_anim::Skeleton, host: &kh_anim::Skeleton) -> Retarget {
        let bones = outfit.bones.iter().map(|b| RBone { parent: b.parent, host: host.index_of(&b.name), reference: b.reference, own_rest: false }).collect();
        let names = outfit.bones.iter().enumerate().map(|(i, b)| (b.name.clone(), i)).collect();
        let hands = ["R_buki", "L_buki"].iter().filter_map(|n| Some((host.index_of(n)?, outfit.index_of(n)?))).collect();
        let scale = match (reference_height(outfit, "koshi"), reference_height(host, "koshi")) {
            (Some(o), Some(h)) if o > 1.0 && h > 1.0 => (o / h).clamp(0.25, 4.0),
            _ => 1.0,
        };
        Retarget { rigid: 0, sim: None, kine: None, clip: Vec::new(), guard: None, hand_ik: None, bones, names, host_reference: host.bones.iter().map(|b| b.reference).collect(), host_parent: host.bones.iter().map(|b| b.parent).collect(), hands, scale }
    }

    pub fn len(&self) -> usize {
        self.bones.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bones.is_empty()
    }

    /// The outfit's bone of a name (the skin's joints are named by it).
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.names.get(name).copied()
    }

    /// (ER70, kh-er limbs.rs: the hands of a borrowed action for the worn outfit) `pose_full`'s rotation rule backwards:
    /// the local rotation Sora's bone must have for the outfit's bone `bone` to get `outfit_local` (None: not shared).
    pub fn host_rotation_for(&self, bone: usize, outfit_local: glam::Quat) -> Option<(usize, glam::Quat)> {
        let b = self.bones.get(bone)?;
        let h = b.host?;
        let rest = self.host_reference.get(h)?;
        Some((h, (outfit_local * b.reference.rotation.inverse() * rest.rotation).normalize()))
    }

    /// (ER70) The parent of the outfit's bone.
    pub fn parent_of(&self, bone: usize) -> Option<usize> {
        self.bones.get(bone)?.parent
    }

    /// THE KEYBLADE IN THE OUTFIT'S HAND: the host hangs the weapon on HIS hand (`weapon_to_world`, game space); an
    /// outfit's arm has its own length, so a weapon within 30 cm of one of his weapon sockets (`R_buki`, `L_buki`) is
    /// moved by the change from his socket to the outfit's (the same bone of the outfit's pose `posed`). Else unchanged.
    pub fn weapon_to_world(&self, weapon_to_world: &Trs, component: &[Trs], posed: &[Trs], component_to_world: &Trs) -> Trs {
        let world = |t: &Trs| t.then(component_to_world);
        let best = self
            .hands
            .iter()
            .filter_map(|(h, o)| Some((world(component.get(*h)?), world(posed.get(*o)?))))
            .map(|(his, its)| ((his.translation - weapon_to_world.translation).length(), his, its))
            .filter(|(d, ..)| *d < 30.0)
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let Some((_, his, its)) = best else { return *weapon_to_world };
        let m = |t: &Trs| glam::Mat4::from_scale_rotation_translation(t.scale, t.rotation, t.translation);
        let moved = m(&its) * m(&his).inverse() * m(weapon_to_world);
        let (scale, rotation, translation) = moved.to_scale_rotation_translation();
        Trs { translation, rotation: rotation.normalize(), scale }
    }

    /// Bones the outfit shares with Sora.
    pub fn shared(&self) -> usize {
        self.bones.iter().filter(|b| b.host.is_some()).count()
    }

    /// The outfit's bones in component space for Sora's frame: `local` (relative to the parent) and `component`
    /// in his skeleton's order; `local` may be empty (it is then made from `component`).
    /// (ER69) THE FRAME'S POSE (locals, component space): the retargeted pose, then the outfit's own Bonamik stepped by
    /// the frame clock's change (once per clock value: every user of the frame gets the same).
    pub fn frame(&self, clock: f32, component_to_world: &Trs, local: &[Trs], component: &[Trs]) -> Posed {
        let Some(sim) = &self.sim else { return std::sync::Arc::new(self.pose_full_at(local, component, Some(clock))) };
        let Ok(mut s) = sim.0.lock() else { return std::sync::Arc::new(self.pose_full_at(local, component, Some(clock))) };
        if let Some((at, posed)) = &s.cached {
            if *at == clock {
                return posed.clone();
            }
        }
        let (locals, mut comps) = self.pose_full_at(local, component, Some(clock));
        let dt = s.last.map_or(1.0 / 60.0, |l| (clock - l).clamp(0.0, 0.1));
        s.last = Some(clock);
        if dt > 0.0 && !s.broken {
            let before = comps.clone();
            // (ER79, the user: "KH1 Sora's vertices sometimes stretch ... when running") THE KINEDRIVER'S SCALE STAYS OUT OF
            // THE SOLVER. KH1's own driver squashes its shorts' cloth bones (L/R_zubonC2..G4: x scale down to 0.01 while the
            // thigh is raised); the solver keeps, from its 30 Hz evaluation, each bone's change against the animated local
            // transform (scale included: sim / animated) and applies it to the next frames' animated locals, so a change
            // taken at x 0.01 and applied at x 0.64 scaled the bone by ~64 (the hem's E2 / E3 thrown 2-3 m for two frames,
            // replayed offline: kh-anim tests/outfit_stand.rs replay_outfit_sim). The solver gets every bone at unit scale;
            // the driver's scale goes back on afterwards (it only shapes the skin of its own bone, as the driver means it).
            let scaled: Vec<(usize, Vec3)> = comps.iter().enumerate().filter(|(_, t)| (t.scale - Vec3::ONE).abs().max_element() > 1.0e-4).map(|(k, t)| (k, t.scale)).collect();
            for (k, _) in &scaled {
                comps[*k].scale = Vec3::ONE;
            }
            let solver = &mut s.solver;
            let stepped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                solver.frame(dt, &mut comps, component_to_world, &kh_anim::BonamikHost::default());
            }));
            for (k, scale) in &scaled {
                comps[*k].scale = *scale;
            }
            if stepped.is_err() {
                s.broken = true;
                comps = before;
            }
            // (ER69) a step that left a bone non-finite (an exploded solver): the pose before, the sim stopped
            if !s.broken && comps.iter().any(|t| !t.translation.is_finite() || !t.rotation.is_finite()) {
                s.broken = true;
                comps = before_check(&self.pose_full(local, component).1);
            }
            let moved = comps.iter().zip(&self.pose_full(local, component).1).filter(|(a, b)| (a.translation - b.translation).length() > 0.05).count();
            if let Ok(mut r) = SIM_REPORT.lock() {
                *r = format!("outfit Bonamik: {} ({} bones off the retargeted pose; fallback branches {:#x})", if s.broken { "STOPPED" } else { "running" }, moved, kh_anim::bonamik::UNPORTED.load(std::sync::atomic::Ordering::Relaxed));
            }
        }
        self.unclip(&mut comps);
        let posed: Posed = std::sync::Arc::new((locals, comps));
        s.cached = Some((clock, posed.clone()));
        posed
    }

    /// (ER72) THE POSE AN EFFECT HANGS ON with this outfit on: every bone of Sora's skeleton the outfit has takes the
    /// outfit's bone (retargeted, without its own Bonamik), its scale times the body scale; the others his. In his
    /// skeleton's order (`skeleton` = his).
    pub fn effect_component(&self, skeleton: &kh_anim::Skeleton, local: &[Trs], component: &[Trs]) -> Vec<Trs> {
        let (_, comps) = self.pose_full(local, component);
        let mut out = component.to_vec();
        for (k, bone) in skeleton.bones.iter().enumerate() {
            if let (Some(j), Some(t)) = (self.index_of(&bone.name), out.get_mut(k)) {
                if let Some(c) = comps.get(j) {
                    // (ER73) the Keyblade is drawn at its full size in the outfit's hand (`weapon_to_world`): the weapon
                    // sockets' bones keep their scale, so the trail and the blade's sockets lie along the drawn blade
                    let weapon = WEAPON_BONES.contains(&bone.name.as_str());
                    *t = Trs { scale: if weapon { c.scale } else { c.scale * self.scale }, ..*c };
                }
            }
        }
        out
    }

    /// Whether the outfit has a Bonamik of its own.
    pub fn simulated(&self) -> bool {
        self.sim.is_some()
    }

    pub fn pose(&self, local: &[Trs], component: &[Trs]) -> Vec<Trs> {
        self.pose_full(local, component).1
    }

    /// The outfit's bones relative to their parents AND in component space (bone-driven material parameters read both).
    pub fn pose_full(&self, local: &[Trs], component: &[Trs]) -> (Vec<Trs>, Vec<Trs>) {
        self.pose_full_at(local, component, None)
    }

    /// (ER83) `pose_full` of the drawn frame at `clock` (the hand IK's weights step once per clock value).
    fn pose_full_at(&self, local: &[Trs], component: &[Trs], clock: Option<f32>) -> (Vec<Trs>, Vec<Trs>) {
        let made;
        let local = if local.len() == self.host_reference.len() {
            local
        } else {
            made = self.local_from_component(component);
            &made
        };
        let mut out: Vec<Trs> = Vec::with_capacity(self.bones.len());
        let mut locals: Vec<Trs> = Vec::with_capacity(self.bones.len());
        for b in &self.bones {
            let l = match b.host.filter(|_| !b.own_rest).and_then(|h| Some((local.get(h)?, self.host_reference.get(h)?))) {
                Some((now, rest)) => {
                    let ratio = |a: f32, c: f32| if c.abs() > 1.0e-6 { a / c } else { 1.0 };
                    Trs {
                        translation: b.reference.translation + (now.translation - rest.translation) * self.scale,
                        rotation: (now.rotation * rest.rotation.inverse() * b.reference.rotation).normalize(),
                        scale: b.reference.scale * Vec3::new(ratio(now.scale.x, rest.scale.x), ratio(now.scale.y, rest.scale.y), ratio(now.scale.z, rest.scale.z)),
                    }
                }
                None => b.reference,
            };
            let c = match b.parent.and_then(|p| out.get(p)) {
                Some(parent) => l.then(parent),
                None => l,
            };
            out.push(c);
            locals.push(l);
        }
        self.drive(&mut locals, &mut out);
        // (ER83) its hands on the Keyblade and on its body where his are (after the retarget and its KineDriver, before
        // its Bonamik and the keychain)
        if let Some(ik) = self.hand_ik.as_ref().filter(|_| crate::handik::enabled() && component.len() == self.host_reference.len()) {
            ik.apply_at(component, &mut locals, &mut out, clock);
        }
        (locals, out)
    }

    /// (ER73) The anti-clip pass: each rule's simulated bones out of its capsules (after the Bonamik step).
    fn unclip(&self, comps: &mut [Trs]) {
        for (bones, capsules) in &self.clip {
            for &k in bones {
                let Some(mut p) = comps.get(k).map(|t| t.translation) else { continue };
                for c in capsules {
                    p = c.push(comps, p);
                }
                if p.is_finite() {
                    let moved = (p - comps[k].translation).length();
                    if moved > 0.0 {
                        if let Ok(mut m) = UNCLIP_MAX.lock() {
                            *m = m.max(moved);
                        }
                    }
                    comps[k].translation = p;
                }
            }
        }
    }

    /// (ER73) Bones kept out of the body by the anti-clip pass.
    pub fn unclipped(&self) -> usize {
        self.clip.iter().map(|(b, _)| b.len()).sum()
    }

    /// (ER73) Whether the outfit's own KineDriver runs on its pose.
    pub fn driven(&self) -> bool {
        self.kine.as_ref().is_some_and(|k| k.0.lock().map(|g| !g.1).unwrap_or(false))
    }

    /// (ER73) THE OUTFIT'S KINEDRIVER on its component-space pose, after the retarget (as KH3 runs a pawn's driver after
    /// its anim graph); the bones it writes get their locals back from it. Never stops the frame: a panic or a non-finite
    /// bone switches the driver off for this outfit (the pose without it).
    fn drive(&self, locals: &mut [Trs], comps: &mut [Trs]) {
        let Some(kine) = &self.kine else { return };
        let Ok(mut g) = kine.0.lock() else { return };
        if g.1 {
            return;
        }
        let before = comps.to_vec();
        let driver = &mut g.0;
        let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| driver.evaluate(comps, locals)));
        let written: Vec<usize> = g.0.written().to_vec();
        if ran.is_err() || written.iter().any(|&k| comps.get(k).is_some_and(|t| !t.translation.is_finite() || !t.rotation.is_finite() || !t.scale.is_finite())) {
            g.1 = true;
            comps.copy_from_slice(&before);
            return;
        }
        for k in written {
            if let (Some(c), Some(l)) = (comps.get(k).copied(), locals.get_mut(k)) {
                *l = match self.bones[k].parent.and_then(|p| comps.get(p)) {
                    Some(parent) => c.relative_to(parent),
                    None => c,
                };
            }
        }
    }

    fn local_from_component(&self, component: &[Trs]) -> Vec<Trs> {
        (0..self.host_reference.len())
            .map(|i| match (component.get(i), self.host_parent[i].and_then(|p| component.get(p))) {
                (Some(c), Some(p)) => {
                    let inv = p.rotation.inverse();
                    let ratio = |a: f32, b: f32| if b.abs() > 1.0e-6 { a / b } else { 1.0 };
                    Trs { translation: inv * (c.translation - p.translation) / p.scale, rotation: (inv * c.rotation).normalize(), scale: Vec3::new(ratio(c.scale.x, p.scale.x), ratio(c.scale.y, p.scale.y), ratio(c.scale.z, p.scale.z)) }
                }
                (Some(c), None) => *c,
                _ => self.host_reference[i],
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Quat;

    fn skel(json: &str) -> kh_anim::Skeleton {
        kh_anim::Skeleton::from_manifest(&kh_anim::json::Json::parse(json).unwrap()).unwrap()
    }

    /// Sora: root -> arm (10 cm out). The outfit: root -> arm (14 cm out, turned 10 degrees) -> cape (an extra bone).
    fn pair() -> (kh_anim::Skeleton, kh_anim::Skeleton) {
        let host = skel(r#"{"skeleton":{"bones":[
            {"name":"root","parent":-1,"translation_ue":[0,0,0],"rotation_ue":[0,0,0,1],"scale_ue":[1,1,1]},
            {"name":"arm","parent":0,"translation_ue":[10,0,0],"rotation_ue":[0,0,0,1],"scale_ue":[1,1,1]}]}}"#);
        let q = Quat::from_rotation_z(10f32.to_radians());
        let outfit = skel(&format!(
            r#"{{"skeleton":{{"bones":[
            {{"name":"root","parent":-1,"translation_ue":[0,0,0],"rotation_ue":[0,0,0,1],"scale_ue":[1,1,1]}},
            {{"name":"arm","parent":0,"translation_ue":[14,0,0],"rotation_ue":[{},{},{},{}],"scale_ue":[1,1,1]}},
            {{"name":"cape","parent":1,"translation_ue":[0,5,0],"rotation_ue":[0,0,0,1],"scale_ue":[1,1,1]}}]}}}}"#,
            q.x, q.y, q.z, q.w
        ));
        (host, outfit)
    }

    fn chain(skeleton: &kh_anim::Skeleton, local: &[Trs]) -> Vec<Trs> {
        let mut out: Vec<Trs> = Vec::new();
        for (k, b) in skeleton.bones.iter().enumerate() {
            let t = match b.parent {
                Some(p) => local[k].then(&out[p]),
                None => local[k],
            };
            out.push(t);
        }
        out
    }

    #[test]
    fn in_soras_reference_the_outfit_is_in_its_own() {
        let (host, outfit) = pair();
        let r = Retarget::new(&outfit, &host);
        assert_eq!((r.len(), r.shared(), r.index_of("cape")), (3, 2, Some(2)));
        let local = host.reference_pose();
        let posed = r.pose(&local, &chain(&host, &local));
        let bind = chain(&outfit, &outfit.reference_pose());
        for (a, b) in posed.iter().zip(&bind) {
            assert!((a.translation - b.translation).length() < 1.0e-4 && a.rotation.dot(b.rotation).abs() > 0.99999, "{a:?} {b:?}");
        }
    }

    #[test]
    fn soras_motion_moves_the_outfit_and_its_extra_bones() {
        let (host, outfit) = pair();
        let r = Retarget::new(&outfit, &host);
        // his arm turns 90 degrees about Z and the root moves 3 cm up
        let mut local = host.reference_pose();
        local[0].translation = Vec3::new(0.0, 0.0, 3.0);
        local[1].rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let posed = r.pose(&local, &chain(&host, &local));
        // the outfit's arm: its own length (14), turned 90 more degrees than its own 10, the root's lift
        assert!((posed[1].translation - Vec3::new(14.0, 0.0, 3.0)).length() < 1.0e-4);
        let want = Quat::from_rotation_z(100f32.to_radians());
        assert!(posed[1].rotation.dot(want).abs() > 0.99999);
        // the cape keeps its place on the arm
        assert!((posed[2].translation - (posed[1].translation + want * Vec3::new(0.0, 5.0, 0.0))).length() < 1.0e-4);
        // the same with no local pose given: made from the component pose
        let again = r.pose(&[], &chain(&host, &local));
        assert!((again[2].translation - posed[2].translation).length() < 1.0e-3);
    }

    /// KH1 Sora is shorter: his hips' bob is scaled by the hip-height ratio (feet stay on the floor).
    #[test]
    fn a_shorter_body_moves_its_hips_by_its_scale() {
        let host = skel(r#"{"skeleton":{"bones":[
            {"name":"root","parent":-1,"translation_ue":[0,0,0],"rotation_ue":[0,0,0,1],"scale_ue":[1,1,1]},
            {"name":"koshi","parent":0,"translation_ue":[0,0,100],"rotation_ue":[0,0,0,1],"scale_ue":[1,1,1]}]}}"#);
        let outfit = skel(r#"{"skeleton":{"bones":[
            {"name":"root","parent":-1,"translation_ue":[0,0,0],"rotation_ue":[0,0,0,1],"scale_ue":[1,1,1]},
            {"name":"koshi","parent":0,"translation_ue":[0,0,80],"rotation_ue":[0,0,0,1],"scale_ue":[1,1,1]}]}}"#);
        let r = Retarget::new(&outfit, &host);
        assert!((r.scale - 0.8).abs() < 1.0e-6);
        // he crouches 10 cm: the outfit's hips go down 8
        let mut local = host.reference_pose();
        local[1].translation.z = 90.0;
        let posed = r.pose(&local, &chain(&host, &local));
        assert!((posed[1].translation.z - 72.0).abs() < 1.0e-4, "{:?}", posed[1]);
        // the same skeleton: scale 1
        assert_eq!(Retarget::new(&host, &host).scale, 1.0);
    }

    /// (ER72) A bone his Bonamik moves and the outfit's own data leaves still (Toy Box's hair) keeps the outfit's pose.
    #[test]
    fn a_bone_the_outfits_data_leaves_still_does_not_take_his_motion() {
        let (host, outfit) = pair();
        let mut r = Retarget::new(&outfit, &host);
        let mut local = host.reference_pose();
        local[1].rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let moved = r.pose(&local, &chain(&host, &local));
        r.bones[1].own_rest = true;
        let still = r.pose(&local, &chain(&host, &local));
        let bind = chain(&outfit, &outfit.reference_pose());
        assert!(moved[1].rotation.dot(bind[1].rotation).abs() < 0.99);
        assert!(still[1].rotation.dot(bind[1].rotation).abs() > 0.99999);
    }

    #[test]
    fn the_keyblade_moves_to_the_outfits_hand() {
        let host = skel(r#"{"skeleton":{"bones":[
            {"name":"root","parent":-1,"translation_ue":[0,0,0],"rotation_ue":[0,0,0,1],"scale_ue":[1,1,1]},
            {"name":"R_buki","parent":0,"translation_ue":[10,0,0],"rotation_ue":[0,0,0,1],"scale_ue":[1,1,1]}]}}"#);
        let outfit = skel(r#"{"skeleton":{"bones":[
            {"name":"root","parent":-1,"translation_ue":[0,0,0],"rotation_ue":[0,0,0,1],"scale_ue":[1,1,1]},
            {"name":"R_buki","parent":0,"translation_ue":[14,0,0],"rotation_ue":[0,0,0,1],"scale_ue":[1,1,1]}]}}"#);
        let r = Retarget::new(&outfit, &host);
        let local = host.reference_pose();
        let component = chain(&host, &local);
        let posed = r.pose(&local, &component);
        let c2w = Trs { translation: Vec3::new(100.0, 0.0, 0.0), rotation: Quat::from_rotation_z(std::f32::consts::FRAC_PI_2), scale: Vec3::ONE };
        // the weapon 2 cm past his hand: it goes 2 cm past the outfit's (4 cm further out along his turned X = world +Y)
        let socket = Trs { translation: Vec3::new(2.0, 0.0, 0.0), rotation: Quat::IDENTITY, scale: Vec3::ONE };
        let weapon = socket.then(&component[1]).then(&c2w);
        let moved = r.weapon_to_world(&weapon, &component, &posed, &c2w);
        assert!((moved.translation - (weapon.translation + Vec3::new(0.0, 4.0, 0.0))).length() < 1.0e-3, "{moved:?}");
        // a weapon far from both hands (thrown) is left alone
        let far = Trs { translation: Vec3::new(500.0, 0.0, 0.0), ..weapon };
        assert_eq!(r.weapon_to_world(&far, &component, &posed, &c2w).translation, far.translation);
    }

    /// (ER73, the user: "effects for keyblade need to match for KH1 sora's keyblade position") A shorter outfit (body
    /// scale 0.8): the effects' weapon sockets on `effect_component`'s hand land where the drawn Keyblade's do
    /// (`weapon_to_world`), the blade's tip too (the weapon's bone keeps its scale).
    #[test]
    fn the_keyblades_effect_sockets_follow_the_drawn_keyblade() {
        let host = skel(r#"{"skeleton":{"bones":[
            {"name":"root","parent":-1,"translation_ue":[0,0,0],"rotation_ue":[0,0,0,1],"scale_ue":[1,1,1]},
            {"name":"koshi","parent":0,"translation_ue":[0,0,100],"rotation_ue":[0,0,0,1],"scale_ue":[1,1,1]},
            {"name":"R_buki","parent":1,"translation_ue":[30,0,20],"rotation_ue":[0,0,0,1],"scale_ue":[1,1,1]}]}}"#);
        let outfit = skel(r#"{"skeleton":{"bones":[
            {"name":"root","parent":-1,"translation_ue":[0,0,0],"rotation_ue":[0,0,0,1],"scale_ue":[1,1,1]},
            {"name":"koshi","parent":0,"translation_ue":[0,0,80],"rotation_ue":[0,0,0,1],"scale_ue":[1,1,1]},
            {"name":"R_buki","parent":1,"translation_ue":[24,0,16],"rotation_ue":[0,0,0,1],"scale_ue":[1,1,1]}]}}"#);
        let r = Retarget::new(&outfit, &host);
        assert!((r.scale - 0.8).abs() < 1.0e-6);
        let mut local = host.reference_pose();
        local[1].rotation = Quat::from_rotation_z(0.7);
        let component = chain(&host, &local);
        let posed = r.pose(&local, &component);
        let c2w = Trs { translation: Vec3::new(100.0, 50.0, 0.0), rotation: Quat::from_rotation_z(1.2), scale: Vec3::ONE };
        let effect = r.effect_component(&host, &local, &component);
        // the weapon's root on his hand (R_wep_skt), moved to the outfit's; a socket of the blade (its tip 60 cm out) on it
        let root = r.weapon_to_world(&component[2].then(&c2w), &component, &posed, &c2w);
        for tip in [0.0, 60.0] {
            let socket = Trs { translation: Vec3::new(tip, 0.0, 0.0), rotation: Quat::IDENTITY, scale: Vec3::ONE };
            let drawn = socket.then(&root);
            let effect_socket = socket.then(&effect[2]).then(&c2w);
            assert!((drawn.translation - effect_socket.translation).length() < 1.0e-3, "tip {tip}: {drawn:?} {effect_socket:?}");
        }
        // a body bone's effect shrinks with him
        assert!((effect[1].scale.x - 0.8).abs() < 1.0e-6);
    }

    /// (ER73) THE OUTFITS' OWN KINEDRIVER on the pack's data: in Sora's reference pose every bone it writes is where the
    /// outfit's bind pose has it (the helpers' rules give back the rest pose at rest), and nothing is non-finite.
    #[test]
    fn the_outfits_kinedriver_gives_back_its_bind_pose_at_rest() {
        let pack = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../build/sora_pack"));
        let Ok(text) = std::fs::read_to_string(pack.join("manifest.json")) else { return };
        let host = kh_anim::Skeleton::from_manifest(&kh_anim::json::Json::parse(&text).unwrap()).unwrap();
        let mut report = Vec::new();
        for id in ["kh2", "kh1", "toybox", "monstropolis", "caribbean", "sanfransokyo"] {
            if !pack.join("outfits").join(id).join("kinedriver.json").is_file() {
                continue;
            }
            let r = retarget_for(pack, id, &host).unwrap().unwrap();
            assert!(r.driven(), "{id}: its KineDriver is loaded");
            let text = std::fs::read_to_string(pack.join("outfits").join(id).join("manifest.json")).unwrap();
            let outfit = kh_anim::Skeleton::from_manifest(&kh_anim::json::Json::parse(&text).unwrap()).unwrap();
            let local = host.reference_pose();
            let (_, posed) = r.pose_full(&local, &chain(&host, &local));
            let bind = chain(&outfit, &outfit.reference_pose());
            // (the bones its Bonamik moves are left out: the simulation has the last word on them)
            let sim: std::collections::HashSet<String> = std::fs::read_to_string(pack.join("outfits").join(id).join("bonamik.json"))
                .ok()
                .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
                .map(|j| j["bones"].as_array().into_iter().flatten().filter(|b| b["bonamik"].as_str() != Some("none")).filter_map(|b| b["name"].as_str().map(str::to_string)).collect())
                .unwrap_or_default();
            let written: Vec<usize> = r.kine.as_ref().unwrap().0.lock().unwrap().0.written().iter().copied().filter(|&k| !sim.contains(&outfit.bones[k].name)).collect();
            let (mut dt, mut da, mut worst) = (0.0f32, 0.0f32, String::new());
            for &k in &written {
                let (a, b) = (posed[k], bind[k]);
                assert!(a.translation.is_finite() && a.rotation.is_finite(), "{id}: {} non-finite", outfit.bones[k].name);
                let t = (a.translation - b.translation).length();
                let g = a.rotation.angle_between(b.rotation).to_degrees();
                if t > dt || g > da {
                    worst = outfit.bones[k].name.clone();
                }
                dt = dt.max(t);
                da = da.max(g);
            }
            assert!(r.driven(), "{id}: its KineDriver stopped");
            report.push(format!("{id}: {} bones, at most {dt:.2} cm / {da:.1} deg off the bind pose ({worst})", written.len()));
            // (Sora's own driver turns his shoulder pieces ~8 degrees off their bind at rest too, L_kata_a_spo: the same
            // operator in Monstropolis's asset; Toy Box's aim / position constraints, the reconstructed ones, must be exact)
            let turn = if id == "toybox" { 1.0 } else { 10.0 };
            assert!(dt < 1.0 && da < turn, "{}", report.join("; "));
        }
        eprintln!("{}", report.join("
"));
    }

    /// (ER73) The anti-clip capsule: a point inside goes to its surface (radius tapered along it), one outside stays.
    #[test]
    fn a_simulated_bone_is_kept_out_of_the_body() {
        let comps = [Trs { translation: Vec3::new(0.0, 0.0, 100.0), ..Trs::IDENTITY }, Trs { translation: Vec3::new(0.0, 0.0, 60.0), ..Trs::IDENTITY }];
        let c = Capsule { a: 0, b: 1, t: (0.0, 1.0), r: (10.0, 14.0) };
        // halfway down (radius 12), 5 cm out: pushed to 12
        assert!((c.push(&comps, Vec3::new(5.0, 0.0, 80.0)) - Vec3::new(12.0, 0.0, 80.0)).length() < 1.0e-4);
        let out = Vec3::new(20.0, 0.0, 80.0);
        assert_eq!(c.push(&comps, out), out);
        // the pack's rules: KH1's belt chain, Monstropolis's tail
        let pack = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../build/sora_pack"));
        let Ok(text) = std::fs::read_to_string(pack.join("manifest.json")) else { return };
        let host = kh_anim::Skeleton::from_manifest(&kh_anim::json::Json::parse(&text).unwrap()).unwrap();
        for (id, at_least) in [("kh1", 10), ("monstropolis", 2)] {
            if pack.join("outfits").join(id).join("anticlip.json").is_file() {
                let r = retarget_for(pack, id, &host).unwrap().unwrap();
                assert!(r.unclipped() >= at_least, "{id}: {} bones", r.unclipped());
            }
        }
    }

    #[test]
    fn the_wanted_outfit_and_the_loaded_one() {
        set_wanted("kh3");
        assert!(wanted().is_empty() && is_main(&wanted()));
        set_wanted("kh2");
        assert!(change_wanted());
        let id = begin_load();
        end_load(&id, true);
        assert!(!change_wanted() && loaded() == "kh2");
        set_wanted("nope");
        end_load("nope", false);
        assert!(!change_wanted(), "a failed outfit is not tried again");
        set_wanted("");
        assert!(change_wanted());
        end_load("", true);
        let pack = Path::new("Z:/no/pack");
        assert_eq!(look_dir(pack, "kh2"), pack.to_path_buf());
        assert_eq!(model_file(pack, ""), pack.join("sora.glb"));
    }

    /// (ER79) KH1's KineDriver squashes its shorts' cloth bones (x scale down to 0.01) while a thigh is raised; the outfit's
    /// Bonamik, given that scale, threw the hem's bones 2-3 m for two frames in a run (the user: "vertices sometimes
    /// stretch ... when running"). With the scale kept out of the solver, no simulated bone leaves its animated place by
    /// more than a cloth's swing over two run loops (the shorts' bones; the 2-3 m throw itself is replayed from a harness dump
    /// by kh-anim tests/outfit_stand.rs replay_outfit_sim).
    #[test]
    fn the_kinedrivers_scale_does_not_throw_the_shorts_in_a_run() {
        let pack = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../build/sora_pack"));
        if !pack.join("outfits").join("kh1").join("bonamik.json").is_file() {
            return;
        }
        let animator = kh_anim::Animator::new(kh_anim::Pack::load(pack).unwrap(), kh_anim::Settings::default()).unwrap();
        let host = animator.skeleton().clone();
        let r = retarget_for(pack, "kh1", &host).unwrap().unwrap();
        assert!(r.simulated() && r.driven());
        let length = animator.clip_info("0003").unwrap().length;
        let to_world = Trs::IDENTITY;
        let mut worst = (0.0f32, String::new());
        let names = std::fs::read_to_string(pack.join("outfits/kh1/manifest.json")).unwrap();
        let outfit = kh_anim::Skeleton::from_manifest(&kh_anim::json::Json::parse(&names).unwrap()).unwrap();
        for step in 0..240 {
            let t = (step as f32 / 60.0) % length;
            let local = animator.sample("0003", t);
            let component = chain(&host, &local);
            let posed = r.frame(step as f32 / 60.0, &to_world, &local, &component);
            let animated = r.pose(&local, &component);
            for (k, (a, b)) in posed.1.iter().zip(&animated).enumerate().filter(|(k, _)| outfit.bones[*k].name.contains("zubon")) {
                let d = (a.translation - b.translation).length();
                if d > worst.0 {
                    worst = (d, format!("{} at step {step}", outfit.bones[k].name));
                }
            }
        }
        println!("kh1 in a run: the farthest a bone left its animated place: {:.1} cm ({})", worst.0, worst.1);
        // (without the fix: R_zubonE4 39.8 cm; with it 18.3 cm)
        assert!(worst.0 < 25.0, "a shorts bone thrown {:.1} cm ({})", worst.0, worst.1);
    }

    /// (ER79) The leg guard goes into the palette's last six entries: capsules (pack space) in [314..317], the cloth bits in
    /// [318], (-, count, -, 1) in [319] column 0; nothing for a skin of more than 314 joints.
    #[test]
    fn the_leg_guard_fills_the_palettes_last_entries() {
        let (host, outfit) = pair();
        let mut r = Retarget::new(&outfit, &host);
        r.guard = Some(LegGuard { capsules: vec![Capsule { a: 0, b: 1, t: (0.0, 1.0), r: (3.0, 2.0) }], cloth: ["cape".to_string()].into_iter().collect(), flags: [0; 16], joints: 0 });
        let joints = vec![("root".to_string(), glam::Mat4::IDENTITY), ("arm".to_string(), glam::Mat4::IDENTITY), ("cape".to_string(), glam::Mat4::IDENTITY)];
        let r = r.with_guard_joints(&joints);
        assert!(r.guard.as_ref().unwrap().is_cloth(2) && !r.guard.as_ref().unwrap().is_cloth(1));
        let local = host.reference_pose();
        let posed = r.pose(&local, &chain(&host, &local));
        let mut skin = vec![0.0f32; 16 * (1 + 320)];
        r.write_leg_guard(&mut skin, &posed);
        let entry = |k: usize| 16 * (1 + k);
        assert_eq!(&skin[entry(319)..entry(319) + 4], &[0.0, 1.0, 0.0, 1.0]);
        assert_eq!(skin[entry(318)].to_bits(), 0b100);
        let caps = r.guard_capsules_pack(&posed);
        assert_eq!(&skin[entry(314)..entry(314) + 4], &[caps[0].0.x, caps[0].0.y, caps[0].0.z, caps[0].1]);
        // 3 cm and 2 cm in pack units (metres)
        assert!((caps[0].1 - 0.03).abs() < 1.0e-5 && (caps[0].3 - 0.02).abs() < 1.0e-5, "{caps:?}");
        // a skin too large for the guard: untouched
        let big: Vec<(String, glam::Mat4)> = (0..315).map(|k| (format!("j{k}"), glam::Mat4::IDENTITY)).collect();
        let r = r.with_guard_joints(&big);
        let mut skin = vec![0.0f32; 16 * (1 + 320)];
        r.write_leg_guard(&mut skin, &posed);
        assert!(skin.iter().all(|x| *x == 0.0));
    }

    /// (ER79) twosided.json marks its (opaque / masked) slots two-sided with the interior factor; off or not enabled: nothing.
    #[test]
    fn the_cloth_slots_of_twosided_json_are_drawn_from_both_sides() {
        let pack = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../build/sora_pack"));
        let Ok(mut library) = kh_look::library::Library::load(&pack.join("outfits").join("kh1")) else { return };
        let dir = std::env::temp_dir().join(format!("er79_twosided_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("twosided.json"), r#"{"enabled":true,"interior":0.4,"slots":["p_ex005_mat0","p_ex005_mat1","p_ex005_E_mat2"]}"#).unwrap();
        let before: Vec<bool> = library.slots.iter().map(|s| s.two_sided).collect();
        set_cloth_twosided(false);
        assert_eq!(apply_cloth_twosided(&dir, &mut library), 0);
        set_cloth_twosided(true);
        let n = apply_cloth_twosided(&dir, &mut library);
        // the translucent E_mat2 is left alone
        assert_eq!(n, 2);
        for (k, slot) in library.slots.iter().enumerate() {
            let listed = slot.slot == "p_ex005_mat0" || slot.slot == "p_ex005_mat1";
            assert_eq!(slot.two_sided, before[k] || listed, "{}", slot.slot);
            assert_eq!(slot.scalars.get(INTERIOR_SCALAR).copied(), listed.then_some(0.4), "{}", slot.slot);
        }
        let _ = std::fs::remove_file(dir.join("twosided.json"));
        let _ = std::fs::remove_dir(&dir);
    }
}
