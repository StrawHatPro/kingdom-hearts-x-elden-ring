//! (MIMIC-RENDER, THE USER 2026-11-05: "Sora's copy should only be silver for three seconds like the rest") THE SORA COPIES ON THE
//! RENDERER'S OWN SORA ROUTE (crate::sora_copy has the rule and the host's side): each copy in his KH3 look is a second instance
//! of Sora's models (the main pack's `sora` mesh or the worn outfit's, the Keyblade in HIS hand, HIS form weapon parts) drawn by
//! exactly the passes the player's draws take (gfx.rs `Core::draw`, after the player's own draws): his colour draws with the
//! slots' own pipeline states (both Styles: the frame's key picks the player's set), his early depth with the characters'
//! stencil, his records for the hybrid's G-buffer pass, his hair fringe, his glow mask, his motion vectors and real shadow
//! (`velo_skins`). His own frame block (the player's with no blood and no camera fade), his own bone-driven material
//! parameters (fed for his draws, the player's fed back after), his Keyblade's opacity, no Rage Form look.
//! Nothing here runs without a copy submitted: the player's frame is then recorded exactly as before.

use std::collections::HashMap;
use std::sync::Mutex;

use glam::{Mat4, Vec3};
use kh_anim::Trs;
use kh_look::library::{ObjectRows, Slot};
use windows::Win32::Graphics::Direct3D12::*;

use super::{math, motion_continues, Assets, MAX_JOINTS, MAX_JOINTS_OUTFIT, SLOT_TEXTURES};
use crate::copy_wear::{files_of, Choice, CopyWears, TablePool};
use crate::hair::HairDraw;
use crate::sora_copy::SoraCopyDraw;

/// A skin of the frame before: its floats, its frame number, where its component stood.
type PrevSkin = (Vec<f32>, u64, Trs);
/// Each copy's skin of the frame before per (copy, model): his motion vectors (as the player's `prev_skin`).
static PREV: Mutex<Option<HashMap<(usize, usize), PrevSkin>>> = Mutex::new(None);

/// What the copies' draws need of the frame (gfx.rs `Core::draw` at the end of the player's draws).
pub(super) struct CopyCx<'a> {
    /// (MIMIC-BOSS hotfix 3, MB3-FORMLEAK) the device (his own descriptor tables are written with it)
    pub device: &'a ID3D12Device,
    pub list: &'a ID3D12GraphicsCommandList,
    /// The early list and where its stencil-writing early-depth states start (None: no early depth this frame).
    pub early: Option<(&'a ID3D12GraphicsCommandList, usize)>,
    /// The frame's pipeline states (the player's set for this key: both Styles).
    pub pipelines: &'a [ID3D12PipelineState],
    /// The hair fringe's states (None: no fringe this frame).
    pub hair_set: Option<&'a [Option<ID3D12PipelineState>]>,
    /// The hybrid's G-buffer pass runs (his records go to `gb_draws`).
    pub hy_on: bool,
    /// The glow mask / light shafts run (his opaque records go to `glow_draws`).
    pub glow_on: bool,
    /// The player's frame block as uploaded (every row the frame wrote into it) and its two bindings to put back.
    pub block: &'a [f32],
    pub blood_at: usize,
    pub frame_constants: u64,
    pub early_constants: u64,
    pub rig: &'a kh_look::Rig,
    pub sh: [glam::Vec4; 8],
    pub env_level: f32,
    pub env_tint: Vec3,
    pub look: kh_look::library::LookFrame,
    /// `r.only` (-1 = all slots).
    pub only: i32,
    pub frame_no: u64,
    pub camera_game: Vec3,
    pub height: u32,
    pub tan_y: f32,
    /// The player's pose (his bone parameters are fed back after the copies' draws) and the frame clock (his outfit's frame).
    pub player: Option<&'a crate::Body>,
}

/// Where the copies' records go (the frame's own lists).
pub(super) struct CopyOut<'a> {
    pub gb_draws: &'a mut Vec<HairDraw>,
    pub glow_draws: &'a mut Vec<HairDraw>,
    pub velo_skins: &'a mut Vec<(usize, u64, u64)>,
    pub early_draws: &'a mut u32,
}

/// THE COPY'S FRAME BLOCK: the player's as uploaded, without his blood (blood.rs block amounts) and his camera fade (camfade.rs).
pub(super) fn copy_block(block: &[f32], blood_at: usize) -> Vec<f32> {
    let mut b = block.to_vec();
    if let Some(amounts) = b.get_mut(blood_at..blood_at + 2) {
        amounts.fill(0.0);
    }
    if let Some(fade) = b.get_mut(crate::camfade::AMOUNT_AT) {
        *fade = 0.0;
    }
    b
}

/// The early list's copy of a block: the hybrid's block off (the early depth runs inside the deferred stage, as `early_constants`).
pub(super) fn early_block(block: &[f32]) -> Vec<f32> {
    let mut b = block.to_vec();
    let n = b.len();
    if n >= crate::hybrid::BLOCK_FLOATS {
        b[n - crate::hybrid::BLOCK_FLOATS..].fill(0.0);
    }
    b
}

/// What a copy changes on a slot's scalars: (`OpacityFade` to set when his opacity differs from the slot's, `AntiVolume` to
/// zero: the player's Rage Form look is not his).
pub(super) fn slot_changes(scalars: &HashMap<String, f32>, opacity: Option<f32>) -> (Option<f32>, bool) {
    let fade = opacity.filter(|o| scalars.get("OpacityFade").is_some_and(|now| (now - o).abs() > 1.0e-4));
    (fade, scalars.get("AntiVolume").is_some_and(|v| *v != 0.0))
}

/// A slot as a copy draws it: his Keyblade's / form part's opacity, no Rage Form. None = the slot as it is (no clone).
/// (MIMIC-BOSS hotfix 3, MB3-FORMLEAK) `own_look`: his own wear state set the slot's Rage Form parameters (his AntiVolume is kept).
fn copy_slot(slot: &Slot, opacity: Option<f32>, own_look: bool) -> Option<Slot> {
    let (fade, anti) = slot_changes(&slot.scalars, opacity);
    let anti = anti && !own_look;
    if fade.is_none() && !anti {
        return None;
    }
    let mut s = slot.clone();
    if let Some(o) = fade {
        s.scalars.insert("OpacityFade".to_string(), o);
    }
    if anti {
        s.scalars.insert("AntiVolume".to_string(), 0.0);
    }
    Some(s)
}

/// (MIMIC-BOSS hotfix 3, MB3-FORMLEAK) THE COPIES' OWN LOOKS of the drawn library (crate::copy_wear: each copy's wear form, his Rage fade) and THEIR
/// DESCRIPTOR TABLES (after the slots' tables and the frame's three, gfx.rs `Assets::create`): a slot whose textures a copy
/// wears otherwise than the player's table holds now is bound from a table of his own. The KH route's draws, the Mimic route's
/// (gfx.rs) and every pass that replays their records (the hybrid's G-buffer, the Mimic G-buffer, the glow mask, the shaft
/// mask: [`CopyLooks::table_of_record`]) take it. A copy never writes the player's library or tables.
pub(super) struct CopyLooks {
    pub(super) wears: CopyWears,
    pool: TablePool,
    /// The first copy table of the heap.
    first: usize,
    /// This frame: (owner, slot) -> his table; a record's material constants -> its table (only where it is not the slot's).
    owned: HashMap<(u8, usize), usize>,
    by_material: HashMap<u64, usize>,
}

/// What writing a copy's table needs of the assets.
pub(super) struct CopyGpu<'a> {
    pub device: &'a ID3D12Device,
    pub heap: &'a ID3D12DescriptorHeap,
    pub srv_size: usize,
    pub by_file: &'a HashMap<String, ID3D12Resource>,
    pub white: &'a ID3D12Resource,
}

/// One copy's frame: his drives' owner (copy member m = m + 1), HIS core's wear form, the game clock, the frame number.
#[derive(Clone, Copy, Debug)]
pub(super) struct LookStep {
    pub owner: u8,
    pub wear: u8,
    pub clock: f32,
    pub frame: u64,
}

impl CopyLooks {
    /// For a library as loaded; its tables from `first` on (`TablePool::count_for` of them).
    pub(super) fn new(lib: &kh_look::library::Library, first: usize) -> CopyLooks {
        CopyLooks { wears: CopyWears::new(lib), pool: TablePool::new(TablePool::count_for(lib), super::FRAME_SLOTS as u64 + 1), first, owned: HashMap::new(), by_material: HashMap::new() }
    }

    /// The heap's copy tables.
    pub(super) fn tables(&self) -> std::ops::Range<usize> {
        self.first..self.first + self.pool.tables.len()
    }

    /// Every frame, before any copy is drawn: this frame's bindings start empty; the copies gone are forgotten.
    pub(super) fn begin_frame(&mut self, frame: u64) {
        self.owned.clear();
        self.by_material.clear();
        self.wears.forget_gone(frame);
    }

    /// HIS LOOK THIS FRAME: his wear stepped once (his own form, never the player's), and a table of his own for every slot
    /// whose textures he wears otherwise than the player's table holds now. Returns his key (the owner).
    pub(super) unsafe fn prepare(&mut self, gpu: &CopyGpu, lib: &kh_look::library::Library, step: LookStep) -> u8 {
        let before = self.wears.get(step.owner).map(crate::copy_wear::CopyWear::worn);
        let w = self.wears.step(lib, step.owner, step.wear, step.clock, step.frame);
        let worn = w.worn();
        if before != Some(worn) {
            crate::rlog!("render: (MIMIC-BOSS hotfix 3, MB3-FORMLEAK) the copy (owner {}) wears HIS OWN form: worn {} -> {} (his core's wear form {}, the player's {}; his Rage look {:.2})", step.owner, worn.0, worn.1, step.wear, crate::wear_form(), w.volume());
        }
        let own: Vec<(usize, Vec<Option<kh_look::library::TextureRef>>)> = (0..lib.slots.len()).filter(|k| w.differs(lib, *k)).filter_map(|k| Some((k, w.textures(k)?.to_vec()))).collect();
        for (k, textures) in own {
            if self.owned.contains_key(&(step.owner, k)) {
                continue;
            }
            match self.pool.choose(k, &files_of(&textures), step.frame) {
                Choice::Hit(t) => {
                    self.owned.insert((step.owner, k), self.first + t);
                }
                Choice::Write(t) => {
                    write_textures(gpu, self.first + t, &textures);
                    self.owned.insert((step.owner, k), self.first + t);
                }
                // (none free: the player's table this frame; the pool holds twice what the copies drawn can use)
                Choice::Full => {}
            }
        }
        step.owner
    }

    /// The slot a copy with a look draws for `slot` (his own Rage twin); None: no look of his (the route's own rule).
    pub(super) fn look_slot(&self, look: Option<u8>, slot: usize) -> Option<usize> {
        look.and_then(|o| self.wears.get(o)).map(|w| w.draw_slot(slot))
    }

    /// HIS MATERIAL of a slot: the player's drives taken off and his put on (vfx.rs `copy_drive_slot`, `drives` = his owner),
    /// then his wear (his slide, his Rage parameters, his textures). None = the library's slot as it is.
    pub(super) fn material(&self, lib: &kh_look::library::Library, look: Option<u8>, drives: Option<u8>, slot: usize) -> Option<Slot> {
        let base = lib.slots.get(slot)?;
        let driven = drives.and_then(|o| crate::vfx::copy_drive_slot(lib, slot, o));
        let Some(w) = look.and_then(|o| self.wears.get(o)) else { return driven };
        if !w.needs(lib, slot, driven.as_ref().unwrap_or(base), crate::rage::volume()) {
            return driven;
        }
        let mut own = driven.unwrap_or_else(|| base.clone());
        w.apply(slot, &mut own);
        Some(own)
    }

    /// His Rage glow rows on a material block (rage.rs `rows_with`: his head, his fade).
    pub(super) fn rage_rows(&self, look: Option<u8>, material: &Slot, m: &mut [glam::Vec4], head: Option<[f32; 3]>, clock: f32) {
        if let Some(w) = look.and_then(|o| self.wears.get(o)) {
            crate::rage::rows_with(material, m, head, w.volume(), clock);
        }
    }

    /// The table a copy's draw of `slot` binds (the slot's own unless he has one).
    pub(super) fn table(&self, look: Option<u8>, slot: usize) -> usize {
        look.and_then(|o| self.owned.get(&(o, slot))).copied().unwrap_or(slot)
    }

    /// A record drawn with `table` (its material constants are its name in this frame).
    pub(super) fn note(&mut self, material: u64, table: usize, slot: usize) {
        if table != slot {
            self.by_material.insert(material, table);
        }
    }

    /// The table a replayed record binds: a copy's own, else its slot's.
    pub(super) fn table_of_record(&self, slot: usize, material: u64) -> usize {
        self.by_material.get(&material).copied().unwrap_or(slot)
    }
}

/// A copy table's material textures (the slots' rule of gfx.rs `Assets::create`; its fixed entries are written there and with
/// the slots' own: the colour table, the cube, the game data, the hybrid's capture).
unsafe fn write_textures(gpu: &CopyGpu, table: usize, textures: &[Option<kh_look::library::TextureRef>]) {
    let start = gpu.heap.GetCPUDescriptorHandleForHeapStart();
    for texture in 0..kh_look::library::SLOT_TEXTURES_MAX {
        let resource = textures.get(texture).and_then(Option::as_ref).and_then(|t| gpu.by_file.get(&t.file)).unwrap_or(gpu.white);
        gpu.device.CreateShaderResourceView(resource, None, D3D12_CPU_DESCRIPTOR_HANDLE { ptr: start.ptr + (table * SLOT_TEXTURES + super::texture_descriptor(texture)) * gpu.srv_size });
    }
}

/// THE BONE-DRIVEN MATERIAL PARAMETERS of a pose (gfx.rs `Core::draw`, the materials' state: the same rule), with an outfit's
/// posed bones (locals, components) when one is worn.
fn bone_values(assets: &Assets, body: &crate::Body, outfit: Option<(&crate::outfit::Retarget, &[Trs], &[Trs])>) -> Vec<kh_look::BoneValue> {
    kh_look::bone_parameters(&assets.library.bone_table, |name| {
        if let Some((r, locals, comps)) = outfit {
            if let Some(k) = r.index_of(name) {
                let component = *comps.get(k)?;
                return Some((*locals.get(k)?, component, component.then(&body.component_to_world)));
            }
        }
        let bone = *assets.bones.get(name)?;
        let component = *body.component.get(bone)?;
        Some((*body.local.get(bone)?, component, component.then(&body.component_to_world)))
    })
}

/// One model of a copy this frame: its place, its pose, the opacity its slots take (None: the body's).
struct Item {
    model_index: usize,
    to_world: Trs,
    pose: Vec<Trs>,
    opacity: Option<f32>,
}

/// The models of a copy and their poses (the player's rules of gfx.rs, with HIS keyblade, weapon and form parts).
fn items_of(assets: &Assets, copy: &SoraCopyDraw, outfit: Option<&crate::outfit::Retarget>, outfit_pose: Option<&[Trs]>) -> Vec<Item> {
    let hand = crate::sora_copy::hand_for(copy.keyblade.as_deref());
    let body = &copy.body;
    let in_hand = |w: &Trs| match (outfit, outfit_pose) {
        (Some(r), Some(p)) => r.weapon_to_world(w, &body.component, p, &body.component_to_world),
        _ => *w,
    };
    let mut out = Vec::new();
    for (model_index, model) in assets.models.iter().enumerate() {
        // (a routed fragment model is drawn once per routed instance of the player's frame: not for a copy)
        if model.mesh.strip_prefix(crate::xsiweapon::PREFIX).is_some_and(crate::xsiweapon::is_shard_model) {
            continue;
        }
        let (to_world, pose, opacity) = match model.mesh {
            "sora" => (body.component_to_world, if model.retarget.is_some() { outfit_pose.map_or_else(|| body.component.clone(), <[Trs]>::to_vec) } else { body.component.clone() }, None),
            m => match crate::keyblade::role(m, &hand, &copy.forms) {
                crate::keyblade::Role::Skip => continue,
                crate::keyblade::Role::Form => match crate::keyblade::form_for(m, None, &copy.forms) {
                    Some(f) => {
                        let own = if !f.component.is_empty() && f.component.len() == model.reference.len() { f.component.clone() } else { model.reference.clone() };
                        match (outfit, outfit_pose) {
                            // (FORMWEP-FIX) a transform unit at the copy's mesh origin: its pieces follow the outfit's hands (as the player's)
                            (Some(r), Some(p)) if f.to_world == body.component_to_world => {
                                let skeleton = crate::keyblade::table().and_then(|t| t.model(m.strip_prefix(crate::keyblade::PREFIX)?)).and_then(|k| k.skeleton.clone());
                                let pose = skeleton.map_or_else(|| own.clone(), |sk| r.mesh_weapon_pose(&f.to_world, &own, &sk, &body.component, p, &body.component_to_world));
                                (f.to_world, pose, Some(1.0))
                            }
                            _ => (in_hand(&f.to_world), own, Some(1.0)),
                        }
                    }
                    None => continue,
                },
                crate::keyblade::Role::Hand => match &copy.weapon {
                    Some(w) if w.opacity > 0.0 => (in_hand(&w.to_world), if w.component.len() == model.reference.len() { w.component.clone() } else { model.reference.clone() }, Some(w.opacity)),
                    _ => continue,
                },
            },
        };
        out.push(Item { model_index, to_world, pose, opacity });
    }
    out
}

/// THE COPIES' DRAWS (after the player's, before the Mimic route's): returns the colour draws made. A model whose constants do
/// not fit in the upload buffer is left out (never the frame).
pub(super) unsafe fn record(cx: &CopyCx, assets: &mut Assets, copies: &[&SoraCopyDraw], put: &mut dyn FnMut(&[f32]) -> Option<u64>, out: CopyOut<'_>) -> u32 {
    if copies.is_empty() {
        return 0;
    }
    let CopyOut { gb_draws, glow_draws, velo_skins, early_draws } = out;
    let block = copy_block(cx.block, cx.blood_at);
    let Some(copy_constants) = put(&block) else {
        crate::sora_copy::set_status("not drawn: the upload buffer is full".to_string());
        return 0;
    };
    let copy_early = if cx.early_constants == cx.frame_constants { Some(copy_constants) } else { put(&early_block(&block)) };
    let outfit_at = assets.models.iter().position(|m| m.retarget.is_some());
    let mut prev_guard = PREV.lock().unwrap_or_else(|e| e.into_inner());
    let prev = prev_guard.get_or_insert_with(HashMap::new);
    let srv_stride = (SLOT_TEXTURES * assets.srv_size) as u64;
    let mut drawn = 0u32;
    let mut skin: Vec<f32> = Vec::with_capacity(16 * (1 + MAX_JOINTS_OUTFIT));
    // (MIMIC-BOSS hotfix 3, MB3-FORMLEAK) (material constants, table, slot) of his records: the passes that replay them bind his table
    let mut notes: Vec<(u64, usize, usize)> = Vec::new();
    cx.list.SetGraphicsRootConstantBufferView(0, copy_constants);
    if let (Some((la, _)), Some(at)) = (cx.early, copy_early) {
        la.SetGraphicsRootConstantBufferView(0, at);
    }
    for (n, copy) in copies.iter().enumerate() {
        // (an outfit: its retargeted pose without its cloth simulation, which is the player's: outfit.rs `pose_full`); his
        // bone-driven parameters (eyes, mouth, the face's head frame) onto the library for his draws
        let (values, items) = {
            let a: &Assets = assets;
            let r = outfit_at.and_then(|i| a.models[i].retarget.as_ref());
            let posed = r.map(|r| r.pose_full(&copy.body.local, &copy.body.component));
            (bone_values(a, &copy.body, r.zip(posed.as_ref()).map(|(r, (l, c))| (r, l.as_slice(), c.as_slice()))), items_of(a, copy, r, posed.as_ref().map(|(_, c)| c.as_slice())))
        };
        assets.library.feed("sora", &values);
        // (MIMIC-BOSS hotfix 3, MB3-FORMLEAK) HIS OWN WEAR FORM (his slide, his Rage look, his tables), never the player's
        let look = match (copy.owner, copy.wear_form) {
            (Some(owner), Some(wear)) => {
                let gpu = CopyGpu { device: cx.device, heap: &assets.srv_heap, srv_size: assets.srv_size, by_file: &assets.by_file, white: &assets.white };
                Some(assets.copies.prepare(&gpu, &assets.library, LookStep { owner, wear, clock: cx.look.clock, frame: cx.frame_no }))
            }
            _ => None,
        };
        let head = look.and(crate::rage::head_point(&assets.library));
        let assets: &Assets = assets;
        // (HAIRPERF) his hair's coverage at one point a pixel, as the player's
        let hair_point = cx.hair_set.is_some() && {
            let head = assets.bones.get("atama").and_then(|b| copy.body.component.get(*b)).map(|t| t.then(&copy.body.component_to_world).translation).filter(|h| h.is_finite());
            let d = (head.unwrap_or(copy.body.component_to_world.translation) - cx.camera_game).length().max(1.0);
            crate::hair::point_wanted(cx.height as f32 / (2.0 * cx.tan_y.max(1.0e-3) * d))
        };
        // the skins (both passes use them)
        let mut skins: Vec<Option<(u64, u64)>> = Vec::with_capacity(items.len());
        for item in &items {
            let model = &assets.models[item.model_index];
            skin.clear();
            skin.extend_from_slice(&math::game_from_pack(&item.to_world).to_cols_array());
            for (joint, bone) in model.joint_bones.iter().enumerate() {
                let matrix = match bone.and_then(|b| item.pose.get(b)) {
                    Some(t) => math::joint_in_pack(t) * model.inverse_bind[joint],
                    None => Mat4::IDENTITY,
                };
                skin.extend_from_slice(&matrix.to_cols_array());
            }
            skin.resize(16 * (1 + if model.retarget.is_some() { MAX_JOINTS_OUTFIT } else { MAX_JOINTS }), 0.0);
            if let Some(r) = &model.retarget {
                r.write_leg_guard(&mut skin, &item.pose);
            }
            let Some(at) = put(&skin) else {
                skins.push(None);
                continue;
            };
            let p = prev.entry((n, item.model_index)).or_insert_with(|| (Vec::new(), 0, Trs::IDENTITY));
            let before = if p.0.len() == skin.len() && motion_continues(p.1, &p.2, cx.frame_no, &item.to_world) { put(&p.0).unwrap_or(at) } else { at };
            p.0.clone_from(&skin);
            (p.1, p.2) = (cx.frame_no, item.to_world);
            velo_skins.push((item.model_index, at, before));
            skins.push(Some((at, before)));
        }
        let mut hair: Vec<HairDraw> = Vec::new();
        for translucent in [false, true] {
            // the hair fringe between his opaque / masked and his translucent draws (his own, always in his colour list)
            if let (true, Some(set)) = (translucent, cx.hair_set) {
                if !hair.is_empty() {
                    crate::hair::draw_fringe(cx.list, set, &hair, &|m| assets.models.get(m).map(|model| (model.vertex_view, model.index_view)), assets.srv_heap.GetGPUDescriptorHandleForHeapStart().ptr, srv_stride);
                }
            }
            for (item, made) in items.iter().zip(skins.iter()) {
                let Some((at, before)) = *made else { continue };
                let model = &assets.models[item.model_index];
                if !model.draws.iter().any(|d| assets.library.slots[d.slot].translucent == translucent) {
                    continue;
                }
                let object = ObjectRows::new(&item.to_world, assets.library.bounds_of(if model.mesh.starts_with(crate::xsiweapon::PREFIX) && crate::xsiweapon::table().is_none_or(|t| t.own_materials.is_none()) { "kingdom_key" } else { model.mesh }));
                cx.list.IASetVertexBuffers(0, Some(&[model.vertex_view]));
                cx.list.IASetIndexBuffer(Some(&model.index_view));
                cx.list.SetGraphicsRootConstantBufferView(2, at);
                cx.list.SetGraphicsRootConstantBufferView(6, before);
                if let (Some((la, _)), false) = (cx.early, translucent) {
                    la.IASetVertexBuffers(0, Some(&[model.vertex_view]));
                    la.IASetIndexBuffer(Some(&model.index_view));
                    la.SetGraphicsRootConstantBufferView(2, at);
                    la.SetGraphicsRootConstantBufferView(6, before);
                }
                for draw in &model.draws {
                    // (no Rage Form twin of the player's: the player's Rage look is not his) (MIMIC-BOSS hotfix 3, MB3-FORMLEAK) his own twin by HIS Rage look
                    let slot = assets.copies.look_slot(look, draw.slot).unwrap_or(draw.slot);
                    let base = &assets.library.slots[slot];
                    if base.translucent != translucent || (cx.only >= 0 && cx.only as usize != slot) {
                        continue;
                    }
                    // (MIMIC-BOSS drives fix) HIS material drives (form glow, in-light, aura), not the player's
                    // (MIMIC-BOSS hotfix 3, MB3-FORMLEAK) and HIS wear (slide, Rage parameters, textures)
                    let driven = assets.copies.material(&assets.library, look, copy.owner, slot);
                    let base = driven.as_ref().unwrap_or(base);
                    let own = copy_slot(base, item.opacity, look.is_some());
                    let material = own.as_ref().unwrap_or(base);
                    let weapon_light = crate::vfx::weapon_in_light(material, &object);
                    let material = weapon_light.as_ref().unwrap_or(material);
                    let mut params = assets.library.params(material, &object, cx.rig, assets.khs.as_ref().map(|(_, s)| s), cx.look);
                    params.sh = cx.sh;
                    params.khs.w = material.scalars.get(crate::outfit::INTERIOR_SCALAR).copied().unwrap_or(0.0);
                    params.look.w = crate::cgfield::real_sss();
                    params.fade.z = f32::from(u8::from(hair_point));
                    params.env.y = cx.env_level;
                    (params.env.z, params.env.w) = (cx.env_tint.x - 1.0, cx.env_tint.z - 1.0);
                    assets.copies.rage_rows(look, material, &mut params.m, head, cx.look.clock); // (MIMIC-BOSS hotfix 3, MB3-FORMLEAK) his Rage glow, not the player's
                    let Some(material_constants) = put(params.as_floats()) else { continue };
                    let Some(state) = cx.pipelines.get(slot) else { continue };
                    // (MIMIC-BOSS hotfix 3, MB3-FORMLEAK) his own table where he wears other textures than the player's slot holds
                    let table = assets.copies.table(look, slot);
                    notes.push((material_constants, table, slot));
                    cx.list.SetPipelineState(state);
                    cx.list.SetGraphicsRootConstantBufferView(1, material_constants);
                    cx.list.SetGraphicsRootDescriptorTable(3, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: assets.srv_heap.GetGPUDescriptorHandleForHeapStart().ptr + table as u64 * srv_stride });
                    cx.list.DrawIndexedInstanced(draw.index_count, 1, draw.first_index, draw.base_vertex, 0);
                    drawn += 1;
                    if translucent {
                        continue;
                    }
                    let rec: HairDraw = (item.model_index, slot, draw.index_count, draw.first_index, draw.base_vertex, at, before, material_constants);
                    if cx.hair_set.is_some_and(|s| s.get(slot).is_some_and(Option::is_some)) {
                        hair.push(rec);
                    }
                    if cx.glow_on {
                        glow_draws.push(rec);
                    }
                    // his early depth (+ stencil), then his record for the hybrid's G-buffer pass
                    if let Some((la, early_at)) = cx.early {
                        if let Some(state) = cx.pipelines.get(early_at + slot) {
                            la.SetPipelineState(state);
                            la.SetGraphicsRootConstantBufferView(1, material_constants);
                            la.SetGraphicsRootDescriptorTable(3, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: assets.srv_heap.GetGPUDescriptorHandleForHeapStart().ptr + table as u64 * srv_stride });
                            la.DrawIndexedInstanced(draw.index_count, 1, draw.first_index, draw.base_vertex, 0);
                            *early_draws += 1;
                            if cx.hy_on {
                                gb_draws.push(rec);
                            }
                        }
                    }
                }
            }
        }
    }
    prev.retain(|(n, _), _| *n < copies.len());
    drop(prev_guard);
    for (material, table, slot) in notes {
        assets.copies.note(material, table, slot);
    }
    // the player's own bindings and bone parameters again (as his draws had them)
    cx.list.SetGraphicsRootConstantBufferView(0, cx.frame_constants);
    if let Some((la, _)) = cx.early {
        la.SetGraphicsRootConstantBufferView(0, cx.early_constants);
    }
    if let Some(body) = cx.player {
        let values = {
            let a: &Assets = assets;
            let r = outfit_at.and_then(|i| a.models[i].retarget.as_ref());
            let posed = r.map(|r| r.frame(cx.look.clock, &body.component_to_world, &body.local, &body.component));
            bone_values(a, body, r.zip(posed.as_ref()).map(|(r, p)| (r, p.0.as_slice(), p.1.as_slice())))
        };
        assets.library.feed("sora", &values);
    }
    crate::sora_copy::set_status(format!("drawn: {} copy(ies) in his KH3 look on the Sora route, {drawn} colour draw(s){}", copies.len(), if cx.hy_on { " (hybrid G-buffer)" } else { "" }));
    drawn
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_his_block_has_no_blood_and_no_camera_fade_and_the_early_one_no_hybrid() {
        let n = crate::hybrid::BLOCK_FLOATS + 64;
        let block: Vec<f32> = (0..n).map(|k| k as f32 + 1.0).collect();
        let b = copy_block(&block, 40);
        assert_eq!((b[40], b[41]), (0.0, 0.0), "no blood");
        assert_eq!(b[crate::camfade::AMOUNT_AT], 0.0, "no camera fade");
        assert_eq!(b[42], block[42]);
        assert_eq!(b.len(), block.len());
        assert_eq!(b[n - 1], block[n - 1], "the hybrid's block kept for his colour draws");
        let e = early_block(&b);
        assert!(e[n - crate::hybrid::BLOCK_FLOATS..].iter().all(|x| *x == 0.0));
        assert_eq!(e[..n - crate::hybrid::BLOCK_FLOATS], b[..n - crate::hybrid::BLOCK_FLOATS]);
        // a short block is never indexed out of range
        assert_eq!(copy_block(&[1.0, 2.0], 40), vec![1.0, 2.0]);
    }

    #[test]
    fn copy_his_slots_take_his_opacity_and_never_the_rage_look() {
        let mut scalars: HashMap<String, f32> = HashMap::new();
        assert_eq!(slot_changes(&scalars, None), (None, false), "nothing to change: no clone");
        assert_eq!(slot_changes(&scalars, Some(0.5)), (None, false), "no OpacityFade on the slot: nothing to set");
        scalars.insert("OpacityFade".to_string(), 0.0);
        assert_eq!(slot_changes(&scalars, Some(1.0)), (Some(1.0), false), "the player's sheathed keyblade is not the copy's");
        scalars.insert("OpacityFade".to_string(), 1.0);
        assert_eq!(slot_changes(&scalars, Some(1.0)), (None, false));
        scalars.insert("AntiVolume".to_string(), 0.7);
        assert_eq!(slot_changes(&scalars, None), (None, true), "the player's Rage Form is not on the copy");
    }
}
