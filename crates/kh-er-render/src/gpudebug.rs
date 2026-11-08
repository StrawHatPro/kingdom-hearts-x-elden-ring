//! (GPU-VALIDATE) The offline harnesses' Direct3D 12 validation: the debug layer, GPU-based validation, synchronized
//! command-queue validation, DRED, every debug message captured into a log, and a device on WARP instead of the GPU.
//!
//! HARNESS-SIDE ONLY. Nothing here runs unless a harness (examples/effects.rs, hud.rs, standalone.rs, the tests) creates its
//! device through [`create_device`]; the DLL in the game never does, so [`active`] stays false there and [`name`] does
//! nothing. The switches, all default off:
//! - `KH_D3D12_DEBUG=1` (or `gbv`): the debug layer + GPU-based validation + synchronized queue validation + DRED;
//!   `KH_D3D12_DEBUG=layer`: the debug layer without GPU-based validation (much faster);
//! - `KH_D3D12_WARP=1`: the device on WARP (Microsoft's software rasterizer: a second, vendor-neutral implementation);
//! - `KH_D3D12_LOG=FILE`: where the messages go (default `kh_d3d12_debug.log` in the working folder).
//!
//! Why (THE USER 2026-11-08: "test if the mod will crash on AMD cards"): NVIDIA's driver tolerates missing barriers, wrong
//! states, uninitialised descriptors and the like that AMD's does not; the debug layer and GPU-based validation report them on
//! any GPU. Every message is logged once per distinct text (addresses masked) with its ID, then only counted; [`finish`]
//! writes the summary and, after a device removal, the DRED breadcrumbs and page fault. build/sora_tmp/gpu_validate/STATE.md.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use windows::core::{Interface, HSTRING, PCSTR};
use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::*;

/// The switches of one harness run (from the environment: [`Mode::from_env`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Mode {
    /// The debug layer (and DRED).
    pub layer: bool,
    /// GPU-based validation and synchronized command-queue validation (needs `layer`).
    pub gbv: bool,
    /// The device on the WARP adapter.
    pub warp: bool,
    /// Where the messages are written.
    pub log: PathBuf,
}

impl Mode {
    /// The mode the variables give: `KH_D3D12_DEBUG` (`1`/`gbv`/`layer`), `KH_D3D12_WARP`, `KH_D3D12_LOG`.
    pub fn from_env() -> Mode {
        let var = |k: &str| std::env::var(k).ok().map(|v| v.trim().to_ascii_lowercase()).unwrap_or_default();
        Mode::parse(&var("KH_D3D12_DEBUG"), &var("KH_D3D12_WARP"), std::env::var("KH_D3D12_LOG").ok())
    }

    /// The mode of these variable values (empty: unset).
    pub fn parse(debug: &str, warp: &str, log: Option<String>) -> Mode {
        let on = |v: &str| !v.is_empty() && v != "0" && v != "off" && v != "false";
        let layer = on(debug);
        Mode { layer, gbv: layer && debug != "layer", warp: on(warp), log: PathBuf::from(log.filter(|l| !l.trim().is_empty()).unwrap_or_else(|| "kh_d3d12_debug.log".to_string())) }
    }

    /// One line for the harness's output.
    pub fn describe(&self) -> String {
        format!("debug layer {}, GPU-based validation {}, adapter {}", if self.layer { "ON" } else { "off" }, if self.gbv { "ON" } else { "off" }, if self.warp { "WARP" } else { "default" })
    }
}

static ACTIVE: AtomicBool = AtomicBool::new(false);

/// True once a harness made its device through [`create_device`] with the debug layer (never in the game).
pub fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

/// One distinct message (its text with the addresses masked).
struct Seen {
    severity: i32,
    id: i32,
    count: u64,
    text: String,
}

#[derive(Default)]
struct Capture {
    file: Option<std::io::BufWriter<std::fs::File>>,
    seen: BTreeMap<String, Seen>,
    queue: Option<ID3D12InfoQueue>,
    callback: bool,
    cookie: u32,
    device: Option<ID3D12Device>,
}

// SAFETY: the info queue and the device are free-threaded COM objects (D3D12's objects may be used from any thread).
unsafe impl Send for Capture {}

static CAPTURE: Mutex<Option<Capture>> = Mutex::new(None);

/// How many times one distinct message is written out in full (after that it is only counted).
const FULL_LINES: u64 = 3;

fn severity_name(s: i32) -> &'static str {
    match D3D12_MESSAGE_SEVERITY(s) {
        D3D12_MESSAGE_SEVERITY_CORRUPTION => "CORRUPTION",
        D3D12_MESSAGE_SEVERITY_ERROR => "ERROR",
        D3D12_MESSAGE_SEVERITY_WARNING => "WARNING",
        D3D12_MESSAGE_SEVERITY_INFO => "INFO",
        _ => "MESSAGE",
    }
}

/// The text with every hexadecimal address and every plain number of 5+ digits masked: the key that groups one message.
pub fn message_key(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let b = text.as_bytes();
    let mut k = 0;
    while k < b.len() {
        if b[k] == b'0' && k + 1 < b.len() && (b[k + 1] == b'x' || b[k + 1] == b'X') {
            let mut j = k + 2;
            while j < b.len() && b[j].is_ascii_hexdigit() {
                j += 1;
            }
            out.push_str("0x#");
            k = j;
            continue;
        }
        if b[k].is_ascii_digit() && (k == 0 || !b[k - 1].is_ascii_alphanumeric()) {
            let mut j = k;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            if j - k >= 5 {
                out.push('#');
                k = j;
                continue;
            }
        }
        out.push(b[k] as char);
        k += 1;
    }
    out
}

fn record(severity: i32, id: i32, text: &str) {
    let Ok(mut guard) = CAPTURE.lock() else { return };
    let Some(c) = guard.as_mut() else { return };
    if severity == D3D12_MESSAGE_SEVERITY_MESSAGE.0 || severity == D3D12_MESSAGE_SEVERITY_INFO.0 {
        // (object creation and live-object chatter: counted, never written in full)
        let key = format!("{severity}|{id}|info");
        c.seen.entry(key).or_insert(Seen { severity, id, count: 0, text: String::new() }).count += 1;
        return;
    }
    let key = format!("{severity}|{id}|{}", message_key(text));
    let entry = c.seen.entry(key).or_insert(Seen { severity, id, count: 0, text: text.to_string() });
    entry.count += 1;
    let first = entry.count == 1;
    if entry.count <= FULL_LINES {
        if let Some(f) = c.file.as_mut() {
            let _ = writeln!(f, "[{}] id={id} {text}", severity_name(severity));
            // (record-time messages arrive on the calling thread: where in OUR code the call was made, once per distinct message)
            if first {
                for frame in our_frames() {
                    let _ = writeln!(f, "    at {frame}");
                }
            }
        }
    }
}

/// The frames of this thread's stack inside our crates (the call that made the message, when it was made on this thread).
fn our_frames() -> Vec<String> {
    let trace = std::backtrace::Backtrace::force_capture().to_string();
    let mut out: Vec<String> = Vec::new();
    let mut last_ours = false;
    for line in trace.lines().map(str::trim) {
        match line.split_once(": ") {
            Some((_, name)) => {
                last_ours = (name.starts_with("kh_") || name.starts_with("effects::") || name.starts_with("hud::") || name.starts_with("standalone::")) && !name.starts_with("kh_er_render::gpudebug");
                if last_ours {
                    out.push(name.to_string());
                }
            }
            None if last_ours && line.starts_with("at ") => {
                if let Some(l) = out.last_mut() {
                    *l = format!("{l} {line}");
                }
                last_ours = false;
            }
            None => {}
        }
        if out.len() >= 8 {
            break;
        }
    }
    out
}

unsafe extern "system" fn on_message(_category: D3D12_MESSAGE_CATEGORY, severity: D3D12_MESSAGE_SEVERITY, id: D3D12_MESSAGE_ID, description: PCSTR, _context: *mut core::ffi::c_void) {
    // SAFETY: the debug layer passes a NUL-terminated string valid for the call.
    let text = if description.is_null() { String::new() } else { String::from_utf8_lossy(unsafe { description.as_bytes() }).into_owned() };
    let _ = std::panic::catch_unwind(|| record(severity.0, id.0, &text));
}

/// Makes the harness's device with the switches of [`Mode::from_env`] (all off: exactly `D3D12CreateDevice(None, 11_0)` with a
/// plain factory, as before). Call before anything else makes a device (the debug layer must be on before the first one).
///
/// # Safety
/// Plain Direct3D 12 / DXGI object creation.
pub unsafe fn create_device() -> windows::core::Result<(IDXGIFactory4, ID3D12Device, Mode)> {
    let mode = Mode::from_env();
    unsafe { create_device_with(&mode) }.map(|(f, d)| (f, d, mode))
}

/// [`create_device`] with a given mode.
///
/// # Safety
/// Plain Direct3D 12 / DXGI object creation.
pub unsafe fn create_device_with(mode: &Mode) -> windows::core::Result<(IDXGIFactory4, ID3D12Device)> {
    if mode.layer {
        let mut debug: Option<ID3D12Debug> = None;
        unsafe { D3D12GetDebugInterface(&mut debug) }?;
        let debug = debug.ok_or_else(windows::core::Error::empty)?;
        unsafe { debug.EnableDebugLayer() };
        if let Ok(d1) = debug.cast::<ID3D12Debug1>() {
            unsafe {
                d1.SetEnableGPUBasedValidation(mode.gbv);
                // (on in both modes: it is what checks every barrier's "before" against the state the queue knows)
                d1.SetEnableSynchronizedCommandQueueValidation(true);
            }
        }
        if let Ok(d5) = debug.cast::<ID3D12Debug5>() {
            // (unnamed objects still say what they are in the messages)
            unsafe { d5.SetEnableAutoName(true) };
        }
        let mut dred: Option<ID3D12DeviceRemovedExtendedDataSettings> = None;
        if unsafe { D3D12GetDebugInterface(&mut dred) }.is_ok() {
            if let Some(dred) = dred {
                unsafe {
                    dred.SetAutoBreadcrumbsEnablement(D3D12_DRED_ENABLEMENT_FORCED_ON);
                    dred.SetPageFaultEnablement(D3D12_DRED_ENABLEMENT_FORCED_ON);
                }
            }
        }
    }
    let flags = if mode.layer { DXGI_CREATE_FACTORY_DEBUG } else { DXGI_CREATE_FACTORY_FLAGS(0) };
    // (the DXGI debug flag needs dxgidebug.dll; without it, a plain factory)
    let factory: IDXGIFactory4 = match unsafe { CreateDXGIFactory2(flags) } {
        Ok(f) => f,
        Err(_) => unsafe { CreateDXGIFactory1() }?,
    };
    let mut device: Option<ID3D12Device> = None;
    if mode.warp {
        let adapter: IDXGIAdapter = unsafe { factory.EnumWarpAdapter() }?;
        unsafe { D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_11_0, &mut device) }?;
    } else {
        unsafe { D3D12CreateDevice(None, D3D_FEATURE_LEVEL_11_0, &mut device) }?;
    }
    let device = device.ok_or_else(windows::core::Error::empty)?;
    if mode.layer {
        unsafe { start_capture(&device, mode) };
    }
    Ok((factory, device))
}

/// The adapter's description (for the harness's first line: proves which device the run is on).
pub fn adapter_name(factory: &IDXGIFactory4, device: &ID3D12Device) -> String {
    // SAFETY: COM queries on objects we hold.
    unsafe {
        let Ok(adapter) = factory.EnumAdapterByLuid::<IDXGIAdapter1>(device.GetAdapterLuid()) else { return "?".to_string() };
        let Ok(desc) = adapter.GetDesc1() else { return "?".to_string() };
        let n = desc.Description.iter().position(|c| *c == 0).unwrap_or(desc.Description.len());
        String::from_utf16_lossy(&desc.Description[..n])
    }
}

unsafe fn start_capture(device: &ID3D12Device, mode: &Mode) {
    let file = std::fs::File::create(&mode.log).ok().map(std::io::BufWriter::new);
    let mut capture = Capture { file, device: Some(device.clone()), ..Default::default() };
    if let Some(f) = capture.file.as_mut() {
        let _ = writeln!(f, "# kh-er-render D3D12 validation log: {}", mode.describe());
    }
    if let Ok(queue) = device.cast::<ID3D12InfoQueue>() {
        unsafe {
            // break on nothing; keep everything
            let _ = queue.SetBreakOnSeverity(D3D12_MESSAGE_SEVERITY_CORRUPTION, false);
            let _ = queue.SetBreakOnSeverity(D3D12_MESSAGE_SEVERITY_ERROR, false);
            let _ = queue.SetBreakOnSeverity(D3D12_MESSAGE_SEVERITY_WARNING, false);
            let _ = queue.PushEmptyStorageFilter();
            let _ = queue.PushEmptyRetrievalFilter();
            // (NOT muted: muting the debug output also silences the message callback)
        }
        if let Ok(q1) = queue.cast::<ID3D12InfoQueue1>() {
            let mut cookie = 0u32;
            capture.callback = unsafe { q1.RegisterMessageCallback(Some(on_message), D3D12_MESSAGE_CALLBACK_FLAG_NONE, std::ptr::null_mut(), &mut cookie) }.is_ok();
            capture.cookie = cookie;
        }
        capture.queue = Some(queue);
    }
    if let Ok(mut g) = CAPTURE.lock() {
        *g = Some(capture);
    }
    ACTIVE.store(true, Ordering::SeqCst);
}

/// Reads the messages stored since the last call (only needed where the info queue has no callback: older layers).
pub fn pump() {
    let queue = match CAPTURE.lock() {
        Ok(g) => match g.as_ref() {
            Some(c) if !c.callback => c.queue.clone(),
            _ => None,
        },
        Err(_) => None,
    };
    let Some(queue) = queue else { return };
    // SAFETY: GetMessage's two-call protocol into a buffer of the size it asks for.
    unsafe {
        let n = queue.GetNumStoredMessagesAllowedByRetrievalFilter();
        for k in 0..n {
            let mut size = 0usize;
            if queue.GetMessage(k, None, &mut size).is_err() || size == 0 {
                continue;
            }
            let mut buffer = vec![0u64; size.div_ceil(8)];
            let message = buffer.as_mut_ptr() as *mut D3D12_MESSAGE;
            if queue.GetMessage(k, Some(message), &mut size).is_ok() {
                let m = &*message;
                let bytes = std::slice::from_raw_parts(m.pDescription, m.DescriptionByteLength.saturating_sub(1));
                record(m.Severity.0, m.ID.0, &String::from_utf8_lossy(bytes));
            }
        }
        queue.ClearStoredMessages();
    }
}

/// The counts of one run.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub corruption: u64,
    pub errors: u64,
    pub warnings: u64,
    /// Distinct error / corruption messages (addresses masked): (id, count, text).
    pub distinct_errors: Vec<(i32, u64, String)>,
    /// Distinct warnings: (id, count, text).
    pub distinct_warnings: Vec<(i32, u64, String)>,
    /// The device was removed (the reason), with what DRED saw.
    pub removed: Option<String>,
}

impl Summary {
    /// Errors (and corruption) whose text passes `keep` (e.g. the ones naming our objects).
    pub fn errors_where(&self, keep: impl Fn(&str) -> bool) -> Vec<&(i32, u64, String)> {
        self.distinct_errors.iter().filter(|e| keep(&e.2)).collect()
    }
}

/// The summary so far (does not end the capture).
pub fn summary() -> Summary {
    pump();
    let Ok(guard) = CAPTURE.lock() else { return Summary::default() };
    let Some(c) = guard.as_ref() else { return Summary::default() };
    let mut s = Summary::default();
    for seen in c.seen.values() {
        match D3D12_MESSAGE_SEVERITY(seen.severity) {
            D3D12_MESSAGE_SEVERITY_CORRUPTION => {
                s.corruption += seen.count;
                s.distinct_errors.push((seen.id, seen.count, seen.text.clone()));
            }
            D3D12_MESSAGE_SEVERITY_ERROR => {
                s.errors += seen.count;
                s.distinct_errors.push((seen.id, seen.count, seen.text.clone()));
            }
            D3D12_MESSAGE_SEVERITY_WARNING => {
                s.warnings += seen.count;
                s.distinct_warnings.push((seen.id, seen.count, seen.text.clone()));
            }
            _ => {}
        }
    }
    // SAFETY: a query on the device we hold.
    if let Some(device) = c.device.as_ref() {
        if let Err(e) = unsafe { device.GetDeviceRemovedReason() } {
            s.removed = Some(format!("{e}"));
        }
    }
    s
}

/// Ends the run: the summary into the log (every distinct message with its count; after a device removal the DRED
/// breadcrumbs and page fault) and onto stdout. Returns the summary. Safe to call when nothing was captured.
pub fn finish(tag: &str) -> Summary {
    let s = summary();
    // the callback is taken back here: the debug layer's messages while the process ends (live objects) would otherwise call
    // into a process being torn down (seen: exit code 127 after a clean run)
    if let Ok(mut guard) = CAPTURE.lock() {
        if let Some(c) = guard.as_mut() {
            if c.callback {
                if let Some(q1) = c.queue.as_ref().and_then(|q| q.cast::<ID3D12InfoQueue1>().ok()) {
                    // SAFETY: the cookie RegisterMessageCallback gave
                    let _ = unsafe { q1.UnregisterMessageCallback(c.cookie) };
                }
                c.callback = false;
            }
        }
    }
    let dred = s.removed.as_ref().map(|_| dred_report()).unwrap_or_default();
    if let Ok(mut guard) = CAPTURE.lock() {
        if let Some(c) = guard.as_mut() {
            let lines: Vec<String> = c.seen.values().map(|v| format!("  {:>8} x [{}] id={} {}", v.count, severity_name(v.severity), v.id, if v.text.is_empty() { "(info)" } else { &v.text })).collect();
            if let Some(f) = c.file.as_mut() {
                let _ = writeln!(f, "# SUMMARY {tag}: corruption {} errors {} warnings {} (distinct: {} errors, {} warnings){}", s.corruption, s.errors, s.warnings, s.distinct_errors.len(), s.distinct_warnings.len(), s.removed.as_ref().map(|r| format!("; DEVICE REMOVED: {r}")).unwrap_or_default());
                for l in &lines {
                    let _ = writeln!(f, "{l}");
                }
                for l in &dred {
                    let _ = writeln!(f, "{l}");
                }
                let _ = f.flush();
            }
        }
    }
    if active() {
        println!("gpudebug: {tag}: corruption {} errors {} warnings {} (distinct {} / {}){}", s.corruption, s.errors, s.warnings, s.distinct_errors.len(), s.distinct_warnings.len(), s.removed.as_ref().map(|r| format!("; DEVICE REMOVED: {r}")).unwrap_or_default());
        for (id, n, text) in &s.distinct_errors {
            println!("gpudebug:   ERROR x{n} id={id} {}", text.chars().take(400).collect::<String>());
        }
        for l in &dred {
            println!("gpudebug: {l}");
        }
    }
    s
}

/// DRED's view of a removal: the last breadcrumbs of every list in flight and the page fault's allocations.
fn dred_report() -> Vec<String> {
    let device = CAPTURE.lock().ok().and_then(|g| g.as_ref().and_then(|c| c.device.clone()));
    let Some(device) = device else { return Vec::new() };
    let mut out = vec!["DRED:".to_string()];
    let Ok(dred) = device.cast::<ID3D12DeviceRemovedExtendedData1>() else {
        out.push("  (no DRED interface)".to_string());
        return out;
    };
    let wide = |p: windows::core::PCWSTR| if p.is_null() { String::new() } else { unsafe { p.to_string() }.unwrap_or_default() };
    // SAFETY: DRED's linked lists stay valid while the device lives; every node pointer is checked.
    unsafe {
        if let Ok(crumbs) = dred.GetAutoBreadcrumbsOutput1() {
            let mut node = crumbs.pHeadAutoBreadcrumbNode;
            while !node.is_null() {
                let n = &*node;
                let done = if n.pLastBreadcrumbValue.is_null() { 0 } else { *n.pLastBreadcrumbValue };
                out.push(format!("  list '{}' on queue '{}': {} of {} operations done", wide(n.pCommandListDebugNameW), wide(n.pCommandQueueDebugNameW), done, n.BreadcrumbCount));
                if done < n.BreadcrumbCount && !n.pCommandHistory.is_null() {
                    let from = done.saturating_sub(4);
                    let to = (done + 4).min(n.BreadcrumbCount);
                    for k in from..to {
                        out.push(format!("    {}{:?}", if k == done { "> " } else { "  " }, *n.pCommandHistory.add(k as usize)));
                    }
                }
                node = n.pNext;
            }
        }
        if let Ok(fault) = dred.GetPageFaultAllocationOutput1() {
            out.push(format!("  page fault at {:#x}", fault.PageFaultVA));
            for (what, head) in [("existing", fault.pHeadExistingAllocationNode), ("recently freed", fault.pHeadRecentFreedAllocationNode)] {
                let mut node = head;
                while !node.is_null() {
                    let n = &*node;
                    out.push(format!("    {what}: '{}' type {}", wide(n.ObjectNameW), n.AllocationType.0));
                    node = n.pNext;
                }
            }
        }
    }
    out
}

/// Names a Direct3D 12 object for the messages ("kh:..." for ours, "harness:..." for a harness's stand-in of the game); does
/// nothing unless the debug capture is active (so never in the game).
pub fn name(object: &impl Interface, text: &str) {
    if !active() {
        return;
    }
    if let Ok(o) = object.cast::<ID3D12Object>() {
        // SAFETY: SetName copies the string.
        let _ = unsafe { o.SetName(&HSTRING::from(text)) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_switches_default_off_and_parse() {
        assert_eq!(Mode::parse("", "", None), Mode { layer: false, gbv: false, warp: false, log: PathBuf::from("kh_d3d12_debug.log") });
        let m = Mode::parse("1", "1", Some("x.log".into()));
        assert!(m.layer && m.gbv && m.warp && m.log == std::path::Path::new("x.log"));
        let m = Mode::parse("layer", "0", None);
        assert!(m.layer && !m.gbv && !m.warp);
        assert!(!Mode::parse("0", "off", None).layer);
    }

    #[test]
    fn addresses_are_masked_in_the_key() {
        assert_eq!(message_key("Resource 0x0000023A4F5E6000:'kh:main' state 12345678 slot 3"), "Resource 0x#:'kh:main' state # slot 3");
    }
}
