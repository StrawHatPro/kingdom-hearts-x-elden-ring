//! (IMPL_ER21) THE CAMP MENU'S PORTRAIT: the posed Sora drawn into a colour + depth target of OUR OWN at Present, for
//! the 2D overlay to lay into the camp menu's character panel (hud.rs `set_picture`): the test app's menu model
//! (kh-app/src/menu_model.rs) in this renderer, by the void's approach (IMPL_ER15): nothing of the game's frame is
//! read or written.
//!
//! - THE TARGET is as large as the part of the picture that shows on screen (the host says where: `PortraitFrame::
//!   visible`, in back buffer pixels); the whole capture is mapped through the viewport, which sticks out of the
//!   target where the panel's mask cuts the picture. So one texel is one pixel of the screen, as every picture of
//!   the overlay. R8G8B8A8_UNORM, display-encoded, cleared to the panel's dark blue with alpha 0; his pixels carry
//!   alpha (the masked materials' dither coverage in quarters, as at Present).
//! - THE PIPELINES are the character's own with one difference: alpha is written (a `PsoKey` of its own, made by
//!   the worker like every other set).
//! - OWN LISTS, upload buffers and views (three slots); the frame constants are laid out exactly as `Core::draw`
//!   lays them out (the shaders are the same), with the game's light, shadows and probes off.
//! - IT COSTS NOTHING WHILE NO PORTRAIT IS HANDED OVER: `Core::portrait_at_present` returns at its first line; the
//!   objects are made at the first picture.

use std::ffi::c_void;
use std::sync::Arc;
use std::time::Instant;

use glam::{Mat4, Vec4};
use kh_look::library::ObjectRows;
use windows::core::Interface;
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use super::{buffer_desc, committed, texture_desc, transition, Core, PsoKey, MAX_JOINTS, MAX_JOINTS_OUTFIT, SLOT_TEXTURES, UPLOAD_BYTES};
use crate::{math, rlog, PortraitFrame};

const SLOTS: usize = 3;
const FORMAT: DXGI_FORMAT = DXGI_FORMAT_R8G8B8A8_UNORM;

struct Slot {
    allocator: ID3D12CommandAllocator,
    list: ID3D12GraphicsCommandList,
    upload: ID3D12Resource,
    mapped: *mut u8,
    busy_until: u64,
}

pub(super) struct Kit {
    slots: Vec<Slot>,
    rtv: ID3D12DescriptorHeap,
    dsv: ID3D12DescriptorHeap,
    /// Colour and depth, and their size (they only grow, in steps of 64).
    target: Option<(ID3D12Resource, ID3D12Resource, u32, u32)>,
    khs: Option<(String, kh_look::KhsState)>,
    told: bool,
    pub drawn: u32,
    pub micros: u64,
}

impl Kit {
    unsafe fn new(device: &ID3D12Device) -> windows::core::Result<Kit> {
        let mut slots = Vec::new();
        for _ in 0..SLOTS {
            let allocator: ID3D12CommandAllocator = device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
            let list: ID3D12GraphicsCommandList = device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator, None)?;
            list.Close()?;
            let upload = committed(device, D3D12_HEAP_TYPE_UPLOAD, &buffer_desc(UPLOAD_BYTES as u64), D3D12_RESOURCE_STATE_GENERIC_READ, None)?;
            let mut at: *mut c_void = std::ptr::null_mut();
            upload.Map(0, None, Some(&mut at))?;
            slots.push(Slot { allocator, list, upload, mapped: at as *mut u8, busy_until: 0 });
        }
        let rtv: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV, NumDescriptors: 1, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE, NodeMask: 0 })?;
        let dsv: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_DSV, NumDescriptors: 1, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE, NodeMask: 0 })?;
        Ok(Kit { slots, rtv, dsv, target: None, khs: None, told: false, drawn: 0, micros: 0 })
    }

    /// The upload buffers are unmapped (the kit goes away with the core).
    pub(super) unsafe fn unmap(&self) {
        for slot in &self.slots {
            slot.upload.Unmap(0, None);
        }
    }
}

/// (IMPL_ER34) THE SIZE OF A NEW TARGET for a visible part of `width` x `height`: a quarter more on each side, in
/// steps of 64. The panel grows while the menu's animations run (the user's log: three new targets in three
/// consecutive frames of one close animation, 384x960 -> 448x960 -> 448x1024, each a pair of committed resources made
/// on the presenting thread): with the headroom one target serves a whole animation.
pub(crate) fn target_size(width: u32, height: u32) -> (u32, u32) {
    let grow = |v: u32| (v + v / 4).next_multiple_of(64).clamp(64, 8192);
    (grow(width), grow(height))
}

#[cfg(test)]
mod tests {
    use super::target_size;

    #[test]
    fn a_new_target_has_room_for_the_panels_animation() {
        // the user's session at 2560 x 1440: the first picture 366x868, the close animation up to 412x977
        let (w, h) = target_size(366, 868);
        assert!(w >= 412 && h >= 977, "{w}x{h}");
        assert_eq!((w % 64, h % 64), (0, 0));
        // never smaller than what is asked, never beyond a texture's limit
        for (a, b) in [(2, 2), (528, 1095), (8000, 8192)] {
            let (w, h) = target_size(a, b);
            assert!(w >= a.min(8192) && h >= b.min(8192) && w <= 8192 && h <= 8192, "{a}x{b} -> {w}x{h}");
        }
    }
}

impl Core {
    /// AT PRESENT, before the 2D overlay: the portrait of this frame, when the host handed one over.
    pub(super) fn portrait_at_present(&mut self) {
        let Some(frame) = crate::portrait_now() else { return };
        if self.portrait_failed.is_some_and(|at| at.elapsed().as_secs() < 5) {
            return;
        }
        // SAFETY: our own objects on the game's device and queue, on the presenting thread (as `draw`).
        match unsafe { self.draw_portrait(&frame) } {
            Ok(()) => self.portrait_failed = None,
            Err(e) => {
                rlog!("render: the camp menu's portrait could not be drawn ({e}): tried again in 5 s");
                self.portrait_failed = Some(Instant::now());
                crate::hud::set_picture(None);
            }
        }
    }

    unsafe fn draw_portrait(&mut self, frame: &Arc<PortraitFrame>) -> windows::core::Result<()> {
        let started = Instant::now();
        let (width, height) = (frame.visible.2.clamp(2, 8192), frame.visible.3.clamp(2, 8192));
        let options = match crate::SHARED.lock().ok().and_then(|g| g.as_ref().map(|s| s.options.clone())) {
            Some(options) => options,
            None => return Ok(()),
        };
        if self.assets.is_none() {
            return Ok(());
        }
        let key = PsoKey { target: FORMAT.0, depth: DXGI_FORMAT_D32_FLOAT.0, front_ccw: options.front_ccw, cull: options.cull, alpha: true };
        if !self.pso_sets.contains_key(&key) {
            // (the worker makes the set; the scene's own set is asked for by `draw` every frame and comes first)
            self.wanted_more = Some(key);
            return Ok(());
        }
        self.device.GetDeviceRemovedReason()?;
        if self.portrait.is_none() {
            self.portrait = Some(Kit::new(&self.device)?);
        }
        let completed = self.fence.GetCompletedValue();
        self.retired.retain(|(value, _)| *value > completed);
        let Core { device, queue, root, sampler_heap, sampler_index, fence, fence_value, assets, pso_sets, portrait, retired, glow, .. } = self;
        let (assets, kit, pipelines) = (assets.as_mut().unwrap(), portrait.as_mut().unwrap(), &pso_sets[&key]);
        let Some(slot_index) = (0..kit.slots.len()).find(|k| kit.slots[*k].busy_until <= completed) else { return Ok(()) };
        // ---- the target: at least the visible part's size
        if !kit.target.as_ref().is_some_and(|(_, _, w, h)| *w >= width && *h >= height) {
            let (w, h) = target_size(width, height);
            let clear = D3D12_CLEAR_VALUE { Format: FORMAT, Anonymous: D3D12_CLEAR_VALUE_0 { Color: frame.clear } };
            let colour = committed(device, D3D12_HEAP_TYPE_DEFAULT, &texture_desc(w, h, 1, 1, FORMAT, D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET), D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, Some(&clear))?;
            let clear = D3D12_CLEAR_VALUE { Format: DXGI_FORMAT_D32_FLOAT, Anonymous: D3D12_CLEAR_VALUE_0 { DepthStencil: D3D12_DEPTH_STENCIL_VALUE { Depth: 0.0, Stencil: 0 } } };
            let depth = committed(device, D3D12_HEAP_TYPE_DEFAULT, &texture_desc(w, h, 1, 1, DXGI_FORMAT_D32_FLOAT, D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL), D3D12_RESOURCE_STATE_DEPTH_WRITE, Some(&clear))?;
            // (the overlay may still hold the old picture: it is told before the old texture is retired)
            crate::hud::set_picture(None);
            if let Some((old_colour, old_depth, ..)) = kit.target.replace((colour, depth, w, h)) {
                retired.push((*fence_value, old_colour));
                retired.push((*fence_value, old_depth));
            }
            let (colour, depth, ..) = kit.target.as_ref().unwrap();
            device.CreateRenderTargetView(colour, None, kit.rtv.GetCPUDescriptorHandleForHeapStart());
            device.CreateDepthStencilView(depth, None, kit.dsv.GetCPUDescriptorHandleForHeapStart());
            rlog!("render: the camp menu's portrait has its own target {w}x{h} (colour {} + depth; the picture's visible part is {width}x{height})", FORMAT.0);
        }
        let (colour, _, target_w, target_h) = kit.target.as_ref().map(|(c, d, w, h)| (c.clone(), d.clone(), *w, *h)).unwrap();
        let slot = &mut kit.slots[slot_index];

        // ---- the frame's constants: THE LAYOUT OF `Core::draw` (the same shaders read them)
        let input = &frame.input;
        let Some(body) = input.body.as_ref() else { return Ok(()) };
        let camera = input.camera;
        let clip = math::clip_from_game(&camera, input.origin);
        let camera_game = math::game_pos_of_host(camera.position, input.origin);
        let tan_y = (camera.fov * 0.5).tan();
        let (depth_a, depth_b) = math::depth_to_view(camera.near, camera.far);
        let rig = &frame.rig;
        let (picture_w, picture_h) = (frame.picture.2.max(1.0), frame.picture.3.max(1.0));
        let mut constants: Vec<f32> = Vec::with_capacity(16 * 3 + 4 * 18 + 8 * 16);
        constants.extend_from_slice(&clip.to_cols_array());
        for v in [
            camera_game.extend(input.clock),
            math::game_dir_of_host(camera.right).extend(0.0),
            math::game_dir_of_host(camera.up).extend(0.0),
            math::game_dir_of_host(camera.forward).extend(0.0),
            Vec4::new(width as f32, height as f32, 1.0 / width as f32, 1.0 / height as f32),
            // (the rig is lit by its own spots: the key light's row is not used)
            Vec4::new(0.0, 0.0, 1.0, 0.0),
            Vec4::new(1.0, 1.0, 1.0, 0.0),
            Vec4::new(tan_y * camera.aspect, tan_y, camera.near, camera.far),
            Vec4::new(0.0, options.bias, depth_a, depth_b),
            Vec4::new(1.0, 1.0, 1.0, 1.0),
            // (display-encoded output; no premultiplication; the dither coverage becomes alpha)
            Vec4::new(1.0, 0.0, 0.0, 1.0),
            Vec4::ZERO,
            // (no contact shadow, no thrown shadow)
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
        // (IMPL_ER44) no point lights of his own in the portrait (the *_pl shaders read this block: count 0)
        constants.extend_from_slice(&[0.0; crate::pointlight::SORA_BLOCK_FLOATS]);
        // (IMPL_ER60) no blood in the portrait (the *_bl shaders read this block: amounts 0)
        constants.extend_from_slice(&[0.0; crate::blood::BLOCK_FLOATS]);
        // (IMPL_ER78) not lit by the game in the portrait (the *_hy shaders read this block: zeros = today's light)
        constants.extend_from_slice(&[0.0; crate::hybrid::BLOCK_FLOATS]);

        // ---- the materials' state: the menu's post-process preset, the bone-fed parameters of THIS pose
        if kit.khs.as_ref().map(|(name, _)| name.as_str()) != Some(frame.khs.as_str()) {
            let on = frame.khs != "off" && !assets.library.khs.presets.is_empty();
            kit.khs = Some((frame.khs.clone(), assets.library.khs.preset_state(&frame.khs, on)));
        }
        {
            let table = assets.library.bone_table.clone();
            let outfit = assets.models.iter().find_map(|m| m.retarget.as_ref()).map(|r| (r, r.pose_full(&body.local, &body.component)));
            let values = kh_look::bone_parameters(&table, |name| {
                if let Some((r, (locals, comps))) = &outfit {
                    if let Some(k) = r.index_of(name) {
                        let component = *comps.get(k)?;
                        return Some((*locals.get(k)?, component, component.then(&body.component_to_world)));
                    }
                }
                let bone = *assets.bones.get(name)?;
                let component = *body.component.get(bone)?;
                Some((*body.local.get(bone)?, component, component.then(&body.component_to_world)))
            });
            // (the scene's own draw feeds its pose again before it draws: `Core::draw`)
            assets.library.feed("sora", &values);
        }

        let mut cursor = 0usize;
        let upload_address = slot.upload.GetGPUVirtualAddress();
        let mut put = |floats: &[f32]| -> Option<u64> {
            let bytes = floats.len() * 4;
            if cursor + bytes > UPLOAD_BYTES {
                return None;
            }
            std::ptr::copy_nonoverlapping(floats.as_ptr() as *const u8, slot.mapped.add(cursor), bytes);
            let at = upload_address + cursor as u64;
            cursor = (cursor + bytes + 255) & !255;
            Some(at)
        };
        let overflow = || windows::core::Error::from_hresult(windows::Win32::Foundation::E_OUTOFMEMORY);
        let frame_constants = put(&constants).ok_or_else(overflow)?;

        slot.allocator.Reset()?;
        slot.list.Reset(&slot.allocator, None)?;
        let list = &slot.list;
        let (target_view, depth_view) = (kit.rtv.GetCPUDescriptorHandleForHeapStart(), kit.dsv.GetCPUDescriptorHandleForHeapStart());
        list.ResourceBarrier(&[transition(&colour, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, D3D12_RESOURCE_STATE_RENDER_TARGET)]);
        list.OMSetRenderTargets(1, Some(&target_view), false, Some(&depth_view));
        list.ClearRenderTargetView(target_view, &frame.clear, None);
        list.ClearDepthStencilView(depth_view, D3D12_CLEAR_FLAG_DEPTH, 0.0, 0, None);
        // The WHOLE capture through the viewport; the target is the part of it that shows.
        list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: frame.picture.0 - frame.visible.0 as f32, TopLeftY: frame.picture.1 - frame.visible.1 as f32, Width: picture_w, Height: picture_h, MinDepth: 0.0, MaxDepth: 1.0 }]);
        list.RSSetScissorRects(&[RECT { left: 0, top: 0, right: width.min(target_w) as i32, bottom: height.min(target_h) as i32 }]);
        list.SetGraphicsRootSignature(&*root);
        list.SetDescriptorHeaps(&[Some(assets.srv_heap.clone()), Some(sampler_heap.clone())]);
        list.SetGraphicsRootConstantBufferView(0, frame_constants);
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        list.SetGraphicsRootDescriptorTable(4, sampler_heap.GetGPUDescriptorHandleForHeapStart());
        list.SetGraphicsRootShaderResourceView(5, sampler_index.GetGPUVirtualAddress());
        let srv_start = assets.srv_heap.GetGPUDescriptorHandleForHeapStart();
        let sh = rig.sh();
        let mut drawn = 0u32;
        let mut skin: Vec<f32> = Vec::with_capacity(16 * (1 + MAX_JOINTS_OUTFIT));
        let mut skin_constants: Option<u64> = None;
        // (ER87) the hair fringe after his draws, as in the scene (hair.rs)
        let hair_set: Option<Vec<Option<ID3D12PipelineState>>> = glow.hair_sets.get(&(key, false)).filter(|s| crate::hair::on() && s.len() == assets.library.slots.len()).cloned();
        let mut hair_draws: Vec<crate::hair::HairDraw> = Vec::new();
        for translucent in [false, true] {
            for (model_index, model) in assets.models.iter().enumerate().filter(|(_, m)| m.mesh == "sora") {
                if !model.draws.iter().any(|d| assets.library.slots[d.slot].translucent == translucent) {
                    continue;
                }
                let skin_at = match skin_constants {
                    Some(at) => at,
                    None => {
                        skin.clear();
                        skin.extend_from_slice(&math::game_from_pack(&body.component_to_world).to_cols_array());
                        // (IMPL_ER59) an outfit: its own skeleton, posed from his frame
                        let outfit_pose = model.retarget.as_ref().map(|r| r.pose(&body.local, &body.component));
                        let pose = outfit_pose.as_deref().unwrap_or(&body.component);
                        for (joint, bone) in model.joint_bones.iter().enumerate() {
                            let matrix = match bone.and_then(|b| pose.get(b)) {
                                Some(t) => math::joint_in_pack(t) * model.inverse_bind[joint],
                                None => Mat4::IDENTITY,
                            };
                            skin.extend_from_slice(&matrix.to_cols_array());
                        }
                        skin.resize(16 * (1 + if model.retarget.is_some() { MAX_JOINTS_OUTFIT } else { MAX_JOINTS }), 0.0);
                        let at = put(&skin).ok_or_else(overflow)?;
                        skin_constants = Some(at);
                        at
                    }
                };
                let object = ObjectRows::new(&body.component_to_world, assets.library.bounds_of(model.mesh));
                list.IASetVertexBuffers(0, Some(&[model.vertex_view]));
                list.IASetIndexBuffer(Some(&model.index_view));
                list.SetGraphicsRootConstantBufferView(2, skin_at);
                // (no motion: the skin of the frame before is this frame's)
                list.SetGraphicsRootConstantBufferView(6, skin_at);
                for draw in &model.draws {
                    let material = &assets.library.slots[draw.slot];
                    if material.translucent != translucent {
                        continue;
                    }
                    let mut params = assets.library.params(material, &object, rig, kit.khs.as_ref().map(|(_, s)| s), input.clock, frame.exposure, 0.0);
                    params.sh = sh;
                    // (the reflection environment: the renderer's one cube at its own level; see lib.rs `PortraitFrame`)
                    params.env.y = frame.env_level;
                    let material_constants = put(params.as_floats()).ok_or_else(overflow)?;
                    list.SetPipelineState(&pipelines[draw.slot]);
                    list.SetGraphicsRootConstantBufferView(1, material_constants);
                    list.SetGraphicsRootDescriptorTable(3, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: srv_start.ptr + (draw.slot * SLOT_TEXTURES * assets.srv_size) as u64 });
                    list.DrawIndexedInstanced(draw.index_count, 1, draw.first_index, draw.base_vertex, 0);
                    drawn += 1;
                    if !translucent && hair_set.as_ref().is_some_and(|s| s.get(draw.slot).is_some_and(Option::is_some)) {
                        hair_draws.push((model_index, draw.slot, draw.index_count, draw.first_index, draw.base_vertex, skin_at, skin_at, material_constants));
                    }
                }
            }
        }
        if let Some(set) = hair_set.as_ref() {
            crate::hair::draw_fringe(list, set, &hair_draws, &|m| assets.models.get(m).map(|model| (model.vertex_view, model.index_view)), srv_start.ptr, (SLOT_TEXTURES * assets.srv_size) as u64);
        }
        list.ResourceBarrier(&[transition(&colour, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE)]);
        list.Close()?;
        queue.ExecuteCommandLists(&[Some(list.cast::<ID3D12CommandList>()?)]);
        *fence_value += 1;
        queue.Signal(&*fence, *fence_value)?;
        slot.busy_until = *fence_value;
        // The overlay lays it into the panel in this very Present (its list follows ours on the queue).
        crate::hud::set_picture(Some(crate::hud::Picture { texture: colour, before: frame.before_layer, with_layer: frame.with_layer, rect: (frame.visible.0, frame.visible.1, width.min(target_w) as usize, height.min(target_h) as usize), alpha: frame.alpha }));
        if !kit.told {
            kit.told = true;
            rlog!("render: THE CAMP MENU'S PORTRAIT IS DRAWN ({drawn} draws into {width}x{height} at ({}, {}) of the back buffer; the whole capture is {:.0}x{:.0}; {} spot light(s); under overlay layer {})", frame.visible.0, frame.visible.1, picture_w, picture_h, rig.spots.len(), frame.before_layer);
        }
        kit.drawn += 1;
        kit.micros += started.elapsed().as_micros() as u64;
        Ok(())
    }
}
