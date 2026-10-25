//! The renderer outside the game: a window, a Direct3D 12 device and a swap chain of our own, the
//! same hooks, the same worker, Sora in his reference pose in front of a grey "scene". With the
//! debug layer when it is installed, so a wrong root signature or state shows as a message here
//! instead of as a crash in the game. Writes PNG screenshots of the back buffer.
//!
//! ```text
//! cargo run --release -p kh-er-render --example standalone -- [--frames N] [--shot FILE.png] [--weapon] [--options "ccw=1;cull=0;flat=1"]
//! ```
//!
//! The "game" side here: one command list a frame that clears the back buffer and a depth target
//! of its own (reversed Z, cleared to 0) and draws nothing else; its depth target goes through
//! DEPTH_WRITE -> PIXEL_SHADER_RESOURCE each frame as a deferred renderer's would, so the survey
//! finds it.

use std::ffi::c_void;
use std::mem::ManuallyDrop;
use std::path::PathBuf;

use glam::{Quat, Vec3};
use kh_anim::Trs;
use kh_er_render::{Body, Camera, FrameInput, Options, Weapon};
use windows::core::{w, Interface};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;

unsafe extern "system" fn window_proc(window: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    DefWindowProcW(window, message, w, l)
}

fn transition(resource: &ID3D12Resource, before: D3D12_RESOURCE_STATES, after: D3D12_RESOURCE_STATES) -> D3D12_RESOURCE_BARRIER {
    D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
        Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
        Anonymous: D3D12_RESOURCE_BARRIER_0 { Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER { pResource: unsafe { std::mem::transmute_copy(resource) }, Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, StateBefore: before, StateAfter: after }) },
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |name: &str| args.iter().position(|a| a == name).and_then(|k| args.get(k + 1)).cloned();
    let frames: u32 = value("--frames").and_then(|v| v.parse().ok()).unwrap_or(240);
    let shot = value("--shot").map(PathBuf::from);
    let with_weapon = args.iter().any(|a| a == "--weapon");
    let pack = value("--pack").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../build/sora_pack")));
    let distance: f32 = value("--distance").and_then(|v| v.parse().ok()).unwrap_or(2.6);
    let height: f32 = value("--height").and_then(|v| v.parse().ok()).unwrap_or(0.85);
    // (IMPL_ER59) `--look-height M`: the point the camera looks at (default 0.85 m)
    let look_height: f32 = value("--look-height").and_then(|v| v.parse().ok()).unwrap_or(0.85);
    let yaw: f32 = value("--yaw").and_then(|v| v.parse().ok()).unwrap_or(0.0);
    // (this harness has no deferred stage: the back-buffer arrangement)
    let mut options = Options { at: kh_er_render::At::Composite, ..Options::default() };
    for part in value("--options").unwrap_or_default().split(';') {
        let Some((k, v)) = part.split_once('=') else { continue };
        let on = v == "1";
        match k.trim() {
            "ccw" => options.front_ccw = on,
            "cull" => options.cull = on,
            "flat" => options.flat = v.parse().unwrap_or(0),
            "depth" => options.depth = v.parse().unwrap_or(1),
            "debug" => options.debug = v.parse().unwrap_or(0.0),
            "khs" => options.khs = v.to_string(),
            "only" => options.only = v.parse().unwrap_or(-1),
            "survey" => options.survey = v.parse().unwrap_or(0),
            "exposure" => options.exposure = v.parse().unwrap_or(1.0),
            "ghost" => options.ghost = v.parse().unwrap_or(1.0),
            "shadow" => options.shadow = v.parse().unwrap_or(0.0),
            "present" => options.at_present = on,
            "lag" => options.lag = v.parse().unwrap_or(0),
            "poselag" => options.pose_lag = v.parse().unwrap_or(0),
            other => println!("unknown option {other}"),
        }
    }

    // SAFETY: plain Win32 / Direct3D 12 object creation and use on one thread.
    unsafe {
        let mut debug: Option<ID3D12Debug> = None;
        let debug_layer = D3D12GetDebugInterface(&mut debug).is_ok() && debug.is_some() && !args.iter().any(|a| a == "--no-debug");
        if debug_layer {
            debug.as_ref().unwrap().EnableDebugLayer();
        }
        println!("standalone: debug layer {}", if debug_layer { "ON" } else { "off (not installed or --no-debug)" });
        let instance = GetModuleHandleW(None).unwrap();
        let class = WNDCLASSW { lpfnWndProc: Some(window_proc), hInstance: instance.into(), lpszClassName: w!("kh-er-render-standalone"), ..Default::default() };
        RegisterClassW(&class);
        let window = CreateWindowExW(WINDOW_EX_STYLE(0), w!("kh-er-render-standalone"), w!("kh-er-render standalone"), WS_OVERLAPPEDWINDOW | WS_VISIBLE, 60, 60, WIDTH as i32 + 16, HEIGHT as i32 + 39, None, None, Some(instance.into()), None).unwrap();
        let factory: IDXGIFactory2 = CreateDXGIFactory1().unwrap();
        let mut device: Option<ID3D12Device> = None;
        D3D12CreateDevice(None, D3D_FEATURE_LEVEL_11_0, &mut device).unwrap();
        let device = device.unwrap();
        let queue: ID3D12CommandQueue = device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC { Type: D3D12_COMMAND_LIST_TYPE_DIRECT, ..Default::default() }).unwrap();
        let desc = DXGI_SWAP_CHAIN_DESC1 { Width: WIDTH, Height: HEIGHT, Format: DXGI_FORMAT_R8G8B8A8_UNORM, SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 }, BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT, BufferCount: 3, SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL, ..Default::default() };
        let swap: IDXGISwapChain3 = factory.CreateSwapChainForHwnd(&queue, window, &desc, None, None).unwrap().cast().unwrap();
        let allocator: ID3D12CommandAllocator = device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT).unwrap();
        let list: ID3D12GraphicsCommandList = device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator, None).unwrap();
        list.Close().unwrap();
        let fence: ID3D12Fence = device.CreateFence(0, D3D12_FENCE_FLAG_NONE).unwrap();
        let mut fence_value = 0u64;
        let rtv_heap: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV, NumDescriptors: 1, ..Default::default() }).unwrap();
        let dsv_heap: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_DSV, NumDescriptors: 1, ..Default::default() }).unwrap();
        // The "scene depth": the format Elden Ring's has (R32G8X24 typeless, D32_FLOAT_S8X24 view).
        let depth_desc = D3D12_RESOURCE_DESC { Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D, Width: WIDTH as u64, Height: HEIGHT, DepthOrArraySize: 1, MipLevels: 1, Format: DXGI_FORMAT_R32G8X24_TYPELESS, SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 }, Flags: D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL, ..Default::default() };
        let clear = D3D12_CLEAR_VALUE { Format: DXGI_FORMAT_D32_FLOAT_S8X24_UINT, Anonymous: D3D12_CLEAR_VALUE_0 { DepthStencil: D3D12_DEPTH_STENCIL_VALUE { Depth: 0.0, Stencil: 0 } } };
        let mut depth: Option<ID3D12Resource> = None;
        device.CreateCommittedResource(&D3D12_HEAP_PROPERTIES { Type: D3D12_HEAP_TYPE_DEFAULT, ..Default::default() }, D3D12_HEAP_FLAG_NONE, &depth_desc, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, Some(&clear), &mut depth).unwrap();
        let depth = depth.unwrap();
        let depth_view_desc = D3D12_DEPTH_STENCIL_VIEW_DESC { Format: DXGI_FORMAT_D32_FLOAT_S8X24_UINT, ViewDimension: D3D12_DSV_DIMENSION_TEXTURE2D, ..Default::default() };
        device.CreateDepthStencilView(&depth, Some(&depth_view_desc), dsv_heap.GetCPUDescriptorHandleForHeapStart());
        // A read-back buffer for the screenshot.
        let mut footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default();
        let mut total = 0u64;
        let back_desc = swap.GetBuffer::<ID3D12Resource>(0).unwrap().GetDesc();
        device.GetCopyableFootprints(&back_desc, 0, 1, 0, Some(&mut footprint), None, None, Some(&mut total));
        let mut readback: Option<ID3D12Resource> = None;
        let buffer_desc = D3D12_RESOURCE_DESC { Dimension: D3D12_RESOURCE_DIMENSION_BUFFER, Width: total, Height: 1, DepthOrArraySize: 1, MipLevels: 1, SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 }, Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR, ..Default::default() };
        device.CreateCommittedResource(&D3D12_HEAP_PROPERTIES { Type: D3D12_HEAP_TYPE_READBACK, ..Default::default() }, D3D12_HEAP_FLAG_NONE, &buffer_desc, D3D12_RESOURCE_STATE_COPY_DEST, None, &mut readback).unwrap();
        let readback = readback.unwrap();

        kh_er_render::install(&pack, |text| println!("{text}"));
        kh_er_render::set_options(options);

        // Sora at the origin in his reference pose, the camera in front of him.
        let manifest = std::fs::read_to_string(pack.join("manifest.json")).expect("the pack's manifest.json");
        let skeleton = kh_anim::Skeleton::from_manifest(&kh_anim::json::Json::parse(&manifest).unwrap()).unwrap();
        // (IMPL_ER59) `--clip anims/0011.glb --clip-time 0.3`: a frame of one of his clips instead of the reference pose;
        // (ER69) `--clip-play`: the clip plays on from that time (looping), frame by frame
        let clip = value("--clip").map(|file| kh_anim::pack::Clip::load(&pack.join(&file), &file).expect("the clip"));
        let clip_time: f32 = value("--clip-time").and_then(|v| v.parse().ok()).unwrap_or(0.0);
        let clip_len: f32 = clip.as_ref().map_or(0.0, |c| c.tracks.iter().flat_map(|t| t.rotation.times.last().copied().into_iter().chain(t.translation.times.last().copied())).fold(0.0, f32::max));
        let no_root = args.iter().any(|a| a == "--no-root");
        let pose_at = |time: f32| -> (Vec<Trs>, Vec<Trs>) {
            let mut local = skeleton.reference_pose();
            if let Some(clip) = &clip {
                for track in &clip.tracks {
                    let Some(bone) = skeleton.index_of(&track.bone) else { continue };
                    if let Some((a, b, f)) = track.translation.around(time) {
                        local[bone].translation = a.lerp(b, f);
                    }
                    if let Some((a, b, f)) = track.rotation.around(time) {
                        local[bone].rotation = a.slerp(b, f).normalize();
                    }
                    if let Some((a, b, f)) = track.scale.around(time) {
                        local[bone].scale = a.lerp(b, f);
                    }
                }
            }
            // (IMPL_ER59) `--no-root`: the root bones' own translation taken out (the game moves the component by it instead)
            if no_root {
                for name in ["kh_sk", "trans"] {
                    if let Some(k) = skeleton.index_of(name) {
                        local[k].translation = skeleton.bones[k].reference.translation;
                    }
                }
            }
            let mut component: Vec<Trs> = Vec::new();
            for (k, bone) in skeleton.bones.iter().enumerate() {
                let t = match bone.parent {
                    Some(parent) => local[k].then(&component[parent]),
                    None => local[k],
                };
                component.push(t);
            }
            (local, component)
        };
        let play = args.iter().any(|a| a == "--clip-play");
        let (mut local, mut component) = pose_at(clip_time);
        // (ER69) `--form-style 0`: KH1 Sora's plain Second Form recolour
        if let Some(v) = value("--form-style").and_then(|v| v.parse::<u8>().ok()) {
            kh_er_render::set_form_style(v);
        }
        // (IMPL_ER59) `--outfit kh2` from the first frame, `--outfits "kh2:120,kh3:400"` from given frames
        if let Some(id) = value("--outfit") {
            kh_er_render::set_outfit(&id);
        }
        let hand = skeleton.index_of("R_buki");
        let wait = |fence_value: &mut u64| {
            *fence_value += 1;
            queue.Signal(&fence, *fence_value).unwrap();
            while fence.GetCompletedValue() < *fence_value {
                std::thread::sleep(std::time::Duration::from_micros(200));
            }
        };
        let started = std::time::Instant::now();
        for frame in 0..frames {
            let mut message = MSG::default();
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            // ---- the host's frame
            // (IMPL_ER23B) `--wear "5:120,0:300"`: the wear form (5 = Second Form) from a frame on: the costume change's proof
            for (id, at) in value("--outfits").unwrap_or_default().split(',').filter_map(|p| p.split_once(':').and_then(|(f, a)| Some((f.trim().to_string(), a.trim().parse::<u32>().ok()?)))) {
                if at == frame {
                    kh_er_render::set_outfit(&id);
                }
            }
            for (form, at) in value("--wear").unwrap_or_default().split(',').filter_map(|p| p.split_once(':').and_then(|(f, a)| Some((f.trim().parse::<u8>().ok()?, a.trim().parse::<u32>().ok()?)))) {
                if at == frame {
                    kh_er_render::set_wear_form(form);
                }
            }
            if play && clip_len > 0.0 {
                (local, component) = pose_at((clip_time + frame as f32 / 60.0) % clip_len);
            }
            let turn = Quat::from_rotation_z((yaw - 90.0).to_radians());
            let to_world = Trs { translation: Vec3::ZERO, rotation: turn, scale: Vec3::ONE };
            // Sora faces game +X = host +Z at yaw 0; the camera stands in front of him, looking back.
            let eye = Vec3::new(0.0, height, distance);
            let forward = (Vec3::new(0.0, look_height, 0.0) - eye).normalize();
            let right = Vec3::Y.cross(forward).normalize();
            let up = forward.cross(right);
            let socket = Trs { translation: Vec3::ZERO, rotation: Quat::from_xyzw(std::f32::consts::FRAC_1_SQRT_2, 0.0, 0.0, std::f32::consts::FRAC_1_SQRT_2), scale: Vec3::ONE };
            kh_er_render::submit(FrameInput {
                camera: Camera { right, up, forward, position: eye, fov: 0.8378, aspect: WIDTH as f32 / HEIGHT as f32, near: 0.05, far: 10000.0 },
                origin: Vec3::ZERO,
                clock: frame as f32 / 60.0,
                body: Some(Body { component_to_world: to_world, local: local.clone(), component: component.clone() }),
                weapon: hand.filter(|_| with_weapon).map(|hand| Weapon { to_world: socket.then(&component[hand]).then(&to_world), component: Vec::new(), opacity: 1.0 }),
                forms: Vec::new(),
            });
            // ---- the "game": clear colour and depth
            let index = swap.GetCurrentBackBufferIndex();
            let back: ID3D12Resource = swap.GetBuffer(index).unwrap();
            device.CreateRenderTargetView(&back, None, rtv_heap.GetCPUDescriptorHandleForHeapStart());
            allocator.Reset().unwrap();
            list.Reset(&allocator, None).unwrap();
            list.ResourceBarrier(&[transition(&back, D3D12_RESOURCE_STATE_PRESENT, D3D12_RESOURCE_STATE_RENDER_TARGET), transition(&depth, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, D3D12_RESOURCE_STATE_DEPTH_WRITE)]);
            list.ClearRenderTargetView(rtv_heap.GetCPUDescriptorHandleForHeapStart(), &[0.35, 0.37, 0.40, 1.0], None);
            list.ClearDepthStencilView(dsv_heap.GetCPUDescriptorHandleForHeapStart(), D3D12_CLEAR_FLAG_DEPTH | D3D12_CLEAR_FLAG_STENCIL, 0.0, 0, None);
            list.ResourceBarrier(&[transition(&back, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_PRESENT), transition(&depth, D3D12_RESOURCE_STATE_DEPTH_WRITE, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE)]);
            list.Close().unwrap();
            queue.ExecuteCommandLists(&[Some(list.cast().unwrap())]);
            if let Err(e) = swap.Present(1, DXGI_PRESENT(0)).ok() {
                println!("standalone: Present failed: {e}");
                break;
            }
            wait(&mut fence_value);
            if frame % 60 == 59 {
                println!("standalone: frame {} ({:.1} s): {}", frame + 1, started.elapsed().as_secs_f32(), kh_er_render::status());
            }
            if frame + 1 == frames {
                if let Some(path) = &shot {
                    // The buffer just presented keeps its picture (sequential flip): read it back.
                    allocator.Reset().unwrap();
                    list.Reset(&allocator, None).unwrap();
                    list.ResourceBarrier(&[transition(&back, D3D12_RESOURCE_STATE_PRESENT, D3D12_RESOURCE_STATE_COPY_SOURCE)]);
                    let from = D3D12_TEXTURE_COPY_LOCATION { pResource: std::mem::transmute_copy(&back), Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX, Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { SubresourceIndex: 0 } };
                    let to = D3D12_TEXTURE_COPY_LOCATION { pResource: std::mem::transmute_copy(&readback), Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT, Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { PlacedFootprint: footprint } };
                    list.CopyTextureRegion(&to, 0, 0, 0, &from, None);
                    list.ResourceBarrier(&[transition(&back, D3D12_RESOURCE_STATE_COPY_SOURCE, D3D12_RESOURCE_STATE_PRESENT)]);
                    list.Close().unwrap();
                    queue.ExecuteCommandLists(&[Some(list.cast().unwrap())]);
                    wait(&mut fence_value);
                    let mut at: *mut c_void = std::ptr::null_mut();
                    readback.Map(0, None, Some(&mut at)).unwrap();
                    let mut pixels = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);
                    for y in 0..HEIGHT as usize {
                        pixels.extend_from_slice(std::slice::from_raw_parts((at as *const u8).add(footprint.Offset as usize + y * footprint.Footprint.RowPitch as usize), WIDTH as usize * 4));
                    }
                    readback.Unmap(0, None);
                    let file = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
                    let mut encoder = png::Encoder::new(file, WIDTH, HEIGHT);
                    encoder.set_color(png::ColorType::Rgba);
                    encoder.set_depth(png::BitDepth::Eight);
                    encoder.write_header().unwrap().write_image_data(&pixels).unwrap();
                    println!("standalone: wrote {}", path.display());
                }
            }
        }
        // What the debug layer had to say.
        if let Ok(info) = device.cast::<ID3D12InfoQueue>() {
            let count = info.GetNumStoredMessages();
            println!("standalone: {count} debug-layer message(s)");
            for k in 0..count.min(40) {
                let mut size = 0usize;
                let _ = info.GetMessage(k, None, &mut size);
                let mut buffer = vec![0u8; size.max(1)];
                if info.GetMessage(k, Some(buffer.as_mut_ptr() as *mut D3D12_MESSAGE), &mut size).is_ok() {
                    let message = &*(buffer.as_ptr() as *const D3D12_MESSAGE);
                    let text = std::slice::from_raw_parts(message.pDescription, message.DescriptionByteLength.saturating_sub(1));
                    println!("  [{}] {}", message.Severity.0, String::from_utf8_lossy(text));
                }
            }
        }
        println!("standalone: {}", kh_er_render::outfit::sim_report());
        kh_er_render::shutdown();
        println!("standalone: done");
    }
}
