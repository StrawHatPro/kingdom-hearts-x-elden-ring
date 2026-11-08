//! (GPU-VALIDATE) The harnesses' validation switches (src/gpudebug.rs) really capture the debug layer's messages: a deliberate
//! error on a WARP device (works on any Windows machine with the Graphics Tools; skipped with a line where the debug layer is not
//! installed). Its own test binary, because the debug layer is process-wide. build/sora_tmp/gpu_validate/STATE.md.
#![cfg(windows)]

use std::mem::ManuallyDrop;

use kh_er_render::gpudebug::{self, Mode};
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;

fn layer_installed() -> bool {
    let mut debug: Option<ID3D12Debug> = None;
    // SAFETY: a query of the debug interface (no device yet).
    unsafe { D3D12GetDebugInterface(&mut debug) }.is_ok() && debug.is_some()
}

#[test]
fn the_capture_sees_a_deliberate_error_on_warp() {
    if !layer_installed() {
        println!("SKIPPED: the D3D12 debug layer (Windows Graphics Tools) is not installed");
        return;
    }
    let log = std::env::temp_dir().join("kh_gpu_validate_test.log");
    let mode = Mode { layer: true, gbv: false, warp: true, log: log.clone() };
    // SAFETY: plain Direct3D 12 object creation and one deliberately invalid barrier.
    unsafe {
        let (_factory, device) = gpudebug::create_device_with(&mode).expect("a WARP device with the debug layer");
        assert!(gpudebug::active());
        let desc = D3D12_RESOURCE_DESC { Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D, Width: 64, Height: 64, DepthOrArraySize: 1, MipLevels: 1, Format: DXGI_FORMAT_R8G8B8A8_UNORM, SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 }, Flags: D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET, ..Default::default() };
        let mut buffer: Option<ID3D12Resource> = None;
        device.CreateCommittedResource(&D3D12_HEAP_PROPERTIES { Type: D3D12_HEAP_TYPE_DEFAULT, ..Default::default() }, D3D12_HEAP_FLAG_NONE, &desc, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, None, &mut buffer).unwrap();
        let buffer = buffer.unwrap();
        gpudebug::name(&buffer, "kh:test texture");
        let queue: ID3D12CommandQueue = device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC { Type: D3D12_COMMAND_LIST_TYPE_DIRECT, ..Default::default() }).unwrap();
        let allocator: ID3D12CommandAllocator = device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT).unwrap();
        let list: ID3D12GraphicsCommandList = device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator, None).unwrap();
        gpudebug::name(&list, "kh:test list");
        let barrier = |before, after| D3D12_RESOURCE_BARRIER {
            Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
            Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
            Anonymous: D3D12_RESOURCE_BARRIER_0 { Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER { pResource: std::mem::transmute_copy(&buffer), Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, StateBefore: before, StateAfter: after }) },
        };
        // the classic bug NVIDIA tolerates: a transition whose "before" is not the state the texture is in (it is a shader resource)
        list.ResourceBarrier(&[barrier(D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_COPY_SOURCE)]);
        list.Close().unwrap();
        queue.ExecuteCommandLists(&[Some(windows::core::Interface::cast(&list).unwrap())]);
        let fence: ID3D12Fence = device.CreateFence(0, D3D12_FENCE_FLAG_NONE).unwrap();
        queue.Signal(&fence, 1).unwrap();
        while fence.GetCompletedValue() < 1 {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
    let s = gpudebug::finish("test");
    assert!(s.errors + s.corruption > 0, "the deliberate error was not captured: {s:?}");
    assert!(s.distinct_errors.iter().any(|e| e.2.contains("kh:test texture")), "the message names our object: {s:?}");
    let text = std::fs::read_to_string(&log).unwrap();
    assert!(text.contains("# SUMMARY test"), "{text}");
}

/// The effects harness next to this test binary (cargo builds the examples before the tests of `cargo test -p kh-er-render`).
fn harness() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let profile = exe.parent()?.parent()?;
    Some(profile.join("examples").join("effects.exe")).filter(|p| p.exists())
}

/// THE CI-STYLE SCENE: Sora in the Elden Ring frame's arrangement (the hybrid light, the forward stage and its depth copy, the
/// glow chain, the TAA stand-in, the late pass, blood, a combo) drawn by the effects harness on the GPU with the debug layer
/// (`KH_GPU_VALIDATE_GBV=1`: with GPU-based validation too); fails on any error or corruption message that is not the
/// harness's own stand-in game's (its objects are named `harness:*`), and on a device removal. Skipped with a line when the
/// debug layer, the harness binary (`cargo test --test gpu_validate` alone does not build it) or the pack is missing.
#[test]
fn the_elden_ring_frame_has_no_debug_layer_error_of_ours() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let pack = root.join("build/sora_pack");
    let (Some(exe), true, true) = (harness(), layer_installed(), pack.join("manifest.json").exists()) else {
        println!("SKIPPED: needs the effects harness built beside this test, the D3D12 debug layer and build/sora_pack");
        return;
    };
    let log = std::env::temp_dir().join("kh_gpu_validate_scene.log");
    let out = std::env::temp_dir().join("kh_gpu_validate_scene");
    let gbv = std::env::var("KH_GPU_VALIDATE_GBV").is_ok_and(|v| v == "1");
    let run = std::process::Command::new(&exe)
        .current_dir(&root)
        .env("KH_D3D12_DEBUG", if gbv { "1" } else { "layer" })
        .env("KH_D3D12_LOG", &log)
        .env_remove("KH_D3D12_WARP")
        .args(["--no-focus", "--size", "640x360", "--hybrid", "--fxtest", "--glowtest", "--taa", "--late-pass", "--blood", "0.8,0.8,1", "--script", "stop 0.3; tap attack; stop 0.6", "--shots", "40", "--out"])
        .arg(&out)
        .output()
        .expect("the effects harness runs");
    let text = String::from_utf8_lossy(&run.stdout).into_owned();
    assert!(run.status.success(), "the harness failed ({:?}):\n{}\n{}", run.status, text.lines().rev().take(30).collect::<Vec<_>>().join("\n"), String::from_utf8_lossy(&run.stderr));
    let summary = text.lines().find(|l| l.starts_with("gpudebug: effects:")).unwrap_or_else(|| panic!("no validation summary (was the device made through gpudebug?):\n{text}"));
    assert!(!summary.contains("DEVICE REMOVED"), "{summary}");
    let ours: Vec<&str> = text.lines().filter(|l| l.starts_with("gpudebug:   ERROR") && !l.contains("'harness:")).collect();
    assert!(ours.is_empty(), "debug-layer errors of our renderer ({summary}); log {}:\n{}", log.display(), ours.join("\n"));
}
