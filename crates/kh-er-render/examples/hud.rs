//! (hud) THE HUD OUTSIDE THE GAME: the offline proof of the 2D overlay (kh-er-render/src/hud.rs) and of the
//! shared HUD host (kh_host::hud / hud_layers / menu). A window, a Direct3D 12 device and a swap chain of our
//! own; the "game" fills the back buffer (a clear colour, or a PNG: e.g. the test app's picture of the same
//! state with its HUD off) and presents; the renderer's Present hook draws the overlay. Sora is the real core
//! (kh_host::rig) with a training dummy; the HUD is driven exactly as the test app drives it (one fixed step
//! and one stage frame per presented frame, the menus' pause skips the step), through the PAD path.
//!
//! ```text
//! cargo run --release -p kh-er-render --example hud -- --script "stop 1; hurt 100; stop 2; tap start; stop 2"
//!     --shots 170:a.png,290:b.png [--size 2560x1440] [--background app.png] [--clear 0,0,0] [--gamma]
//!     [--dummy 300] [--config-external] [--async] [--frames N]
//! ```
//! Script words: the rig's (`stop N; walk N; run N; turn DEG; jump; tap BTN; press BTN; release BTN; hold BTN N;
//! cast NAME`) and `hurt N`, `recover`, `hud-cmd WORDS`, `die` (HP to 0), instant, as in the test app.
//! Run from the project's root (the sheets are read by relative path).

use std::ffi::c_void;
use std::mem::ManuallyDrop;
use std::path::PathBuf;
use std::time::Instant;

use glam::{Mat4, Vec3};
use kh_core::TargetId;
use kh_host::hud::{Frame, Hud, HudTargets, TargetView, View};
use kh_host::hud_layers::{stage_root, LayerEvent, Layers};
use kh_host::menu::MenuInput;
use kh_host::rig::{Dummy, Rig, Script, STEP};
use windows::core::{w, Interface};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

/// `kh-app`'s loadout reader (the test stock of the camp menu), shared as source.
#[allow(dead_code)]
#[path = "../../kh-host/src/loadout_shared.rs"]
mod loadout;

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

struct RigTargets(Option<Dummy>);

impl HudTargets for RigTargets {
    fn target(&self, id: TargetId) -> Option<TargetView> {
        self.0.filter(|d| d.info.id == id).map(|d| TargetView { center: d.info.center, hp: d.info.hp, hp_max: d.info.hp_max, large: false })
    }
}

fn same(p: kh_core::Vec3) -> kh_core::Vec3 {
    p
}

fn read_png(path: &std::path::Path) -> (u32, u32, Vec<u8>) {
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))));
    let mut reader = decoder.read_info().expect("png header");
    let mut data = vec![0u8; reader.output_buffer_size().expect("png size")];
    let info = reader.next_frame(&mut data).expect("png data");
    let rgba = match info.color_type {
        png::ColorType::Rgba => data[..info.buffer_size()].to_vec(),
        png::ColorType::Rgb => data[..info.buffer_size()].chunks(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        other => panic!("{}: colour type {other:?} is not handled", path.display()),
    };
    (info.width, info.height, rgba)
}

/// (IMPL_ER21) What the renderer needs for the camp menu's portrait, from the shared posed copy (as kh-er/src/portrait.rs).
fn portrait_frame(data: &kh_host::menu_portrait::PortraitData, out: kh_host::menu_portrait::PortraitOut, exposure: f32, env_level: f32) -> kh_er_render::PortraitFrame {
    // GAME (KH3: cm, X forward, Y right, Z up) -> the renderer's host space (m, X right, Y up, Z forward)
    let host = |v: Vec3| Vec3::new(v.y, v.z, v.x);
    kh_er_render::PortraitFrame {
        input: kh_er_render::FrameInput {
            camera: kh_er_render::Camera { right: host(out.right), up: host(out.up), forward: host(out.forward), position: host(out.eye) * 0.01, fov: out.fov_y, aspect: out.aspect, near: 0.3, far: 50.0 },
            origin: Vec3::ZERO,
            clock: out.clock,
            body: Some(kh_er_render::Body { component_to_world: out.component_to_world, local: out.local, component: out.component }),
            weapon: None,
            forms: Vec::new(),
        },
        rig: data.rig.clone(),
        env_level,
        exposure,
        khs: kh_look::KHS_MENU_PRESET.to_string(),
        visible: out.visible,
        picture: out.picture,
        alpha: out.alpha,
        before_layer: 7,
        with_layer: 6,
        clear: kh_host::menu_portrait::CLEAR,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |name: &str| args.iter().position(|a| a == name).and_then(|k| args.get(k + 1)).cloned();
    let flag = |name: &str| args.iter().any(|a| a == name);
    // --compare A.png B.png [TOLERANCE]: how two pictures differ (the proof against the test app's pictures).
    if let Some(k) = args.iter().position(|a| a == "--compare") {
        let (aw, ah, a) = read_png(std::path::Path::new(&args[k + 1]));
        let (bw, bh, b) = read_png(std::path::Path::new(&args[k + 2]));
        assert!((aw, ah) == (bw, bh), "the pictures differ in size");
        let tolerance: i32 = args.get(k + 3).and_then(|v| v.parse().ok()).unwrap_or(2);
        let (mut over, mut any, mut max, mut bbox) = (0u32, 0u32, 0i32, (u32::MAX, u32::MAX, 0u32, 0u32));
        for (i, (pa, pb)) in a.chunks(4).zip(b.chunks(4)).enumerate() {
            let d = (0..3).map(|c| (pa[c] as i32 - pb[c] as i32).abs()).max().unwrap();
            max = max.max(d);
            any += (d > 0) as u32;
            if d > tolerance {
                over += 1;
                let (x, y) = (i as u32 % aw, i as u32 / aw);
                bbox = (bbox.0.min(x), bbox.1.min(y), bbox.2.max(x), bbox.3.max(y));
            }
        }
        println!("compare: {} pixels | {any} differ at all | {over} differ by more than {tolerance} | largest channel difference {max}{}", aw * ah, if over > 0 { format!(" | box of those: x {}..={} y {}..={}", bbox.0, bbox.2, bbox.1, bbox.3) } else { String::new() });
        return;
    }
    let (width, height) = value("--size").and_then(|v| v.split_once('x').and_then(|(w, h)| Some((w.parse::<u32>().ok()?, h.parse::<u32>().ok()?)))).unwrap_or((1280, 720));
    let pack = value("--pack").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../build/sora_pack")));
    let loadout_path = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../sheets/port/sora-loadout-99.json"));
    let clear: Vec<f32> = value("--clear").map(|v| v.split(',').filter_map(|c| c.parse().ok()).collect()).unwrap_or_else(|| vec![0.0, 0.0, 0.0]);
    let background = value("--background").map(PathBuf::from);
    let dummy_cm: f32 = value("--dummy").and_then(|v| v.parse().ok()).unwrap_or(300.0);
    let shots: Vec<(u32, PathBuf)> = value("--shots").unwrap_or_default().split(',').filter_map(|p| p.split_once(':').and_then(|(f, path)| Some((f.trim().parse().ok()?, PathBuf::from(path.trim()))))).collect();

    // ---- the script: the rig's words, with the instant words of this harness taken out by step
    let text = value("--script").unwrap_or_else(|| "stop 2".to_string());
    let (mut pad_words, mut instants): (String, Vec<(usize, String)>) = (String::new(), Vec::new());
    for word in text.split(';').map(str::trim).filter(|w| !w.is_empty()) {
        if ["hurt", "recover", "hud-cmd", "die", "savepoint"].contains(&word.split_whitespace().next().unwrap_or("")) {
            let at = Script::parse(&pad_words).map(|s| s.len()).unwrap_or(0);
            instants.push((at, word.to_string()));
        } else {
            pad_words.push_str(word);
            pad_words.push(';');
        }
    }
    let script = Script::parse(&pad_words).unwrap_or_else(|e| panic!("--script: {e}"));
    let frames: u32 = value("--frames").and_then(|v| v.parse().ok()).unwrap_or(script.len() as u32 + 30);

    // ---- Sora and his HUD, as the test app sets them up
    let mut rig = Rig::new(&pack, &loadout_path, (dummy_cm > 0.0).then(|| Vec3::new(dummy_cm, 0.0, 0.0))).unwrap_or_else(|e| panic!("rig: {e}"));
    let stock = loadout::test_stock(None);
    // (IMPL_ER21) `--exp TOTAL`: Sora's total EXP (his level follows from the game's table): the EXP gauges' proof
    if let Some(exp) = value("--exp").and_then(|v| v.parse::<i32>().ok()) {
        let status = rig.sora.status_mut();
        status.exp = exp;
        status.rebuild();
        println!("hud harness: --exp {exp}: level {}, this level from {}, next at {} (gauge fraction {:.4})", status.level, status.exp_level, status.exp_next, status.exp_fraction());
    }
    kh_core::command::customize::ensure(&mut rig.sora);
    for (item, count) in &stock.items {
        rig.sora.magic_mut().pouch.stock_add(*item, *count);
    }
    let (mut hud, fonts) = Hud::load(&pack.join("ui")).unwrap_or_else(|e| panic!("the UI pack: {e}"));
    hud.stock_equipment(&stock.equipment);
    hud.config_external = flag("--config-external");
    // (IMPL_ER21) `--title-entry`: "Title Screen" in the camp menu and in the save-point menu, as the Elden Ring host has it
    hud.title_entry = flag("--title-entry");
    // (IMPL_ER59) `--outfits`: the camp menu's Outfits entry, from the pack's outfits.json (as kh-er's boc.rs offers it)
    if flag("--outfits") {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../build/sora_pack/outfits/outfits.json")).unwrap_or_default();
        let list: Vec<(String, String)> = serde_json::from_str::<serde_json::Value>(&text).ok().and_then(|j| j["outfits"].as_array().cloned()).unwrap_or_default().iter().filter_map(|o| Some((o["id"].as_str()?.to_string(), o["name"].as_str()?.to_string()))).collect();
        hud.outfit_menu = Some((list, "kh3".to_string()));
    }
    hud.camp_hook = Some(kh_host::menu::pad_camp_hook);
    let mut layers = Layers::new(fonts, !flag("--async") && !flag("--all-async"));
    if flag("--all-async") {
        layers.first_worker_layer = 0;
    }
    // (IMPL_ER21) `--perf`: the pictures' switches as the Elden Ring host sets them (split dirty rectangles, parallel bands)
    if flag("--perf") {
        layers.perf = kh_host::hud_layers::Perf { split_dirty: true, bands: std::thread::available_parallelism().map_or(4, |n| n.get().saturating_sub(2)).clamp(1, 12), band_pixels: 150_000 };
    }
    // (IMPL_ER21) `--portrait`: Sora's model in the camp menu's panel, as the Elden Ring host draws it (`--portrait-exposure X`, `--portrait-env X`)
    let mut portrait = flag("--portrait").then(|| kh_host::menu_portrait::MenuPortrait::load(&pack).unwrap_or_else(|e| panic!("the menu portrait: {e}")));
    let mut portrait_sync = kh_host::menu_portrait::PictureSync::default();
    let portrait_exposure: f32 = value("--portrait-exposure").and_then(|v| v.parse().ok()).unwrap_or(kh_host::menu_portrait::MENU_EXPOSURE);
    let portrait_env: f32 = value("--portrait-env").and_then(|v| v.parse().ok()).unwrap_or(1.0);
    let mut menu_input = MenuInput::new();
    println!("hud harness: {} | {width}x{height} | script {} steps, {} frames", hud.status_line(), script.len(), frames);

    // SAFETY: plain Win32 / Direct3D 12 object creation and use on one thread.
    unsafe {
        let instance = GetModuleHandleW(None).unwrap();
        let class = WNDCLASSW { lpfnWndProc: Some(window_proc), hInstance: instance.into(), lpszClassName: w!("kh-er-render-hud"), ..Default::default() };
        RegisterClassW(&class);
        // (CV1) `--no-focus`: the window never takes the focus (a tool window off-screen, shown without activation: ER88's rule
        // of the effects harness), so a run cannot steal it from a game on the same desktop
        let window = if flag("--no-focus") {
            let w = CreateWindowExW(WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW, w!("kh-er-render-hud"), w!("kh-er-render HUD harness"), WS_POPUP, -32000, -32000, width as i32, height as i32, None, None, Some(instance.into()), None).unwrap();
            let _ = ShowWindow(w, SW_SHOWNOACTIVATE);
            w
        } else {
            CreateWindowExW(WINDOW_EX_STYLE(0), w!("kh-er-render-hud"), w!("kh-er-render HUD harness"), if flag("--hidden") { WS_OVERLAPPEDWINDOW } else { WS_OVERLAPPEDWINDOW | WS_VISIBLE }, 40, 40, 1280 + 16, 720 + 39, None, None, Some(instance.into()), None).unwrap()
        };
        let factory: IDXGIFactory2 = CreateDXGIFactory1().unwrap();
        let mut device: Option<ID3D12Device> = None;
        D3D12CreateDevice(None, D3D_FEATURE_LEVEL_11_0, &mut device).unwrap();
        let device = device.unwrap();
        let queue: ID3D12CommandQueue = device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC { Type: D3D12_COMMAND_LIST_TYPE_DIRECT, ..Default::default() }).unwrap();
        let format = if flag("--ten-bit") { DXGI_FORMAT_R10G10B10A2_UNORM } else { DXGI_FORMAT_R8G8B8A8_UNORM };
        // (the buffers have the asked size whatever the window's: DXGI stretches them into it)
        let desc = DXGI_SWAP_CHAIN_DESC1 { Width: width, Height: height, Format: format, SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 }, BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT, BufferCount: 3, SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL, Scaling: DXGI_SCALING_STRETCH, ..Default::default() };
        let swap: IDXGISwapChain3 = factory.CreateSwapChainForHwnd(&queue, window, &desc, None, None).unwrap().cast().unwrap();
        let allocator: ID3D12CommandAllocator = device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT).unwrap();
        let list: ID3D12GraphicsCommandList = device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator, None).unwrap();
        list.Close().unwrap();
        let fence: ID3D12Fence = device.CreateFence(0, D3D12_FENCE_FLAG_NONE).unwrap();
        let mut fence_value = 0u64;
        let rtv_heap: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV, NumDescriptors: 1, ..Default::default() }).unwrap();
        let back_desc = swap.GetBuffer::<ID3D12Resource>(0).unwrap().GetDesc();
        let mut footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default();
        let mut total = 0u64;
        device.GetCopyableFootprints(&back_desc, 0, 1, 0, Some(&mut footprint), None, None, Some(&mut total));
        let buffer_desc = D3D12_RESOURCE_DESC { Dimension: D3D12_RESOURCE_DIMENSION_BUFFER, Width: total, Height: 1, DepthOrArraySize: 1, MipLevels: 1, SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 }, Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR, ..Default::default() };
        let mut readback: Option<ID3D12Resource> = None;
        device.CreateCommittedResource(&D3D12_HEAP_PROPERTIES { Type: D3D12_HEAP_TYPE_READBACK, ..Default::default() }, D3D12_HEAP_FLAG_NONE, &buffer_desc, D3D12_RESOURCE_STATE_COPY_DEST, None, &mut readback).unwrap();
        let readback = readback.unwrap();
        let wait = |fence_value: &mut u64| {
            *fence_value += 1;
            queue.Signal(&fence, *fence_value).unwrap();
            while fence.GetCompletedValue() < *fence_value {
                std::thread::sleep(std::time::Duration::from_micros(100));
            }
        };
        // The "game's picture" from a PNG: an upload buffer the back buffer is filled from each frame.
        let picture: Option<ID3D12Resource> = background.as_ref().map(|path| {
            let (pw, ph, rgba) = read_png(path);
            assert!((pw, ph) == (width, height) && format == DXGI_FORMAT_R8G8B8A8_UNORM, "--background: the PNG must have the size of the back buffer ({width}x{height}), 8-bit");
            let mut upload: Option<ID3D12Resource> = None;
            device.CreateCommittedResource(&D3D12_HEAP_PROPERTIES { Type: D3D12_HEAP_TYPE_UPLOAD, ..Default::default() }, D3D12_HEAP_FLAG_NONE, &buffer_desc, D3D12_RESOURCE_STATE_GENERIC_READ, None, &mut upload).unwrap();
            let upload = upload.unwrap();
            let mut at: *mut c_void = std::ptr::null_mut();
            upload.Map(0, None, Some(&mut at)).unwrap();
            for y in 0..height as usize {
                std::ptr::copy_nonoverlapping(rgba.as_ptr().add(y * width as usize * 4), (at as *mut u8).add(footprint.Offset as usize + y * footprint.Footprint.RowPitch as usize), width as usize * 4);
            }
            upload.Unmap(0, None);
            upload
        });

        kh_er_render::install(&pack, |text| println!("{text}"));
        kh_er_render::hud::set_options(kh_er_render::hud::HudOptions { on: !flag("--no-hud"), gamma: flag("--gamma"), linear_scale: 1.0, opacity: 1.0 });

        // The camera of the projection pass: behind and above him, looking along his facing (core space).
        let eye = Vec3::new(-450.0, 0.0, 190.0);
        let clip_from_world = Mat4::perspective_lh(55f32.to_radians(), 16.0 / 9.0, 10.0, 100_000.0) * Mat4::look_at_lh(eye, Vec3::new(0.0, 0.0, 110.0), Vec3::Z);
        let mut audio = kh_host::seam::AudioQueue::default();
        let mut effects = kh_host::seam::EffectQueue::default();
        let (mut hud_seconds, mut hud_max, mut raster_seconds, mut raster_max, mut uploads) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0u64);
        let mut output = (0u32, 0u32);
        let mut steps = 0u64;
        let started = Instant::now();
        for frame in 0..frames {
            let mut message = MSG::default();
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            // ---- the host's frame: instant words, the pad, one fixed step unless a menu holds the pause
            let mut words: Vec<String> = Vec::new();
            for (_, word) in instants.iter().filter(|(at, _)| *at == frame as usize) {
                let parts: Vec<&str> = word.split_whitespace().collect();
                match parts[0] {
                    "hurt" => rig.sora.status_request(kh_core::StatusRequest::Damage { amount: parts.get(1).and_then(|v| v.parse().ok()).unwrap_or(30.0), hit: kh_core::status::HitOnSora::default() }),
                    "die" => rig.sora.status_request(kh_core::StatusRequest::Damage { amount: 100_000.0, hit: kh_core::status::HitOnSora::default() }),
                    "recover" => rig.sora.status_request(kh_core::StatusRequest::FullRecover),
                    // (IMPL_ER21) `savepoint`: the save-point menu with the Elden Ring host's four commands
                    "savepoint" => {
                        use kh_ui::savepoint::SavePointCommand as C;
                        if parts.get(1) == Some(&"all") {
                            hud.open_savepoint(&[]);
                            continue;
                        }
                        // (IMPL_ER26) the menu as the mod opens it (kh_host::savepoint_menu): `savepoint` = the default five entries,
                        // `savepoint sit` = with Site of Grace (savepoint.sit = 1), `savepoint old` = the four entries before IMPL_ER26
                        if parts.get(1) == Some(&"old") {
                            hud.open_savepoint_with(&[C::Save, C::WorldMap, C::SelectArea, C::TitleScreen], &[], &[(C::Save, "Save", "Rest and save your game progress."), (C::WorldMap, "Select an Area", "Open the map and travel to another site of grace."), (C::SelectArea, "Site of Grace", "Rest at the site of grace: pass time, sort the chest and more.")]);
                            continue;
                        }
                        let _ = C::Save;
                        hud.open_savepoint_grace(kh_host::savepoint_menu::GraceMenu { sit: parts.get(1) == Some(&"sit"), talk: parts.get(1) == Some(&"talk") /* (IMPL_ER33) `savepoint talk` */, ..kh_host::savepoint_menu::GraceMenu::DEFAULT });
                    }
                    _ => words.push(word["hud-cmd".len()..].trim().to_string()),
                }
            }
            let input = rig.input(&script, frame as usize, 0.0);
            menu_input.step(&input, STEP);
            if !hud.paused() {
                rig.step_with(&input, &mut audio, &mut effects);
                steps += 1;
            }
            audio.0.clear();
            effects.0.clear();
            // ---- the HUD's frame (what the test app's gfx_hud does)
            let size = kh_er_render::hud::output_size().unwrap_or((width, height));
            let resized = size != output;
            output = size;
            let (root, stage) = stage_root(size.0 as f32, size.1 as f32);
            let hud_started = Instant::now();
            let (pressed, nav) = menu_input.take();
            let targets = RigTargets(rig.dummy);
            hud.frame(&mut Frame { snapshot: &rig.ui.snapshot, sora: &rig.sora, events: &rig.out.events, state: rig.out.state, step_dt: STEP, play_seconds: (steps as f32 * STEP) as u32, targets: &targets, to_view: same, view: Some(View { clip_from_world, size: (stage.2, stage.3) }), words: &words, dt: STEP, ticks: 1, pressed, nav });
            hud.apply_actions(&mut rig.sora);
            if hud.take_config_change() {
                use kh_core::config::ConfigOption as O;
                let config = *hud.config();
                let command = &mut rig.sora.command_mut().config;
                command.command_slow = config.get(O::CommandSelection) != 0;
                command.cursor_lock = config.get(O::CursorPosition) != 0;
                command.prioritize_newer = config.get(O::SituationCommands) != 0;
                command.control_type_b = config.control_type_b();
            }
            if hud.take_title_request() {
                println!("hud harness: frame {frame}: THE TITLE SCREEN WAS ASKED FOR (camp menu, confirmed)");
            }
            if let Some(result) = hud.take_savepoint_result() {
                println!("hud harness: frame {frame}: the save-point menu answered {result:?}");
            }
            if hud.take_system_menu() {
                println!("hud harness: frame {frame}: THE HOST'S SYSTEM MENU WAS ASKED FOR (Config entry; the camp menu closes)");
            }
            drop(hud.take_sounds());
            let spent = hud_started.elapsed().as_secs_f64();
            hud_seconds += spent;
            hud_max = hud_max.max(spent);
            let raster_started = Instant::now();
            layers.update(&hud, &root, (size.0 as f32, size.1 as f32), resized, &mut |index, event| match event {
                LayerEvent::Show { rect, rgba, dirty } => {
                    uploads += 1;
                    kh_er_render::hud::show_layer(index, rect, &rgba, dirty)
                }
                LayerEvent::Hide => kh_er_render::hud::hide_layer(index),
            });
            // (IMPL_ER23B) as kh-er/src/portrait.rs: off AT ONCE when the menu's movie is gone or the panel does not show.
            // (IMPL_ER34) AFTER the layers: placed by the menu state whose picture is on screen (--portrait-nosync: by this frame's)
            if let Some(p) = portrait.as_mut() {
                let now = hud.movie("CampMenu").and_then(|movie| kh_host::menu_portrait::placement(p.data.target, movie, &root, size));
                let camp = &layers.layers[6];
                let placed = if hud.movie("CampMenu").is_none() { portrait_sync.clear(); None } else if flag("--portrait-nosync") { now } else { portrait_sync.step(now, camp.sent, camp.taken, camp.busy()) };
                let out = placed.map(|at| p.frame_at(at, STEP));
                if flag("--portrait-trace") {
                    println!("portrait-trace: frame {frame} menu {} | {}{}", hud.movie("CampMenu").is_some(), out.as_ref().map_or("OFF".to_string(), |o| format!("alpha {:.3} visible {:?}", o.alpha, o.visible)), if placed != now { " (held: the menu picture on screen is older)" } else { "" });
                }
                match out {
                    Some(out) => kh_er_render::set_portrait(Some(portrait_frame(&p.data, out, portrait_exposure, portrait_env))),
                    None => kh_er_render::set_portrait(None),
                }
            }
            let spent = raster_started.elapsed().as_secs_f64();
            raster_seconds += spent;
            raster_max = raster_max.max(spent);

            // ---- the "game": its picture into the back buffer, Present
            let index = swap.GetCurrentBackBufferIndex();
            let back: ID3D12Resource = swap.GetBuffer(index).unwrap();
            allocator.Reset().unwrap();
            list.Reset(&allocator, None).unwrap();
            match &picture {
                Some(picture) => {
                    list.ResourceBarrier(&[transition(&back, D3D12_RESOURCE_STATE_PRESENT, D3D12_RESOURCE_STATE_COPY_DEST)]);
                    let from = D3D12_TEXTURE_COPY_LOCATION { pResource: std::mem::transmute_copy(picture), Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT, Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { PlacedFootprint: footprint } };
                    let to = D3D12_TEXTURE_COPY_LOCATION { pResource: std::mem::transmute_copy(&back), Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX, Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { SubresourceIndex: 0 } };
                    list.CopyTextureRegion(&to, 0, 0, 0, &from, None);
                    list.ResourceBarrier(&[transition(&back, D3D12_RESOURCE_STATE_COPY_DEST, D3D12_RESOURCE_STATE_PRESENT)]);
                }
                None => {
                    device.CreateRenderTargetView(&back, None, rtv_heap.GetCPUDescriptorHandleForHeapStart());
                    list.ResourceBarrier(&[transition(&back, D3D12_RESOURCE_STATE_PRESENT, D3D12_RESOURCE_STATE_RENDER_TARGET)]);
                    list.ClearRenderTargetView(rtv_heap.GetCPUDescriptorHandleForHeapStart(), &[clear[0], clear.get(1).copied().unwrap_or(0.0), clear.get(2).copied().unwrap_or(0.0), 1.0], None);
                    list.ResourceBarrier(&[transition(&back, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_PRESENT)]);
                }
            }
            list.Close().unwrap();
            queue.ExecuteCommandLists(&[Some(list.cast().unwrap())]);
            if let Err(e) = swap.Present(u32::from(flag("--vsync")), DXGI_PRESENT(0)).ok() {
                println!("hud harness: Present failed: {e}");
                break;
            }
            wait(&mut fence_value);
            if frame % 60 == 59 {
                println!("hud harness: frame {} ({:.1} s): {}", frame + 1, started.elapsed().as_secs_f32(), kh_er_render::hud::status());
            }
            for (_, path) in shots.iter().filter(|(at, _)| *at == frame) {
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
                let mut pixels = Vec::with_capacity((width * height * 4) as usize);
                for y in 0..height as usize {
                    let row = std::slice::from_raw_parts((at as *const u8).add(footprint.Offset as usize + y * footprint.Footprint.RowPitch as usize), width as usize * 4);
                    if format == DXGI_FORMAT_R10G10B10A2_UNORM {
                        for px in row.chunks(4) {
                            let v = u32::from_le_bytes([px[0], px[1], px[2], px[3]]);
                            pixels.extend_from_slice(&[((v & 1023) >> 2) as u8, ((v >> 10 & 1023) >> 2) as u8, ((v >> 20 & 1023) >> 2) as u8, 255]);
                        }
                    } else {
                        pixels.extend_from_slice(row);
                    }
                }
                readback.Unmap(0, None);
                let file = std::io::BufWriter::new(std::fs::File::create(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())));
                let mut encoder = png::Encoder::new(file, width, height);
                encoder.set_color(png::ColorType::Rgba);
                encoder.set_depth(png::BitDepth::Eight);
                encoder.write_header().unwrap().write_image_data(&pixels).unwrap();
                println!("hud harness: frame {frame}: wrote {} (camp screen {:?}, paused {}, HP {})", path.display(), hud.camp_screen(), hud.paused(), rig.ui.snapshot.gauges.hp);
            }
        }
        let n = frames.max(1) as f64;
        println!("hud harness: {frames} frames in {:.2} s | host: HUD logic {:.3} ms per frame (max {:.2}), pictures {:.3} ms per frame (max {:.2}; {} layer picture(s) handed over) | layers: {}", started.elapsed().as_secs_f32(), hud_seconds * 1000.0 / n, hud_max * 1000.0, raster_seconds * 1000.0 / n, raster_max * 1000.0, uploads, layers.report());
        println!("hud harness: {}", kh_er_render::hud::status());
        kh_er_render::shutdown();
        println!("hud harness: done");
    }
}
