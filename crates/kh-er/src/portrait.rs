//! (IMPL_ER21) SORA IN THE CAMP MENU'S CHARACTER PANEL, in Elden Ring (the user, 2026-10-13: "Sora is not visible in the
//! menu"). The test app draws him there with a second, separately posed copy of the model (kh-app/src/menu_model.rs);
//! here the same copy is `kh_host::menu_portrait::MenuPortrait` (the game's `Idle_CM` with `IdleBreak_CM` every 2 .. 5
//! loops, hair and cloth running, the UI actor's capture and two spot lights from `PACK/menu/menu.json`), placed by the
//! SAME code on the movie's clip `charaWindow01.charaAnm.chara` (and the character picture of the Abilities and Stats
//! screens), and drawn by the renderer into a target of its own at Present (`kh_er_render::PortraitFrame`: the void's
//! approach; nothing of the game's frame is read or written), which the 2D overlay lays over the camp menu's picture
//! and under its dialogs, cut at the panel's mask, with the panel's opacity.
//!
//! COST: nothing while no camp menu is up (one call that finds no movie). The copy's pose evaluator is read on a thread
//! of its own the first time a camp menu opens (about a second: the panel is empty until then).
//!
//! NOT THE GAME'S / STAND-INS: the reflection environment is the renderer's one cube (the stand-in sky) instead of the
//! menu's ambient cube map; no multisampling (the dither coverage of his masked materials becomes alpha in quarters);
//! the face layer of the menu's anim blueprint (his blinking there) is not played (as in the test app).
//!
//! | key | default | meaning |
//! |---|---|---|
//! | `menu.portrait` | 1 | 0 = the panel stays empty (as before IMPL_ER21) |
//! | `menu.portrait.exposure` | 1.667 | the factor in front of the tone curve (the test app's `MENU_EXPOSURE`) |
//! | `menu.portrait.env` | 1 | level of the reflection cube |

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use glam::Vec3;
use kh_host::menu_portrait::{placement, MenuPortrait, PictureSync, PortraitData, PortraitOut, CLEAR, MENU_EXPOSURE};
use kh_ui::{Matrix, Movie};

use crate::control::Control;
use crate::erlog;

/// The overlay layer the picture lies under: `LAYERS[7]` ("GameHelp"), right after the camp menu's own picture.
const BEFORE_LAYER: usize = 7;
/// (IMPL_ER34) The camp menu's own layer (`LAYERS[6]`): the picture goes with ITS pictures.
pub const CAMP_LAYER: usize = BEFORE_LAYER - 1;

struct State {
    copy: MenuPortrait,
    shown: bool,
    frames: u32,
    /// (IMPL_ER34) his placement follows the menu picture that is on screen
    sync: PictureSync,
    /// (IMPL_ER34) frames in which he was placed by an older state than the movie's (the worker was busy)
    held: u32,
}

// SAFETY: the animator holds plain data; it is made on a thread of its own and from then on only used under the mutex.
unsafe impl Send for State {}

static STATE: Mutex<Option<State>> = Mutex::new(None);
static LOADING: Mutex<Option<std::thread::JoinHandle<()>>> = Mutex::new(None);
static FAILED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The copy's own pose evaluator is read (once, on a thread of its own).
fn prepare(pack: &Path) {
    if FAILED.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    let Ok(mut loading) = LOADING.lock() else { return };
    if loading.is_some() {
        return;
    }
    let pack: PathBuf = pack.to_path_buf();
    *loading = std::thread::Builder::new()
        .name("kh-portrait-load".into())
        .spawn(move || {
            let started = std::time::Instant::now();
            match std::panic::catch_unwind(|| MenuPortrait::load(&pack)) {
                Ok(Ok(copy)) => {
                    erlog!("portrait: the camp menu's own copy of Sora is read in {:.2} s (clip {} {} s{}; capture {}x{} FOV {} over its larger side; {} spot light(s))", started.elapsed().as_secs_f32(), copy.data.clip.0, copy.data.clip.2, copy.data.idle_break.as_ref().map_or(String::new(), |b| format!(", break {} {} s", b.0, b.2)), copy.data.target.0, copy.data.target.1, copy.data.fov_deg, copy.data.rig.spots.len());
                    *STATE.lock().unwrap_or_else(|e| e.into_inner()) = Some(State { copy, shown: false, frames: 0, sync: PictureSync::default(), held: 0 });
                }
                Ok(Err(e)) => {
                    FAILED.store(true, std::sync::atomic::Ordering::Relaxed);
                    erlog!("portrait: the camp menu's copy of Sora could not be read ({e}): the panel stays empty");
                }
                Err(_) => {
                    FAILED.store(true, std::sync::atomic::Ordering::Relaxed);
                    erlog!("portrait: PANIC while reading the camp menu's copy of Sora: the panel stays empty");
                }
            }
        })
        .ok();
}

/// What the renderer needs, from the shared copy's answer. GAME (KH3: cm, X forward, Y right, Z up) -> the renderer's
/// host space (m, X right, Y up, Z forward) with the UI actor at the origin.
pub fn frame_of(data: &PortraitData, out: PortraitOut, exposure: f32, env_level: f32) -> kh_er_render::PortraitFrame {
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
        before_layer: BEFORE_LAYER,
        with_layer: BEFORE_LAYER - 1,
        clear: CLEAR,
    }
}

/// ONCE A HUD FRAME (hud.rs): `movie` = the CampMenu movie while a camp menu is up (else `None`: nothing is done),
/// `root` = stage -> back buffer pixels, `output` = the back buffer's size, `dt` = the frame's real time.
/// (IMPL_ER34) Called AFTER the layers' update; `camp` = the camp layer's (states sent to the raster worker, results
/// taken back, busy): he is placed by the state whose picture is on screen (`menu.portrait.sync = 0`: by this frame's).
/// (ER party I) `party` = the camp menu's panel order and the member its sub screens show (`Hud::camp_panels`,
/// `Hud::camp_member`): Sora is placed on HIS panel (`sora_placement`); `None` = the old placement.
#[allow(clippy::too_many_arguments)]
pub fn frame(control: &Control, movie: Option<&Movie>, root: &Matrix, output: (u32, u32), dt: f32, pack: Option<&Path>, camp: (u64, u64, bool), party: Option<(&[kh_host::party::PartyChara], usize)>) {
    let Some(movie) = movie.filter(|_| control.flag("menu.portrait") != Some(false)) else {
        // (cheap: one lock, only while a picture was up)
        if let Ok(mut guard) = STATE.try_lock() {
            if let Some(state) = guard.as_mut().filter(|s| s.shown) {
                state.shown = false;
                state.sync.clear();
                kh_er_render::set_portrait(None);
                erlog!("portrait: the camp menu's picture of Sora is taken off ({} frame(s) shown, {} of them placed by the menu picture still on screen; posing took {:.3} ms a frame)", state.frames, state.held, state.copy.seconds * 1000.0 / state.copy.frames.max(1) as f64);
                erlog!("menu: portrait OFF (the camp menu is gone)");
                state.frames = 0;
                state.held = 0;
            }
        }
        return;
    };
    let Ok(mut guard) = STATE.lock() else { return };
    let Some(state) = guard.as_mut() else {
        if let Some(pack) = pack {
            prepare(pack);
        }
        return;
    };
    let now = sora_placement(state.copy.data.target, movie, root, output, party); // (ER party I) his own panel
    let placed = if control.flag("menu.portrait.sync") == Some(false) { now } else { state.sync.step(now, camp.0, camp.1, camp.2) };
    if placed != now {
        state.held += 1;
    }
    match placed.map(|p| state.copy.frame_at(p, dt)) {
        Some(out) => {
            if !state.shown {
                state.shown = true;
                erlog!("menu: portrait ON at ({}, {}) {}x{} opacity {:.2}", out.visible.0, out.visible.1, out.visible.2, out.visible.3, out.alpha);
                erlog!("portrait: SORA IS IN THE CAMP MENU'S PANEL: the picture's visible part {}x{} at ({}, {}) of the back buffer {}x{}, the whole capture {:.0}x{:.0} at ({:.0}, {:.0}), opacity {:.2}", out.visible.2, out.visible.3, out.visible.0, out.visible.1, output.0, output.1, out.picture.2, out.picture.3, out.picture.0, out.picture.1, out.alpha);
            }
            state.frames += 1;
            let exposure = control.f32("menu.portrait.exposure").filter(|v| (0.05..=20.0).contains(v)).unwrap_or(MENU_EXPOSURE);
            let env = control.f32("menu.portrait.env").filter(|v| (0.0..=20.0).contains(v)).unwrap_or(1.0);
            kh_er_render::set_portrait(Some(frame_of(&state.copy.data, out, exposure, env)));
        }
        None => {
            if state.shown {
                state.shown = false;
                kh_er_render::set_portrait(None);
                erlog!("menu: portrait OFF (the panel does not show: a sub screen)");
            }
        }
    }
}

/// (ER party I, S4's request) SORA'S PANEL IN A PARTY'S CAMP MENU, by kh-app menu_model.rs's rule: the panels' order
/// (`Hud::camp_panels`, `charaWindow01` ..) may put him elsewhere (a friend leading) or nowhere (out of the party); the
/// sub screens' picture is his only while they show him (camp member 0). With Sora first (or no party) and member 0 this
/// is `placement` exactly (the build without the party, and `party.er = 0`).
pub fn sora_placement(target: (u32, u32), movie: &Movie, root: &Matrix, output: (u32, u32), party: Option<(&[kh_host::party::PartyChara], usize)>) -> Option<kh_host::menu_portrait::Placement> {
    use kh_host::menu_portrait::{panel_at, placement_of, top_panel, PANELS};
    let Some((panels, member)) = party else { return placement(target, movie, root, output) };
    let sora_top = panels.iter().position(|c| *c == kh_host::party::PartyChara::Sora);
    if !panels.is_empty() && sora_top != Some(0) {
        // (AUD) his top panel, or the sub screens' picture while they show him (his camp slot, as a friend's: partydraw.rs)
        sora_top.and_then(|i| panel_at(movie, &top_panel(i), target, 1.0, 0.0).or_else(|| if member == i { PANELS[1..].iter().find_map(|p| panel_at(movie, p, target, 1.0, 0.0)) } else { None })).and_then(|p| placement_of(&p, root, output))
    } else if member == 0 {
        placement(target, movie, root, output)
    } else {
        panel_at(movie, PANELS[0], target, 1.0, 0.0).and_then(|p| placement_of(&p, root, output))
    }
}

/// (IMPL_ER34) THE CAMP MENU'S SCREEN STACK IN THE LOG, for a report like "leaving the equipment menu is glitchy":
/// every change of the stack with the time since the change before, and every frame longer than 20 ms within 0.6 s of
/// a change (a transition's animation runs about ten frames). `screen` = `Hud::camp_trace`, `dt` = the frame's real time.
pub fn menu_trace(screen: Option<String>, dt: f32) {
    static TRACE: Mutex<Option<MenuTrace>> = Mutex::new(None);
    let Ok(mut guard) = TRACE.try_lock() else { return };
    let trace = guard.get_or_insert_with(MenuTrace::default);
    for line in trace.step(screen, dt) {
        erlog!("{line}");
    }
}

/// The pure part of [`menu_trace`].
#[derive(Default)]
pub struct MenuTrace {
    screen: Option<String>,
    /// Seconds and frames since the last change, and what it was.
    since: f32,
    frames: u32,
    change: String,
    spikes: u32,
}

impl MenuTrace {
    pub const SPIKE_S: f32 = 0.020;
    pub const WINDOW_S: f32 = 0.6;

    pub fn step(&mut self, screen: Option<String>, dt: f32) -> Vec<String> {
        let mut lines = Vec::new();
        if screen.is_none() && self.screen.is_none() {
            return lines;
        }
        self.since += dt;
        self.frames += 1;
        if screen != self.screen {
            let word = |s: &Option<String>| s.clone().unwrap_or_else(|| "closed".to_string());
            self.change = format!("{} -> {}", word(&self.screen), word(&screen));
            lines.push(format!("menu: {} ({:.0} ms after the change before; this frame {:.1} ms)", self.change, self.since * 1000.0, dt * 1000.0));
            self.screen = screen;
            (self.since, self.frames, self.spikes) = (0.0, 0, 0);
        } else if dt > Self::SPIKE_S && self.since <= Self::WINDOW_S && self.spikes < 8 {
            self.spikes += 1;
            lines.push(format!("menu: FRAME OF {:.1} ms, {} frame(s) / {:.0} ms into the transition {}", dt * 1000.0, self.frames, self.since * 1000.0, self.change));
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::MenuTrace;

    #[test]
    fn the_menu_trace_logs_changes_and_long_frames_of_a_transition() {
        let mut trace = MenuTrace::default();
        let word = |s: &str| Some(s.to_string());
        // nothing while no menu is up
        assert!(trace.step(None, 0.016).is_empty());
        let opened = trace.step(word("Top"), 0.016);
        assert_eq!(opened.len(), 1);
        assert!(opened[0].starts_with("menu: closed -> Top"), "{opened:?}");
        // ordinary frames say nothing; a long one inside the transition is named
        assert!(trace.step(word("Top"), 0.016).is_empty());
        let spike = trace.step(word("Top"), 0.035);
        assert!(spike.len() == 1 && spike[0].contains("FRAME OF 35.0 ms") && spike[0].contains("closed -> Top"), "{spike:?}");
        // after the transition's window a long frame is no longer the menu's business
        for _ in 0..60 {
            trace.step(word("Top"), 0.016);
        }
        assert!(trace.step(word("Top"), 0.050).is_empty());
        let deeper = trace.step(word("Equipment/Slots"), 0.016);
        assert!(deeper[0].starts_with("menu: Top -> Equipment/Slots"), "{deeper:?}");
        let back = trace.step(word("Top"), 0.016);
        assert!(back[0].starts_with("menu: Equipment/Slots -> Top (16 ms after the change before"), "{back:?}");
        let closed = trace.step(None, 0.016);
        assert!(closed[0].starts_with("menu: Top -> closed"), "{closed:?}");
        assert!(trace.step(None, 0.1).is_empty());
    }
}

/// The payload goes away.
pub fn shutdown() {
    kh_er_render::set_portrait(None);
    if let Some(handle) = LOADING.lock().ok().and_then(|mut l| l.take()) {
        let _ = handle.join();
    }
    if let Ok(mut guard) = STATE.lock() {
        *guard = None;
    }
}
