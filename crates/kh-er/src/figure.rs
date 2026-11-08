//! (IMPL_ER16) THE FIGURE A REAL-TIME CUTSCENE DRAWS IN THE PLAYER'S PLACE, found, hidden and read.
//!
//! WHAT IT IS (MEASURED, IMPL_ER16_STATE.md): not a character. The cutscene manager's environment (`CSRemoEnv`,
//! reached from `CSRemoMan + 0x150`, which points at the environment's scene draw parameters at `+0x9A8`) keeps the
//! scene's acts in a vector at `+0xED0 / +0xED8`. The `CSRemoAnimAct`s named `c0000_NNNN` animate a `CSViewerModelIns`
//! named `c0000` (`act + 0x5D8`) with the player's equipment assembled on it by a `CSChrAsmModelIns` of the scene's own
//! (`act + 0x5E8`: `+0x08` the viewer model, `+0x18` its `FgModel` (face, hands, skin), `+0x28` 27 part models
//! `CSPartsModelIns`). Both act fields are zero while the act's cut is not playing.
//!
//! Every read goes through `probe::read` (the system copies or refuses: a freed object cannot fault).

use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

use crate::probe::{class_of, dl_string, read, u32_at, word};

const ENV_FROM_MAN: usize = 0x150;
const ENV_DRAW_PARAM: usize = 0x9A8;
const ENV_ACTS: usize = 0xED0;
const ACT_NAME: usize = 0xB0;
const ACT_MODEL: usize = 0x5D8;
const ACT_ASM: usize = 0x5E8;
const ASM_MODEL: usize = 0x08;
const ASM_FG: usize = 0x18;
const ASM_PARTS: usize = 0x28;
const PARTS: usize = 27;
const MODEL_DISP: usize = 0x18;
const DISP_FLAGS: usize = 0x20;

/// The vtables of the classes compared every frame, learnt once by their RTTI names (0 = not yet).
static VT_ANIM_ACT: AtomicUsize = AtomicUsize::new(0);
static VT_ASM: AtomicUsize = AtomicUsize::new(0);
static VT_ENV: AtomicUsize = AtomicUsize::new(0);

fn is(p: usize, known: &AtomicUsize, class: &str) -> bool {
    let Some(vt) = word(p) else { return false };
    let have = known.load(Relaxed);
    if have != 0 {
        return vt == have;
    }
    if class_of(p).is_some_and(|c| c == class) {
        known.store(vt, Relaxed);
        return true;
    }
    false
}

/// The cutscene manager's environment (present while a scene is loaded).
pub fn env() -> Option<usize> {
    use fromsoftware_shared::FromStatic;
    // SAFETY: the game's singleton; one pointer read.
    let remo = unsafe { eldenring::cs::CSRemo::instance() }.ok()?;
    let man = word(remo as *const _ as usize + 8)?;
    let env = word(man + ENV_FROM_MAN)?.checked_sub(ENV_DRAW_PARAM)?;
    (is(env, &VT_ENV, "CSRemoEnv@CS") && word(env + 0x10) == Some(man)).then_some(env)
}

/// The scene's acts (pointers), at most 4096.
pub fn acts(env: usize) -> Vec<usize> {
    let (Some(begin), Some(end)) = (word(env + ENV_ACTS), word(env + ENV_ACTS + 8)) else { return Vec::new() };
    if begin == 0 || end < begin || (end - begin) % 8 != 0 || end - begin > 8 * 4096 {
        return Vec::new();
    }
    let mut bytes = vec![0u8; end - begin];
    if !read(begin, &mut bytes) {
        return Vec::new();
    }
    bytes.as_chunks::<8>().0.iter().map(|c| usize::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]])).collect()
}

/// A UTF-16 name at a pointer (the act's object name, e.g. `c0000_0000`).
fn wide(at: usize) -> Option<String> {
    let mut b = [0u8; 40];
    if !read(at, &mut b) {
        return None;
    }
    let units: Vec<u16> = b.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|u| *u != 0).collect();
    let s = String::from_utf16(&units).ok()?;
    (!s.is_empty() && s.chars().all(|c| c.is_ascii_graphic())).then_some(s)
}

/// The figure of the running scene.
#[derive(Clone, Debug, PartialEq)]
pub struct Figure {
    /// The act that animates it now.
    pub act: usize,
    /// `CSViewerModelIns` "c0000".
    pub model: usize,
    /// `CSChrAsmModelIns`.
    pub asm: usize,
    /// `FgModel` (0 = none).
    pub fg: usize,
    /// The part models that exist.
    pub parts: Vec<usize>,
}

/// The acts of the scene that can animate the player's figure (named `c0000..`): looked up once per scene.
pub fn player_acts(env: usize) -> Vec<usize> {
    acts(env).into_iter().filter(|a| is(*a, &VT_ANIM_ACT, "CSRemoAnimAct@CS") && word(a + ACT_NAME).and_then(wide).is_some_and(|n| n.starts_with("c0000"))).collect()
}

/// (IMPL_ER33, cutgrace.rs) The scene's animation acts whose object name begins with one of `prefixes` (e.g.
/// `AEG099_060`, a site of grace): (the act, its name). Looked up a few times per scene, not every frame.
pub fn named_acts(env: usize, prefixes: &[String]) -> Vec<(usize, String)> {
    acts(env).into_iter().filter(|a| is(*a, &VT_ANIM_ACT, "CSRemoAnimAct@CS")).filter_map(|a| word(a + ACT_NAME).and_then(wide).filter(|n| prefixes.iter().any(|p| n.starts_with(p.as_str()))).map(|n| (a, n))).collect()
}

/// (IMPL_ER33) Every animation act's object name of the scene (for the log: what a scene is made of).
pub fn act_names(env: usize) -> Vec<String> {
    acts(env).into_iter().filter(|a| is(*a, &VT_ANIM_ACT, "CSRemoAnimAct@CS")).filter_map(|a| word(a + ACT_NAME).and_then(wide)).collect()
}

/// (IMPL_ER33) The display flag word of the model an act animates now (0 while the act's cut is not playing): the
/// model's display entity is checked by its class the first time.
pub fn act_flag_word(act: usize) -> Option<usize> {
    // (the display entity's class is looked up by name once per vtable: up to four kinds accepted, four turned down)
    static GOOD: [AtomicUsize; 4] = [AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0)];
    static BAD: [AtomicUsize; 4] = [AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0)];
    if !is(act, &VT_ANIM_ACT, "CSRemoAnimAct@CS") {
        return None;
    }
    let model = word(act + ACT_MODEL).filter(|m| *m != 0)?;
    let disp = word(model + MODEL_DISP).filter(|d| *d != 0)?;
    let vt = word(disp).filter(|v| *v != 0)?;
    if BAD.iter().any(|b| b.load(Relaxed) == vt) {
        return None;
    }
    if !GOOD.iter().any(|g| g.load(Relaxed) == vt) {
        let good = class_of(disp).is_some_and(|c| c.starts_with("CSModelDispEntity"));
        let list = if good { &GOOD } else { &BAD };
        match list.iter().find(|s| s.load(Relaxed) == 0) {
            Some(slot) => slot.store(vt, Relaxed),
            None if good => {}
            None => return None,
        }
        if !good {
            return None;
        }
    }
    Some(disp + DISP_FLAGS)
}

/// The figure, when one of the player's acts holds it now.
pub fn find(player_acts: &[usize]) -> Option<Figure> {
    for act in player_acts {
        if !is(*act, &VT_ANIM_ACT, "CSRemoAnimAct@CS") {
            continue;
        }
        let (Some(model), Some(asm)) = (word(act + ACT_MODEL), word(act + ACT_ASM)) else { continue };
        if model == 0 || asm == 0 || !is(asm, &VT_ASM, "CSChrAsmModelIns@CS") || word(asm + ASM_MODEL) != Some(model) {
            continue;
        }
        let mut raw = [0u8; PARTS * 8];
        if !read(asm + ASM_PARTS, &mut raw) {
            continue;
        }
        let parts = raw.as_chunks::<8>().0.iter().map(|c| usize::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]])).filter(|p| *p != 0).collect();
        return Some(Figure { act: *act, model, asm, fg: word(asm + ASM_FG).unwrap_or(0), parts });
    }
    None
}

/// The display flag words of everything the figure is drawn from (the viewer model and each part).
pub fn flag_words(f: &Figure) -> Vec<usize> {
    std::iter::once(f.model).chain(f.parts.iter().copied()).filter_map(|m| word(m + MODEL_DISP)).filter(|d| *d != 0).map(|d| d + DISP_FLAGS).collect()
}

/// (`dbg.fig`) What the scene holds: every act with a model, and the figure.
pub fn dump() {
    let Some(env) = env() else {
        crate::erlog!("fig: no cutscene environment");
        return;
    };
    let all = acts(env);
    crate::erlog!("fig: environment {env:#x}, {} act(s)", all.len());
    let mut shown = 0;
    for a in &all {
        if !is(*a, &VT_ANIM_ACT, "CSRemoAnimAct@CS") {
            continue;
        }
        let name = word(a + ACT_NAME).and_then(wide).unwrap_or_default();
        let (model, asm) = (word(a + ACT_MODEL).unwrap_or(0), word(a + ACT_ASM).unwrap_or(0));
        if (model != 0 || name.starts_with("c0000")) && shown < 80 {
            shown += 1;
            crate::erlog!("fig:   act {a:#x} {name} model {model:#x} {} {} asm {asm:#x} {}", class_of(model).unwrap_or_default(), word(model + 0x10).and_then(|i| dl_string(i + 0x6C0)).unwrap_or_default(), class_of(asm).unwrap_or_default());
        }
    }
    let mine = player_acts(env);
    match find(&mine) {
        Some(f) => crate::erlog!("fig: THE FIGURE: {f:x?}; flags {:x?}", flag_words(&f).iter().map(|w| u32_at(*w).unwrap_or(0)).collect::<Vec<_>>()),
        None => crate::erlog!("fig: no figure now ({} act(s) of the player's)", mine.len()),
    }
}

/// A UTF-16 name of any script at a pointer (FLVER node names are partly Japanese).
fn wide_any(at: usize) -> String {
    let mut b = [0u8; 96];
    if !read(at, &mut b) {
        return String::new();
    }
    let units: Vec<u16> = b.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|u| *u != 0).collect();
    String::from_utf16_lossy(&units)
}

fn f32s(at: usize, n: usize) -> Vec<f32> {
    let mut b = vec![0u8; n * 4];
    if !read(at, &mut b) {
        return Vec::new();
    }
    b.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
}

/// (`dbg.figskel = ADDR of a CSFD4ModelItem`) everything of a model's skeleton into a text file: the FLVER's nodes
/// (name, links, the two matrix tables), the location matrices (the pose), the exporter's raw words.
pub fn dump_skeleton(item: usize, out: &std::path::Path) {
    use std::fmt::Write as _;
    let mut s = String::new();
    let flv = word(item + 0x68).unwrap_or(0);
    let n = u32_at(flv + 0x18).unwrap_or(0).min(2048) as usize;
    let (nodes, ma, mb) = (word(flv + 0x20).unwrap_or(0), word(flv + 0x50).unwrap_or(0), word(flv + 0x70).unwrap_or(0));
    let _ = writeln!(s, "item {item:#x} name {:?} flver {flv:#x} nodes {n} at {nodes:#x} matrices {ma:#x} {mb:#x}", dl_string(item + 0x6C0));
    for k in 0..n {
        let e = nodes + k * 0x30;
        let mut raw = [0u8; 8];
        read(e + 0x28, &mut raw);
        let links: Vec<i16> = raw.as_chunks::<2>().0.iter().map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
        let _ = writeln!(s, "node {k} {:?} links {links:?} A {:?} B {:?}", wide_any(word(e + 0x20).unwrap_or(0)), f32s(ma + k * 0x30, 12), f32s(mb + k * 0x30, 12));
    }
    let loc = word(item + 0x640).unwrap_or(0);
    let (count, ptr) = (word(loc + 0x68).unwrap_or(0).min(2048), word(loc + 0x70).unwrap_or(0));
    let _ = writeln!(s, "location {loc:#x} {} count {count} at {ptr:#x}", class_of(loc).unwrap_or_default());
    for k in 0..count {
        let _ = writeln!(s, "pose {k} {:?}", f32s(ptr + k * 0x30, 12));
    }
    for (label, at, words) in [("location", loc, 40usize), ("exporter", word(item + 0x658).unwrap_or(0), 80), ("flver", flv, 200), ("item", item, 220)] {
        let _ = writeln!(s, "{label} {at:#x}:\n{}", crate::probe::peek_text(at, words, crate::probe::exe()));
    }
    let _ = std::fs::write(out, s);
    crate::erlog!("fig: skeleton of {item:#x} written to {}", out.display());
}
// ---------------------------------------------------------------- the figure's pose, shown on Sora

use glam::{Mat3, Quat, Vec3};
use kh_anim::retarget::{BodyFrame, MapRow, Retarget};
use kh_anim::Trs;

const fn row(source: &'static str, target: &'static str, aim: Option<(&'static str, &'static str)>) -> MapRow {
    MapRow { source, target, aim }
}

/// ELDEN RING'S PLAYER RIG (FLVER node names of c0000, MEASURED: er16/figskel_b.txt) -> SORA'S (`kh-anim` skeleton
/// names): the body's root, spine, neck / head, arms, legs and (IMPL_ER17) fingers. Face, helper bones and cloth keep Sora's own.
pub const ROWS: [MapRow; 52] = [
    row("Pelvis", "center", None),
    row("Spine", "hara1", Some(("Spine1", "hara2"))),
    row("Spine1", "hara2", Some(("Spine2", "mune"))),
    row("Spine2", "mune", Some(("Neck", "kubi"))),
    row("Neck", "kubi", Some(("Head", "atama"))),
    row("Head", "atama", None),
    row("L_Clavicle", "L_kata", Some(("L_UpperArm", "L_ude1"))),
    row("L_UpperArm", "L_ude1", Some(("L_Forearm", "L_ude2"))),
    row("L_Forearm", "L_ude2", Some(("L_Hand", "L_te"))),
    row("L_Hand", "L_te", None),
    row("R_Clavicle", "R_kata", Some(("R_UpperArm", "R_ude1"))),
    row("R_UpperArm", "R_ude1", Some(("R_Forearm", "R_ude2"))),
    row("R_Forearm", "R_ude2", Some(("R_Hand", "R_te"))),
    row("R_Hand", "R_te", None),
    row("L_Thigh", "L_momo", Some(("L_Calf", "L_sune"))),
    row("L_Calf", "L_sune", Some(("L_Foot", "L_ashi1"))),
    row("L_Foot", "L_ashi1", Some(("L_Toe0", "L_ashi2"))),
    row("L_Toe0", "L_ashi2", None),
    row("R_Thigh", "R_momo", Some(("R_Calf", "R_sune"))),
    row("R_Calf", "R_sune", Some(("R_Foot", "R_ashi1"))),
    row("R_Foot", "R_ashi1", Some(("R_Toe0", "R_ashi2"))),
    row("R_Toe0", "R_ashi2", None),
    // (IMPL_ER17) the fingers: Finger0 = the thumb (yubiA) .. Finger4 = the little finger (yubiE), three joints each
    row("L_Finger0", "L_yubiA1", Some(("L_Finger01", "L_yubiA2"))),
    row("L_Finger01", "L_yubiA2", Some(("L_Finger02", "L_yubiA3"))),
    row("L_Finger02", "L_yubiA3", None),
    row("L_Finger1", "L_yubiB1", Some(("L_Finger11", "L_yubiB2"))),
    row("L_Finger11", "L_yubiB2", Some(("L_Finger12", "L_yubiB3"))),
    row("L_Finger12", "L_yubiB3", None),
    row("L_Finger2", "L_yubiC1", Some(("L_Finger21", "L_yubiC2"))),
    row("L_Finger21", "L_yubiC2", Some(("L_Finger22", "L_yubiC3"))),
    row("L_Finger22", "L_yubiC3", None),
    row("L_Finger3", "L_yubiD1", Some(("L_Finger31", "L_yubiD2"))),
    row("L_Finger31", "L_yubiD2", Some(("L_Finger32", "L_yubiD3"))),
    row("L_Finger32", "L_yubiD3", None),
    row("L_Finger4", "L_yubiE1", Some(("L_Finger41", "L_yubiE2"))),
    row("L_Finger41", "L_yubiE2", Some(("L_Finger42", "L_yubiE3"))),
    row("L_Finger42", "L_yubiE3", None),
    row("R_Finger0", "R_yubiA1", Some(("R_Finger01", "R_yubiA2"))),
    row("R_Finger01", "R_yubiA2", Some(("R_Finger02", "R_yubiA3"))),
    row("R_Finger02", "R_yubiA3", None),
    row("R_Finger1", "R_yubiB1", Some(("R_Finger11", "R_yubiB2"))),
    row("R_Finger11", "R_yubiB2", Some(("R_Finger12", "R_yubiB3"))),
    row("R_Finger12", "R_yubiB3", None),
    row("R_Finger2", "R_yubiC1", Some(("R_Finger21", "R_yubiC2"))),
    row("R_Finger21", "R_yubiC2", Some(("R_Finger22", "R_yubiC3"))),
    row("R_Finger22", "R_yubiC3", None),
    row("R_Finger3", "R_yubiD1", Some(("R_Finger31", "R_yubiD2"))),
    row("R_Finger31", "R_yubiD2", Some(("R_Finger32", "R_yubiD3"))),
    row("R_Finger32", "R_yubiD3", None),
    row("R_Finger4", "R_yubiE1", Some(("R_Finger41", "R_yubiE2"))),
    row("R_Finger41", "R_yubiE2", Some(("R_Finger42", "R_yubiE3"))),
    row("R_Finger42", "R_yubiE3", None),
];
/// (IMPL_ER17) The rows of each arm: upper bone, lower bone, hand (left, right).
pub const ARMS: [[usize; 3]; 2] = [[7, 8, 9], [11, 12, 13]];
/// (ER70) The rows of each leg: thigh, calf, foot (left, right): a planted foot is brought to its place (propfit.rs).
pub const LEGS: [[usize; 3]; 2] = [[14, 15, 16], [18, 19, 20]];
pub const BODY: BodyFrame = BodyFrame { pelvis: ("Pelvis", "koshi"), head: ("Head", "atama"), left_thigh: ("L_Thigh", "L_momo"), right_thigh: ("R_Thigh", "R_momo"), foot: ("L_Foot", "L_ashi1") };

/// An Elden Ring 3 x 4 matrix (rows, the translation in the fourth column; MEASURED) as place and rotation.
fn matrix(f: &[f32]) -> (Vec3, Quat) {
    let m = Mat3::from_cols(Vec3::new(f[0], f[4], f[8]).normalize_or_zero(), Vec3::new(f[1], f[5], f[9]).normalize_or_zero(), Vec3::new(f[2], f[6], f[10]).normalize_or_zero());
    (Vec3::new(f[3], f[7], f[11]), Quat::from_mat3(&m).normalize())
}

/// A rotation in Elden Ring's axes, in KH3's (kh = (er.z, er.x, er.y): the axis is relabelled the same way).
pub fn rotation_kh(q: Quat) -> Quat {
    Quat::from_xyzw(q.z, q.x, q.y, q.w)
}

/// A model's skeleton as far as the map needs it: the FLVER node of each row, the retargeting made from its bind pose.
pub struct Rig {
    pub flver: usize,
    nodes: Vec<usize>,
    pub retarget: Retarget,
    /// (ER party S2) The rows the retarget was made from (indices into `ROWS`) when the target skeleton lacks some
    /// (Donald and Goofy: four fingers, Goofy no `kubi`); None = all of them (Sora's, as always).
    #[cfg(feature = "party")]
    pub keep: Option<Vec<usize>>,
}

/// (ER party S2) THE ROWS A SKELETON HAS: a row is kept when its target bone is in `skeleton` (or the bone's alias:
/// Goofy's neck is `kubi1`); its aim is dropped (the bone keeps the turn the bind gives it) when the aim's target bone is
/// not. Donald's fingers B - D have two joints (the third rows go), neither friend has the little finger (yubiE).
/// Returns the rows and their `ROWS` indices.
#[cfg(any(test, feature = "party"))]
pub fn rows_for(skeleton: &kh_anim::Skeleton) -> (Vec<MapRow>, Vec<usize>) {
    const ALIASES: [(&str, &str); 1] = [("kubi", "kubi1")];
    let resolve = |name: &'static str| -> Option<&'static str> {
        if skeleton.index_of(name).is_some() {
            return Some(name);
        }
        ALIASES.iter().find(|(a, _)| *a == name).map(|(_, b)| *b).filter(|b| skeleton.index_of(b).is_some())
    };
    let mut rows = Vec::new();
    let mut keep = Vec::new();
    for (k, r) in ROWS.iter().enumerate() {
        let Some(target) = resolve(r.target) else { continue };
        let aim = r.aim.and_then(|(source, t)| resolve(t).map(|t| (source, t)));
        rows.push(MapRow { source: r.source, target, aim });
        keep.push(k);
    }
    (rows, keep)
}

/// (ER party S2) A retarget onto any skeleton from the body's bind pose per `ROWS` row (KH3 axes, `Rig::source_binds`),
/// with the rows that skeleton has (`rows_for`). Pure (the binds read once).
#[cfg(any(test, feature = "party"))]
pub fn retarget_from_binds(binds: &[(Vec3, Quat)], skeleton: &kh_anim::Skeleton) -> Result<(Retarget, Vec<usize>), String> {
    if binds.len() != ROWS.len() {
        return Err(format!("{} binds for {} rows", binds.len(), ROWS.len()));
    }
    let (rows, keep) = rows_for(skeleton);
    let bind = |name: &str| ROWS.iter().position(|r| r.source == name).map(|k| binds[k]);
    let retarget = Retarget::new(skeleton, &rows, &BODY, &bind)?;
    Ok((retarget, keep))
}

impl Rig {
    /// (ER party S2) The bind pose of every `ROWS` source bone of a `CSFD4ModelItem` (KH3 axes), as `build` reads it.
    pub fn source_binds(item: usize) -> Result<Vec<(Vec3, Quat)>, String> {
        let flver = word(item + 0x68).filter(|f| *f != 0).ok_or("the model has no FLVER data")?;
        let n = u32_at(flver + 0x18).unwrap_or(0) as usize;
        let (nodes, binds) = (word(flver + 0x20).unwrap_or(0), word(flver + 0x70).unwrap_or(0));
        if n == 0 || n > 2048 || nodes == 0 || binds == 0 {
            return Err(format!("the FLVER data of {item:#x} is not as measured ({n} nodes)"));
        }
        let mut table = vec![0u8; n * 0x30];
        if !read(nodes, &mut table) {
            return Err("the FLVER's node table is not readable".to_string());
        }
        let mut names: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in 0..n {
            let at = usize::from_le_bytes(table[k * 0x30 + 0x20..k * 0x30 + 0x28].try_into().unwrap());
            let name = wide_any(at);
            if !name.is_empty() {
                names.entry(name).or_insert(k);
            }
        }
        ROWS.iter()
            .map(|r| {
                let k = *names.get(r.source).ok_or_else(|| format!("the FLVER has no bone {}", r.source))?;
                let f = f32s(binds + k * 0x30, 12);
                (f.len() == 12).then(|| matrix(&f)).map(|(p, q)| (crate::coords::dir_er_to_kh(p), rotation_kh(q))).ok_or_else(|| format!("the bind of {} is not readable", r.source))
            })
            .collect()
    }

    /// (ER party S2) `build` for a skeleton that may lack some rows' bones (a party member's): the retarget is made from
    /// the rows it has (`keep`); `read` still gives every row (pick the kept ones with `pick`).
    #[cfg(feature = "party")]
    pub fn build_for(item: usize, skeleton: &kh_anim::Skeleton) -> Result<Rig, String> {
        let flver = word(item + 0x68).filter(|f| *f != 0).ok_or("the model has no FLVER data")?;
        let binds = Rig::source_binds(item)?;
        let (retarget, keep) = retarget_from_binds(&binds, skeleton)?;
        // the FLVER nodes of every row, as `build` finds them
        let n = u32_at(flver + 0x18).unwrap_or(0) as usize;
        let nodes_at = word(flver + 0x20).unwrap_or(0);
        let mut table = vec![0u8; n * 0x30];
        if !read(nodes_at, &mut table) {
            return Err("the FLVER's node table is not readable".to_string());
        }
        let mut names: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in 0..n {
            let at = usize::from_le_bytes(table[k * 0x30 + 0x20..k * 0x30 + 0x28].try_into().unwrap());
            let name = wide_any(at);
            if !name.is_empty() {
                names.entry(name).or_insert(k);
            }
        }
        let nodes = ROWS.iter().map(|r| names.get(r.source).copied().ok_or_else(|| format!("the FLVER has no bone {}", r.source))).collect::<Result<Vec<_>, _>>()?;
        Ok(Rig { flver, nodes, retarget, keep: Some(keep) })
    }
}

impl Rig {
    /// From a `CSFD4ModelItem` (the FLVER's node table: 0x30 bytes a node, the name pointer at + 0x20; the bind
    /// matrices in model space in the second table, MEASURED).
    pub fn build(item: usize, skeleton: &kh_anim::Skeleton) -> Result<Rig, String> {
        let flver = word(item + 0x68).filter(|f| *f != 0).ok_or("the model has no FLVER data")?;
        let n = u32_at(flver + 0x18).unwrap_or(0) as usize;
        let (nodes, binds) = (word(flver + 0x20).unwrap_or(0), word(flver + 0x70).unwrap_or(0));
        if n == 0 || n > 2048 || nodes == 0 || binds == 0 {
            return Err(format!("the FLVER data of {item:#x} is not as measured ({n} nodes)"));
        }
        let mut table = vec![0u8; n * 0x30];
        if !read(nodes, &mut table) {
            return Err("the FLVER's node table is not readable".to_string());
        }
        let mut names: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in 0..n {
            let at = usize::from_le_bytes(table[k * 0x30 + 0x20..k * 0x30 + 0x28].try_into().unwrap());
            let name = wide_any(at);
            if !name.is_empty() {
                names.entry(name).or_insert(k);
            }
        }
        let bind = |name: &str| -> Option<(Vec3, Quat)> {
            let k = *names.get(name)?;
            let f = f32s(binds + k * 0x30, 12);
            (f.len() == 12).then(|| matrix(&f)).map(|(p, q)| (crate::coords::dir_er_to_kh(p), rotation_kh(q)))
        };
        let retarget = Retarget::new(skeleton, &ROWS, &BODY, &bind)?;
        let nodes = ROWS.iter().map(|r| names[r.source]).collect();
        Ok(Rig { flver, nodes, retarget, #[cfg(feature = "party")] keep: None }) // (ER party S2) all rows
    }

    /// The rows' bones now, in Elden Ring's world (`CSFD4LocationMtx43ArrayEntity`: item + 0x640 -> + 0x70 = the
    /// matrices, one per FLVER node, WORLD space; MEASURED).
    pub fn read(&self, item: usize) -> Option<Vec<(Vec3, Quat)>> {
        let pose = word(word(item + 0x640)? + 0x70).filter(|p| *p != 0)?;
        let (lo, hi) = (*self.nodes.iter().min()?, *self.nodes.iter().max()?);
        let mut bytes = vec![0u8; (hi - lo + 1) * 0x30];
        if !read(pose + lo * 0x30, &mut bytes) {
            return None;
        }
        let mut out = Vec::with_capacity(self.nodes.len());
        for node in &self.nodes {
            let f: Vec<f32> = bytes[(node - lo) * 0x30..(node - lo + 1) * 0x30].as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
            let (p, q) = matrix(&f);
            if !p.is_finite() || !q.is_finite() || p.abs().max_element() > 1.0e6 {
                return None;
            }
            out.push((p, q));
        }
        Some(out)
    }
}

/// What a frame of the scene asks of Sora's drawing.
pub struct Posed {
    /// The rows' bones in KH3's world (cm).
    pub source: Vec<(Vec3, Quat)>,
    /// Where Sora's pelvis goes, and his component (at the floor point under it, not turned).
    pub pelvis: Vec3,
    pub component_to_world: Trs,
    /// (CUTSCENE-ANIM) The figure's measures (its FLVER's bind pose; None: not readable): cutfit.rs fits the leader with them.
    pub body: Option<crate::cutfit::Body>,
}

/// The running scene, as last looked at.
#[derive(Default)]
pub struct Cut {
    env: usize,
    acts: Vec<usize>,
    looked: Option<std::time::Instant>,
    pub figure: Option<Figure>,
    /// The flag words we cleared bit 0 of (put back when the hiding ends while the scene still runs).
    cleared: Vec<usize>,
    rig: Option<Rig>,
    rig_refused: usize,
    /// (ER party S2) the figure's rig for a party leader's skeleton (FLVER, the skeleton's bone count) and the one refused
    #[cfg(feature = "party")]
    rig_party: Option<(usize, usize, Rig)>,
    #[cfg(feature = "party")]
    rig_party_refused: (usize, usize),
    /// (CUTSCENE-ANIM) The figure's measures per FLVER (its bind pose read once).
    body: Option<(usize, Option<crate::cutfit::Body>)>,
    /// (CUTSCENE-ANIM) The figure's acts whose scene models were listed (`census`), at most `CENSUS_PER_SCENE` a scene.
    censused: Vec<usize>,
    /// The scene whose models were listed in the log.
    listed: usize,
    /// For the log: flag words found set again, the time spent, the looks.
    pub rewrites: u32,
    pub spent: std::time::Duration,
    pub looks: u32,
    /// The slowest look of the second and where its time went (environment, act list, figure, flags), microseconds.
    pub worst: [u128; 5],
}

impl Cut {
    /// EVERY PHASE while a cutscene plays: the figure is looked for and (with `hide`) held hidden (bit 0 of the display
    /// flags of its model and parts: MEASURED to hide all of it when held from the act's first frame).
    pub fn phase(&mut self, hide: bool) {
        let started = std::time::Instant::now();
        let mut lap = [0u128; 5];
        let env = env().unwrap_or(0);
        lap[1] = started.elapsed().as_micros();
        if env == 0 {
            self.figure = None;
            self.acts.clear();
            self.env = 0;
            return;
        }
        if env != self.listed && self.looked.is_none_or(|t| t.elapsed().as_millis() > 400) && !acts(env).is_empty() {
            self.listed = env;
            // (IMPL_ER17) a new scene: which models its acts animate (a player figure under another name than c0000 would show here)
            let mut names: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
            for act in acts(env).into_iter().filter(|a| is(*a, &VT_ANIM_ACT, "CSRemoAnimAct@CS")) {
                if let Some(n) = word(act + ACT_NAME).and_then(wide) {
                    *names.entry(n.split('_').next().unwrap_or("").to_string()).or_default() += 1;
                }
            }
            crate::erlog!("cutfig: A SCENE: its animated models (name: acts): {}", names.iter().map(|(n, c)| format!("{n}: {c}")).collect::<Vec<_>>().join(", "));
        }
        if env != self.env || self.looked.is_none_or(|t| t.elapsed().as_millis() > 500) {
            self.env = env;
            self.looked = Some(std::time::Instant::now());
            self.acts = player_acts(env);
        }
        lap[2] = started.elapsed().as_micros();
        self.figure = find(&self.acts);
        lap[3] = started.elapsed().as_micros();
        if let Some(f) = &self.figure {
            if hide {
                for w in flag_words(f) {
                    if let Some(now) = u32_at(w) {
                        if now & 1 != 0 {
                            if self.cleared.contains(&w) {
                                self.rewrites += 1;
                            } else if self.cleared.len() < 64 {
                                self.cleared.push(w);
                            }
                            // (A PLAIN WRITE: the word was read through the system an instant ago, so its page is there;
                            // WriteProcessMemory took ~80 ms A WORD on this machine: MEASURED, a 1 s stall at each act's start)
                            // SAFETY: a flag word of a live model's display entity, just read; game thread.
                            unsafe { std::ptr::write_volatile(w as *mut u32, now & !1) };
                        }
                    }
                }
            } else {
                self.show();
            }
        }
        self.spent += started.elapsed();
        self.looks += 1;
        lap[0] = started.elapsed().as_micros();
        lap[4] = self.acts.len() as u128;
        if lap[0] > self.worst[0] {
            self.worst = lap;
        }
    }

    /// The hidden parts get their bit back (the hiding was switched off, the payload goes) when they still exist.
    pub fn show(&mut self) {
        if self.cleared.is_empty() {
            return;
        }
        if let Some(f) = &self.figure {
            for w in flag_words(f) {
                if self.cleared.contains(&w) {
                    if let Some(now) = u32_at(w) {
                        // SAFETY: as in `phase`.
                        unsafe { std::ptr::write_volatile(w as *mut u32, now | 1) };
                    }
                }
            }
        }
        self.cleared.clear();
    }

    /// The figure's pose for Sora (KH3 world through `frame`).
    pub fn posed(&mut self, skeleton: &kh_anim::Skeleton, frame: &crate::coords::Frame) -> Option<Posed> {
        let item = word(self.figure.as_ref()?.model + 0x10)?;
        let flver = word(item + 0x68)?;
        if self.rig.as_ref().is_none_or(|r| r.flver != flver) {
            if self.rig_refused == flver {
                return None;
            }
            match Rig::build(item, skeleton) {
                Ok(rig) => {
                    crate::erlog!("cutfig: the figure's skeleton is read (FLVER {flver:#x}); Sora's legs are {:.2} of its", rig.retarget.ratio);
                    self.rig = Some(rig);
                }
                Err(e) => {
                    crate::erlog!("cutfig: THE FIGURE'S SKELETON IS NOT USABLE ({e}): Sora is not drawn in its place");
                    self.rig_refused = flver;
                    return None;
                }
            }
        }
        let rig = self.rig.as_ref()?;
        let er = rig.read(item)?;
        // the floor under it: its lowest mapped bone, less the flesh under a bone
        let floor_er = er.iter().map(|(p, _)| p.y).fold(f32::MAX, f32::min) - FLOOR_UNDER_BONE;
        let source: Vec<(Vec3, Quat)> = er.iter().map(|(p, q)| (frame.pos_er_to_kh(*p), rotation_kh(*q))).collect();
        let under = frame.pos_er_to_kh(Vec3::new(er[0].0.x, floor_er, er[0].0.z));
        let pelvis = rig.retarget.pelvis_place(source[0].0, Vec3::Z, under.z);
        let body = self.body_of(item, flver);
        Some(Posed { source, pelvis, component_to_world: Trs { translation: under, rotation: Quat::IDENTITY, scale: Vec3::ONE }, body })
    }

    /// (CUTSCENE-ANIM) The figure's measures (cutfit.rs), read from its FLVER's bind once per FLVER.
    fn body_of(&mut self, item: usize, flver: usize) -> Option<crate::cutfit::Body> {
        if let Some((f, b)) = self.body {
            if f == flver {
                return b;
            }
        }
        let b = Rig::source_binds(item).ok().and_then(|binds| crate::cutfit::tarnished(&binds));
        match &b {
            Some(b) => crate::erlog!("cutfig: (CUTSCENE-ANIM) the figure's measures: hips {:.0} cm, legs {:.0}, arms {:.0}, ankles {:.0}: the leader rests where it rests, at his own proportions", b.dims.hip_h, b.dims.limb[2], b.dims.limb[0], b.dims.ankle_h),
            None => crate::erlog!("cutfig: (CUTSCENE-ANIM) the figure's measures are NOT readable: the leader is placed as before"),
        }
        self.body = Some((flver, b));
        b
    }

    pub fn retarget(&self) -> Option<&Retarget> {
        self.rig.as_ref().map(|r| &r.retarget)
    }

    /// (ER party S2) `posed` for a FRIEND LEADING (U6: the leader stands in for the Tarnished): the figure retargeted onto
    /// his skeleton (the rows it has). Returns the pose (every row's source, the pelvis, the place), the retarget and the
    /// rows it keeps. Its own rig cache: Sora's (`rig`) is untouched.
    #[cfg(feature = "party")]
    pub fn posed_party(&mut self, skeleton: &kh_anim::Skeleton, frame: &crate::coords::Frame) -> Option<(Posed, Retarget, Vec<usize>)> {
        let item = word(self.figure.as_ref()?.model + 0x10)?;
        let flver = word(item + 0x68)?;
        if self.rig_party.as_ref().is_none_or(|(f, n, _)| *f != flver || *n != skeleton.len()) {
            if self.rig_party_refused == (flver, skeleton.len()) {
                return None;
            }
            match Rig::build_for(item, skeleton) {
                Ok(rig) => {
                    crate::erlog!("cutfig: the figure's skeleton is read for the party leader (FLVER {flver:#x}, {} of {} rows); his legs are {:.2} of its", rig.keep.as_ref().map_or(ROWS.len(), |k| k.len()), ROWS.len(), rig.retarget.ratio);
                    self.rig_party = Some((flver, skeleton.len(), rig));
                }
                Err(e) => {
                    crate::erlog!("cutfig: THE FIGURE'S SKELETON IS NOT USABLE FOR THE PARTY LEADER ({e}): he is not drawn in its place");
                    self.rig_party_refused = (flver, skeleton.len());
                    return None;
                }
            }
        }
        let (_, _, rig) = self.rig_party.as_ref()?;
        let er = rig.read(item)?;
        let floor_er = er.iter().map(|(p, _)| p.y).fold(f32::MAX, f32::min) - FLOOR_UNDER_BONE;
        let all: Vec<(Vec3, Quat)> = er.iter().map(|(p, q)| (frame.pos_er_to_kh(*p), rotation_kh(*q))).collect();
        let under = frame.pos_er_to_kh(Vec3::new(er[0].0.x, floor_er, er[0].0.z));
        let pelvis = rig.retarget.pelvis_place(all[0].0, Vec3::Z, under.z);
        let keep = rig.keep.clone().unwrap_or_else(|| (0..ROWS.len()).collect());
        let retarget = rig.retarget.clone();
        let body = self.body_of(item, flver); // (CUTSCENE-ANIM)
        Some((Posed { source: all, pelvis, component_to_world: Trs { translation: under, rotation: Quat::IDENTITY, scale: Vec3::ONE }, body }, retarget, keep))
    }
}

/// Metres from a bone's centre to the floor it rests on (a hand or a toe lying on the ground; by eye).
const FLOOR_UNDER_BONE: f32 = 0.04;

/// The scene's figure for every user in the payload (frame.rs looks each phase; sora.rs takes the pose).
pub static CUT: std::sync::Mutex<Option<Cut>> = std::sync::Mutex::new(None);

/// EVERY PHASE (frame.rs): `cut.sora` = retarget (default) | hide | game | double. Returns nothing; sora.rs reads `CUT`.
pub fn phase(control: &crate::control::Control, tick: bool) {
    let playing = crate::game::remo_state().is_some_and(|s| s >= 2);
    let Ok(mut guard) = CUT.lock() else { return };
    if !playing {
        if guard.is_some() {
            *guard = None;
        }
        return;
    }
    let mode = control.str("cut.sora").unwrap_or("retarget");
    let cut = guard.get_or_insert_with(Cut::default);
    cut.phase(matches!(mode, "retarget" | "hide"));
    // (CUTSCENE-ANIM) the scene's models around the figure, listed once per figure act (read only; `cut.census = 0`: off)
    if control.flag("cut.census") != Some(false) {
        if let Some(f) = cut.figure.clone() {
            if !cut.censused.contains(&f.act) && cut.censused.len() < CENSUS_PER_SCENE {
                cut.censused.push(f.act);
                census(&f, cut.env);
            }
        }
    }
    if tick && cut.looks > 0 {
        crate::erlog!("cutfig: {} | cut.sora = {mode} | {} look(s) this second, {:.0} us each, {} flag word(s) found set again; slowest look {:?} us (whole, after env, acts, find; acts)", if cut.figure.is_some() { "THE SCENE'S FIGURE OF THE PLAYER IS THERE" } else { "no figure of the player in this cut" }, cut.looks, cut.spent.as_secs_f64() * 1.0e6 / cut.looks as f64, cut.rewrites, cut.worst);
        cut.worst = [0; 5];
        cut.looks = 0;
        cut.spent = std::time::Duration::ZERO;
        cut.rewrites = 0;
    }
}

/// The payload goes: what we hid is shown again.
pub fn release() {
    if let Ok(mut guard) = CUT.lock() {
        if let Some(cut) = guard.as_mut() {
            cut.show();
        }
        *guard = None;
    }
}
// ---------------------------------------------------------------- (CUTSCENE-ANIM) the scene's models around the figure

/// The figure's acts listed per scene (the census costs a few hundred checked reads: once, not every frame).
pub const CENSUS_PER_SCENE: usize = 4;
/// A model of the scene this near the figure's root (cm) is listed (a weapon held, a prop handed over).
pub const CENSUS_NEAR_CM: f32 = 300.0;
/// At most this many of the scene's acts are looked at.
const CENSUS_ACTS: usize = 320;

/// Where a model is (its location matrices' first node, Elden Ring's world, metres): item + 0x640 -> + 0x70 (MEASURED for the
/// figure's viewer model, IMPL_ER16; for other models it is read through the system, so a different layout reads as nothing).
fn place_of(model: usize) -> Option<Vec3> {
    let item = word(model + 0x10).filter(|p| *p != 0)?;
    let mats = word(word(item + 0x640).filter(|p| *p != 0)? + 0x70).filter(|p| *p != 0)?;
    let f = f32s(mats, 12);
    let p = Vec3::new(*f.get(3)?, *f.get(7)?, *f.get(11)?);
    (p.is_finite() && p.abs().max_element() < 1.0e6).then_some(p)
}

/// What a model is called (its FLVER's name, lower case stem: `wp_a_0200`, `bd_m_1100`, `c0000`).
fn model_label(model: usize) -> String {
    word(model + 0x10).filter(|p| *p != 0).and_then(|item| dl_string(item + 0x6C0)).map(|n| kh_erfile::part_stem(&n)).unwrap_or_default()
}

/// A model's display flag words (`CSModelDispEntity` + 0x20, + 0x24; bit 0 of the first = drawn).
fn disp_of(model: usize) -> Option<(u32, u32)> {
    let disp = word(model + MODEL_DISP).filter(|d| *d != 0)?;
    Some((u32_at(disp + DISP_FLAGS)?, u32_at(disp + DISP_FLAGS + 4)?))
}

/// Is a model's name a weapon's (Elden Ring's `WP_*` parts: held weapons, arrows and bolts; IMPL_ZDRAW: a c0000 wears
/// `wp_a_0200` among its dressed parts)?
pub fn weapon_like(label: &str) -> bool {
    label.to_ascii_lowercase().starts_with("wp_")
}

/// The census line of one model (address, class, name, drawn or not, distance from the figure).
fn census_line(model: usize, root: Option<Vec3>) -> String {
    let label = model_label(model);
    let away = match (place_of(model), root) {
        (Some(p), Some(r)) => format!("{:.0} cm from the figure", (p - r).length() * 100.0),
        _ => "place not readable".to_string(),
    };
    let disp = disp_of(model).map_or("display not readable".to_string(), |(a, b)| format!("display {a:#x} {b:#x} ({})", if a & 1 != 0 { "DRAWN" } else { "hidden" }));
    format!("{model:#x} {} {label:?}{} | {disp} | {away}", class_of(model).unwrap_or_default(), if weapon_like(&label) { " = A WEAPON PART" } else { "" })
}

/// The non-empty model pointers of a dressing object's 27 part slots (slot, pointer).
fn dressed(asm: usize) -> Vec<(usize, usize)> {
    let mut raw = [0u8; PARTS * 8];
    if asm == 0 || !read(asm + ASM_PARTS, &mut raw) {
        return Vec::new();
    }
    raw.as_chunks::<8>().0.iter().enumerate().map(|(k, c)| (k, usize::from_le_bytes(*c))).filter(|(_, p)| *p != 0).collect()
}

/// (CUTSCENE-ANIM, the coordinator 2026-11-07: the Tarnished's crossbow seen by Sora's hand in the Rennala library scene) THE
/// SCENE'S MODELS AROUND THE FIGURE, READ ONLY, written to the log once per figure act (at most `CENSUS_PER_SCENE` a scene):
/// - the figure's 27 dressed parts (their FLVER names: a weapon is a `wp_*` part);
/// - any other pointer the dressing object holds past them that the RTTI calls a model (a weapon kept apart);
/// - every other act's model within `CENSUS_NEAR_CM` of the figure or named as a weapon (a prop of the scene);
/// - the real player's own model and dressed parts.
///
/// Each with its class, its display flags (drawn or not) and its distance from the figure. Nothing is written into any object.
pub fn census(f: &Figure, env: usize) {
    let root = place_of(f.model);
    crate::erlog!("cutfig: (CUTSCENE-ANIM census) THE FIGURE {:#x} {} {:?} act {:#x} {:?}, dressed by {:#x}, at {:?} (ER m); read only", f.model, class_of(f.model).unwrap_or_default(), model_label(f.model), f.act, word(f.act + ACT_NAME).and_then(wide).unwrap_or_default(), f.asm, root);
    for (k, part) in dressed(f.asm) {
        crate::erlog!("cutfig: (census)   part {k:2}: {}", census_line(part, root));
    }
    let mut extra = [0u8; 0x100];
    if read(f.asm + ASM_PARTS + PARTS * 8, &mut extra) {
        for (k, c) in extra.as_chunks::<8>().0.iter().enumerate() {
            let p = usize::from_le_bytes(*c);
            if p != 0 && class_of(p).is_some_and(|n| n.contains("Model")) {
                crate::erlog!("cutfig: (census)   dressing + {:#x}: {}", ASM_PARTS + PARTS * 8 + k * 8, census_line(p, root));
            }
        }
    }
    let mut listed = 0;
    for act in acts(env).into_iter().take(CENSUS_ACTS) {
        if act == f.act || !is(act, &VT_ANIM_ACT, "CSRemoAnimAct@CS") {
            continue;
        }
        let Some(model) = word(act + ACT_MODEL).filter(|m| *m != 0) else { continue };
        let near = matches!((place_of(model), root), (Some(p), Some(r)) if (p - r).length() * 100.0 <= CENSUS_NEAR_CM);
        if !near && !weapon_like(&model_label(model)) {
            continue;
        }
        listed += 1;
        crate::erlog!("cutfig: (census)   act {act:#x} {:?}: {}", word(act + ACT_NAME).and_then(wide).unwrap_or_default(), census_line(model, root));
        if listed >= 40 {
            break;
        }
    }
    if let Some(player) = crate::game::player_mut() {
        let model = crate::word!(player.chr_ins.chr_model_ins);
        let asm = crate::word!(player.chr_asm_model_ins);
        crate::erlog!("cutfig: (census)   THE REAL PLAYER's model: {}", census_line(model, root));
        for (k, part) in dressed(asm) {
            crate::erlog!("cutfig: (census)   real player's part {k:2}: {}", census_line(part, root));
        }
    }
    crate::erlog!("cutfig: (census) end ({listed} other model(s) of the scene near the figure or named as weapons)");
}

#[cfg(test)]
mod census_tests {
    #[test]
    fn weapon_parts_are_named_wp() {
        assert!(super::weapon_like("wp_a_0200") && super::weapon_like("WP_A_4300"));
        assert!(!super::weapon_like("bd_m_1100") && !super::weapon_like("c0000") && !super::weapon_like("aeg099_060"));
    }
}
