//! (ER party S4) THE PARTY IN THE RENDERER'S FRAME: Donald's, Goofy's and the team props' own libraries, models, shaders and
//! pipeline states (one `Member` each, made on the worker like Sora's), drawn inside `Core::draw` by the very passes Sora's
//! draws take (crate::party has the host's side and the list of what each friend gets).
//!
//! ORDER IN A FRAME (Core::draw, every hook marked `(ER party S4)`):
//! 1. `prepare` (after the frame block is made): per member a copy of the frame block with HIS point lights (`_pl`), his
//!    blood (`_bl`), his feet for the contact shadow and his capsules for the thrown shadow; his bone-fed material values,
//!    his skins (now and a frame ago); the game-data texture and the hybrid's light / mask into his tables.
//! 2. `contact_shadows`: his contact / thrown shadow (Sora's blob pipeline, his frame block).
//! 3. `draw_pass` inside Sora's `for translucent` loop, after Sora's draws: colour, early depth (+ stencil), records for the
//!    glow mask, the shaft mask and the G-buffer pass.
//! 4. `glow_pass`, `shaft_pass`, `gbuffer_pass` after Sora's own; the velocity and the real shadow from `VeloFrame::party`.
//! Every switch of the descriptor heap is put back to Sora's before his code goes on, so with no party draw the command
//! lists are exactly Sora's.

use std::collections::HashMap;
use std::ffi::c_void;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use glam::{Mat4, Vec3, Vec4};
use kh_anim::Trs;
use kh_look::library::{Library, ObjectRows};
use windows::core::Interface;
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use super::{buffer_desc, committed, velocity_own_pipeline, early_base, early_stencil_base, gbuffer_base, glow_pipelines, layout, pipelines, texture_desc, transition, upload_buffer, Assets, Core, Draw, GpuModel, PsoKey, Shaders, CORE, DEPTH_TEXTURE, HY_LIGHT_TEXTURE, HY_MASK_TEXTURE, MAX_JOINTS_OUTFIT, SLOT_TEXTURES, UPLOAD_BYTES};
use crate::model::Model;
use crate::party::{CharDraw, TEAM};
use crate::{math, rlog, Body};

/// The floats of a party skin (the party packs' shaders have room for 320 joints: kh-shaderc `--party`).
const SKIN_FLOATS: usize = 16 * (1 + MAX_JOINTS_OUTFIT);
/// The frame block's rows the party copies change (lib.rs / gfx.rs `Core::draw` layout): the contact shadow (row 11) and
/// the thrown shadow's capsules (after the clip matrix, 18 rows and the two host matrices).
const BLOB_AT: usize = 16 + 4 * 11;
const CAPS_AT: usize = 16 + 4 * 18 + 32;
/// (ER party I) his own light probe and light rows: `er.probe` (row 13), the out scale (14), the lit key / ambient / ambient up (15..17).
const PROBE_AT: usize = 16 + 4 * 13;
const LIGHT_AT: usize = 16 + 4 * 14;
/// (RSF) the previous frame's game -> host matrix (`prev_host_from_game`, after the clip matrix, 18 rows and this frame's
/// game -> host): ER88's own-record route writes R x (game -> host) there with its w row 2, which only skin_v88 reads as such.
const PREV_AT: usize = 16 + 4 * 18 + 16;

// (ER party I) each friend's own steady light probe (the coordinator's request after ER86)
#[path = "party_probe.rs"]
pub(super) mod probe;

/// One library of the party (Donald, Goofy, the team props).
pub(super) struct Member {
    pub chara: u8,
    pub npc: &'static str,
    /// (RSF2 round 13) `r.hybrid` was on when his shaders were read (off: read again when it goes on, `Job::Shaders`)
    pub hy_read: bool,
    pub assets: Assets,
    pub shaders: Arc<Shaders>,
    /// The body's model (index in `assets.models`), the models by mesh name (weapons, props).
    pub body: Option<usize>,
    pub by_mesh: HashMap<String, usize>,
    pub sets: HashMap<PsoKey, Vec<ID3D12PipelineState>>,
    pub failed: Vec<PsoKey>,
    pub glow: HashMap<PsoKey, Vec<ID3D12PipelineState>>,
    pub glow_failed: Vec<PsoKey>,
    /// Per model: the skin DRAWN the frame before (his motion vectors), with the renderer frame it was drawn in and where its
    /// component stood (RSF: used only for the very next frame and when the model did not jump; else no motion).
    /// (keyed by model and its ordinal in his frame: two team props of one model keep their own records)
    pub prev_skin: HashMap<(usize, usize), PrevSkin>,
    /// (RSF) his own-record velocity state per pipeline key (ER88's route: velocity_own_pipeline), and the keys it failed for.
    pub velo_own: HashMap<PsoKey, ID3D12PipelineState>,
    pub velo_own_failed: Vec<PsoKey>,
    /// The resource whose view is in every slot's DEPTH_TEXTURE entry (the game-data texture; 0 = white).
    pub data_bound: usize,
    pub hy_views: [usize; 2],
    /// (r.shadow_real) the depth-only pipeline with HIS skin shader (the main pack's shadow.vs has 200 joints).
    pub cast: Option<ID3D12PipelineState>,
    pub cast_failed: bool,
    /// His bones by name (the thrown shadow's capsules, the bone-fed material values, the blood's places).
    pub bones: HashMap<String, usize>,
    /// (TEAM) a skinned prop drawn without a skeleton, in its bind pose: the matrix of every joint (party_team_pack.js's
    /// glbs keep UE centimetres under their `UE_to_glTF` root: x 0.01; a static prop is baked by model.rs: identity).
    pub prop_joint: HashMap<usize, Mat4>,
}

/// (RSF) A model's skin as drawn in a frame: the previous pose of his motion vectors.
pub(super) struct PrevSkin {
    pub skin: Vec<f32>,
    pub frame: u64,
    pub to_world: Trs,
}

use super::motion_continues;
#[cfg(test)]
use super::{SNAP_CM, SNAP_DEG};

/// (RSF2) The rows of a member's frame block that carry a light level, set to HIS out scale `level` (what his shaders multiply
/// their KH-unit result with at the end): the hybrid's k (the game's light -> KH units = 1 / level) and its fog colour (x k).
/// A block with the hybrid off (k 0) stays off. Pure: unit-tested (rsf2_tests).
pub(super) fn rescale_block(frame: &mut [f32], level: f32) {
    let n = frame.len();
    if n < crate::hybrid::BLOCK_FLOATS || !(level.is_finite() && level > 0.0) {
        return;
    }
    let hy = n - crate::hybrid::BLOCK_FLOATS;
    let k = frame[hy + 13];
    if frame[hy] == 0.0 || !(k.is_finite() && k > 0.0) {
        return;
    }
    let ratio = (1.0 / level) / k;
    frame[hy + 13] = 1.0 / level;
    for f in &mut frame[hy + 16..hy + 19] {
        *f *= ratio;
    }
}

/// (RSF2) A member's hybrid rows without Sora's outfit field (gfx.rs `hybrid::block` with `own_set` / `lift` of SORA's own set):
/// the rig share is `r.hybrid_rig`, the lift `r.hybrid_lift` alone. A block with the hybrid off stays off.
pub(super) fn member_hybrid_rows(frame: &mut [f32], s: &crate::hybrid::Settings) {
    let n = frame.len();
    if n < crate::hybrid::BLOCK_FLOATS {
        return;
    }
    let hy = n - crate::hybrid::BLOCK_FLOATS;
    if frame[hy] == 0.0 {
        return;
    }
    frame[hy + 2] = s.rig;
    frame[hy + 31] = s.lift.clamp(0.0, 8.0);
}

/// (RSF2) The light level of the drawn leader when Sora is not drawn: the own out scale of the member standing nearest `carrier`
/// (the hero light's carrier = the leader), else of the first member with a light of his own.
pub(super) fn leader_level(plan: &Plan, carrier: Option<Vec3>) -> Option<f32> {
    let lit = plan.members.iter().filter_map(|m| Some((m.feet?, m.light?.out_scale))).filter(|(_, s)| s.is_finite() && *s > 0.0);
    match carrier {
        Some(c) => lit.min_by(|a, b| (a.0 - c).length().total_cmp(&(b.0 - c).length())).map(|(_, s)| s),
        None => lit.map(|(_, s)| s).next(),
    }
}

/// (RSF) HIS VELOCITY ROUTE, the same rule for every drawn member: with his own-record state (`own_state`) his copy of the frame
/// block stays as it is (skin_v88 reads the own-record matrix, flagged or plain, as Sora's does); without it the plain previous
/// matrix goes into his copy (the plain skin reads only a plain one). True = the own-record state draws him.
pub(super) fn velocity_route(frame: &mut [f32], own_state: bool, prev_plain: &Mat4) -> bool {
    if !own_state && frame.len() >= PREV_AT + 16 {
        frame[PREV_AT..PREV_AT + 16].copy_from_slice(&prev_plain.to_cols_array());
    }
    own_state
}

/// A picture target of a friend's camp panel (gfx/portrait.rs `Kit`, one per friend).
pub(super) struct PicKit {
    slots: Vec<(ID3D12CommandAllocator, ID3D12GraphicsCommandList, ID3D12Resource, *mut u8, u64)>,
    rtv: ID3D12DescriptorHeap,
    dsv: ID3D12DescriptorHeap,
    target: Option<(ID3D12Resource, ID3D12Resource, u32, u32)>,
    khs: Option<(String, kh_look::KhsState)>,
}

#[derive(Default)]
pub(super) struct PartyKit {
    pub members: Vec<Member>,
    pub failed: Vec<u8>,
    /// The pipeline-state keys the party's sets are wanted for (the scene's, the portraits', the void's).
    pub keys: Vec<PsoKey>,
    pub glow_key: Option<PsoKey>,
    pub pics: Vec<(u8, PicKit)>,
    pub pics_failed: Option<Instant>,
    // (for the log, a second's worth)
    pub frames: u32,
    pub draws: u32,
    pub glow_draws: u32,
    pub gb_draws: u32,
    pub early_draws: u32,
    pub shaft_draws: u32,
    pub velo_draws: u32,
    /// (RSF) of them, drawn with his own-record velocity state
    pub velo_own_draws: u32,
    /// (RSF) model skins drawn without motion this second (a jump, a turn, a gap: INT's snap rule)
    pub snaps: u32,
    /// (RSF) PER DRAWN CHARACTER (his pack folder), this second: [frames, early-depth draws, his stencil + 1 (0 = none),
    /// G-buffer draws (inside his box), glow-mask draws, shaft-mask draws, velocity draws, of them on the own-record route]:
    /// every member through the same paths, whoever he is (the log line proves it per character)
    pub tally: Vec<(&'static str, [u32; 8])>,
    /// (RSF2) per drawn character, the latest frame: his light level (out scale; own = from his own probe), Sora's, his reflection level
    pub levels: Vec<(&'static str, f32, bool, f32, f32, Option<(f32, Vec3, Vec3)>)>,
    /// (RSF2) the leader's light level of the frame before (the shared level while Sora is not drawn)
    pub shared_level: Option<f32>,
    pub cast_draws: u32,
    pub pictures: u32,
    pub void_frames: u32,
    pub micros: u64,
    pub micros_max: u64,
    pub waiting: &'static str,
}

// SAFETY: as Core (D3D12 objects are free-threaded; the mapped pointers are only used under CORE's mutex).
unsafe impl Send for PicKit {}

/// What one member draws this frame (made by `prepare`).
pub(super) struct PlanMember {
    pub member: usize,
    pub frame_at: u64,
    pub early_at: u64,
    /// (model index, skin now, skin a frame ago, its component in game space)
    pub models: Vec<(usize, u64, u64, Trs)>,
    pub glow_mask: bool,
    pub caps: Vec<(Vec3, Vec3, f32)>,
    /// (RSF2) his reflection level at the frame's shared light level (None: Sora's, cx.env_level)
    pub env_level: Option<f32>,
    /// (RSF2) the level his light was measured at / the frame's shared level (rig_of)
    pub light_ratio: f32,
    /// (RSF) his screen box's reach (hybrid.rs): his capsules, a ball round every bone of his pose, his weapon's and props'
    /// bounds; the G-buffer pass's scissor and the capture box are made of it (every member alike, no names).
    pub reach: Vec<(Vec3, Vec3, f32)>,
    pub feet: Option<Vec3>,
    /// Offsets of his frame copies in the upload buffer (the hybrid's block is zeroed there when the G-buffer pass fails).
    pub frame_offset: usize,
    pub early_offset: usize,
    /// (ER party I) his own light (his probe at his feet): the rig his materials take; None = Sora's.
    pub light: Option<crate::light::RigLight>,
    /// (ER party I) the game-data texture row his probe is drawn into (gfx.rs probe pass).
    pub probe_row: Option<usize>,
    /// (RSF) his motion vectors take his own-record velocity state (ER88's route) this frame; false = the plain route, his
    /// frame copy then carries the plain previous matrix.
    pub velo_own: bool,
}

/// One draw recorded for the glow / shaft / G-buffer passes: (plan member, model, slot, index count, first index, base
/// vertex, skin now, skin before, material constants).
pub(super) type Rec = (usize, usize, usize, u32, u32, i32, u64, u64, u64);

#[derive(Default)]
pub(super) struct Plan {
    pub members: Vec<PlanMember>,
    /// (RSF) per plan member: early-depth draws this frame; and the stencil his early depth wrote
    pub early_per: Vec<u32>,
    pub stencil: Option<u8>,
    pub glow: Vec<Rec>,
    pub gb: Vec<Rec>,
    pub drawn: u32,
}

/// (VeloFrame) what the velocity pass and the real shadow draw of the party: (member, frame constants, its models'
/// (model index, skin now, skin before)).
/// (RSF) + whether his own-record velocity state draws them.
pub(super) type VeloParty = Vec<(usize, u64, Vec<(usize, u64, u64)>, bool)>;

impl Plan {
    pub fn velo(&self) -> VeloParty {
        self.members.iter().map(|m| (m.member, m.frame_at, m.models.iter().map(|(k, a, b, _)| (*k, *a, *b)).collect(), m.velo_own)).collect()
    }
}

// ---------------------------------------------------------------------------------------------------- the worker

/// The party's shader set (gfx.rs `read_shaders` without its side effects: the same order of preference, the sets of the
/// member's own folder `PACK/party/<npc>/materials/er`).
fn read_shaders(pack_dir: &Path, variants: &[String]) -> Result<Shaders, String> {
    let dir = pack_dir.join("materials").join("er");
    let read = |name: String| std::fs::read(dir.join(&name)).map_err(|e| format!("{}: {e} (kh-shaderc --party)", dir.join(&name).display()));
    let all = |suffix: &str| variants.iter().all(|v| dir.join(format!("{v}{suffix}.ps.dxil")).is_file());
    let own = crate::pointlight::own_shaders_wanted() && all("_pl");
    let blood = own && crate::blood::shaders_wanted() && all("_bl");
    // (HY) the hybrid's third version (`*_hy3` = the *_bl code + the game's light from the renderer's capture; `*_gb3`), complete
    // for every slot and `r.hybrid` on when the member is made: with the hybrid off for a frame it draws what *_bl draws (the
    // ER78b *_hy2 / *_gb2 files are no longer read). A member is not made again when the key changes: off later = the block
    // zero (today's light from the same shaders).
    // (RSF2 round 13, the user: "Donald and Goofy also don't seem affected by r.hybrid=1") A member read with the key OFF is read
    // again when it goes on (worker_tick `Job::Shaders`, `Member::hy_read`): in the user's session the friends were made while
    // r.hybrid was 0 and stayed on the *_bl set for the whole session (G-buffer 0 draws), while Sora switched.
    let hybrid = own && all("_hy3") && all("_gb3") && crate::hybrid::settings().on;
    let mut pixel = HashMap::new();
    let mut gbuffer = HashMap::new();
    for variant in variants {
        let variant = variant.clone();
        if !pixel.contains_key(&variant) {
            pixel.insert(variant.clone(), read(if hybrid { format!("{variant}_hy3.ps.dxil") } else if blood { format!("{variant}_bl.ps.dxil") } else if own { format!("{variant}_pl.ps.dxil") } else { format!("{variant}.ps.dxil") })?);
        }
        if hybrid && !gbuffer.contains_key(&variant) {
            gbuffer.insert(variant.clone(), read(format!("{variant}_gb3.ps.dxil"))?);
        }
    }
    rlog!("render: (ER party S4) {}: shaders {}", pack_dir.display(), if hybrid { "(HY) the *_hy3 + *_gb3 sets (lit by the game's own render pass; blood, point lights)" } else if blood { "the *_bl set" } else if own { "the *_pl set" } else { "the plain set" });
    // (the contact shadow's pair is only a marker here: pipelines() builds the velocity, early-depth and G-buffer states after it)
    let blob = read("blob.vs.dxil".to_string()).and_then(|v| Ok((v, read("blob.ps.dxil".to_string())?))).ok();
    // ((ER87) no hair path for the party: `fringe` empty)
    // (RSF) his own-record velocity vertex shader (kh-shaderc --party --only velo88: NEW file skin_v88.vs.dxil; without it the
    // plain route with the plain previous matrix, see prepare)
    let velo_own = read("skin_v88.vs.dxil".to_string()).ok();
    Ok(Shaders { vertex: read("skin.vs.dxil".to_string())?, blob, velo: read("velo.ps.dxil".to_string()).ok(), pixel, gbuffer, fringe: HashMap::new(), msaa: HashMap::new(), sora_velocity: false, velo_own })
}

/// A model of a party library: its primitives on the library's slots of `mesh`, its joints on `skeleton`'s bones (none:
/// every joint identity, the mesh drawn in its bind pose: the team props).
unsafe fn add_model(device: &ID3D12Device, assets: &mut Assets, mesh: &'static str, file: &Path, skeleton: Option<&kh_anim::Skeleton>) -> Result<usize, String> {
    let model = Model::load(file)?;
    if model.joints.len() > MAX_JOINTS_OUTFIT {
        return Err(format!("{mesh}: {} joints, the shader has room for {MAX_JOINTS_OUTFIT}", model.joints.len()));
    }
    let text = |what: &str, e: windows::core::Error| format!("{what}: {e}");
    let vertices = upload_buffer(device, model.vertex_bytes()).map_err(|e| text("vertex buffer", e))?;
    let indices = upload_buffer(device, model.index_bytes()).map_err(|e| text("index buffer", e))?;
    let library = &assets.library;
    let draws = model
        .primitives
        .iter()
        .filter_map(|p| {
            let slot = library.slots.iter().position(|s| s.mesh == mesh && s.slot == p.material);
            if slot.is_none() {
                rlog!("render: (ER party S4) {mesh}: primitive {} has no material slot: not drawn", p.material);
            }
            Some(Draw { slot: slot?, first_index: p.first_index, index_count: p.index_count, base_vertex: p.base_vertex })
        })
        .collect();
    let reference = |skeleton: &kh_anim::Skeleton| -> Vec<Trs> {
        let mut component: Vec<Trs> = Vec::with_capacity(skeleton.len());
        for bone in &skeleton.bones {
            let t = match bone.parent {
                Some(parent) => bone.reference.then(&component[parent]),
                None => bone.reference,
            };
            component.push(t);
        }
        component
    };
    assets.models.push(GpuModel {
        mesh,
        vertex_view: D3D12_VERTEX_BUFFER_VIEW { BufferLocation: vertices.GetGPUVirtualAddress(), SizeInBytes: model.vertex_bytes().len() as u32, StrideInBytes: std::mem::size_of::<crate::model::Vertex>() as u32 },
        index_view: D3D12_INDEX_BUFFER_VIEW { BufferLocation: indices.GetGPUVirtualAddress(), SizeInBytes: model.index_bytes().len() as u32, Format: DXGI_FORMAT_R32_UINT },
        _vertices: vertices,
        _indices: indices,
        draws,
        joint_bones: model.joints.iter().map(|(name, _)| skeleton.and_then(|s| s.index_of(name))).collect(),
        inverse_bind: model.joints.iter().map(|(_, m)| *m).collect(),
        reference: skeleton.map(reference).unwrap_or_default(),
        retarget: None,
    });
    Ok(assets.models.len() - 1)
}

fn read_json(path: &Path) -> Result<serde_json::Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("{}: {e}", path.display()))
}

fn skeleton_of(manifest: &Path) -> Result<kh_anim::Skeleton, String> {
    let text = std::fs::read_to_string(manifest).map_err(|e| format!("{}: {e}", manifest.display()))?;
    kh_anim::Skeleton::from_manifest(&kh_anim::json::Json::parse(text.trim_start_matches('\u{feff}'))?)
}

/// A member's library, textures, models and shaders (the worker, without CORE's lock).
unsafe fn load_member(device: &ID3D12Device, pack: &Path, chara: u8) -> Result<Member, String> {
    // (RSF) a built-in folder, else the roster's (any later KH character: no code names him)
    let npc: &'static str = match crate::party::dir_of(chara) {
        Some(d) => d,
        None => Box::leak(crate::party::dir_in_roster(pack, chara).ok_or("no such member in PACK/party/party.json")?.into_boxed_str()),
    };
    let dir = pack.join("party").join(npc);
    let mut library = Library::load(&dir)?;
    // the world's post-process material (KHS: the main pack's, as kh-app merges the libraries) and its colour table
    let main = read_json(&pack.join("materials").join("materials.json"))?;
    library.khs = kh_look::Khs::parse(&main);
    if let Some((file, srgb)) = library.khs.table.take() {
        let main = pack.join("materials").join(file);
        library.khs.table = Some((std::path::absolute(&main).unwrap_or(main).to_string_lossy().to_string(), srgb));
    }
    // his bone -> material parameter table (mesh_facts.<npc>: his eyelids and the like)
    let own = read_json(&dir.join("materials").join("materials.json"))?;
    if let Some(facts) = own.get("mesh_facts").and_then(|f| f.get(npc)) {
        library.bone_table = kh_look::BoneParam::parse_table(facts);
    }
    let hy_read = crate::hybrid::settings().on;
    let shaders = read_shaders(&dir, &library.slots.iter().map(|s| s.variant()).collect::<Vec<_>>())?;
    let queue: ID3D12CommandQueue = device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC { Type: D3D12_COMMAND_LIST_TYPE_DIRECT, ..Default::default() }).map_err(|e| format!("CreateCommandQueue: {e}"))?;
    let mut assets = Assets::create_full(device, &queue, &dir, library, "", false)?;
    let mut by_mesh = HashMap::new();
    let mut prop_joint = HashMap::new();
    let mut body = None;
    if chara == TEAM {
        // the props (party_team_pack.js): the meshes of the library's slots, drawn in their bind pose
        let meshes: Vec<String> = assets.library.slots.iter().map(|s| s.mesh.clone()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
        for mesh in meshes {
            let file = dir.join("props").join(format!("{mesh}.glb"));
            let name: &'static str = Box::leak(mesh.clone().into_boxed_str());
            match add_model(device, &mut assets, name, &file, None) {
                Ok(k) => {
                    let skinned = assets.models[k].inverse_bind.len() > 1 || assets.models[k].inverse_bind.first().is_some_and(|m| *m != Mat4::IDENTITY);
                    // (KITFX) a skinned prop in its bind pose: its joint matrix is the joint node's world matrix in the glb (the
                    // UE_to_glTF root included) x its inverse bind, as glTF skinning gives. A plain 0.01 scale left out the
                    // root's Y / Z swap, so Flare Force's missile (o_ex208_0, its vertices in UE axes) stood UPRIGHT in Elden
                    // Ring (the user, 2026-10-23: "the fireworks itself was what was upright"); kh-app draws it through the
                    // glTF scene and was right
                    let bind = crate::party::prop_bind_joint(&file, assets.models[k].inverse_bind.first().copied().unwrap_or(Mat4::IDENTITY));
                    prop_joint.insert(k, if skinned { bind.unwrap_or(Mat4::from_scale(Vec3::splat(0.01))) } else { Mat4::IDENTITY });
                    by_mesh.insert(mesh, k);
                }
                Err(e) => rlog!("render: (ER party S4) team prop {mesh}: {e}: not drawn"),
            }
        }
    } else {
        let skeleton = skeleton_of(&dir.join("manifest.json"))?;
        body = Some(add_model(device, &mut assets, npc, &dir.join("model.glb"), Some(&skeleton))?);
        // his weapons: every weapon mesh his library has slots for (party.json's 14), each with its own skeleton
        let meshes: Vec<String> = assets.library.slots.iter().filter(|s| s.mesh != npc).map(|s| s.mesh.clone()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
        for mesh in meshes {
            let wdir = pack.join("party").join("weapons").join(&mesh);
            let name: &'static str = Box::leak(mesh.clone().into_boxed_str());
            match skeleton_of(&wdir.join("manifest.json")).and_then(|s| add_model(device, &mut assets, name, &wdir.join("weapon.glb"), Some(&s))) {
                Ok(k) => drop(by_mesh.insert(mesh, k)),
                Err(e) => rlog!("render: (ER party S4) {npc}'s weapon {mesh}: {e}: not drawn"),
            }
        }
    }
    Ok(Member { chara, npc, bones: assets.bones.clone(), assets, shaders: Arc::new(shaders), body, by_mesh, sets: HashMap::new(), failed: Vec::new(), glow: HashMap::new(), glow_failed: Vec::new(), prev_skin: HashMap::new(), velo_own: HashMap::new(), velo_own_failed: Vec::new(), data_bound: usize::MAX, hy_views: [usize::MAX; 2], cast: None, cast_failed: false, prop_joint, hy_read })
}

/// THE WORKER'S PARTY JOB (gfx.rs `worker`, when nothing of Sora's waits): a member asked for and not made yet, then the
/// pipeline states of a wanted key, then the glow mask's. True = something was done.
pub(super) fn worker_tick(pack: &Path) -> bool {
    enum Job {
        Load(ID3D12Device, u8),
        Sets(ID3D12Device, ID3D12RootSignature, u8, PsoKey, Vec<(String, bool, bool, bool)>, Arc<Shaders>, bool),
        Shaders(u8, &'static str, Vec<String>),
    }
    let wanted = crate::party::wanted();
    if wanted.is_empty() {
        return false;
    }
    let job = {
        let Ok(guard) = CORE.lock() else { return false };
        let Some(core) = guard.as_ref().filter(|c| !c.lost && !c.disabled && c.assets.is_some()) else { return false };
        let kit = &core.party;
        if let Some(chara) = wanted.iter().copied().find(|c| !kit.members.iter().any(|m| m.chara == *c) && !kit.failed.contains(c)) {
            Some(Job::Load(core.device.clone(), chara))
        } else {
            let mut found = None;
            // (RSF2 round 13) a member read with r.hybrid off while it is on now: his shaders read again first
            if crate::hybrid::settings().on {
                if let Some(m) = kit.members.iter().find(|m| !m.hy_read) {
                    found = Some(Job::Shaders(m.chara, m.npc, m.assets.library.slots.iter().map(|s| s.variant()).collect()));
                }
            }
            'members: for m in kit.members.iter().filter(|_| found.is_none()) {
                let slots = || m.assets.library.slots.iter().map(|s| (s.variant(), s.two_sided, s.translucent, s.masked)).collect::<Vec<_>>();
                for key in &kit.keys {
                    if !m.sets.contains_key(key) && !m.failed.contains(key) {
                        found = Some(Job::Sets(core.device.clone(), core.root.clone(), m.chara, *key, slots(), m.shaders.clone(), false));
                        break 'members;
                    }
                }
                if let Some(key) = kit.glow_key.filter(|k| !m.glow.contains_key(k) && !m.glow_failed.contains(k)) {
                    found = Some(Job::Sets(core.device.clone(), core.root.clone(), m.chara, key, slots(), m.shaders.clone(), true));
                    break 'members;
                }
            }
            found
        }
    };
    match job {
        None => false,
        Some(Job::Shaders(chara, npc, variants)) => {
            let read = read_shaders(&pack.join("party").join(npc), &variants);
            let mut guard = CORE.lock().unwrap();
            if let Some(m) = guard.as_mut().and_then(|c| c.party.members.iter_mut().find(|m| m.chara == chara)) {
                m.hy_read = true;
                match read {
                    Ok(shaders) if !shaders.gbuffer.is_empty() => {
                        rlog!("render: (RSF2 round 13) {npc}: r.hybrid went on after he was made: his shaders read again (the *_hy3 + *_gb3 sets), his pipeline states made again");
                        m.shaders = Arc::new(shaders);
                        m.sets.clear();
                        m.failed.clear();
                        m.glow.clear();
                        m.glow_failed.clear();
                        m.velo_own.clear();
                        m.velo_own_failed.clear();
                    }
                    Ok(_) => rlog!("render: (RSF2 round 13) {npc}: r.hybrid is on but his pack has no complete *_hy3 + *_gb3 set: he keeps today's"),
                    Err(e) => rlog!("render: (RSF2 round 13) {npc}: his shaders could not be read again ({e}): he keeps today's"),
                }
            }
            true
        }
        Some(Job::Load(device, chara)) => {
            let started = Instant::now();
            // SAFETY: object creation on the game's device from our own thread (free-threaded API), as Sora's assets.
            let made = crate::hook::as_ours(|| unsafe { load_member(&device, pack, chara) });
            let mut guard = CORE.lock().unwrap();
            if let Some(core) = guard.as_mut() {
                match made {
                    Ok(m) => {
                        rlog!("render: (ER party S4) {} ready in {:.2} s: {} material slots, {} textures ({} MB), models {}", m.npc, started.elapsed().as_secs_f32(), m.assets.library.slots.len(), m.assets._textures.len(), m.assets.texture_bytes / 1_048_576, m.assets.models.iter().map(|g| format!("{} ({} draws, {} joints)", g.mesh, g.draws.len(), g.joint_bones.len())).collect::<Vec<_>>().join(", "));
                        core.party.members.push(m);
                    }
                    Err(e) => {
                        rlog!("render: (ER party S4) party member {chara} could not be made ready ({e}): not drawn");
                        core.party.failed.push(chara);
                    }
                }
            }
            true
        }
        Some(Job::Sets(device, root, chara, key, slots, shaders, glow)) => {
            let started = Instant::now();
            // SAFETY: as above.
            let made = crate::hook::as_ours(|| unsafe { if glow { glow_pipelines(&device, &root, key, &slots, &shaders) } else { pipelines(&device, &root, key, &slots, &shaders) } });
            // (RSF) his own-record velocity state beside the set (not fatal: without it the plain route, see prepare)
            let own = if glow { None } else { shaders.velo_own.as_ref().zip(shaders.velo.as_ref()).map(|(vs, ps)| crate::hook::as_ours(|| unsafe { velocity_own_pipeline(&device, &root, key, vs, ps) })) };
            let mut guard = CORE.lock().unwrap();
            if let Some(m) = guard.as_mut().and_then(|c| c.party.members.iter_mut().find(|m| m.chara == chara)) {
                match made {
                    Ok(set) => {
                        rlog!("render: (ER party S4) {}: {} {}pipeline states for {key:?} in {:.2} s", m.npc, set.len(), if glow { "glow-mask " } else { "" }, started.elapsed().as_secs_f32());
                        match own {
                            Some(Ok(state)) => {
                                rlog!("render: (RSF) {}: his own-record velocity state (ER88's route: skin_v88) for {key:?}", m.npc);
                                m.velo_own.insert(key, state);
                            }
                            Some(Err(e)) => {
                                rlog!("render: (RSF) {}: his own-record velocity state failed ({e}): the plain route with the plain previous matrix", m.npc);
                                m.velo_own_failed.push(key);
                            }
                            None if !glow => rlog!("render: (RSF) {}: no skin_v88.vs.dxil in his pack: his motion vectors take the plain route with the plain previous matrix", m.npc),
                            None => {}
                        }
                        if glow { m.glow.insert(key, set) } else { m.sets.insert(key, set) };
                    }
                    Err(e) => {
                        rlog!("render: (ER party S4) {}: {}pipeline states for {key:?} failed ({e})", m.npc, if glow { "glow-mask " } else { "" });
                        if glow { m.glow_failed.push(key) } else { m.failed.push(key) };
                    }
                }
            }
            true
        }
    }
}

// ---------------------------------------------------------------------------------------------------- the frame

/// What `prepare` needs of `Core::draw`'s frame.
pub(super) struct PrepCx<'a> {
    pub device: &'a ID3D12Device,
    pub key: PsoKey,
    pub frame: &'a [f32],
    /// The early list's copies have the hybrid's block off (gfx.rs `early_constants`).
    pub early_hy_off: bool,
    pub upload_base: u64,
    pub lights: &'a [crate::pointlight::PassLight],
    pub camera_game: Vec3,
    pub out_scale: f32,
    pub clock: f32,
    pub input: &'a crate::FrameInput,
    pub khs: &'a str,
    pub void: bool,
    /// The game-data texture (light.rs) and the hybrid's light / mask of this frame (0 = white).
    pub data: Option<&'a ID3D12Resource>,
    pub hy_views: Option<[usize; 2]>,
    pub white: &'a ID3D12Resource,
    /// (ER party I) the friends' own light: None = no game light this frame (Sora's light, no probe of theirs).
    pub light: Option<LightIn>,
    /// (RSF) the renderer's frame number (his previous skin is used for the very next frame only) and the plain previous
    /// game -> host matrix (his frame copy's `prev_host_from_game` when he draws his motion vectors on the plain route).
    pub frame_no: u64,
    pub prev_plain: Mat4,
    /// (RSF2) the reflection level's inputs (gfx.rs: the game's light as the reflection reads it, `r.env`)
    pub env_light: Option<crate::light::GameLight>,
    pub env_scale: f32,
    /// (RSF2) Sora is drawn this frame: his level is the frame's shared one
    pub sora_drawn: bool,
}

/// (ER party I) What a friend's own light is made from (gfx.rs, this frame): Sora's steady game light (its world values),
/// the rig's fallback key direction and gains (as Sora's `light::rig` call), and the probe: `probe_on` = the game's
/// targets are read this frame, `world` = Some(radius cm) when the world probe (probe3) is in use.
#[derive(Clone, Copy, Debug)]
pub(super) struct LightIn {
    pub steady: crate::light::GameLight,
    pub fallback: Vec3,
    pub key_gain: f32,
    pub ambient_gain: f32,
    pub probe_on: bool,
    pub world: Option<f32>,
}

thread_local! {
    /// (RSF2) the latest own_light reading (taps, surface, scene) for the light-level line
    static PROBE_SEEN: std::cell::Cell<Option<(f32, Vec3, Vec3)>> = const { std::cell::Cell::new(None) };
}

/// (ER party I) HIS PROBE AND HIS LIGHT in his frame copy: `er.probe` at his feet (drawn into his row by gfx.rs), and when
/// a reading of his exists, his rig (light.rs `rig` on Sora's steady world values with his probes) in the light rows.
fn own_light(frame: &mut [f32], chara: u8, feet: Vec3, li: &LightIn) -> (Option<crate::light::RigLight>, Option<usize>) {
    let row = probe::row_of(chara).filter(|_| probe::enabled());
    let Some(row) = row else { return (None, None) };
    if frame.len() < LIGHT_AT + 16 {
        return (None, None);
    }
    let clip = Mat4::from_cols_slice(&frame[0..16]);
    let row4 = |n: usize| 16 + 4 * n;
    let viewport = (frame[row4(4)], frame[row4(4) + 1]);
    let tan_y = frame[row4(7) + 1];
    let at = if li.probe_on { probe::probe_at(&clip, feet, viewport, tan_y, li.world) } else { Vec4::ZERO };
    frame[PROBE_AT..PROBE_AT + 4].copy_from_slice(&at.to_array());
    let probe_row = (at.w > 0.0).then_some(row);
    let Some(light) = probe::steady(chara, &li.steady, feet) else { return (None, probe_row) };
    // (RSF2, for the log) what his probe measured at his feet: taps, surface colour, scene colour
    PROBE_SEEN.with(|p| p.set(Some((light.taps, light.albedo_px, light.scene_px))));
    let r = crate::light::rig_for(&light, li.fallback, li.key_gain, li.ambient_gain, crate::light::open_level_now()); // (RSF2) the open level
    // (as gfx.rs's rows: out scale, lit key = key / pi x out scale (key tint 1 with the game's light), ambient, ambient up)
    frame[LIGHT_AT..LIGHT_AT + 3].copy_from_slice(&[r.out_scale; 3]);
    frame[LIGHT_AT + 4..LIGHT_AT + 8].copy_from_slice(&(r.key / std::f32::consts::PI * r.out_scale).extend(0.0).to_array());
    frame[LIGHT_AT + 8..LIGHT_AT + 12].copy_from_slice(&(r.ambient * r.out_scale).extend(0.0).to_array());
    frame[LIGHT_AT + 12..LIGHT_AT + 16].copy_from_slice(&(r.ambient_up * r.out_scale).extend(0.0).to_array());
    (Some(r), probe_row)
}

/// (ER party I) The probe pass's draws of the friends (gfx.rs, right after Sora's probe, the probe pipeline and its table
/// bound): each friend's frame copy, his row of the game-data texture (`viewport` covers the party rows).
pub(super) unsafe fn probe_draws(plan: &Plan, list: &ID3D12GraphicsCommandList, viewport: D3D12_VIEWPORT, width: u32) -> u32 {
    let mut n = 0;
    for pm in &plan.members {
        let Some(row) = pm.probe_row else { continue };
        if n == 0 {
            list.RSSetViewports(&[viewport]);
        }
        list.SetGraphicsRootConstantBufferView(0, pm.frame_at);
        list.RSSetScissorRects(&[RECT { left: 0, top: row as i32, right: width as i32, bottom: row as i32 + 1 }]);
        list.DrawInstanced(3, 1, 0, 0);
        n += 1;
    }
    n
}

/// (ER party I) The rig and indirect rows his materials take: his own light, else Sora's.
fn rig_of(pm: &PlanMember, cx: &PassCx) -> (kh_look::Rig, [Vec4; 8]) {
    // (RSF2) his light handed to his materials at the frame's ONE light level (`light_ratio` = the level his light was measured
    // at / the shared level): what reaches the picture is his own light; KH3's look (bands, colour table) works at the same level
    // as everybody's
    let q = pm.light_ratio;
    match pm.light {
        Some(r) => {
            let mut rig = cx.rig.clone();
            (rig.key, rig.ambient) = (r.key * q, r.ambient * q);
            let mut sh = rig.sh();
            for c in 0..3 {
                sh[c].z = r.ambient_up[c] * q / 1.023_328;
            }
            (rig, sh)
        }
        None if (q - 1.0).abs() > 1.0e-4 => {
            let mut rig = cx.rig.clone();
            (rig.key, rig.ambient) = (rig.key * q, rig.ambient * q);
            let mut sh = cx.sh;
            for row in sh.iter_mut().take(3) {
                *row *= q;
            }
            (rig, sh)
        }
        None => (cx.rig.clone(), cx.sh),
    }
}

/// (ER party I, the coordinator's request after ER86) stable.rs `hero_toward` FOR ONE FRIEND: the point the hero light
/// on him is placed toward and how far, by the same `hero.anchor` rule as Sora's (0 = the camera as before; 1 = ahead of
/// HIS facing eased by `hero.anchor_tau_s`; 2 = his feet, straight above him), each character with his own eased facing.
/// `r.light_stable = 0` = the camera (S4's placement).
fn hero_toward_for(chara: u8, camera: Vec3, feet: Vec3, facing: Option<Vec3>, toward_before: f32) -> (Vec3, f32) {
    static FACING: std::sync::Mutex<Vec<(u8, Vec3, Instant)>> = std::sync::Mutex::new(Vec::new());
    let s = crate::stable::settings();
    if !s.on || s.hero_anchor == 0 {
        return (camera, toward_before);
    }
    let level = facing.map(|f| Vec3::new(f.x, f.y, 0.0)).and_then(|f| f.try_normalize());
    let now = Instant::now();
    let mut kept = FACING.lock().unwrap_or_else(|e| e.into_inner());
    let eased = match (kept.iter_mut().find(|e| e.0 == chara), level) {
        (Some(e), Some(l)) => {
            let dt = now.duration_since(e.2).as_secs_f32().clamp(0.0, 0.25);
            e.1 = e.1.lerp(l, crate::stable::blend(dt, s.tau_facing)).try_normalize().unwrap_or(l);
            e.2 = now;
            Some(e.1)
        }
        (Some(e), None) => {
            e.2 = now;
            Some(e.1)
        }
        (None, Some(l)) => {
            kept.push((chara, l, now));
            Some(l)
        }
        (None, None) => None,
    };
    match (s.hero_anchor, eased) {
        (1, Some(f)) => (feet + f * 1000.0, s.hero_forward),
        _ => (feet, 0.0),
    }
}

/// (ER party I, S4's request) THE HERO LIGHT'S BODY SHADOW FROM THE FRIENDS TOO. The light pass (plight2, `er.caps`)
/// reads its body capsules from the frame block bound at root 0, which is Sora's. This gives the light pass a COPY of
/// that block whose slots it does not read as Sora's body (empty, or his Keyblade's: w = 2, which the light pass leaves
/// out anyway) carry the friends' capsules (w = 1). Each friend's full set when all fit, else one capsule around each
/// friend. `None` = no friend has capsules this frame: the light pass reads Sora's block as before.
pub(super) fn light_pass_frame(plan: &Plan, frame: &[f32]) -> Option<Vec<f32>> {
    let friends: Vec<&Vec<(Vec3, Vec3, f32)>> = plan.members.iter().map(|m| &m.caps).filter(|c| !c.is_empty()).collect();
    if friends.is_empty() || frame.len() < CAPS_AT + 16 * 8 {
        return None;
    }
    let free: Vec<usize> = (0..16).filter(|k| !(0.5..=1.5).contains(&frame[CAPS_AT + 8 * k + 7])).collect();
    let full: usize = friends.iter().map(|c| c.len()).sum();
    let caps: Vec<(Vec3, Vec3, f32)> = if full <= free.len() { friends.iter().flat_map(|c| c.iter().copied()).collect() } else { friends.iter().map(|c| around(c)).collect() };
    if free.is_empty() {
        return None;
    }
    let mut out = frame.to_vec();
    for (k, (a, b, r)) in free.iter().zip(caps) {
        let row = CAPS_AT + 8 * k;
        out[row..row + 4].copy_from_slice(&a.extend(r).to_array());
        out[row + 4..row + 8].copy_from_slice(&b.extend(1.0).to_array());
    }
    Some(out)
}

/// (ER party I) One upright capsule around a friend's capsules (the hero light's body shadow when his full set does not fit).
fn around(caps: &[(Vec3, Vec3, f32)]) -> (Vec3, Vec3, f32) {
    let n = (caps.len() * 2).max(1) as f32;
    let centre = caps.iter().fold(Vec3::ZERO, |s, (a, b, _)| s + *a + *b) / n;
    let widest = caps.iter().map(|c| c.2).fold(0.0f32, f32::max);
    let reach = caps.iter().flat_map(|(a, b, r)| [(*a - centre).truncate().length() + r, (*b - centre).truncate().length() + r]).fold(0.0f32, f32::max);
    let radius = reach.clamp(widest, 40.0);
    let low = caps.iter().flat_map(|(a, b, r)| [a.z - r, b.z - r]).fold(f32::MAX, f32::min) + radius;
    let high = (caps.iter().flat_map(|(a, b, r)| [a.z + r, b.z + r]).fold(f32::MIN, f32::max) - radius).max(low);
    (Vec3::new(centre.x, centre.y, low), Vec3::new(centre.x, centre.y, high), radius)
}

/// The capsules of his thrown shadow (gfx.rs's segments by his bone names; the friends' skeletons have most of them).
fn caps_of(bones: &HashMap<String, usize>, body: &Body) -> Vec<(Vec3, Vec3, f32)> {
    let at = |name: &str| -> Option<Vec3> {
        let bone = *bones.get(name)?;
        Some(body.component.get(bone)?.then(&body.component_to_world).translation).filter(|p| p.is_finite())
    };
    const SEGMENTS: [(&str, &str, f32); 12] = [
        ("koshi", "mune", 17.0),
        ("mune", "kubi", 15.0),
        ("L_ude1", "L_ude2", 6.5),
        ("L_ude2", "L_te", 6.0),
        ("R_ude1", "R_ude2", 6.5),
        ("R_ude2", "R_te", 6.0),
        ("L_momo", "L_sune", 9.0),
        ("L_sune", "L_ashi1", 8.0),
        ("L_ashi1", "L_ashi2", 8.0),
        ("R_momo", "R_sune", 9.0),
        ("R_sune", "R_ashi1", 8.0),
        ("R_ashi1", "R_ashi2", 8.0),
    ];
    let mut caps = Vec::with_capacity(16);
    for (from, to, radius) in SEGMENTS {
        if let (Some(a), Some(b)) = (at(from), at(to)) {
            caps.push((a, b, radius));
        }
    }
    let neck = at("kubi").or_else(|| at("kubi1"));
    if let (Some(neck), Some(head)) = (neck, at("atama")) {
        let up = (head - neck).normalize_or(Vec3::Z);
        caps.push((head + up * 6.0, head + up * 20.0, 22.0));
    }
    if caps.is_empty() {
        let feet = body.component_to_world.translation;
        caps.push((feet + Vec3::Z * 24.0, feet + Vec3::Z * 100.0, 24.0));
    }
    caps.truncate(16);
    caps
}

/// (RSF2 round 16, testing aid) where `r.party_pin_test` puts the first friend this frame (game space, cm); None = off.
pub(super) static PIN_TARGET: std::sync::Mutex<Option<Vec3>> = std::sync::Mutex::new(None);
/// (RSF2 round 16, testing aid) `r.party_pin_test` in metres (0 = off).
pub(crate) static PIN_METRES: std::sync::Mutex<f32> = std::sync::Mutex::new(0.0);

/// THE PARTY OF THIS FRAME, made ready (see the file's head). `None`: nobody of the party is drawn (with the feature on
/// and nothing submitted the frame is Sora's alone, untouched).
pub(super) unsafe fn prepare(kit: &mut PartyKit, cx: &PrepCx, put: &mut dyn FnMut(&[f32]) -> Option<u64>) -> Option<Plan> {
    // what is drawn: the void's fallen leader, else the frame's list
    let list: Vec<CharDraw> = if cx.void {
        let v = crate::party::void_char_now()?;
        vec![CharDraw { chara: v.chara, body: Some(v.body.clone()), weapon: None, props: Vec::new(), glow_mask: false, blood: Default::default() }]
    } else {
        crate::party::draws_now()?.iter().cloned().collect()
    };
    // (RSF2 round 16, TESTING AID `r.party_pin_test = N`, off by default) the first friend DRAWN (render only: his game body, his AI
    // and his collision stay where they are) N metres from Sora along the sun: N > 0 down-sun (Sora's shadow on him), N < 0 up-sun
    // (his shadow on Sora). For the in-game proof of the KH characters' shadows on each other.
    let mut list = list;
    if let Some(target) = *PIN_TARGET.lock().unwrap_or_else(|e| e.into_inner()) {
        if let Some(d) = list.iter_mut().find(|d| d.chara != crate::party::TEAM && d.body.is_some()) {
            let from = d.body.as_ref().map(|b| b.component_to_world.translation).unwrap_or(target);
            let delta = target - from;
            if let Some(b) = d.body.as_mut() {
                b.component_to_world.translation += delta;
            }
            if let Some(w) = d.weapon.as_mut() {
                w.to_world.translation += delta;
            }
        }
    }
    if !kit.keys.contains(&cx.key) {
        kit.keys.push(cx.key);
    }
    let n = cx.frame.len();
    let xl_at = n - crate::hybrid::BLOCK_FLOATS - crate::blood::BLOCK_FLOATS - crate::pointlight::SORA_BLOCK_FLOATS;
    let bl_at = xl_at + crate::pointlight::SORA_BLOCK_FLOATS;
    let mut plan = Plan::default();
    let mut kit_snaps = 0u32;
    let mut kit_levels = std::mem::take(&mut kit.levels);
    let shared_before = kit.shared_level;
    for draw in &list {
        let Some(mi) = kit.members.iter().position(|m| m.chara == draw.chara) else {
            kit.waiting = "a member's library is still being made";
            continue;
        };
        let m = &mut kit.members[mi];
        if !m.sets.contains_key(&cx.key) {
            kit.waiting = if m.failed.contains(&cx.key) { "a member's pipeline states could not be made" } else { "a member's pipeline states are still being made" };
            continue;
        }
        // ---- his tables: the game-data texture, the hybrid's light and mask (written when they change)
        let start = m.assets.srv_heap.GetCPUDescriptorHandleForHeapStart();
        let size = m.assets.srv_size;
        let data = cx.data.map_or(0, |d| d.as_raw() as usize);
        if m.data_bound != data {
            for k in 0..m.assets.library.slots.len() {
                cx.device.CreateShaderResourceView(cx.data.unwrap_or(cx.white), None, D3D12_CPU_DESCRIPTOR_HANDLE { ptr: start.ptr + (k * SLOT_TEXTURES + DEPTH_TEXTURE) * size });
            }
            m.data_bound = data;
        }
        if let Some(views) = cx.hy_views.filter(|v| *v != m.hy_views) {
            for k in 0..m.assets.library.slots.len() {
                for (entry, pointer) in [(HY_LIGHT_TEXTURE, views[0]), (HY_MASK_TEXTURE, views[1])] {
                    let raw = pointer as *mut c_void;
                    let at = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: start.ptr + (k * SLOT_TEXTURES + entry) * size };
                    match ID3D12Resource::from_raw_borrowed(&raw) {
                        Some(resource) => cx.device.CreateShaderResourceView(resource, None, at),
                        None => cx.device.CreateShaderResourceView(cx.white, None, at),
                    }
                }
            }
            m.hy_views = views;
        }
        // ---- the materials' state: the preset, his bone-fed values, his weapon's fade
        if m.assets.khs.as_ref().map(|(name, _)| name.as_str()) != Some(cx.khs) {
            let on = cx.khs != "off" && !m.assets.library.khs.presets.is_empty();
            m.assets.khs = Some((cx.khs.to_string(), m.assets.library.khs.preset_state(cx.khs, on)));
        }
        if let Some(body) = &draw.body {
            let table = m.assets.library.bone_table.clone();
            let bones = &m.bones;
            let values = kh_look::bone_parameters(&table, |name| {
                let bone = *bones.get(name)?;
                let component = *body.component.get(bone)?;
                Some((*body.local.get(bone)?, component, component.then(&body.component_to_world)))
            });
            m.assets.library.feed(m.npc, &values);
        }
        // (VFXR) the effects' drives of his materials (SQEX_Material_InLight ...: vfx.rs apply_char_drives; 1 Donald, 2 Goofy)
        match m.npc {
            "n_ex001" => crate::vfx::apply_char_drives(&mut m.assets.library, 1),
            "n_ex002" => crate::vfx::apply_char_drives(&mut m.assets.library, 2),
            _ => {}
        }
        // ---- what is drawn: the body, the weapon, the props
        let mut parts: Vec<(usize, Trs, Vec<Trs>)> = Vec::new();
        if let (Some(body), Some(k)) = (&draw.body, m.body) {
            parts.push((k, body.component_to_world, body.component.clone()));
        }
        if let Some(w) = draw.weapon.as_ref().filter(|w| w.opacity > 0.0) {
            if let Some(&k) = m.by_mesh.get(&w.model) {
                m.assets.library.set_scalar(m.assets.models[k].mesh, "OpacityFade", w.opacity);
                let pose = if w.component.len() == m.assets.models[k].reference.len() { w.component.clone() } else { m.assets.models[k].reference.clone() };
                parts.push((k, w.to_world, pose));
            }
        }
        for (model, to_world) in &draw.props {
            if let Some(&k) = m.by_mesh.get(model) {
                parts.push((k, *to_world, Vec::new()));
            }
        }
        if parts.is_empty() {
            continue;
        }
        // ---- his frame block: his own point lights, his blood, his contact shadow and capsules
        let mut frame = cx.frame.to_vec();
        let feet = draw.body.as_ref().map(|b| b.component_to_world.translation);
        let caps = draw.body.as_ref().map(|b| caps_of(&m.bones, b)).unwrap_or_default();
        let (mut own, mut probe_row) = (None, None); // (ER party I)
        let mut on_him: Option<Vec<crate::pointlight::SoraLight>> = None; // (RSF2) written below at HIS light level
        if let Some(feet) = feet {
            if crate::pointlight::own_shaders() {
                let s = crate::pointlight::settings();
                // (ER party I, ER86's anchor rule per character) the placed hero light toward a point ahead of HIS eased
                // facing (`hero.anchor`), not toward the camera: as steady as Sora's
                let facing = draw.body.as_ref().map(|b| b.component_to_world.rotation * Vec3::Y);
                let (toward, toward_cm) = hero_toward_for(draw.chara, cx.camera_game, feet, facing, s.hero_sora_toward);
                on_him = Some(crate::pointlight::sora_lights(cx.lights, feet, toward, s.hero_sora_height, toward_cm));
            } else {
                frame[xl_at..bl_at].fill(0.0);
            }
            if draw.blood.body > 0.0 || draw.blood.blade > 0.0 {
                let input = crate::FrameInput { camera: cx.input.camera, origin: cx.input.origin, clock: cx.clock, body: draw.body.clone(), weapon: None, forms: Vec::new() };
                let bones = &m.bones;
                let block = crate::blood::block_for(draw.blood, &input, |name| {
                    let body = input.body.as_ref()?;
                    let bone = *bones.get(name)?;
                    Some(body.component.get(bone)?.then(&body.component_to_world).translation)
                });
                frame[bl_at..bl_at + crate::blood::BLOCK_FLOATS].copy_from_slice(&block);
            } else {
                frame[bl_at..bl_at + crate::blood::BLOCK_FLOATS].fill(0.0);
            }
            frame[BLOB_AT..BLOB_AT + 4].copy_from_slice(&feet.extend(45.0).to_array());
            for k in 0..16 {
                let row = CAPS_AT + 8 * k;
                match caps.get(k) {
                    Some((a, b, r)) => {
                        frame[row..row + 4].copy_from_slice(&a.extend(*r).to_array());
                        frame[row + 4..row + 8].copy_from_slice(&b.extend(1.0).to_array());
                    }
                    None => frame[row..row + 8].fill(0.0),
                }
            }
            // (ER party I) his own probe and light (gfx/party_probe.rs)
            if let Some(li) = cx.light.as_ref().filter(|_| !cx.void) {
                PROBE_SEEN.with(|p| p.set(None));
                (own, probe_row) = own_light(&mut frame, draw.chara, feet, li);
            }
        } else {
            frame[xl_at..bl_at + crate::blood::BLOCK_FLOATS].fill(0.0);
        }
        // (RSF2) EVERY LIGHT LEVEL IN HIS BLOCK IS HIS OWN (the user's FINAL3 pictures: the branch-shaped patterns on Goofy's
        // clothes under the Erdtree, the pale blotches on his snout in the chapel, his whole body blown out green at a grace).
        // His copy of the frame block came with SORA's out scale in it; his own light (own_light) puts HIS out scale into the
        // rows his shaders multiply with at the end (tune), but the hybrid's k (the game's light -> KH units), its fog, his point
        // lights (colour / out scale) and the reflection level stayed Sora's. When Sora is not drawn (a friend leads, Sora out of
        // the party) Sora's out scale collapses to its floor (the log's "rig x100000"): his game light x 1e4 (the bands clipped to
        // a two-tone copy of every shadow on him), KH3's sun specular and the reflections at their maximum, a save point's lights
        // x 1e4. With Sora drawn the error is the ratio of the two out scales. Now: the same rule for every drawn character, his
        // block self-consistent at his own level (rescale_block).
        let own_scale = own.map(|r| r.out_scale).filter(|s| s.is_finite() && *s > 0.0);
        // (RSF2, round 3: the FINAL3+RSF2 run of stream GG: Goofy neon lime and oversaturated beside Sora, his probe's level
        // 1.3 - 2.6 x Sora's in the same light) ONE LIGHT LEVEL FOR EVERY DRAWN KH CHARACTER OF THE FRAME, as a camera has one
        // exposure: Sora's when Sora is drawn, else the leader's (the frame before's). Each keeps his OWN light (his probe), handed
        // to his materials at that level (light_ratio): KH3's bands and colour table are not linear, so a character measured
        // darker than the others no longer comes out brighter and more saturated than them.
        let shared = if !crate::party::shared_level() { None } else if cx.sora_drawn { Some(cx.out_scale) } else { shared_before };
        let level = shared.filter(|s| s.is_finite() && *s > 0.0).or(own_scale).unwrap_or(cx.out_scale);
        let measured = own_scale.unwrap_or(cx.out_scale);
        let light_ratio = if measured.is_finite() && measured > 0.0 { measured / level } else { 1.0 };
        if frame.len() >= LIGHT_AT + 3 {
            frame[LIGHT_AT..LIGHT_AT + 3].copy_from_slice(&[level; 3]);
        }
        if let Some(lights) = &on_him {
            frame[xl_at..bl_at].copy_from_slice(&crate::pointlight::sora_block(lights, level));
        }
        if (level - cx.out_scale).abs() > 1.0e-12 {
            rescale_block(&mut frame, level);
        }
        // (RSF2) the hybrid rows that belong to SORA'S outfit (an own set's field: its rig share `r.hybrid_rig_own` and its
        // light lift, cgfield.rs) are the plain ones for a member: his look is never Sora's outfit's
        member_hybrid_rows(&mut frame, &crate::hybrid::settings());
        let env_level = match cx.env_light.as_ref() {
            Some(l) if !cx.void && (level - cx.out_scale).abs() > 1.0e-12 => Some(crate::light::env_level(l, level, cx.env_scale)),
            _ => None,
        };
        let npc_now = m.npc;
        let seen = PROBE_SEEN.with(|p| p.take());
        let row = (npc_now, measured, own_scale.is_some(), level, env_level.unwrap_or(-1.0), seen);
        match kit_levels.iter_mut().find(|l| l.0 == npc_now) {
            Some(l) => *l = row,
            None => kit_levels.push(row),
        }
        // (RSF) HIS MOTION VECTORS' CAMERA PART, ER88's standard: his own-record velocity state (his pack's skin_v88) reads the
        // frame block's `prev_host_from_game` as Sora's does (R x (game -> host), flagged, or the plain matrix). Without that
        // state his velocity draw runs the plain skin, which reads a flagged R x (game -> host) as a plain matrix: the camera's
        // motion counted twice (the user's "INSANE" blur on Donald and Goofy, and the background dragged into him by the TAA).
        // Then his copy carries the plain previous matrix (ER88's "game's previous matrix" route, never the doubled one).
        // (NF) HIS OWN camera fade (camfade.rs: his value in the fade list, never Sora's): every pass of his reads this block
        crate::camfade::write_block(&mut frame, crate::camfade::member_key(draw.chara));
        let velo_own = velocity_route(&mut frame, crate::taa::own_wanted() && m.velo_own.contains_key(&cx.key), &cx.prev_plain);
        let Some(frame_at) = put(&frame) else { break };
        let early_at = if cx.early_hy_off {
            let mut e = frame.clone();
            e[n - crate::hybrid::BLOCK_FLOATS..].fill(0.0);
            match put(&e) {
                Some(at) => at,
                None => break,
            }
        } else {
            frame_at
        };
        // (RSF) his box's reach: capsules + every bone of every posed part + the cooked bounds of his weapon / props
        let mut reach = caps.clone();
        for (k, to_world, pose) in &parts {
            if pose.is_empty() || Some(*k) != m.body {
                reach.push(crate::hybrid::bounds_ball(to_world, m.assets.library.bounds_of(m.assets.models[*k].mesh)));
            }
            if !pose.is_empty() {
                reach.extend(crate::hybrid::bone_balls(pose, to_world));
            }
        }
        // ---- the skins (now and a frame ago)
        let mut models = Vec::new();
        let mut skin: Vec<f32> = Vec::with_capacity(SKIN_FLOATS);
        let mut seen: Vec<usize> = Vec::new();
        for (k, to_world, pose) in parts {
            let instance = (k, seen.iter().filter(|s| **s == k).count());
            seen.push(k);
            let model = &m.assets.models[k];
            skin.clear();
            skin.extend_from_slice(&math::game_from_pack(&to_world).to_cols_array());
            let fixed = m.prop_joint.get(&k).copied().unwrap_or(Mat4::IDENTITY);
            for (joint, bone) in model.joint_bones.iter().enumerate() {
                let matrix = match bone.and_then(|b| pose.get(b)) {
                    Some(t) => math::joint_in_pack(t) * model.inverse_bind[joint],
                    None => fixed,
                };
                skin.extend_from_slice(&matrix.to_cols_array());
            }
            skin.resize(SKIN_FLOATS, 0.0);
            let Some(now) = put(&skin) else { break };
            // (RSF) the skin DRAWN the frame before (this member's own record per model), only for the very next frame and when
            // the model did not jump (INT's snap rule: a warp, a reseat, a load); else no skinned motion this frame
            let before = match m.prev_skin.get(&instance).filter(|p| p.skin.len() == skin.len()) {
                Some(p) if motion_continues(p.frame, &p.to_world, cx.frame_no, &to_world) => put(&p.skin).unwrap_or(now),
                Some(_) => {
                    kit_snaps += 1;
                    now
                }
                None => now,
            };
            m.prev_skin.insert(instance, PrevSkin { skin: skin.clone(), frame: cx.frame_no, to_world });
            models.push((k, now, before, to_world));
        }
        plan.members.push(PlanMember { member: mi, frame_at, early_at, models, glow_mask: draw.glow_mask, caps, feet, frame_offset: (frame_at - cx.upload_base) as usize, early_offset: (early_at - cx.upload_base) as usize, light: own, probe_row, velo_own, reach, env_level, light_ratio });
    }
    kit.snaps += kit_snaps;
    kit.levels = kit_levels;
    // (RSF2) the next frame's shared level when Sora is not drawn: the leader's own (the member at the hero light's carrier)
    kit.shared_level = if cx.sora_drawn { None } else { leader_level(&plan, crate::party::level_anchor_now()).or(shared_before) }; // (ERT4) the leader's feet, lit or not
    (!plan.members.is_empty()).then_some(plan)
}

/// What every pass over the party's draws needs of `Core::draw`.
pub(super) struct PassCx<'a> {
    pub key: PsoKey,
    pub sampler_heap: &'a ID3D12DescriptorHeap,
    /// Sora's heap and frame constants (put back after the party's draws), the early list's constants.
    pub sora_heap: &'a ID3D12DescriptorHeap,
    pub frame_constants: u64,
    pub early_constants: u64,
    pub rig: &'a kh_look::Rig,
    pub sh: [Vec4; 8],
    pub env_level: f32,
    pub env_tint: Vec3,
    pub clock: f32,
    pub exposure: f32,
    pub debug: f32,
}

/// (FP) How many material blocks the two `draw_pass`es of this plan put (one per draw of every planned model): kept free of
/// the routed chandelier fragments (gfx.rs `routed_reserve`), which come before them in the upload buffer.
pub(super) fn material_blocks(plan: &Plan, kit: &PartyKit) -> usize {
    plan.members.iter().map(|pm| pm.models.iter().map(|(k, ..)| kit.members[pm.member].assets.models.get(*k).map_or(0, |m| m.draws.len())).sum::<usize>()).sum()
}

/// THE PARTY'S DRAWS OF ONE HALF OF SORA'S LOOP (opaque / masked, or translucent): colour into `list`, early depth (with
/// the stencil when `early` says so) into `early_list`; the opaque draws recorded for the glow, shaft and G-buffer passes.
#[allow(clippy::too_many_arguments)]
pub(super) unsafe fn draw_pass(plan: &mut Plan, kit: &PartyKit, translucent: bool, list: &ID3D12GraphicsCommandList, early_list: Option<&ID3D12GraphicsCommandList>, early: Option<bool>, stencil: Option<u8>, hy_on: bool, cx: &PassCx, put: &mut dyn FnMut(&[f32]) -> Option<u64>) -> Result<(), ()> {
    let mut any = false;
    if plan.early_per.len() != plan.members.len() {
        plan.early_per = vec![0; plan.members.len()];
    }
    for (pi, pm) in plan.members.iter().enumerate() {
        let m = &kit.members[pm.member];
        let Some(set) = m.sets.get(&cx.key) else { continue };
        let library = &m.assets.library;
        if !pm.models.iter().any(|(k, ..)| m.assets.models[*k].draws.iter().any(|d| library.slots[d.slot].translucent == translucent)) {
            continue;
        }
        any = true;
        let heap = m.assets.srv_heap.clone();
        let srv_start = heap.GetGPUDescriptorHandleForHeapStart();
        list.SetDescriptorHeaps(&[Some(heap.clone()), Some(cx.sampler_heap.clone())]);
        list.SetGraphicsRootConstantBufferView(0, pm.frame_at);
        let (rig, sh) = rig_of(pm, cx); // (ER party I) his own light, else Sora's
        let base = match early {
            Some(true) => early_stencil_base(set.len(), library.slots.len()),
            Some(false) => early_base(set.len(), library.slots.len()),
            None => None,
        };
        let la = early_list.filter(|_| !translucent && base.is_some());
        if let Some(la) = la {
            la.SetDescriptorHeaps(&[Some(heap.clone()), Some(cx.sampler_heap.clone())]);
            la.SetGraphicsRootConstantBufferView(0, pm.early_at);
            // (RSF) his TAA class (ER88: the characters' 0x50) set for HIS draws, not inherited from whatever drew before
            if let (Some(true), Some(v)) = (early, stencil) {
                la.OMSetStencilRef(v as u32);
                plan.stencil = Some(v);
            }
        }
        for (k, now, before, to_world) in &pm.models {
            let model = &m.assets.models[*k];
            if !model.draws.iter().any(|d| library.slots[d.slot].translucent == translucent) {
                continue;
            }
            let object = ObjectRows::new(to_world, library.bounds_of(model.mesh));
            list.IASetVertexBuffers(0, Some(&[model.vertex_view]));
            list.IASetIndexBuffer(Some(&model.index_view));
            list.SetGraphicsRootConstantBufferView(2, *now);
            list.SetGraphicsRootConstantBufferView(6, *before);
            if let Some(la) = la {
                la.IASetVertexBuffers(0, Some(&[model.vertex_view]));
                la.IASetIndexBuffer(Some(&model.index_view));
                la.SetGraphicsRootConstantBufferView(2, *now);
                la.SetGraphicsRootConstantBufferView(6, *before);
            }
            for draw in &model.draws {
                let material = &library.slots[draw.slot];
                if material.translucent != translucent {
                    continue;
                }
                let mut params = library.params(material, &object, &rig, m.assets.khs.as_ref().map(|(_, s)| s), cx.clock, cx.exposure, cx.debug);
                params.sh = sh; // (ER party I) his own light (rig_of)
                params.khs.w = material.scalars.get(crate::outfit::INTERIOR_SCALAR).copied().unwrap_or(0.0);
                params.env.y = pm.env_level.unwrap_or(cx.env_level); // (RSF2) his own level
                (params.env.z, params.env.w) = (cx.env_tint.x - 1.0, cx.env_tint.z - 1.0);
                let Some(material_constants) = put(params.as_floats()) else { return Err(()) };
                let table = D3D12_GPU_DESCRIPTOR_HANDLE { ptr: srv_start.ptr + (draw.slot * SLOT_TEXTURES * m.assets.srv_size) as u64 };
                list.SetPipelineState(&set[draw.slot]);
                list.SetGraphicsRootConstantBufferView(1, material_constants);
                list.SetGraphicsRootDescriptorTable(3, table);
                list.DrawIndexedInstanced(draw.index_count, 1, draw.first_index, draw.base_vertex, 0);
                plan.drawn += 1;
                let rec: Rec = (pi, *k, draw.slot, draw.index_count, draw.first_index, draw.base_vertex, *now, *before, material_constants);
                if !translucent && pm.glow_mask {
                    plan.glow.push(rec);
                }
                if let (Some(la), Some(base)) = (la, base) {
                    if let Some(state) = set.get(base + draw.slot) {
                        la.SetPipelineState(state);
                        la.SetGraphicsRootConstantBufferView(1, material_constants);
                        la.SetGraphicsRootDescriptorTable(3, table);
                        la.DrawIndexedInstanced(draw.index_count, 1, draw.first_index, draw.base_vertex, 0);
                        plan.early_per[pi] += 1;
                        if hy_on {
                            plan.gb.push(rec);
                        }
                    }
                }
            }
        }
    }
    if any {
        // (Sora's root state again)
        list.SetDescriptorHeaps(&[Some(cx.sora_heap.clone()), Some(cx.sampler_heap.clone())]);
        list.SetGraphicsRootConstantBufferView(0, cx.frame_constants);
        if let Some(la) = early_list.filter(|_| !translucent) {
            la.SetDescriptorHeaps(&[Some(cx.sora_heap.clone()), Some(cx.sampler_heap.clone())]);
            la.SetGraphicsRootConstantBufferView(0, cx.early_constants);
        }
    }
    Ok(())
}

/// Draws recorded draws again with another state per slot (`pso(member, slot)`), each member's heap bound; Sora's heap put
/// back afterwards. `frames`: true = each member's frame constants (the G-buffer pass reads the hybrid's block).
unsafe fn redraw(plan: &Plan, kit: &PartyKit, recs: &[Rec], list: &ID3D12GraphicsCommandList, cx: &PassCx, frames: bool, scissor: Option<&dyn Fn(&PlanMember) -> Option<RECT>>, pso: &dyn Fn(&Member, usize) -> Option<ID3D12PipelineState>) -> (u32, Vec<u32>) {
    let mut bound: Option<usize> = None;
    let mut model_bound: Option<(usize, usize)> = None;
    let mut n = 0;
    let mut per = vec![0u32; plan.members.len()];
    for (pi, k, slot, count, first, base, now, before, material) in recs.iter().copied() {
        let pm = &plan.members[pi];
        let m = &kit.members[pm.member];
        let Some(state) = pso(m, slot) else { continue };
        if bound != Some(pi) {
            if let Some(sc) = scissor {
                match sc(pm) {
                    Some(r) => list.RSSetScissorRects(&[r]),
                    None => continue,
                }
            }
            list.SetDescriptorHeaps(&[Some(m.assets.srv_heap.clone()), Some(cx.sampler_heap.clone())]);
            if frames {
                list.SetGraphicsRootConstantBufferView(0, pm.frame_at);
            }
            bound = Some(pi);
        }
        let model = &m.assets.models[k];
        if model_bound != Some((pi, k)) {
            list.IASetVertexBuffers(0, Some(&[model.vertex_view]));
            list.IASetIndexBuffer(Some(&model.index_view));
            model_bound = Some((pi, k));
        }
        list.SetGraphicsRootConstantBufferView(2, now);
        list.SetGraphicsRootConstantBufferView(6, before);
        list.SetPipelineState(&state);
        list.SetGraphicsRootConstantBufferView(1, material);
        list.SetGraphicsRootDescriptorTable(3, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: m.assets.srv_heap.GetGPUDescriptorHandleForHeapStart().ptr + (slot * SLOT_TEXTURES * m.assets.srv_size) as u64 });
        list.DrawIndexedInstanced(count, 1, first, base, 0);
        n += 1;
        per[pi] += 1;
    }
    if bound.is_some() {
        list.SetDescriptorHeaps(&[Some(cx.sora_heap.clone()), Some(cx.sampler_heap.clone())]);
        if frames {
            list.SetGraphicsRootConstantBufferView(0, cx.frame_constants);
        }
    }
    (n, per)
}

/// (RSF) The per-character tally (PartyKit::tally): column `col` of each plan member's character += its count.
fn tally(kit: &mut PartyKit, plan: &Plan, col: usize, per: &[u32]) {
    for (pm, n) in plan.members.iter().zip(per) {
        let npc = kit.members[pm.member].npc;
        add_tally(kit, npc, col, *n);
    }
}

fn add_tally(kit: &mut PartyKit, npc: &'static str, col: usize, n: u32) {
    match kit.tally.iter_mut().find(|t| t.0 == npc) {
        Some(t) => t.1[col] += n,
        None => {
            let mut row = [0u32; 8];
            row[col] = n;
            kit.tally.push((npc, row));
        }
    }
}

/// (RSF) After the frame's draw passes: each drawn character's frame, early-depth draws and stencil into the tally.
pub(super) fn tally_frame(plan: &Plan, kit: &mut PartyKit) {
    let early = plan.early_per.clone();
    tally(kit, plan, 0, &vec![1; plan.members.len()]);
    tally(kit, plan, 1, &early);
    if let Some(v) = plan.stencil {
        for pm in &plan.members {
            let npc = kit.members[pm.member].npc;
            if let Some(t) = kit.tally.iter_mut().find(|t| t.0 == npc) {
                t.1[2] = v as u32 + 1;
            }
        }
    }
}

/// (ER80) THE GLOW MASK over the party's opaque draws: the target and depth are bound by Sora's pass; `gkey` its key.
pub(super) unsafe fn glow_pass(plan: &Plan, kit: &mut PartyKit, gkey: PsoKey, list: &ID3D12GraphicsCommandList, cx: &PassCx) -> u32 {
    kit.glow_key = Some(gkey);
    // (RSF round 18) each friend with HIS OWN frame block (his camera fade, as his colour draw), not Sora's; Sora's put back
    let (n, per) = redraw(plan, kit, &plan.glow, list, cx, true, None, &|m, slot| m.glow.get(&gkey).and_then(|s| s.get(slot)).cloned());
    list.SetGraphicsRootConstantBufferView(0, cx.frame_constants);
    kit.glow_draws += n;
    tally(kit, plan, 4, &per);
    n
}

/// (ER80b) THE SHAFT MASK: the party's opaque draws with their stencil-writing early-depth states (stencil 1 bound by Sora's).
pub(super) unsafe fn shaft_pass(plan: &Plan, kit: &mut PartyKit, list: &ID3D12GraphicsCommandList, cx: &PassCx) -> u32 {
    let key = cx.key;
    // (RSF round 18) his own frame block, as glow_pass; Sora's put back
    let (n, per) = redraw(plan, kit, &plan.glow, list, cx, true, None, &|m, slot| {
        let set = m.sets.get(&key)?;
        set.get(early_stencil_base(set.len(), m.assets.library.slots.len())? + slot).cloned()
    });
    list.SetGraphicsRootConstantBufferView(0, cx.frame_constants);
    kit.shaft_draws += n;
    tally(kit, plan, 5, &per);
    n
}

/// (ER78) THE G-BUFFER PASS over the party's draws (targets bound by the caller), each member's own scissor box.
pub(super) unsafe fn gbuffer_pass(plan: &Plan, kit: &mut PartyKit, list: &ID3D12GraphicsCommandList, cx: &PassCx, clip: &Mat4, tan_y: f32, width: u32, height: u32) -> u32 {
    let key = cx.key;
    let scissor = |pm: &PlanMember| crate::hybrid::scissor_box(&pm.reach, clip, tan_y, width, height, 24.0).map(|s| RECT { left: s[0], top: s[1], right: s[2], bottom: s[3] });
    let (n, per) = redraw(plan, kit, &plan.gb, list, cx, true, Some(&scissor), &|m, slot| {
        let set = m.sets.get(&key)?;
        set.get(gbuffer_base(set.len(), m.assets.library.slots.len())? + slot).cloned()
    });
    kit.gb_draws += n;
    tally(kit, plan, 3, &per);
    n
}

/// The contact / thrown shadow under every member: Sora's blob pipeline and table (its depth and mask views written by
/// the caller), each member's frame block (his feet, his capsules), a box around his feet and the throw.
pub(super) unsafe fn contact_shadows(plan: &Plan, list: &ID3D12GraphicsCommandList, pipeline: &ID3D12PipelineState, table: D3D12_GPU_DESCRIPTOR_HANDLE, clip: &Mat4, tan_y: f32, width: u32, height: u32, key_dir: Vec3, throw: bool, cx: &PassCx) {
    let mut any = false;
    for pm in &plan.members {
        let Some(feet) = pm.feet else { continue };
        let mut points = vec![(feet, 0.6f32)];
        if throw {
            let reach = (Vec3::new(-key_dir.x, -key_dir.y, 0.0) / key_dir.z.max(0.12) * 120.0).clamp_length_max(1000.0);
            points.push((feet + Vec3::Z * 120.0, 0.6));
            points.push((feet + reach, 0.9));
        }
        let (mut low, mut high) = (glam::Vec2::splat(f32::MAX), glam::Vec2::splat(f32::MIN));
        for (point, radius) in points {
            let c = *clip * point.extend(1.0);
            if c.w < 0.3 {
                (low, high) = (glam::Vec2::ZERO, glam::Vec2::new(width as f32, height as f32));
                break;
            }
            let centre = glam::Vec2::new((c.x / c.w * 0.5 + 0.5) * width as f32, (0.5 - c.y / c.w * 0.5) * height as f32);
            let reach = (radius / (c.w * tan_y) * height as f32 * 0.5).clamp(8.0, height as f32);
            low = low.min(centre - glam::Vec2::splat(reach));
            high = high.max(centre + glam::Vec2::splat(reach));
        }
        let scissor = RECT { left: (low.x as i32).clamp(0, width as i32), top: (low.y as i32).clamp(0, height as i32), right: (high.x as i32).clamp(0, width as i32), bottom: (high.y as i32).clamp(0, height as i32) };
        if scissor.right <= scissor.left || scissor.bottom <= scissor.top {
            continue;
        }
        any = true;
        list.RSSetScissorRects(&[scissor]);
        list.SetPipelineState(pipeline);
        list.SetGraphicsRootConstantBufferView(0, pm.frame_at);
        list.SetGraphicsRootDescriptorTable(3, table);
        list.DrawInstanced(3, 1, 0, 0);
    }
    if any {
        list.SetGraphicsRootConstantBufferView(0, cx.frame_constants);
    }
}

/// The hybrid's block of every member's frame copies set to zero (the G-buffer pass was planned but not recorded: the
/// colour list reads today's light, as Sora's).
pub(super) unsafe fn hybrid_off(plan: &Plan, mapped: *mut u8, frame_len: usize) {
    for pm in &plan.members {
        for offset in [pm.frame_offset, pm.early_offset] {
            let at = offset + (frame_len - crate::hybrid::BLOCK_FLOATS) * 4;
            std::ptr::write_bytes(mapped.add(at), 0, crate::hybrid::BLOCK_FLOATS * 4);
        }
    }
}

/// HIS MOTION VECTORS (the velocity split list; Sora's root state, target and depth are bound): the members' opaque draws
/// with their sets' velocity state.
pub(super) unsafe fn velocity(kit: &mut PartyKit, party: &VeloParty, key: PsoKey, list: &ID3D12GraphicsCommandList, sampler_heap: &ID3D12DescriptorHeap) -> u32 {
    let mut n = 0;
    let mut own_n = 0;
    let mut per_member: Vec<(&'static str, u32, u32)> = Vec::new();
    for (mi, frame, models, own) in party {
        let Some(m) = kit.members.get(*mi) else { continue };
        // (RSF) his own-record state when prepare chose it (his frame copy's previous matrix is the one it reads), else the plain
        let plain = m.sets.get(&key).and_then(|s| s.get(m.assets.library.slots.len() + 1));
        let Some(pipeline) = (if *own { m.velo_own.get(&key) } else { plain }) else { continue };
        let (npc, first) = (m.npc, n);
        list.SetDescriptorHeaps(&[Some(m.assets.srv_heap.clone()), Some(sampler_heap.clone())]);
        list.SetGraphicsRootConstantBufferView(0, *frame);
        list.SetGraphicsRootDescriptorTable(3, m.assets.srv_heap.GetGPUDescriptorHandleForHeapStart());
        list.SetPipelineState(pipeline);
        for (k, now, before) in models {
            let Some(model) = m.assets.models.get(*k) else { continue };
            list.IASetVertexBuffers(0, Some(&[model.vertex_view]));
            list.IASetIndexBuffer(Some(&model.index_view));
            list.SetGraphicsRootConstantBufferView(2, *now);
            list.SetGraphicsRootConstantBufferView(6, *before);
            for draw in &model.draws {
                if !m.assets.library.slots[draw.slot].translucent {
                    list.DrawIndexedInstanced(draw.index_count, 1, draw.first_index, draw.base_vertex, 0);
                    n += 1;
                    own_n += u32::from(*own);
                }
            }
        }
        per_member.push((npc, n - first, if *own { n - first } else { 0 }));
    }
    kit.velo_draws += n;
    kit.velo_own_draws += own_n;
    for (npc, v, o) in per_member {
        add_tally(kit, npc, 6, v);
        add_tally(kit, npc, 7, o);
    }
    n
}

/// (IMPL_ER17, r.shadow_real) THE PARTY IN THE GAME'S CASCADE: the members' opaque draws depth-only with their own skin
/// (the caller has bound the cascade's viewport, the atlas and the data table of the light's matrices).
pub(super) unsafe fn cast(kit: &mut PartyKit, party: &VeloParty, device: &ID3D12Device, root: &ID3D12RootSignature, list: &ID3D12GraphicsCommandList, frame_heap: &ID3D12DescriptorHeap, sampler_heap: &ID3D12DescriptorHeap) -> u32 {
    let mut n = 0;
    for (mi, frame, models, _) in party {
        let Some(m) = kit.members.get_mut(*mi) else { continue };
        if m.cast.is_none() && !m.cast_failed {
            m.cast = cast_pipeline(device, root, &m.npc_dir()).map_err(|e| rlog!("render: (ER party S4) {}: no real shadow ({e})", m.npc)).ok();
            m.cast_failed = m.cast.is_none();
        }
        let Some(pipeline) = m.cast.as_ref() else { continue };
        list.SetDescriptorHeaps(&[Some(frame_heap.clone()), Some(sampler_heap.clone())]);
        list.SetGraphicsRootConstantBufferView(0, *frame);
        list.SetGraphicsRootDescriptorTable(3, frame_heap.GetGPUDescriptorHandleForHeapStart());
        list.SetPipelineState(pipeline);
        for (k, now, _) in models {
            let Some(model) = m.assets.models.get(*k) else { continue };
            list.IASetVertexBuffers(0, Some(&[model.vertex_view]));
            list.IASetIndexBuffer(Some(&model.index_view));
            list.SetGraphicsRootConstantBufferView(2, *now);
            list.SetGraphicsRootConstantBufferView(6, *now);
            for draw in &model.draws {
                if !m.assets.library.slots[draw.slot].translucent {
                    list.DrawIndexedInstanced(draw.index_count, 1, draw.first_index, draw.base_vertex, 0);
                    n += 1;
                }
            }
        }
    }
    kit.cast_draws += n;
    n
}

impl Member {
    fn npc_dir(&self) -> std::path::PathBuf {
        self.assets.library.dir.parent().map(|p| p.to_path_buf()).unwrap_or_default()
    }
}

/// gfx.rs `make_cast_kit`'s depth-only pipeline with the member's own `shadow.vs.dxil` (320 joints).
unsafe fn cast_pipeline(device: &ID3D12Device, root: &ID3D12RootSignature, dir: &Path) -> Result<ID3D12PipelineState, String> {
    let file = dir.join("materials").join("er").join("shadow.vs.dxil");
    let vertex = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    let elements = layout();
    let pso = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
        pRootSignature: std::mem::transmute_copy(root),
        VS: D3D12_SHADER_BYTECODE { pShaderBytecode: vertex.as_ptr() as *const c_void, BytecodeLength: vertex.len() },
        BlendState: D3D12_BLEND_DESC::default(),
        SampleMask: u32::MAX,
        RasterizerState: D3D12_RASTERIZER_DESC { FillMode: D3D12_FILL_MODE_SOLID, CullMode: D3D12_CULL_MODE_NONE, DepthClipEnable: false.into(), ..Default::default() },
        DepthStencilState: D3D12_DEPTH_STENCIL_DESC { DepthEnable: true.into(), DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ALL, DepthFunc: D3D12_COMPARISON_FUNC_GREATER_EQUAL, StencilEnable: false.into(), ..Default::default() },
        InputLayout: D3D12_INPUT_LAYOUT_DESC { pInputElementDescs: elements.as_ptr(), NumElements: elements.len() as u32 },
        PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
        NumRenderTargets: 0,
        DSVFormat: DXGI_FORMAT_D16_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        ..Default::default()
    };
    device.CreateGraphicsPipelineState(&pso).map_err(|e| format!("CreatePipelineState: {e}"))
}

/// The once-a-second line (gfx.rs `status`).
pub(super) fn status(kit: &mut PartyKit) -> String {
    let members: Vec<String> = kit.members.iter().map(|m| format!("{} ({} sets{}{})", m.npc, m.sets.len(), if m.glow.is_empty() { "" } else { ", glow" }, if m.velo_own.is_empty() { "" } else { ", own-record velocity" })).collect();
    let line = format!(
        "{} frame(s) with the party, {} draws (early {}, G-buffer {}, glow mask {}, shaft mask {}, velocity {} (RSF: {} with the own-record route, {} skin(s) without motion: a jump / a gap), real shadow {}), {} picture(s), {} void frame(s); CPU {:.3} ms/frame (max {:.3}); made: {}{}{}",
        kit.frames,
        kit.draws,
        kit.early_draws,
        kit.gb_draws,
        kit.glow_draws,
        kit.shaft_draws,
        kit.velo_draws,
        kit.velo_own_draws,
        kit.snaps,
        kit.cast_draws,
        kit.pictures,
        kit.void_frames,
        kit.micros as f64 / 1000.0 / kit.frames.max(1) as f64,
        kit.micros_max as f64 / 1000.0,
        if members.is_empty() { "none".to_string() } else { members.join(", ") },
        if kit.failed.is_empty() { String::new() } else { format!("; FAILED {:?}", kit.failed) },
        if kit.waiting.is_empty() { String::new() } else { format!("; waiting: {}", kit.waiting) },
    );
    (kit.frames, kit.draws, kit.early_draws, kit.gb_draws, kit.glow_draws, kit.shaft_draws, kit.velo_draws, kit.cast_draws, kit.pictures, kit.void_frames, kit.micros, kit.micros_max) = (0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
    (kit.velo_own_draws, kit.snaps) = (0, 0);
    let per: Vec<String> = kit
        .tally
        .iter()
        .map(|(npc, t)| format!("{npc}: {} frame(s), early {} (stencil {}), G-buffer {}, glow {}, shaft {}, velocity {} (own-record {})", t[0], t[1], if t[2] == 0 { "none".to_string() } else { format!("{:#04x}", t[2] - 1) }, t[3], t[4], t[5], t[6], t[7]))
        .collect();
    kit.tally.clear();
    let line = if per.is_empty() { line } else { format!("{line}; (RSF) per character: {}", per.join(" | ")) };
    // (RSF2) the light levels each drawn character's block carries (his own when "own"; Sora's for comparison; his reflection)
    let r3 = |v: Vec3| v.to_array().map(|x| (x * 1.0e4).round() / 1.0e4);
    let lv: Vec<String> = kit.levels.iter().map(|(n, l, own, sora, env, seen)| format!("{n}: measured x{:.1} ({}), drawn at the shared x{:.1}, reflection {}{}", 1.0 / l.max(1.0e-9), if *own { "his own" } else { "Sora's" }, 1.0 / sora.max(1.0e-9), if *env < 0.0 { "Sora's".to_string() } else { format!("x{env:.2}") }, seen.map_or(String::new(), |(t, a, sc)| format!(", his probe: {t:.0} tap(s), surface {:?}, scene {:?}", r3(a), r3(sc))))).collect();
    let line = if lv.is_empty() { line } else { format!("{line}; (RSF2) light levels: {}", lv.join(" | ")) };
    kit.waiting = "";
    if let Ok(mut s) = crate::party::STATUS.lock() {
        s.clone_from(&line);
    }
    line
}

// ---------------------------------------------------------------------------------------------------- the camp pictures

impl PicKit {
    unsafe fn new(device: &ID3D12Device) -> windows::core::Result<PicKit> {
        let mut slots = Vec::new();
        for _ in 0..3 {
            let allocator: ID3D12CommandAllocator = device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
            let list: ID3D12GraphicsCommandList = device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator, None)?;
            list.Close()?;
            let upload = committed(device, D3D12_HEAP_TYPE_UPLOAD, &buffer_desc(UPLOAD_BYTES as u64), D3D12_RESOURCE_STATE_GENERIC_READ, None)?;
            let mut at: *mut c_void = std::ptr::null_mut();
            upload.Map(0, None, Some(&mut at))?;
            slots.push((allocator, list, upload, at as *mut u8, 0u64));
        }
        let rtv: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV, NumDescriptors: 1, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE, NodeMask: 0 })?;
        let dsv: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_DSV, NumDescriptors: 1, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE, NodeMask: 0 })?;
        Ok(PicKit { slots, rtv, dsv, target: None, khs: None })
    }

    pub(super) unsafe fn unmap(&self) {
        for s in &self.slots {
            s.2.Unmap(0, None);
        }
    }
}

const PIC_FORMAT: DXGI_FORMAT = DXGI_FORMAT_R8G8B8A8_UNORM;

impl Core {
    /// AT PRESENT, after Sora's portrait: the friends' camp pictures (gfx/portrait.rs's approach, each into a target of
    /// its own, his UI actor's lights and capture), handed to the overlay together.
    pub(super) fn party_portraits_at_present(&mut self) {
        let Some(list) = crate::party::portraits_now() else { return };
        if self.party.pics_failed.is_some_and(|at| at.elapsed().as_secs() < 5) {
            return;
        }
        let mut pictures = Vec::new();
        for p in list.iter() {
            // SAFETY: our own objects on the game's device and queue, on the presenting thread (as Sora's portrait).
            match unsafe { self.draw_party_portrait(p) } {
                Ok(Some(picture)) => pictures.push(picture),
                Ok(None) => {}
                Err(e) => {
                    rlog!("render: (ER party S4) a friend's camp picture could not be drawn ({e}): tried again in 5 s");
                    self.party.pics_failed = Some(Instant::now());
                    crate::hud::set_party_pictures(Vec::new());
                    return;
                }
            }
        }
        self.party.pictures += pictures.len() as u32;
        crate::hud::set_party_pictures(pictures);
    }

    unsafe fn draw_party_portrait(&mut self, p: &crate::party::PartyPortrait) -> windows::core::Result<Option<crate::hud::Picture>> {
        let frame = &p.frame;
        let (width, height) = (frame.visible.2.clamp(2, 8192), frame.visible.3.clamp(2, 8192));
        let Some(options) = crate::SHARED.lock().ok().and_then(|g| g.as_ref().map(|s| s.options.clone())) else { return Ok(None) };
        let key = PsoKey { target: PIC_FORMAT.0, depth: DXGI_FORMAT_D32_FLOAT.0, front_ccw: options.front_ccw, cull: options.cull, alpha: true };
        if !self.party.keys.contains(&key) {
            self.party.keys.push(key);
        }
        let Some(mi) = self.party.members.iter().position(|m| m.chara == p.chara && m.sets.contains_key(&key)) else { return Ok(None) };
        let Some(body) = frame.input.body.as_ref() else { return Ok(None) };
        self.device.GetDeviceRemovedReason()?;
        if !self.party.pics.iter().any(|(c, _)| *c == p.chara) {
            let kit = PicKit::new(&self.device)?;
            self.party.pics.push((p.chara, kit));
        }
        let completed = self.fence.GetCompletedValue();
        let Core { device, queue, root, sampler_heap, sampler_index, fence, fence_value, party, retired, .. } = self;
        let (members, pics) = (&mut party.members, &mut party.pics);
        let m = &mut members[mi];
        let kit = &mut pics.iter_mut().find(|(c, _)| *c == p.chara).unwrap().1;
        let Some(slot_index) = (0..kit.slots.len()).find(|k| kit.slots[*k].4 <= completed) else { return Ok(None) };
        if !kit.target.as_ref().is_some_and(|(_, _, w, h)| *w >= width && *h >= height) {
            let (w, h) = super::portrait::target_size(width, height);
            let clear = D3D12_CLEAR_VALUE { Format: PIC_FORMAT, Anonymous: D3D12_CLEAR_VALUE_0 { Color: frame.clear } };
            let colour = committed(device, D3D12_HEAP_TYPE_DEFAULT, &texture_desc(w, h, 1, 1, PIC_FORMAT, D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET), D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, Some(&clear))?;
            let clear = D3D12_CLEAR_VALUE { Format: DXGI_FORMAT_D32_FLOAT, Anonymous: D3D12_CLEAR_VALUE_0 { DepthStencil: D3D12_DEPTH_STENCIL_VALUE { Depth: 0.0, Stencil: 0 } } };
            let depth = committed(device, D3D12_HEAP_TYPE_DEFAULT, &texture_desc(w, h, 1, 1, DXGI_FORMAT_D32_FLOAT, D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL), D3D12_RESOURCE_STATE_DEPTH_WRITE, Some(&clear))?;
            crate::hud::set_party_pictures(Vec::new());
            if let Some((old_colour, old_depth, ..)) = kit.target.replace((colour, depth, w, h)) {
                retired.push((*fence_value, old_colour));
                retired.push((*fence_value, old_depth));
            }
            let (colour, depth, ..) = kit.target.as_ref().unwrap();
            device.CreateRenderTargetView(colour, None, kit.rtv.GetCPUDescriptorHandleForHeapStart());
            device.CreateDepthStencilView(depth, None, kit.dsv.GetCPUDescriptorHandleForHeapStart());
            rlog!("render: (ER party S4) {}'s camp picture has its own target {w}x{h}", m.npc);
        }
        let (colour, _, target_w, target_h) = kit.target.as_ref().map(|(c, d, w, h)| (c.clone(), d.clone(), *w, *h)).unwrap();
        // ---- the frame block: gfx/portrait.rs's (the same shaders read it)
        let input = &frame.input;
        let camera = input.camera;
        let clip = math::clip_from_game(&camera, input.origin);
        let camera_game = math::game_pos_of_host(camera.position, input.origin);
        let tan_y = (camera.fov * 0.5).tan();
        let (depth_a, depth_b) = math::depth_to_view(camera.near, camera.far);
        let rig = &frame.rig;
        let (picture_w, picture_h) = (frame.picture.2.max(1.0), frame.picture.3.max(1.0));
        let mut constants: Vec<f32> = Vec::with_capacity(512);
        constants.extend_from_slice(&clip.to_cols_array());
        for v in [
            camera_game.extend(input.clock),
            math::game_dir_of_host(camera.right).extend(0.0),
            math::game_dir_of_host(camera.up).extend(0.0),
            math::game_dir_of_host(camera.forward).extend(0.0),
            Vec4::new(width as f32, height as f32, 1.0 / width as f32, 1.0 / height as f32),
            Vec4::new(0.0, 0.0, 1.0, 0.0),
            Vec4::new(1.0, 1.0, 1.0, 0.0),
            Vec4::new(tan_y * camera.aspect, tan_y, camera.near, camera.far),
            Vec4::new(0.0, options.bias, depth_a, depth_b),
            Vec4::new(1.0, 1.0, 1.0, 1.0),
            Vec4::new(1.0, 0.0, 0.0, 1.0),
            Vec4::ZERO,
            Vec4::new(0.0, 250.0, 150.0, 0.0),
            Vec4::ZERO,
            Vec4::new(1.0, 1.0, 1.0, 0.0),
            Vec4::ZERO,
            Vec4::ZERO,
            Vec4::ZERO,
        ] {
            constants.extend_from_slice(&v.to_array());
        }
        constants.extend_from_slice(&math::host_from_game(input.origin).to_cols_array());
        constants.extend_from_slice(&math::host_from_game(input.origin).to_cols_array());
        constants.extend_from_slice(&[0.0; 8 * 16]);
        constants.extend_from_slice(&[0.0; crate::pointlight::SORA_BLOCK_FLOATS]);
        constants.extend_from_slice(&[0.0; crate::blood::BLOCK_FLOATS]);
        constants.extend_from_slice(&[0.0; crate::hybrid::BLOCK_FLOATS]);
        // ---- his materials: the menu's preset, his bone-fed values of this pose
        if kit.khs.as_ref().map(|(name, _)| name.as_str()) != Some(frame.khs.as_str()) {
            let on = frame.khs != "off" && !m.assets.library.khs.presets.is_empty();
            kit.khs = Some((frame.khs.clone(), m.assets.library.khs.preset_state(&frame.khs, on)));
        }
        {
            let table = m.assets.library.bone_table.clone();
            let bones = &m.bones;
            let values = kh_look::bone_parameters(&table, |name| {
                let bone = *bones.get(name)?;
                let component = *body.component.get(bone)?;
                Some((*body.local.get(bone)?, component, component.then(&body.component_to_world)))
            });
            m.assets.library.feed(m.npc, &values);
        }
        let (allocator, list, upload, mapped, busy) = &mut kit.slots[slot_index];
        let mut cursor = 0usize;
        let upload_address = upload.GetGPUVirtualAddress();
        let mapped = *mapped;
        let mut put = |floats: &[f32]| -> Option<u64> {
            let bytes = floats.len() * 4;
            if cursor + bytes > UPLOAD_BYTES {
                return None;
            }
            std::ptr::copy_nonoverlapping(floats.as_ptr() as *const u8, mapped.add(cursor), bytes);
            let at = upload_address + cursor as u64;
            cursor = (cursor + bytes + 255) & !255;
            Some(at)
        };
        let overflow = || windows::core::Error::from_hresult(windows::Win32::Foundation::E_OUTOFMEMORY);
        let frame_constants = put(&constants).ok_or_else(overflow)?;
        allocator.Reset()?;
        list.Reset(&*allocator, None)?;
        let (target_view, depth_view) = (kit.rtv.GetCPUDescriptorHandleForHeapStart(), kit.dsv.GetCPUDescriptorHandleForHeapStart());
        list.ResourceBarrier(&[transition(&colour, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, D3D12_RESOURCE_STATE_RENDER_TARGET)]);
        list.OMSetRenderTargets(1, Some(&target_view), false, Some(&depth_view));
        list.ClearRenderTargetView(target_view, &frame.clear, None);
        list.ClearDepthStencilView(depth_view, D3D12_CLEAR_FLAG_DEPTH, 0.0, 0, None);
        list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: frame.picture.0 - frame.visible.0 as f32, TopLeftY: frame.picture.1 - frame.visible.1 as f32, Width: picture_w, Height: picture_h, MinDepth: 0.0, MaxDepth: 1.0 }]);
        list.RSSetScissorRects(&[RECT { left: 0, top: 0, right: width.min(target_w) as i32, bottom: height.min(target_h) as i32 }]);
        list.SetGraphicsRootSignature(&*root);
        list.SetDescriptorHeaps(&[Some(m.assets.srv_heap.clone()), Some(sampler_heap.clone())]);
        list.SetGraphicsRootConstantBufferView(0, frame_constants);
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        list.SetGraphicsRootDescriptorTable(4, sampler_heap.GetGPUDescriptorHandleForHeapStart());
        list.SetGraphicsRootShaderResourceView(5, sampler_index.GetGPUVirtualAddress());
        let pipelines = &m.sets[&key];
        let srv_start = m.assets.srv_heap.GetGPUDescriptorHandleForHeapStart();
        let sh = rig.sh();
        let Some(k) = m.body else { return Ok(None) };
        let model = &m.assets.models[k];
        let mut skin: Vec<f32> = Vec::with_capacity(SKIN_FLOATS);
        skin.extend_from_slice(&math::game_from_pack(&body.component_to_world).to_cols_array());
        for (joint, bone) in model.joint_bones.iter().enumerate() {
            let matrix = match bone.and_then(|b| body.component.get(b)) {
                Some(t) => math::joint_in_pack(t) * model.inverse_bind[joint],
                None => Mat4::IDENTITY,
            };
            skin.extend_from_slice(&matrix.to_cols_array());
        }
        skin.resize(SKIN_FLOATS, 0.0);
        let skin_at = put(&skin).ok_or_else(overflow)?;
        let object = ObjectRows::new(&body.component_to_world, m.assets.library.bounds_of(model.mesh));
        list.IASetVertexBuffers(0, Some(&[model.vertex_view]));
        list.IASetIndexBuffer(Some(&model.index_view));
        list.SetGraphicsRootConstantBufferView(2, skin_at);
        list.SetGraphicsRootConstantBufferView(6, skin_at);
        for translucent in [false, true] {
            for draw in &model.draws {
                let material = &m.assets.library.slots[draw.slot];
                if material.translucent != translucent {
                    continue;
                }
                let mut params = m.assets.library.params(material, &object, rig, kit.khs.as_ref().map(|(_, s)| s), input.clock, frame.exposure, 0.0);
                params.sh = sh;
                params.env.y = frame.env_level;
                let material_constants = put(params.as_floats()).ok_or_else(overflow)?;
                list.SetPipelineState(&pipelines[draw.slot]);
                list.SetGraphicsRootConstantBufferView(1, material_constants);
                list.SetGraphicsRootDescriptorTable(3, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: srv_start.ptr + (draw.slot * SLOT_TEXTURES * m.assets.srv_size) as u64 });
                list.DrawIndexedInstanced(draw.index_count, 1, draw.first_index, draw.base_vertex, 0);
            }
        }
        list.ResourceBarrier(&[transition(&colour, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE)]);
        list.Close()?;
        queue.ExecuteCommandLists(&[Some(list.cast::<ID3D12CommandList>()?)]);
        *fence_value += 1;
        queue.Signal(&*fence, *fence_value)?;
        *busy = *fence_value;
        Ok(Some(crate::hud::Picture { texture: colour, before: frame.before_layer, with_layer: frame.with_layer, rect: (frame.visible.0, frame.visible.1, width.min(target_w) as usize, height.min(target_h) as usize), alpha: frame.alpha }))
    }
}

#[cfg(test)]
mod erpi_tests {
    use super::*;

    fn member(caps: Vec<(Vec3, Vec3, f32)>) -> PlanMember {
        PlanMember { member: 0, frame_at: 0, early_at: 0, models: Vec::new(), glow_mask: false, caps, feet: None, frame_offset: 0, early_offset: 0, light: None, probe_row: None, velo_own: false, reach: Vec::new(), env_level: None, light_ratio: 1.0 }
    }

    /// (ER party I) the light pass's copy: Sora's body capsules (w 1) stay, his Keyblade's (w 2) and the empty slots take the
    /// friends' (w 1); no friend = no copy.
    #[test]
    fn erpi_the_light_pass_gets_the_friends_capsules() {
        let mut frame = vec![0.0f32; CAPS_AT + 16 * 8 + 64];
        for k in 0..16 {
            let w = if k < 13 { 1.0 } else if k < 15 { 2.0 } else { 0.0 };
            frame[CAPS_AT + 8 * k + 3] = 10.0;
            frame[CAPS_AT + 8 * k + 7] = w;
        }
        assert!(light_pass_frame(&Plan::default(), &frame).is_none());
        let body = |x: f32| (0..13).map(|k| (Vec3::new(x, 0.0, 10.0 * k as f32), Vec3::new(x, 0.0, 10.0 * k as f32 + 8.0), 9.0)).collect::<Vec<_>>();
        let plan = Plan { members: vec![member(body(100.0)), member(body(-100.0)), member(Vec::new())], ..Plan::default() };
        let out = light_pass_frame(&plan, &frame).expect("a copy");
        // Sora's 13 body capsules untouched
        assert_eq!(out[CAPS_AT..CAPS_AT + 13 * 8], frame[CAPS_AT..CAPS_AT + 13 * 8]);
        // 3 free slots, 26 friend capsules: one around each friend, upright, w 1
        for (k, x) in [(13usize, 100.0f32), (14, -100.0)] {
            let row = CAPS_AT + 8 * k;
            assert_eq!((out[row], out[row + 4], out[row + 7]), (x, x, 1.0));
            assert!(out[row + 2] < out[row + 6] && out[row + 3] >= 9.0 && out[row + 3] <= 40.0, "{:?}", &out[row..row + 8]);
        }
        assert_eq!(out[CAPS_AT + 8 * 15 + 7], 0.0);
        // Sora not drawn (no body capsules): each friend's full set fits
        let mut empty = frame.clone();
        for k in 0..16 {
            empty[CAPS_AT + 8 * k + 7] = 0.0;
        }
        let small = Plan { members: vec![member(body(100.0)[..8].to_vec()), member(body(-100.0)[..8].to_vec())], ..Plan::default() };
        let out = light_pass_frame(&small, &empty).expect("a copy");
        assert!((0..16).all(|k| out[CAPS_AT + 8 * k + 7] == 1.0));
        assert_eq!(out[CAPS_AT + 8 * 9], -100.0);
    }
}

/// (RSF) The friends' motion vectors against Sora's (ER88's standard), on a CPU mirror of er_char.wgsl `vs_main`'s velocity
/// part (the plain skin and skin_v88's ER_VELO_OWN), with the game's matrices of a camera that follows / turns: the error
/// against where each point was really drawn a frame ago, in pixels at 2560 x 1440.
#[cfg(test)]
pub(super) mod rsf_tests {
    use super::*;
    use crate::math::{self, Camera};

    pub const SIZE: (f32, f32) = (2560.0, 1440.0);

    /// A camera at `eye` looking at `at` (GAME space, cm), as the harness makes it.
    pub fn camera(eye_game: Vec3, at_game: Vec3, origin: Vec3) -> Camera {
        let host = |g: Vec3| math::host_from_game(origin).transform_point3(g);
        let (eye, target) = (host(eye_game), host(at_game));
        let forward = (target - eye).normalize();
        let right = Vec3::Y.cross(forward).normalize();
        let up = forward.cross(right);
        Camera { right, up, forward, position: eye, fov: 0.8378, aspect: SIZE.0 / SIZE.1, near: 0.1, far: 5000.0 }
    }

    /// The game's VC_MatrixViewProj of a camera: clip of (host - SC_CameraPos).
    fn game_vp(c: &Camera) -> Mat4 {
        math::clip_from_view(c.fov, c.aspect, c.near, c.far) * math::view_from_host(c) * Mat4::from_translation(c.position)
    }

    fn ndc(v: Vec4) -> glam::Vec2 {
        glam::Vec2::new(v.x / v.w, v.y / v.w)
    }

    fn px(d: glam::Vec2) -> glam::Vec2 {
        glam::Vec2::new(d.x * 0.5 * SIZE.0, -d.y * 0.5 * SIZE.1)
    }

    /// er_char.wgsl vs_main (er.tune.w on): the velocity (px) of a point at `now` / a frame ago at `before` (GAME space), with
    /// the frame block's `prev_host`; `own` = the skin_v88 compilation (ER_VELO_OWN), else the plain skin.
    pub fn shader_velocity(now: Vec3, before: Vec3, cam_now: &Camera, cam_before: &Camera, origin: Vec3, prev_host: Mat4, own: bool) -> glam::Vec2 {
        let host_from_game = math::host_from_game(origin);
        let (vp, prev_vp) = (game_vp(cam_now), game_vp(cam_before));
        let rel = |m: &Mat4, host: Vec3, cam: Vec3| *m * (host - cam).extend(1.0);
        let cur = rel(&vp, host_from_game.transform_point3(now), cam_now.position);
        let host_own = prev_host * before.extend(1.0);
        let prev = if own {
            let own_now = math::clip_from_game(cam_now, origin) * now.extend(1.0);
            let off = (ndc(own_now) - ndc(cur)).abs() * glam::Vec2::new(0.5 * SIZE.0, 0.5 * SIZE.1);
            if host_own.w > 1.5 && off.x < 1.5 && off.y < 1.5 {
                rel(&vp, host_own.truncate(), cam_now.position)
            } else {
                let hb = if host_own.w > 1.5 { host_from_game * before.extend(1.0) } else { host_own };
                rel(&prev_vp, hb.truncate(), cam_before.position)
            }
        } else {
            rel(&prev_vp, host_own.truncate(), cam_before.position)
        };
        px(ndc(cur) - ndc(prev))
    }

    /// Where the point really moved on screen (drawn a frame ago through the camera of that frame).
    pub fn true_velocity(now: Vec3, before: Vec3, cam_now: &Camera, cam_before: &Camera, origin: Vec3) -> glam::Vec2 {
        let cur = game_vp(cam_now) * (math::host_from_game(origin).transform_point3(now) - cam_now.position).extend(1.0);
        let prev = game_vp(cam_before) * (math::host_from_game(origin).transform_point3(before) - cam_before.position).extend(1.0);
        px(ndc(cur) - ndc(prev))
    }

    /// One frame of a scenario: the camera now / a frame ago, a friend's points now / a frame ago (GAME space).
    pub struct Step {
        pub cam_now: Camera,
        pub cam_before: Camera,
        pub points: Vec<(Vec3, Vec3)>,
    }

    /// `run`: a friend LEADS, running at 6 m/s along game +X, the camera following 3.5 m behind and 1.6 m up and turning
    /// 1.5 deg a frame around him; `turn`: he stands, the camera orbits him 3 deg a frame (a stick flick); `ai`: he runs as an
    /// AI friend 1.5 m beside the leader the camera follows. Points: his feet, hips, chest, head, both hands.
    pub fn scenario(kind: &str, frame: u32) -> Step {
        let origin = Vec3::ZERO;
        let body = |feet: Vec3| [0.0f32, 90.0, 130.0, 175.0].iter().map(|z| feet + Vec3::Z * *z).chain([feet + Vec3::new(10.0, 35.0, 100.0), feet + Vec3::new(-10.0, -35.0, 100.0)]).collect::<Vec<_>>();
        let at = |f: u32| -> (Camera, Vec<Vec3>) {
            let t = f as f32;
            if kind == "turn" {
                let feet = Vec3::ZERO;
                let yaw = (3.0 * t).to_radians();
                let eye = feet + Vec3::new(-350.0 * yaw.cos(), -350.0 * yaw.sin(), 160.0);
                (camera(eye, feet + Vec3::Z * 110.0, origin), body(feet))
            } else {
                let leader = Vec3::new(10.0 * t, 0.0, 0.0);
                let yaw = (1.5 * t).to_radians();
                let eye = leader + Vec3::new(-350.0 * yaw.cos(), -350.0 * yaw.sin(), 160.0);
                let feet = if kind == "ai" { leader + Vec3::new(-60.0, 150.0, 0.0) } else { leader };
                (camera(eye, leader + Vec3::Z * 110.0, origin), body(feet))
            }
        };
        let (cam_now, now) = at(frame);
        let (cam_before, before) = at(frame - 1);
        Step { cam_now, cam_before, points: now.into_iter().zip(before).collect() }
    }

    /// The mean and worst error (px) of a route over 30 frames of a scenario and the mean true motion: `flagged` = the frame
    /// block carries the own-record matrix (Sora's own route is on), `own` = the velocity draw runs skin_v88.
    pub fn route_error(kind: &str, flagged: bool, own: bool) -> (f32, f32, f32) {
        let (mut sum, mut worst, mut n, mut speed) = (0.0f32, 0.0f32, 0, 0.0f32);
        for frame in 1..31 {
            let s = scenario(kind, frame);
            let prev_host = if flagged { crate::taa::own_matrix(&s.cam_now, &s.cam_before, Vec3::ZERO) } else { math::host_from_game(Vec3::ZERO) };
            for (now, before) in &s.points {
                let v = shader_velocity(*now, *before, &s.cam_now, &s.cam_before, Vec3::ZERO, prev_host, own);
                let t = true_velocity(*now, *before, &s.cam_now, &s.cam_before, Vec3::ZERO);
                let e = (v - t).length();
                (sum, worst, n, speed) = (sum + e, worst.max(e), n + 1, speed + t.length());
            }
        }
        (sum / n as f32, worst, speed / n as f32)
    }

    #[test]
    fn rsf_the_friends_motion_vectors_match_soras_on_every_route() {
        for kind in ["run", "turn", "ai"] {
            let before_fix = route_error(kind, true, false);
            let after_own = route_error(kind, true, true);
            let after_plain = route_error(kind, false, false);
            println!("RSF velocity {kind}: true motion {:.2} px/frame mean | friends BEFORE (own-record matrix read by the plain skin) error mean {:.2} worst {:.2} px | AFTER own-record route (skin_v88, = Sora's) {:.4} / {:.4} | AFTER plain route (plain matrix) {:.4} / {:.4}", before_fix.2, before_fix.0, before_fix.1, after_own.0, after_own.1, after_plain.0, after_plain.1);
            // the bug: the camera's motion counted twice (several px a frame); the fix: what Sora's route draws (exact)
            assert!(before_fix.0 > 1.0 && before_fix.1 > 5.0, "{kind}: the doubled camera part shows: {before_fix:?}");
            assert!(after_own.1 < 0.01 && after_plain.1 < 0.01, "{kind}: {after_own:?} {after_plain:?}");
        }
    }

    #[test]
    fn rsf_no_skinned_motion_across_a_gap_a_jump_or_a_turn() {
        let at = |x: f32, yaw: f32| Trs { translation: Vec3::new(x, 0.0, 0.0), rotation: glam::Quat::from_rotation_z(yaw.to_radians()), scale: Vec3::ONE };
        assert!(motion_continues(10, &at(0.0, 0.0), 11, &at(15.0, 20.0)), "the next frame, a run step");
        assert!(!motion_continues(10, &at(0.0, 0.0), 12, &at(15.0, 0.0)), "a gap (hidden, a load): no motion");
        assert!(!motion_continues(10, &at(0.0, 0.0), 10, &at(0.0, 0.0)), "the same frame (another instance)");
        assert!(!motion_continues(10, &at(0.0, 0.0), 11, &at(SNAP_CM + 1.0, 0.0)), "a warp / reseat (> 3 m)");
        assert!(!motion_continues(10, &at(0.0, 0.0), 11, &at(0.0, SNAP_DEG + 5.0)), "a facing set at once (> 90 deg)");
        assert!(motion_continues(10, &at(0.0, 0.0), 11, &at(0.0, 80.0)), "a quick turn stays motion");
    }

    /// (RSF, the user's requirement: every KH character we draw, now and later) A MOCKED EXTRA CHARACTER that no code names: a
    /// 2.6 m lanky figure with a long snout, a tall hat, arms out and big feet, its bones without any KH name, a shield-sized
    /// weapon. It gets (1) a screen box holding every part of his mesh (the old capsule box, his named capsules, missed them),
    /// (2) the right motion vectors on both velocity routes, from a frame block carrying Sora's own-record matrix.
    #[test]
    fn rsf_a_mocked_extra_character_gets_the_box_and_the_velocity() {
        let at = |x: f32, y: f32, z: f32| Trs { translation: Vec3::new(x, y, z), ..Trs::IDENTITY };
        // component space (cm): a spine to 220, a snout 45 cm forward at 205, a hat to 265, arms to +-95 at 150, feet 35 forward
        let mut bones: Vec<Trs> = Vec::new();
        for k in 0..12 {
            bones.push(at(0.0, 0.0, 20.0 * k as f32));
        }
        for k in 1..4 {
            bones.push(at(15.0 * k as f32, 0.0, 205.0));
            bones.push(at(0.0, 0.0, 225.0 + 13.0 * k as f32));
            bones.push(at(0.0, 32.0 * k as f32, 150.0));
            bones.push(at(0.0, -32.0 * k as f32, 150.0));
        }
        for side in [-18.0f32, 18.0] {
            for k in 0..3 {
                bones.push(at(12.0 * k as f32, side, 5.0));
            }
        }
        let to_world = Trs { translation: Vec3::new(500.0, 200.0, 0.0), rotation: glam::Quat::from_rotation_z(0.5), scale: Vec3::ONE };
        let body = Body { component_to_world: to_world, local: bones.clone(), component: bones.clone() };
        // his mesh: points up to 25 cm round each bone (a deterministic scatter)
        let mut mesh: Vec<Vec3> = Vec::new();
        for (k, b) in bones.iter().enumerate() {
            for j in 0..12 {
                let a = (k * 12 + j) as f32 * 2.399;
                let d = Vec3::new(a.cos() * (a * 0.7).sin(), a.sin() * (a * 0.7).sin(), (a * 0.7).cos()) * 25.0;
                mesh.push(b.then(&to_world).translation + d);
            }
        }
        // his weapon: a disc of 45 cm at his right hand, its cooked bounds a 45 cm ball
        let hand = bones[bones.len() - 7].then(&to_world).translation;
        let weapon = Trs { translation: hand, ..Trs::IDENTITY };
        for k in 0..16 {
            let a = k as f32 * std::f32::consts::TAU / 16.0;
            mesh.push(hand + Vec3::new(a.cos(), 0.0, a.sin()) * 45.0);
        }
        let origin = Vec3::ZERO;
        let cam = camera(Vec3::new(500.0 - 330.0, 260.0, 160.0), Vec3::new(500.0, 200.0, 140.0), origin);
        let clip = math::clip_from_game(&cam, origin);
        let tan_y = (cam.fov * 0.5).tan();
        let (w, h) = (2560u32, 1440u32);
        let inside = |bx: [i32; 4], p: Vec3| {
            let c = clip * p.extend(1.0);
            let (x, y) = ((c.x / c.w * 0.5 + 0.5) * w as f32, (0.5 - c.y / c.w * 0.5) * h as f32);
            // (a point off the screen needs no box)
            !(0.0..w as f32).contains(&x) || !(0.0..h as f32).contains(&y) || (x >= bx[0] as f32 && x <= bx[2] as f32 && y >= bx[1] as f32 && y <= bx[3] as f32)
        };
        // the old box: his named capsules (none of his bones has a KH name: the single stand-in capsule)
        let caps = caps_of(&HashMap::new(), &body);
        let old = crate::hybrid::scissor_box(&caps, &clip, tan_y, w, h, 24.0).expect("a box");
        let missed_old = mesh.iter().filter(|p| !inside(old, **p)).count();
        // the new box: prepare's reach (capsules + every bone + the weapon's bounds)
        let mut reach = caps.clone();
        reach.extend(crate::hybrid::bone_balls(&body.component, &to_world));
        reach.push(crate::hybrid::bounds_ball(&weapon, (Vec3::ZERO, Vec3::splat(45.0), 45.0)));
        let new = crate::hybrid::scissor_box(&reach, &clip, tan_y, w, h, 24.0).expect("a box");
        let missed_new = mesh.iter().filter(|p| !inside(new, **p)).count();
        println!("RSF mocked character: {} mesh points; outside the old box {missed_old} ({old:?}), outside the new box {missed_new} ({new:?})", mesh.len());
        assert!(missed_old > mesh.len() / 10, "the old box misses his far parts: {missed_old}");
        assert_eq!(missed_new, 0, "every part of him in his box");
        // the velocity: his frame copy from a frame block with the own-record matrix (Sora's route on), both routes
        let mut worst = [0.0f32; 3];
        for frame in 1..31 {
            let s = scenario("run", frame);
            let step = |f: u32| Vec3::new(10.0 * f as f32, 0.0, 0.0);
            let flagged = crate::taa::own_matrix(&s.cam_now, &s.cam_before, origin);
            for (route, own_state) in [(0usize, true), (1, false)] {
                let mut block = vec![0.0f32; PREV_AT + 16];
                block[PREV_AT..PREV_AT + 16].copy_from_slice(&flagged.to_cols_array());
                let own = velocity_route(&mut block, own_state, &math::host_from_game(origin));
                let prev_host = Mat4::from_cols_slice(&block[PREV_AT..PREV_AT + 16]);
                for p in mesh.iter().step_by(7) {
                    let (now, before) = (*p - Vec3::new(500.0, 200.0, 0.0) + step(frame), *p - Vec3::new(500.0, 200.0, 0.0) + step(frame - 1));
                    let e = (shader_velocity(now, before, &s.cam_now, &s.cam_before, origin, prev_host, own) - true_velocity(now, before, &s.cam_now, &s.cam_before, origin)).length();
                    worst[route] = worst[route].max(e);
                    if route == 1 {
                        // the unfixed path: the flagged matrix read by the plain skin
                        let bad = (shader_velocity(now, before, &s.cam_now, &s.cam_before, origin, flagged, false) - true_velocity(now, before, &s.cam_now, &s.cam_before, origin)).length();
                        worst[2] = worst[2].max(bad);
                    }
                }
            }
        }
        println!("RSF mocked character velocity (run): worst error own-record route {:.4} px, plain route {:.4} px; unfixed {:.2} px", worst[0], worst[1], worst[2]);
        assert!(worst[0] < 0.01 && worst[1] < 0.01 && worst[2] > 5.0, "{worst:?}");
    }

    /// (RSF2) Every light level of a member's frame block is his own: with Sora's out scale collapsed to its floor (Sora not drawn:
    /// the FINAL3 log's "rig x100000") and the friend's own at 0.044, the hybrid's k x his tune is 1 (the game's light at its true
    /// level), the fog x k follows, a point light's colour x his tune is its colour x pi; a block with the hybrid off stays off.
    #[test]
    fn rsf2_the_friends_light_levels_are_their_own_whoever_is_drawn() {
        let n = 200 + crate::hybrid::BLOCK_FLOATS;
        let sora_scale = 1.0e-5f32;
        let his = 0.044f32;
        let mut frame = vec![0.0f32; n];
        let hy = n - crate::hybrid::BLOCK_FLOATS;
        frame[hy] = 1.0;
        frame[hy + 12] = 0.18;
        frame[hy + 13] = 1.0 / sora_scale;
        let fog = Vec3::new(0.02, 0.03, 0.04);
        frame[hy + 16..hy + 19].copy_from_slice(&(fog / sora_scale).to_array());
        rescale_block(&mut frame, his);
        assert!((frame[hy + 13] * his - 1.0).abs() < 1.0e-5, "k x tune = 1: {}", frame[hy + 13] * his);
        for (k, f) in fog.to_array().iter().enumerate() {
            assert!((frame[hy + 16 + k] * his - f).abs() < 1.0e-6);
        }
        assert_eq!(frame[hy + 12], 0.18, "the reference white stays");
        // off: untouched
        let mut off = vec![0.0f32; n];
        rescale_block(&mut off, his);
        assert!(off.iter().all(|v| *v == 0.0));
        // a point light: written at his level
        let light = crate::pointlight::SoraLight { position: Vec3::ZERO, color: Vec3::new(2.0, 4.0, 1.0), radius: 300.0, exponent: 2.0 };
        let block = crate::pointlight::sora_block(&[light], his);
        let colour = Vec3::new(block[4 * (1 + crate::pointlight::MAX_ON_SORA)], block[4 * (1 + crate::pointlight::MAX_ON_SORA) + 1], block[4 * (1 + crate::pointlight::MAX_ON_SORA) + 2]);
        assert!((colour * his - light.color * std::f32::consts::PI).length() < 1.0e-4);
        // with Sora's collapsed level (the bug): 4400 x too bright
        let bad = crate::pointlight::sora_block(&[light], sora_scale);
        assert!(bad[4 * (1 + crate::pointlight::MAX_ON_SORA)] * his / (light.color.x * std::f32::consts::PI) > 4000.0);
    }

    /// (ERT4) THE USER'S CASE BY DAY: Goofy leads, Sora out, Donald first in the plan, no hero light (no carrier). The shared
    /// level must be the LEADER's (his feet), not the first member's; with the hero light lit the carrier is the same man.
    #[test]
    fn ert4_the_shared_level_is_the_friend_leaders_without_a_hero_light() {
        let rig = |s: f32| crate::light::RigLight { key_dir: Vec3::Y, key: Vec3::ONE, ambient: Vec3::ONE, ambient_up: Vec3::ZERO, out_scale: s };
        let pm = |x: f32, light: Option<f32>| PlanMember { member: 0, frame_at: 0, early_at: 0, models: Vec::new(), glow_mask: false, caps: Vec::new(), feet: Some(Vec3::new(x, 0.0, 0.0)), frame_offset: 0, early_offset: 0, light: light.map(rig), probe_row: None, velo_own: false, reach: Vec::new(), env_level: None, light_ratio: 1.0 };
        // Donald (first, x1604 in game = his out scale 1/1604) at 0, Goofy (the leader, x640) 300 cm away
        let (donald, goofy) = (1.0 / 1604.0, 1.0 / 640.0);
        let plan = Plan { members: vec![pm(0.0, Some(donald)), pm(300.0, Some(goofy))], ..Default::default() };
        let goofy_feet = Vec3::new(300.0, 0.0, 0.0);
        // before (the bug): no carrier by day -> Donald's
        assert_eq!(leader_level(&plan, crate::party::level_anchor(None, None)), Some(donald));
        // after: the leader's feet, carrier or not
        assert_eq!(leader_level(&plan, crate::party::level_anchor(Some(goofy_feet), None)), Some(goofy), "by day: the leader's own level");
        assert_eq!(leader_level(&plan, crate::party::level_anchor(Some(goofy_feet), Some(goofy_feet))), Some(goofy), "at night: the same man");
        assert_eq!(leader_level(&plan, crate::party::level_anchor(None, Some(goofy_feet))), Some(goofy), "the carrier alone still counts");
        // the leader's feet win over a stale carrier elsewhere
        assert_eq!(leader_level(&plan, crate::party::level_anchor(Some(goofy_feet), Some(Vec3::ZERO))), Some(goofy));
        // (the runtime path set_lead_feet -> level_anchor_now: party.rs's own test, which owns the module switch)
    }

    /// (RSF2) The level our effects are lit at when Sora is not drawn: the leader's (the member at the hero light's carrier), else
    /// the first member with a light of his own; none without one. And Sora's outfit field never reaches a member's hybrid rows.
    #[test]
    fn rsf2_effects_take_the_leaders_level_and_members_never_take_soras_outfit_field() {
        let rig = |s: f32| crate::light::RigLight { key_dir: Vec3::Y, key: Vec3::ONE, ambient: Vec3::ONE, ambient_up: Vec3::ZERO, out_scale: s };
        let pm = |x: f32, light: Option<f32>| PlanMember { member: 0, frame_at: 0, early_at: 0, models: Vec::new(), glow_mask: false, caps: Vec::new(), feet: Some(Vec3::new(x, 0.0, 0.0)), frame_offset: 0, early_offset: 0, light: light.map(rig), probe_row: None, velo_own: false, reach: Vec::new(), env_level: None, light_ratio: 1.0 };
        let plan = Plan { members: vec![pm(0.0, Some(0.03)), pm(500.0, Some(0.07))], ..Default::default() };
        assert_eq!(leader_level(&plan, Some(Vec3::new(480.0, 10.0, 0.0))), Some(0.07), "the carrier's member");
        assert_eq!(leader_level(&plan, None), Some(0.03), "no carrier: the first with a light");
        let none = Plan { members: vec![pm(0.0, None), pm(500.0, None)], ..Default::default() };
        assert_eq!(leader_level(&none, None), None);
        // Sora in an own-set outfit: his block's rig share and lift are the field's; a member's are the plain settings
        let s = crate::hybrid::Settings { on: true, ..Default::default() };
        let n = 100 + crate::hybrid::BLOCK_FLOATS;
        let mut frame = vec![0.0f32; n];
        let hy = n - crate::hybrid::BLOCK_FLOATS;
        (frame[hy], frame[hy + 2], frame[hy + 31]) = (1.0, s.rig_own, 3.5);
        member_hybrid_rows(&mut frame, &s);
        assert_eq!((frame[hy + 2], frame[hy + 31]), (s.rig, s.lift.clamp(0.0, 8.0)));
    }
}
