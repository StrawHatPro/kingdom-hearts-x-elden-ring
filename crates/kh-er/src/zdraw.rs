//! (ZDRAW) ZETTAFLARE'S TARGET DRAWN BY US, the host's half (UNTESTED IN GAME). The user: "it has to look as perfect as the
//! test level". For every Zettaflare finale target (finale.rs `current`, fed by zfinale.rs) the body is found by its handle,
//! its MODEL is read from the game's own files (kh-erfile: `/chr/<name>.chrbnd.dcx` for the name the game gives its model,
//! e.g. `c4311`; once per model, on a thread of its own, read-only) and its POSE from the game's memory every frame (the
//! model item's FLVER node table and location matrices, as figure.rs reads the player's: MEASURED there); kh-er-render
//! zdraw.rs skins it, breaks it apart exactly as kh-app does and writes it into the finale's mask, and draws it in the test
//! level's light grey while Zettaflare holds it. Generic: any character model; its meshes in display groups the character
//! does not show (NpcParam `modelDispMask`) are left out.
//!
//! A HUMAN OF THE `c0000` KIND (its look is its worn parts; EXPERIMENTAL, UNTESTED IN GAME): when its character is a
//! `PlayerIns` (RTTI), every part model its `CSChrAsmModelIns` holds (the SDK's layout, as passive.rs uses it on the player) is
//! read the same way (`/parts/<its name>.partsbnd.dcx`) and posed by ITS OWN model item's matrices, each checked against
//! the game: its FLVER's bones (count and names), and its bones that the body has too lying where the body's are (2 cm).
//! A part that fails is left out (logged); a body with nothing left keeps the screen route.
//!
//! THE ROUTE PER TARGET, logged once each: OUR OWN MODEL when a model is read and its pose matches it; else finale.rs's
//! SCREEN ROUTE stays (a body made of effects, a model still loading, a model the reader cannot read, a pose that does not
//! fit). `r.zdraw` (ctl, default 1) turns ours off.
//!
//! Its body: hidden by zfinale.rs from its first crumble step (finale.rs `breaks_apart` says ours breaks apart), as before.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};

use eldenring::cs::{NpcParam, SoloParamRepository, WorldChrMan};
use fromsoftware_shared::FromStatic;
use glam::{Mat4, Vec3};
use kh_er_render::zdraw::{Ask, Driver, MeshPart, Model, Tri};

use crate::erlog;
use crate::probe::{class_of, dl_string, read, u32_at, word};

/// `r.zdraw` (0 = the screen route only).
static ON: AtomicBool = AtomicBool::new(true);

pub fn set_on(on: bool) {
    ON.store(on, Ordering::Relaxed);
}

/// Which file a model comes from.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Key {
    /// `/chr/<name>.chrbnd.dcx`
    Chr(String),
    /// `/parts/<name>.partsbnd.dcx`
    Part(String),
}

impl std::fmt::Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Key::Chr(n) | Key::Part(n) => f.write_str(n),
        }
    }
}

enum Slot {
    Loading,
    Ready(Arc<Model>),
    Failed(String),
}

struct Shelf {
    models: HashMap<Key, Slot>,
    tx: Option<mpsc::Sender<Key>>,
}

static SHELF: Mutex<Option<Shelf>> = Mutex::new(None);

/// A FLVER as the renderer's model.
pub fn model_of(name: &str, f: &kh_erfile::Flver) -> Model {
    let mut m = Model { name: name.to_string(), ..Default::default() };
    m.bone_names = f.bones.iter().map(|b| b.name.clone()).collect();
    m.inv_bind = f.bones.iter().map(|b| b.bind.inverse()).collect();
    m.parent = f.bones.iter().map(|b| b.parent).collect();
    m.bind = f.bones.iter().map(|b| b.bind).collect();
    for mesh in &f.meshes {
        let base = m.positions.len() as u32;
        m.positions.extend(&mesh.positions);
        m.joints.extend(&mesh.joints);
        m.weights.extend(&mesh.weights);
        let first = m.indices.len();
        m.indices.extend(mesh.indices.iter().map(|i| i + base));
        let material = f.material_names.get(mesh.material.max(0) as usize).cloned().unwrap_or_default();
        m.meshes.push(MeshPart { first, count: mesh.indices.len(), group: kh_er_render::zdraw::disp_group(&material), material });
    }
    m
}

/// The game's folder (the exe's).
fn game_dir() -> std::path::PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())).unwrap_or_else(kh_erfile::archive::default_game_dir)
}

/// The model of `key`, once it is read (asked for the first time: its reading starts on the reader's thread).
fn model(key: &Key) -> Result<Option<Arc<Model>>, String> {
    let Ok(mut guard) = SHELF.lock() else { return Ok(None) };
    let shelf = guard.get_or_insert_with(|| Shelf { models: HashMap::new(), tx: None });
    match shelf.models.get(key) {
        Some(Slot::Ready(m)) => return Ok(Some(m.clone())),
        Some(Slot::Loading) => return Ok(None),
        Some(Slot::Failed(e)) => return Err(e.clone()),
        None => {}
    }
    if shelf.tx.is_none() {
        let (tx, rx) = mpsc::channel::<Key>();
        let spawned = std::thread::Builder::new().name("kh-zdraw-reader".into()).spawn(move || reader(rx));
        if spawned.is_err() {
            return Err("the reader's thread cannot start".to_string());
        }
        shelf.tx = Some(tx);
    }
    shelf.models.insert(key.clone(), Slot::Loading);
    if let Some(tx) = &shelf.tx {
        let _ = tx.send(key.clone());
    }
    Ok(None)
}

/// The reader's thread: the game's archives opened once, each model read as it is asked for (read-only).
fn reader(rx: mpsc::Receiver<Key>) {
    let game = game_dir();
    let mut library: Option<kh_erfile::Library> = None;
    for key in rx {
        let started = std::time::Instant::now();
        let result = (|| -> Result<Model, String> {
            if library.is_none() {
                library = Some(kh_erfile::Library::open(&game)?);
            }
            let lib = library.as_mut().ok_or("no library")?;
            let f = match &key {
                Key::Chr(n) => lib.character(n)?,
                Key::Part(n) => lib.part(n)?,
            };
            for n in &f.notes {
                erlog!("zdraw: {key}: {n}");
            }
            // (a model over the frame's budget reduced once, in its bind pose)
            let m = model_of(&key.to_string(), &f);
            let r = m.reduced(kh_er_render::zdraw::DRAW_TRIS);
            if r.indices.len() < m.indices.len() {
                erlog!("zdraw: {key}: {} triangles reduced to {} for drawing", m.indices.len() / 3, r.indices.len() / 3);
            }
            Ok(r)
        })();
        let slot = match result {
            Ok(m) => {
                erlog!("zdraw: {key}: its model is read from the game's files in {:.2} s: {} bones, {} meshes, {} triangles (display groups {:?})", started.elapsed().as_secs_f32(), m.bone_names.len(), m.meshes.len(), m.indices.len() / 3, m.meshes.iter().filter_map(|p| p.group).collect::<std::collections::BTreeSet<_>>());
                Slot::Ready(Arc::new(m))
            }
            Err(e) => {
                erlog!("zdraw: {key}: ITS MODEL CANNOT BE READ ({e})");
                Slot::Failed(e)
            }
        };
        if let Ok(mut guard) = SHELF.lock() {
            if let Some(s) = guard.as_mut() {
                s.models.insert(key, slot);
            }
        }
    }
}

/// A UTF-16 name at a pointer (FLVER node names).
fn wide(at: usize) -> String {
    let mut b = [0u8; 96];
    if !read(at, &mut b) {
        return String::new();
    }
    let units: Vec<u16> = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|u| *u != 0).collect();
    String::from_utf16_lossy(&units)
}

/// Does the FLVER the game loaded for this model item have the model's bones (count, and the names of a sample of them)?
fn same_bones(item: usize, m: &Model) -> Result<(), String> {
    let flver = word(item + 0x68).filter(|f| *f != 0).ok_or("no FLVER data on its model")?;
    let n = u32_at(flver + 0x18).unwrap_or(0) as usize;
    if n != m.bone_names.len() {
        return Err(format!("the game's FLVER has {n} bones, the file's {}", m.bone_names.len()));
    }
    if n == 0 {
        return Ok(());
    }
    let nodes = word(flver + 0x20).filter(|p| *p != 0).ok_or("no node table")?;
    let mut sample: Vec<usize> = (0..n.min(8)).collect();
    sample.extend([n / 3, n / 2, (2 * n) / 3, n - 1]);
    for k in sample {
        let name = word(nodes + k * 0x30 + 0x20).map(wide).unwrap_or_default();
        if name != m.bone_names[k] {
            return Err(format!("bone {k} is {name:?} in the game, {:?} in the file", m.bone_names[k]));
        }
    }
    Ok(())
}

/// (round 3) What one read of a model item's pose found (for the log).
#[derive(Clone, Debug, Default)]
pub struct PoseRead {
    pub loc: usize,
    pub count32: u32,
    pub count64: usize,
    pub at: usize,
    pub n: usize,
    /// bones whose matrix is not usable (not finite, absurd, over 100 m from the body) and so placed from their parents
    pub mended: usize,
    /// of them, the bones past the array's count
    pub past: usize,
    pub first: Vec3,
    pub body: Vec3,
}

impl std::fmt::Display for PoseRead {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "location {:#x}, count {} (u32) / {:#x} (u64), matrices at {:#x}, {} bones of the file ({} past the count), {} mended, bone 0 at ({:.2}, {:.2}, {:.2}) for the body at ({:.2}, {:.2}, {:.2})", self.loc, self.count32, self.count64, self.at, self.n, self.past, self.mended, self.first.x, self.first.y, self.first.z, self.body.x, self.body.y, self.body.z)
    }
}

/// The bones' world matrices of a model item now (`CSFD4LocationMtx43ArrayEntity`: item + 0x640 -> + 0x70 the 3 x 4 rows, one
/// per FLVER bone; MEASURED by figure.rs on the player and by targets.rs `model_span` on enemies), Elden Ring's world (m).
/// (round 3, Margit: "its bones cannot be read this frame") Not all-or-nothing any more: the count at + 0x68 is only logged
/// (its width is not measured), a bone whose matrix is not usable (not finite, absurd, over 100 m from the body's position
/// node: `model_span` too leaves such bones out) is placed from its parent by the bind (kh-er-render `mend_pose`). Err: the
/// failing step, with its values.
fn bones_now(item: usize, m: &Model, body: Vec3) -> Result<(Vec<Mat4>, PoseRead), String> {
    let n = m.bone_names.len();
    let mut r = PoseRead { n, body, ..Default::default() };
    if n == 0 {
        return Ok((Vec::new(), r));
    }
    r.loc = word(item + 0x640).unwrap_or(0);
    if r.loc == 0 {
        return Err(format!("no location entity at model item {item:#x} + 0x640"));
    }
    r.count32 = u32_at(r.loc + 0x68).unwrap_or(0);
    r.count64 = word(r.loc + 0x68).unwrap_or(0);
    r.at = word(r.loc + 0x70).unwrap_or(0);
    if r.at == 0 {
        return Err(format!("no matrices: {r}"));
    }
    if n > 4096 {
        return Err(format!("{n} bones: too many"));
    }
    // THE COUNT: the array holds a matrix for the first `count` bones only (MEASURED, build/sora_tmp/er/er16/figskel_b.txt: the
    // player's c0000 has 488 FLVER nodes and 179 matrices; round 2 required count >= the FLVER's bones and so refused every
    // body: Margit, 2026-10-23). The bones past it (helpers, cloth ends) are placed from their parents by the bind.
    let take = (r.count32 as usize).min(n);
    if r.count32 == 0 || r.count32 > 8192 {
        return Err(format!("the matrix count is not plausible: {r}"));
    }
    let mut bytes = vec![0u8; take * 0x30];
    if !read(r.at, &mut bytes) {
        return Err(format!("the matrices are not readable: {r}"));
    }
    let mut out = vec![Mat4::IDENTITY; n];
    let mut ok = vec![false; n];
    for (k, c) in bytes.chunks_exact(0x30).enumerate() {
        let f: Vec<f32> = c.chunks_exact(4).map(|x| f32::from_le_bytes([x[0], x[1], x[2], x[3]])).collect();
        let mm = kh_erfile::flver::mat_from_rows34(&f);
        out[k] = mm;
        ok[k] = mm.is_finite() && (mm.w_axis.truncate() - body).length() < 100.0;
    }
    r.first = out[0].w_axis.truncate();
    r.past = n - take;
    let good = ok.iter().filter(|o| **o).count();
    if good * 2 < take {
        return Err(format!("only {good} of the {take} bone matrices usable: {r}"));
    }
    r.mended = kh_er_render::zdraw::mend_pose(m, &mut out, &ok).ok_or_else(|| format!("no bone matrix usable: {r}"))?;
    Ok((out, r))
}

/// The display mask of an NpcParam row (bit n: `modelDispMask<n>`). None: no row.
fn disp_mask(row: i32) -> Option<u32> {
    // SAFETY: game thread; the game's param repository (read only).
    let repo = unsafe { SoloParamRepository::instance() }.ok()?;
    let p = repo.get::<NpcParam>(row as u32)?;
    let bits = [
        p.model_disp_mask0(), p.model_disp_mask1(), p.model_disp_mask2(), p.model_disp_mask3(), p.model_disp_mask4(), p.model_disp_mask5(), p.model_disp_mask6(), p.model_disp_mask7(),
        p.model_disp_mask8(), p.model_disp_mask9(), p.model_disp_mask10(), p.model_disp_mask11(), p.model_disp_mask12(), p.model_disp_mask13(), p.model_disp_mask14(), p.model_disp_mask15(),
        p.model_disp_mask16(), p.model_disp_mask17(), p.model_disp_mask18(), p.model_disp_mask19(), p.model_disp_mask20(), p.model_disp_mask21(), p.model_disp_mask22(), p.model_disp_mask23(),
        p.model_disp_mask24(), p.model_disp_mask25(), p.model_disp_mask26(), p.model_disp_mask27(), p.model_disp_mask28(), p.model_disp_mask29(), p.model_disp_mask30(), p.model_disp_mask31(),
    ];
    Some(bits.iter().enumerate().fold(0u32, |m, (k, b)| m | (u32::from(*b) << k)))
}

/// A model item of a body: its file, the item, whether it is a worn part.
#[derive(Clone, Debug)]
struct Item {
    key: Key,
    item: usize,
}

/// A body of the game this frame: its model items (the character's own first, then its worn parts), its NpcParam row.
struct Body {
    name: String,
    items: Vec<Item>,
    npc_param: i32,
    /// its physics position (Elden Ring's space, m): the frame between the spaces when targets.rs gave none
    er_pos: Vec3,
}

/// Is this a character model's name the reader can look up (`cNNNN`)?
fn chr_name(name: &str) -> bool {
    name.len() == 5 && name.starts_with('c') && name[1..].chars().all(|c| c.is_ascii_digit())
}

/// (c0000) The part models a `PlayerIns` wears (`CSChrAsmModelIns` parts, the SDK's layout): their model items and names.
fn worn_parts(chr: &eldenring::cs::ChrIns) -> Vec<Item> {
    let at = chr as *const eldenring::cs::ChrIns as usize;
    if !class_of(at).is_some_and(|c| c.contains("PlayerIns")) {
        return Vec::new();
    }
    // SAFETY: the RTTI says this ChrIns is the ChrIns of a PlayerIns (its first field).
    let player = unsafe { &*(at as *const eldenring::cs::PlayerIns) };
    let asm = crate::word!(player.chr_asm_model_ins);
    if asm == 0 {
        return Vec::new();
    }
    // `CSChrAsmModelIns`: vftable, chr_model_ins, unk08, facegen_model, flags (u32, padded), parts_model_ins [27] at + 0x28
    let mut raw = [0u8; 27 * 8];
    if !read(asm + 0x28, &mut raw) {
        return Vec::new();
    }
    raw.chunks_exact(8)
        .map(|c| usize::from_le_bytes(c.try_into().unwrap()))
        .filter(|p| *p != 0)
        .filter_map(|part| {
            let item = word(part + 0x10).filter(|p| *p != 0)?;
            let name = kh_erfile::part_stem(&dl_string(item + 0x6C0)?);
            (!name.is_empty()).then_some(Item { key: Key::Part(name), item })
        })
        .collect()
}

fn body(id: u32) -> Option<Body> {
    let handle = crate::zfinale::handle_of(id)?;
    // SAFETY: game thread; the game's own lookup by handle.
    let wcm = unsafe { WorldChrMan::instance_mut() }.ok()?;
    let chr = wcm.chr_ins_by_handle_mut(&handle)?;
    if !crate::game::chr_complete(chr) {
        return None;
    }
    let model = crate::word!(chr.chr_model_ins);
    let item = word(model + 0x10).filter(|p| *p != 0)?;
    let name = dl_string(item + 0x6C0)?.to_ascii_lowercase();
    let mut items = Vec::new();
    if chr_name(&name) {
        items.push(Item { key: Key::Chr(name.clone()), item });
    }
    if name == "c0000" {
        items.extend(worn_parts(chr));
    }
    Some(Body { name, items, npc_param: chr.npc_param_id, er_pos: crate::game::v3(&chr.modules.physics.position) })
}

#[derive(Default)]
struct Live {
    driver: Driver,
    /// the route told per target: (id, route words)
    told: Vec<(u32, String)>,
    /// the model items whose bones were checked: (item, FLVER data, ok)
    checked: Vec<(usize, usize, Result<(), String>)>,
    /// the parts left out, told once: (item, why)
    parts_told: Vec<(usize, String)>,
    /// (round 3) the pose reads told: (target, model item)
    pose_told: Vec<(u32, usize)>,
    /// (2026-10-23, the user's first session: no line at all) what was told of the whole frame: the targets, the switch, the frame's source
    frame_told: String,
}

static LIVE: Mutex<Option<Live>> = Mutex::new(None);

impl Live {
    fn tell(&mut self, id: u32, route: String) {
        if self.told.iter().any(|t| t.0 == id && t.1 == route) {
            return;
        }
        self.told.retain(|t| t.0 != id);
        erlog!("zdraw: target {id}: {route}");
        self.told.push((id, route));
    }

    /// The model item's bones checked against the file's model (once per item and FLVER).
    fn check(&mut self, item: usize, m: &Model) -> Result<(), String> {
        let flver = word(item + 0x68).unwrap_or(0);
        if !self.checked.iter().any(|c| c.0 == item && c.1 == flver) {
            self.checked.retain(|c| c.0 != item);
            if self.checked.len() > 64 {
                let _ = self.checked.remove(0);
            }
            self.checked.push((item, flver, same_bones(item, m)));
        }
        self.checked.iter().find(|c| c.0 == item && c.1 == flver).map_or(Ok(()), |c| c.2.clone())
    }
}

/// (c0000) A worn part's bones lie where the body's bones of the same name lie (2 cm): its matrices are the body's pose. A
/// part sharing no bone (a weapon) is taken when its root lies within 3 m of the body's.
fn part_fits(part: &Model, world: &[Mat4], body: &Model, body_world: &[Mat4]) -> Result<(), String> {
    let mut shared = 0;
    for (k, name) in part.bone_names.iter().enumerate() {
        let Some(j) = body.bone_names.iter().position(|b| b == name) else { continue };
        let (Some(a), Some(b)) = (world.get(k), body_world.get(j)) else { continue };
        let d = (a.w_axis - b.w_axis).truncate().length();
        if d > 0.02 {
            return Err(format!("its bone {name} lies {d:.2} m from the body's"));
        }
        shared += 1;
    }
    if shared == 0 {
        // (a weapon: no bone of the body; held in a hand, its root lies within reach of the body's root)
        let (Some(r), Some(b)) = (world.first(), body_world.first()) else { return Err("no bones".to_string()) };
        let d = (r.w_axis - b.w_axis).truncate().length();
        if d > 3.0 {
            return Err(format!("no bone shared with the body and its root lies {d:.1} m from the body's"));
        }
    }
    Ok(())
}

/// The frame between the spaces as targets.rs last gave it (when, the frame).
static NOTED: Mutex<Option<(std::time::Instant, crate::coords::Frame)>> = Mutex::new(None);

/// targets.rs `refresh` (optional): the frame between the spaces of this frame (exact even for a body that hovers above its
/// position node). Without it each body's own physics position and its row's feet give it.
pub fn note_frame(frame: crate::coords::Frame) {
    if let Ok(mut n) = NOTED.lock() {
        *n = Some((std::time::Instant::now(), frame));
    }
}

/// (2026-10-23: the first in-game session drew nothing of ours: targets.rs had lost its call) ONCE A FRAME, from zfinale.rs
/// `frame` itself, right after the renderer's finale targets are set (game thread): the finale's targets drawn from their own
/// models, handed to the renderer. Every target's route is logged, with its reason, whenever it changes.
pub fn frame() {
    let Ok(mut guard) = LIVE.lock() else { return };
    let live = guard.get_or_insert_with(Live::default);
    live.driver.threaded = true;
    let targets = kh_er_render::finale::current();
    let noted = NOTED.lock().ok().and_then(|n| *n).filter(|(at, _)| at.elapsed().as_secs_f32() < 0.5).map(|(_, f)| f);
    let words = if targets.is_empty() { String::new() } else { format!("{} finale target(s) {:?}; r.zdraw {}; the frame between the spaces {}", targets.len(), targets.iter().map(|t| t.id).collect::<Vec<_>>(), if ON.load(Ordering::Relaxed) { "on" } else { "OFF (0 in the ctl): the screen route only" }, if noted.is_some() { "from targets.rs" } else { "from each body's position" }) };
    if words != live.frame_told {
        if !words.is_empty() {
            erlog!("zdraw: {words}");
        }
        live.frame_told = words;
    }
    if targets.is_empty() || !ON.load(Ordering::Relaxed) {
        live.driver.keep_only(&[]);
        live.told.clear();
        kh_er_render::zdraw::set(Vec::new());
        return;
    }
    let mut soups = Vec::new();
    let mut bodies = Vec::new();
    let ids: Vec<u32> = targets.iter().map(|t| t.id).collect();
    live.driver.keep_only(&ids);
    for t in &targets {
        let ask = Ask { id: t.id, silhouette: t.silhouette, crumble: t.crumble, feet: t.body.feet, radius: t.body.radius, half_height: t.body.half_height, push: t.push };
        // a body whose break-apart is cut needs nothing of the game any more (it may be hidden or gone)
        if live.driver.has(t.id) && t.crumble > 0.0 {
            if let Some(s) = live.driver.step(&ask, None) {
                soups.push((t.id, s));
            }
            continue;
        }
        let Some(b) = body(t.id) else {
            live.tell(t.id, "the SCREEN ROUTE (its character cannot be read by its handle)".to_string());
            continue;
        };
        // the frame between the spaces: targets.rs's, else this body's position node under its row's feet
        let frame = noted.unwrap_or_else(|| crate::coords::Frame::at(b.er_pos - crate::coords::dir_kh_to_er(t.body.feet)));
        let to_game = |p: Vec3| frame.pos_er_to_kh(p) / 100.0;
        if b.items.is_empty() {
            live.tell(t.id, format!("the SCREEN ROUTE (its model {:?} is not a character model)", b.name));
            continue;
        }
        // every item's model (all read before any is drawn: no body drawn in halves)
        let mut models: Vec<(Item, Arc<Model>)> = Vec::new();
        let mut waiting = None;
        let mut failed: Option<String> = None;
        for it in &b.items {
            match model(&it.key) {
                Ok(Some(m)) => models.push((it.clone(), m)),
                Ok(None) => waiting = Some(it.key.to_string()),
                Err(e) if matches!(it.key, Key::Chr(_)) && b.name != "c0000" => failed = Some(format!("{}: {e}", it.key)),
                Err(e) => {
                    let why = format!("not read: {e}");
                    if !live.parts_told.iter().any(|p| p.0 == it.item && p.1 == why) {
                        erlog!("zdraw: target {}: part {} left out ({why})", t.id, it.key);
                        live.parts_told.push((it.item, why));
                    }
                }
            }
        }
        if let Some(e) = failed {
            live.tell(t.id, format!("the SCREEN ROUTE (its model cannot be read: {e})"));
            continue;
        }
        if let Some(k) = waiting {
            live.tell(t.id, format!("{}: its model ({k}) is being read (the screen route meanwhile)", b.name));
            continue;
        }
        // the character's own model: its pose must fit
        let Some((own_item, own)) = models.first().cloned().filter(|(i, _)| matches!(i.key, Key::Chr(_))) else {
            live.tell(t.id, format!("the SCREEN ROUTE ({}: no model of its own)", b.name));
            continue;
        };
        if let Err(e) = live.check(own_item.item, &own) {
            live.tell(t.id, format!("the SCREEN ROUTE ({}: its pose does not fit the file's model: {e})", b.name));
            continue;
        }
        let own_world = match bones_now(own_item.item, &own, b.er_pos) {
            Ok((w, r)) => {
                // (round 3) what the pose read found, once per target and model item
                if !live.pose_told.contains(&(t.id, own_item.item)) {
                    live.pose_told.push((t.id, own_item.item));
                    erlog!("zdraw: target {}: {} pose read: {r}", t.id, b.name);
                }
                w
            }
            Err(e) => {
                // (round 3: retried every frame; the reason with its values, told when it changes)
                live.tell(t.id, format!("the SCREEN ROUTE ({}: its bones cannot be read this frame: {e}; retried every frame)", b.name));
                continue;
            }
        };
        let mask = disp_mask(b.npc_param);
        let shown = |g: u8| mask.is_none_or(|m| m & (1 << g) != 0);
        let mut tris: Vec<Tri> = kh_er_render::zdraw::skin(&own, &own_world, &shown, &to_game);
        let mut parts_used = 0;
        for (it, m) in models.iter().skip(1) {
            let ok = live.check(it.item, m).and_then(|_| {
                let (w, _) = bones_now(it.item, m, b.er_pos).map_err(|e| format!("its bones cannot be read: {e}"))?;
                part_fits(m, &w, &own, &own_world)?;
                Ok(w)
            });
            match ok {
                Ok(w) => {
                    // (a part's own display groups belong to its wearer's equipment: all of them are shown)
                    tris.extend(kh_er_render::zdraw::skin(m, &w, &|_| true, &to_game));
                    parts_used += 1;
                }
                Err(e) => {
                    if !live.parts_told.iter().any(|p| p.0 == it.item && p.1 == e) {
                        erlog!("zdraw: target {}: part {} left out ({e})", t.id, it.key);
                        live.parts_told.push((it.item, e));
                    }
                }
            }
        }
        // (a part far outside its reach, e.g. a ground effect mesh not collapsed: left out)
        let (reach, top) = kh_er_render::finale::reach(&t.body);
        let feet = t.body.feet / 100.0;
        let (reach, top) = (reach * 1.6 / 100.0, top * 1.6 / 100.0);
        tris.retain(|x| Vec3::new(x.c.x - feet.x, x.c.y - feet.y, 0.0).length() < reach && x.c.z - feet.z < top);
        if tris.is_empty() {
            live.tell(t.id, format!("the SCREEN ROUTE ({}: nothing of its model is shown)", b.name));
            continue;
        }
        let n = tris.len();
        let words = format!(
            "{}, {} bones, {}{n} triangles shown, display mask {}",
            b.name,
            own.bone_names.len(),
            if models.len() > 1 { format!("{parts_used} of {} worn parts, ", models.len() - 1) } else { String::new() },
            mask.map_or("unknown: all groups".to_string(), |m| format!("{m:#010x}"))
        );
        // its body in the light (the test level's light grey, by `immerse`) while the game still draws it
        // (once the silhouette has darkened past a third its colour layer covers it whole: no light grey under it)
        if t.crumble <= 0.0 && t.hidden_for.is_none() && !(t.silhouette && t.dark >= 0.35) {
            let mut soup = Vec::with_capacity(n * 3);
            kh_er_render::zdraw::body_soup(&tris, &mut soup);
            bodies.push((t.id, t.immerse, soup));
        }
        if !t.silhouette {
            live.tell(t.id, format!("OUR OWN MODEL in the light ({words})"));
            continue;
        }
        if let Some(s) = live.driver.step(&ask, Some(&tris)) {
            live.tell(t.id, format!("OUR OWN MODEL ({words})"));
            soups.push((t.id, s));
        }
        if let Some((id, cut, took)) = live.driver.last_cut.take() {
            erlog!("zdraw: target {id}: broken apart from its own model ({cut} triangles cut into its pieces in {:.1} ms, on its own thread)", took.as_secs_f64() * 1000.0);
        }
    }
    // (a cut finished for a body read no more this frame: logged next time)
    kh_er_render::zdraw::set_frame(soups, bodies);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(names: &[&str]) -> Model {
        Model { bone_names: names.iter().map(|s| s.to_string()).collect(), ..Default::default() }
    }

    #[test]
    fn zdraw_character_model_names() {
        assert!(chr_name("c4311") && chr_name("c0000"));
        assert!(!chr_name("bd_m_1100") && !chr_name("c431") && !chr_name("cx311"));
    }

    #[test]
    fn zdraw_a_worn_part_must_lie_on_its_wearer() {
        let body = model(&["Pelvis", "Spine", "Head"]);
        let body_world = [Mat4::from_translation(Vec3::new(0.0, 1.0, 0.0)), Mat4::from_translation(Vec3::new(0.0, 1.2, 0.0)), Mat4::from_translation(Vec3::new(0.0, 1.7, 0.0))];
        // an armour on the spine: its bone where the body's is
        let armour = model(&["Spine", "Cloth1"]);
        assert!(part_fits(&armour, &[body_world[1], Mat4::from_translation(Vec3::new(0.0, 1.1, 0.2))], &body, &body_world).is_ok());
        // the same armour posed elsewhere (its matrices not the wearer's pose): left out
        assert!(part_fits(&armour, &[Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0)), Mat4::IDENTITY], &body, &body_world).is_err());
        // a weapon (no bone of the body) in a hand: taken; one far away: left out
        let sword = model(&["WP_A_0200", "dymmy"]);
        assert!(part_fits(&sword, &[Mat4::from_translation(Vec3::new(0.4, 1.0, 0.3)), Mat4::IDENTITY], &body, &body_world).is_ok());
        assert!(part_fits(&sword, &[Mat4::from_translation(Vec3::new(40.0, 1.0, 0.3)), Mat4::IDENTITY], &body, &body_world).is_err());
    }
}
