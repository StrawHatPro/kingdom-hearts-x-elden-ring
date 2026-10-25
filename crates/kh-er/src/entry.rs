//! The three functions `kh_er_loader.dll` calls. This DLL (`kh_er.dll`) has no `DllMain` work
//! and starts no thread: the loader loads it (again whenever the file changes), calls
//! `kh_er_init` once, `kh_er_frame` from the game's frame tasks, and `kh_er_shutdown` before it
//! replaces it. It is never unloaded.
//!
//! Rules kept here: nothing happens on any executable other than Elden Ring 2.7.1.0 or 2.7.2.0 (PORT); no panic
//! unwinds into the game (after one, this payload is inert until the next one is loaded).

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use pelite::pe64::{Pe, PeView};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;

use crate::erlog;
use crate::frame;

// (PORT) The executables this DLL is built for: crate::gamever::SUPPORTED (2.7.1.0 = app 1.17.1, and 2.7.2.0).

static READY: AtomicBool = AtomicBool::new(false);
/// Set after a panic: every call returns at once from then on.
static DEAD: AtomicBool = AtomicBool::new(false);

pub fn panic_text(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "(no text)".to_string()
    }
}

/// Base address of the game's executable in this process.
pub fn exe_base() -> usize {
    // SAFETY: a null name gives the handle of the process's executable, which stays mapped.
    (unsafe { GetModuleHandleW(std::ptr::null()) }) as usize
}

/// The mapped image of the game's executable.
pub fn exe_view() -> PeView<'static> {
    // SAFETY: the executable stays mapped for the life of the process.
    unsafe { PeView::module(exe_base() as *const u8) }
}

/// (PORT) The file version of the game's executable (crate::gamever reads it once).
pub fn exe_file_version() -> Option<(u16, u16, u16, u16)> {
    if exe_base() == 0 {
        return None;
    }
    let info = exe_view().resources().ok()?.version_info().ok()?;
    let v = info.fixed()?.dwFileVersion;
    Some((v.Major, v.Minor, v.Patch, v.Build))
}

/// # Safety
/// `dir` must point to `len` UTF-16 units: the folder the mod's files are in.
#[no_mangle]
pub unsafe extern "C" fn kh_er_init(dir: *const u16, len: usize, generation: u32) -> u32 {
    let result = catch_unwind(|| {
        // SAFETY: the loader's contract.
        let dir = PathBuf::from(String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(dir, len) }));
        let root = dir.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| dir.clone());
        // (IMPL_ER14) THE USER'S OWN SESSION LOG IS er\\logs\\kh-er.log AND NOTHING ELSE WRITES IT: a payload that runs from
        // any folder but `mods` (the test profile's `mods_test`) logs to the folder of the same suffix (`logs_test`).
        let logs = crate::log::folder_for(&dir);
        crate::log::init(&logs, generation == 0);
        std::panic::set_hook(Box::new(|info| {
            erlog!("panic: {info}");
        }));
        erlog!("---- kh-er {} payload {generation} from {}", env!("CARGO_PKG_VERSION"), dir.display());
        match exe_file_version() {
            // (PORT) both supported versions
            Some(v) if crate::gamever::from_file_version(v).is_some() => {
                erlog!("executable version {}: supported (PORT)", crate::gamever::current_name());
            }
            other => {
                erlog!("executable version {other:?} is not {}: kh-er does nothing in this process", crate::gamever::SUPPORTED_TEXT);
                return 0;
            }
        }
        // The renderer: hooks the swap chain and loads the pack on a thread of its own. (Before
        // frame::init, which hands it the control file's switches.)
        let (pack, _) = crate::sora::data_paths(&root);
        // (PACKGEN) THE TABLES MADE FROM KH3 DATA (Sora's and the party's animations, the team cameras): not compiled into
        // the released DLL, read from the pack the setup made on the player's PC (<pack>/gen, kh-gendata). Without them, or
        // made for another build, the mod does nothing (the log says why and what to run).
        kh_gendata::set_project(root.parent().unwrap_or(&root)); // (release) the repository: the sheets the player edits, the game's text
        match kh_gendata::init(&pack.join("gen")) {
            Ok(text) => erlog!("{text}"),
            Err(e) => {
                erlog!("{e}");
                erlog!("kh-er does nothing in this process: run the setup to make the pack's generated data");
                return 0;
            }
        }
        kh_er_render::set_dump_dir(&crate::log::dir());
        kh_er_render::install(&pack, |text| erlog!("{text}"));
        // The game latches its render camera late in the frame: the renderer reads it when it draws.
        kh_er_render::set_camera_source(|| {
            crate::game::camera().map(|c| kh_er_render::Camera { right: c.right, up: c.up, forward: c.forward, position: c.position, fov: c.fov, aspect: c.aspect, near: c.near, far: c.far })
        });
        // Elden Ring's view of the pad (inputs.rs): masked while Sora plays; undone with the renderer's hooks.
        crate::inputs::install();
        // KH3's camera (cam.rs): the game's camera step and audio listener are detoured; undone with the renderer's hooks.
        crate::cam::install();
        // Elden Ring's HUD part by part (erhud.rs): its HUD-group step is detoured; undone with the renderer's hooks.
        crate::erhud::install();
        // Saving only at save points (savegate.rs): the game's save-start functions are detoured; undone with the renderer's hooks.
        crate::savegate::install();
        // The game's own pause (pause.rs): the in-game step's update is detoured; undone with the renderer's hooks.
        crate::pause::install();
        // (IMPL_ER12) the title's flow (title.rs): its step's state setter, BeginNewGame and three dialog factories are detoured; undone with the renderer's hooks.
        crate::title::install();
        crate::tutorial::install(); // (IMPL_ER13)
        crate::remo::install(); // (IMPL_ER12) the cutscene's animation acts
        crate::talkkh::install(); // (CV2) the talk scripts' commands / queries, the talk update and its subtitle: Elden Ring's conversations as KH bubbles (`talk.kh`)

        frame::init(&dir, &root);
        1
    });
    match result {
        Ok(1) => {
            READY.store(true, Ordering::SeqCst);
            1
        }
        Ok(other) => other,
        Err(payload) => {
            DEAD.store(true, Ordering::SeqCst);
            erlog!("PANIC in init: {}", panic_text(&payload));
            0
        }
    }
}

#[no_mangle]
pub extern "C" fn kh_er_frame(phase: u32, dt: f32) {
    if DEAD.load(Ordering::Relaxed) || !READY.load(Ordering::Relaxed) {
        return;
    }
    if let Err(payload) = catch_unwind(AssertUnwindSafe(|| frame::run(phase, dt))) {
        DEAD.store(true, Ordering::SeqCst);
        erlog!("PANIC in phase {phase}: {}; this payload is now inert", panic_text(&payload));
        // Best effort: give the player back.
        let _ = catch_unwind(frame::shutdown);
        let _ = catch_unwind(crate::fx::shutdown); // (fx) our audio device and threads
        let _ = catch_unwind(crate::save::shutdown); // (save)
        let _ = catch_unwind(crate::newgame::shutdown); // (IMPL_ER11) the cover
        let _ = catch_unwind(crate::tutorial::shutdown); // (IMPL_ER13)
    let _ = catch_unwind(|| crate::remo::hide_player_models(false)); // (IMPL_ER12) the scene's model gets its display flag back
    let _ = catch_unwind(crate::figure::release); // (IMPL_ER16) the cutscene's figure is shown again
    let _ = catch_unwind(crate::stain::release); // (IMPL_ER17)
    let _ = catch_unwind(crate::graceflame::release); // (IMPL_ER17) the graces' flames get their size and colour back
    let _ = catch_unwind(crate::tarnfx::release); // (LANT) the Tarnished's effects get their size and colour back
    let _ = catch_unwind(crate::zfinale::release); // (ZFIN) a body hidden by the finale is drawn again
    let _ = catch_unwind(crate::gracehide::release); // (SV)
    let _ = catch_unwind(crate::gracelock::release); // (SV)
    let _ = catch_unwind(crate::erkeyui::release); // (HUD2)
    let _ = catch_unwind(crate::pause::release); // (IMPL_ER11) our hold on the world is taken back before the hooks go
    let _ = catch_unwind(crate::titleload::shutdown); // (IMPL_ER15) the masked summary bytes go back before anything else
    let _ = catch_unwind(crate::hud::shutdown); // (hud) Elden Ring's HUD and speed given back, our threads ended
        let _ = catch_unwind(kh_er_render::shutdown);
        crate::inputs::uninstalled();
    crate::cam::uninstalled();
    crate::erhud::uninstalled();
    crate::loot::uninstalled(); // (loot)
    crate::savegate::uninstalled();
    crate::pause::uninstalled();
    crate::title::uninstalled();
    crate::tutorial::uninstalled();
    crate::remo::uninstalled();
    crate::talkkh::uninstalled(); // (CV2)
    }
}

#[no_mangle]
pub extern "C" fn kh_er_shutdown() {
    if !READY.swap(false, Ordering::SeqCst) || DEAD.load(Ordering::Relaxed) {
        return;
    }
    if let Err(payload) = catch_unwind(frame::shutdown) {
        erlog!("PANIC in shutdown: {}", panic_text(&payload));
    }
    let _ = catch_unwind(crate::fx::shutdown); // (fx) our audio device and threads
    let _ = catch_unwind(crate::save::shutdown); // (save)
    let _ = catch_unwind(crate::newgame::shutdown); // (IMPL_ER11) the cover
        let _ = catch_unwind(crate::tutorial::shutdown); // (IMPL_ER13)
    let _ = catch_unwind(|| crate::remo::hide_player_models(false)); // (IMPL_ER12) the scene's model gets its display flag back
    let _ = catch_unwind(crate::figure::release); // (IMPL_ER16) the cutscene's figure is shown again
    let _ = catch_unwind(crate::stain::release); // (IMPL_ER17)
    let _ = catch_unwind(crate::graceflame::release); // (IMPL_ER17)
    let _ = catch_unwind(crate::tarnfx::release); // (LANT)
    let _ = catch_unwind(crate::zfinale::release); // (ZFIN) a body hidden by the finale is drawn again
    let _ = catch_unwind(crate::gracehide::release); // (SV)
    let _ = catch_unwind(crate::gracelock::release); // (SV)
    let _ = catch_unwind(crate::erkeyui::release); // (HUD2)
    let _ = catch_unwind(crate::pause::release); // (IMPL_ER11) our hold on the world is taken back before the hooks go
    let _ = catch_unwind(crate::titleload::shutdown); // (IMPL_ER15) the masked summary bytes go back before anything else
    let _ = catch_unwind(crate::hud::shutdown); // (hud) Elden Ring's HUD and speed given back, our threads ended
    // The renderer puts its vtable hooks back and frees its Direct3D objects.
    if let Err(payload) = catch_unwind(kh_er_render::shutdown) {
        erlog!("PANIC in the renderer's shutdown: {}", panic_text(&payload));
    }
    crate::inputs::uninstalled();
    crate::cam::uninstalled();
    crate::erhud::uninstalled();
    crate::loot::uninstalled(); // (loot)
    crate::savegate::uninstalled();
    crate::pause::uninstalled();
    crate::title::uninstalled();
    crate::tutorial::uninstalled();
    crate::remo::uninstalled();
    crate::talkkh::uninstalled(); // (CV2)
    erlog!("payload shut down");
}

/// (PACKGEN) THE OFFLINE CHECK OF A RELEASED DLL (release-tools/selfcheck.ps1 calls it outside the game): with `dir` the
/// folder of the DLL (er\mods), finds the pack as `kh_er_init` does, reads every generated table made from KH3 data
/// (kh-gendata) and decodes each of them through the code that uses them. Writes `<dir>\kh-er-selfcheck.txt` (what was
/// read, or the error) and returns the number of tables read, 0 when something is missing or wrong. Touches nothing else:
/// no hook, no thread, no game memory.
///
/// # Safety
/// `dir` must point to `len` UTF-16 units.
#[no_mangle]
pub unsafe extern "C" fn kh_er_selfcheck(dir: *const u16, len: usize) -> u32 {
    // SAFETY: the caller's contract.
    let dir = PathBuf::from(String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(dir, len) }));
    let root = dir.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| dir.clone());
    let report = dir.join("kh-er-selfcheck.txt");
    let result = catch_unwind(AssertUnwindSafe(|| -> Result<Vec<String>, String> {
        let (pack, _) = crate::sora::data_paths(&root);
        let mut lines = vec![format!("kh-er {} self-check, pack {}", env!("CARGO_PKG_VERSION"), pack.display())];
        kh_gendata::set_project(root.parent().unwrap_or(&root));
        lines.push(kh_gendata::init(&pack.join("gen"))?);
        lines.push(format!("game text (the pack's ui/strings): {:?}", kh_gendata::line("kh3_menu.shop.dialog_0020")));
        use kh_core::anim::{ANIMS, NOTIFY_ASSETS, SEQUENCES};
        lines.push(format!("sora_anim: {} sequences, {} animations, {} notify assets; IdleN = {:?}", SEQUENCES.len(), ANIMS.len(), NOTIFY_ASSETS.len(), kh_core::anim::AnimId::from_name("IdleN")));
        #[cfg(feature = "party")]
        {
            use kh_party::Chara;
            for c in [Chara::Donald, Chara::Goofy] {
                let set = kh_party::tables::anim_set(c);
                lines.push(format!("party_clips / party_data {c:?}: {} clips, level 1 row {:?}, attack hit SE {:?}", set.clips.len(), kh_party::tables::level_row(c, 1), kh_party::notify_seam::attack_hit_se(c, "-", "-")));
            }
            lines.push(format!("party_team_gshoot: frame {:?}", kh_party::team::link::gshoot_frame(0, 0.0).root));
        }
        lines.push(format!("party_team_cameras: {} camera anims, {} shakes", kh_camera::camera_anim::TEAM_CAMERA_ANIMS.len(), kh_camera::camera_anim::TEAM_CAMERA_SHAKES.len()));
        Ok(lines)
    }));
    let (text, n) = match result {
        Ok(Ok(lines)) => { let n = lines.len() as u32 - 2; (lines.join("\n"), n.max(1)) }
        Ok(Err(e)) => (format!("FAILED: {e}"), 0),
        Err(payload) => (format!("FAILED: {}", panic_text(&payload)), 0),
    };
    let _ = std::fs::write(&report, text + "\n");
    n
}
