//! What runs each game frame, by phase. Game thread only (the tasks of the game call in here).
//!
//! `mode` of the control file picks what is done:
//! - `probe` (default): nothing is changed; conventions are drawn and logged.
//! - `passive`: the experiment harness for the passive player (switches `p.*`, teleport `tp.*`).
//! - `sora`: Sora's simulation drives the (passive, hidden) player (`sora.rs`).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use glam::{Quat, Vec3};

use crate::control::Control;
use crate::erlog;
use crate::game::{self, phase, Draw};
use crate::passive::{self, Original, Switches};

pub struct State {
    pub control: Control,
    pub root: PathBuf,
    /// Seconds of game time since the last once-a-second work.
    pub since_tick: f32,
    pub tick: bool,
    pub frames: u64,
    pub frames_in_tick: u32,
    pub had_player: bool,
    pub clock: f32,
    /// The player's values before we changed any (present while we hold him).
    pub original: Option<Original>,
    pub probe: Experiment,
    /// The player's position as the last phase of ours left or saw it (origin-shift detection).
    pub last_seen: Option<Vec3>,
    /// Extent of the positions seen this second, and the animations the game played.
    pub seen_min: Vec3,
    pub seen_max: Vec3,
    pub anims: Vec<i32>,
    /// Sora, while mode = sora holds the player.
    pub sora: Option<crate::sora::SoraHost>,
    /// We are holding Elden Ring's HUD hidden (`hud.hide`).
    pub hud_hidden: bool,
    /// The value of `time.set` that was last handed to the game.
    pub time_set: Option<String>,
    pub dead_told: bool,
    /// (IMPL_ER10) After a death that was handed to the game: where its body was last seen, and for how long it has
    /// been alive again in the same object (the tutorial's scripted death does not replace the player).
    pub dead_body: Option<Vec3>,
    pub dead_alive_for: f32,
}

/// Numbers of the passive-player experiment, gathered over a second.
#[derive(Default)]
pub struct Experiment {
    pub anchor: Option<Vec3>,
    pub anchor_generation: u64,
    pub written_pre: Option<Vec3>,
    pub written_last: Option<Vec3>,
    /// Largest move the game made between our PrePhysics write and PostPhysics (horizontal, vertical).
    pub fight_post: (f32, f32),
    /// Largest move between our PostPhysics write and the next PrePhysics.
    pub fight_pre: (f32, f32),
    pub resets: u32,
    pub spin: f32,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

pub fn init(dll_dir: &Path, root: &Path) {
    // (INI) the settings file is kh-er.ini; the old kh-er.ctl only while no kh-er.ini exists (control.rs resolve)
    let mut control = Control::new(dll_dir.join(crate::control::FILE));
    control.poll();
    if let Some(notice) = control.source_notice() {
        erlog!("{notice}");
    }
    erlog!("control: {}", control.dump());
    kh_er_render::set_options(render_options(&control));
    crate::hud::set_root(root);
    crate::fx::prestart(&crate::sora::data_paths(root).0); // (IMPL_ER14) the audio pack and device from the start
    crate::prize::set_root(root); // (prize)
    *STATE.lock().unwrap() = Some(State {
        control,
        root: root.to_path_buf(),
        since_tick: 0.0,
        tick: false,
        frames: 0,
        frames_in_tick: 0,
        had_player: false,
        clock: 0.0,
        original: None,
        probe: Experiment::default(),
        last_seen: None,
        seen_min: Vec3::splat(f32::MAX),
        seen_max: Vec3::splat(f32::MIN),
        anims: Vec::new(),
        sora: None,
        hud_hidden: false,
        time_set: None,
        dead_told: false,
        dead_body: None,
        dead_alive_for: 0.0,
    });
    // ONE-SHOT KEYS THAT ARE ALREADY IN THE FILE WHEN A PAYLOAD STARTS ARE NOT CARRIED OUT (a hot
    // reload under a player once jumped the clock that way): only a change is.
    if let Some(state) = STATE.lock().unwrap().as_mut() {
        state.time_set = state.control.str("time.set").map(str::to_string);
    }
}

/// Puts the player back as the game had him.
pub fn shutdown() {
    let mut guard = STATE.lock().unwrap();
    if let Some(state) = guard.as_mut() {
        release(state);
    }
    *guard = None;
}

fn release(state: &mut State) {
    if state.hud_hidden {
        // SAFETY: as in `hud`.
        if let Ok(fe) = unsafe { <eldenring::cs::CSFeManImp as fromsoftware_shared::FromStatic>::instance_mut() } {
            fe.hud_state = eldenring::cs::CSFeManHudState::Default;
        }
        state.hud_hidden = false;
    }
    if let (Some(original), Some(player)) = (state.original.take(), game::player_mut()) {
        original.restore(player);
        erlog!("player released: original values restored");
    }
    state.probe = Experiment::default();
    if let Some(mut host) = state.sora.take() {
        // (the Tarnished's own maximum HP, the enemies in a reaction, the borrowed param rows)
        if let Some(player) = game::player_mut() {
            host.stop(player);
        }
        erlog!("sora: stopped");
    }
    kh_er_render::clear();
}

/// The renderer's switches: keys `r.*` of the control file (kh_er_render::Options says what each is).
/// (IMPL_ER78) The hybrid's keys; absent = kh-er-render hybrid.rs `Settings::default()`.
fn hybrid_settings(c: &Control) -> kh_er_render::hybrid::Settings {
    let d = kh_er_render::hybrid::Settings::default();
    let f = |key: &str, default: f32| c.f32(key).filter(|v| v.is_finite()).unwrap_or(default);
    let on = |key: &str, default: bool| c.i64(key).map_or(default, |v| v != 0);
    kh_er_render::hybrid::Settings {
        on: on("r.hybrid", d.on),
        cast: on("r.hybrid_cast", d.cast),
        game: f("r.hybrid_game", d.game),
        rig: f("r.hybrid_rig", d.rig),
        spec: f("r.hybrid_spec", d.spec),
        sh: f("r.hybrid_sh", d.sh),
        band_lo: f("r.hybrid_band_lo", d.band_lo),
        band_hi: f("r.hybrid_band_hi", d.band_hi),
        band_soft: f("r.hybrid_band_soft", d.band_soft),
        band_mix: f("r.hybrid_band_mix", d.band_mix),
        shade: f("r.hybrid_shade", d.shade),
        high: f("r.hybrid_high", d.high),
        ao: f("r.hybrid_ao", d.ao),
        fog: f("r.hybrid_fog", d.fog),
        wrap: f("r.hybrid_wrap", d.wrap),
        gloss: f("r.hybrid_gloss", d.gloss),
        lightset: c.i64("r.hybrid_lightset").map_or(d.lightset, |v| v.clamp(0, 1) as u8),
        edge: f("r.hybrid_edge", d.edge),
        // (HY) the third version's keys (r.hybrid_amb_sub, _self, _lift, _match, _search, _debug, _rig_own: hybrid.rs `with_keys`)
        ..kh_er_render::hybrid::with_keys(d, &|key: &str| c.f32(key))
    }
}

/// (IMPL_ER86) The steady light's keys (kh-er-render stable.rs); absent = `Settings::default()` (all on). `r.light_stable = 0`:
/// Sora's light as before IMPL_ER86.
fn stable_settings(c: &Control) -> kh_er_render::stable::Settings {
    let d = kh_er_render::stable::Settings::default();
    let f = |key: &str, default: f32| c.f32(key).filter(|v| v.is_finite() && *v >= 0.0).unwrap_or(default);
    let on = |key: &str, default: bool| c.i64(key).map_or(default, |v| v != 0);
    kh_er_render::stable::Settings {
        on: on("r.light_stable", d.on),
        probe_world: on("r.probe_world", d.probe_world),
        probe_radius: f("r.probe_radius_cm", d.probe_radius),
        tau_move: f("r.light_tau_s", d.tau_move),
        tau_still: f("r.light_tau_still_s", d.tau_still),
        move_speed: f("r.light_move_cms", d.move_speed),
        tau_exposure: f("r.exposure_tau_s", d.tau_exposure),
        tau_env: f("r.env_tau_s", d.tau_env),
        env_screen: on("r.env_screen", d.env_screen),
        env_floor: f("r.env_floor", d.env_floor),
        hold: f("r.light_hold_s", d.hold),
        gate_hold: f("hero.gate_hold_s", d.gate_hold),
        hero_anchor: c.i64("hero.anchor").map_or(d.hero_anchor, |v| v.clamp(0, 2) as u8),
        hero_forward: f("hero.anchor_cm", d.hero_forward),
        tau_facing: f("hero.anchor_tau_s", d.tau_facing),
        soft_cut: on("fx.soft_cut", d.soft_cut),
        rank_near: on("fx.rank_near", d.rank_near),
    }
}

fn render_options(c: &Control) -> kh_er_render::Options {
    let d = kh_er_render::Options::default();
    let on = |key: &str, default: bool| c.flag(key).unwrap_or(default);
    kh_er_render::Options {
        draw: on("r.draw", d.draw),
        depth: c.i64("r.depth").map_or(d.depth, |v| v.max(0) as u32),
        fov_horizontal: on("r.fovx", d.fov_horizontal),
        front_ccw: on("r.ccw", d.front_ccw),
        cull: on("r.cull", d.cull),
        flat: c.i64("r.flat").map_or(d.flat, |v| v.max(0) as u32),
        debug: c.f32("r.debug").unwrap_or(d.debug),
        khs: c.str("r.khs").map_or(d.khs.clone(), str::to_string),
        exposure: c.f32("r.exposure").unwrap_or(d.exposure),
        key: c.f32("r.key").unwrap_or(d.key),
        ambient: c.f32("r.ambient").unwrap_or(d.ambient),
        env: c.f32("r.env").unwrap_or(d.env),
        only: c.i64("r.only").map_or(d.only, |v| v as i32),
        lag: c.i64("r.lag").map_or(d.lag, |v| v.max(0) as u32),
        pose_lag: c.i64("r.poselag").map_or(d.pose_lag, |v| v.max(0) as u32),
        live_camera: on("r.livecam", d.live_camera),
        at_present: on("r.present", d.at_present),
        at: match c.str("r.at") {
            Some("composite") => kh_er_render::At::Composite,
            Some("scene") => kh_er_render::At::Scene,
            _ => d.at,
        },
        hdr_scale: c.f32("r.hdr").unwrap_or(d.hdr_scale),
        survey: c.i64("r.survey").map_or(d.survey, |v| v.max(0) as u32),
        bias: c.f32("r.bias").unwrap_or(d.bias),
        cbcap: c.i64("r.cbcap").map_or(d.cbcap, |v| v.max(0) as u32),
        shscan: c.i64("r.shscan").map_or(d.shscan, |v| v.max(0) as u32),
        ghost: c.f32("r.ghost").unwrap_or(d.ghost),
        shadow: c.f32("r.shadow").unwrap_or(d.shadow),
        shadow_sun: c.f32("r.shadow_sun").unwrap_or(d.shadow_sun),
        light: on("r.light", d.light),
        jitter: on("r.jitter", d.jitter),
        velocity: c.i64("r.velocity").map_or(d.velocity, |v| v as i32),
    }
}

/// (TITLEPERF) Our time in each task phase since the last report (ns, calls): FrameBegin, PrePhysics, PostPhysics,
/// every other phase; and the game's frames (FrameBegin calls).
static PERF: [std::sync::atomic::AtomicU64; 8] = [const { std::sync::atomic::AtomicU64::new(0) }; 8];

pub fn run(phase_id: u32, dt: f32) {
    use std::sync::atomic::Ordering::Relaxed;
    let started = std::time::Instant::now();
    run_phase(phase_id, dt);
    let k = match phase_id {
        phase::FRAME_BEGIN => 0,
        phase::PRE_PHYSICS => 1,
        phase::POST_PHYSICS => 2,
        _ => 3,
    };
    PERF[k * 2].fetch_add(started.elapsed().as_nanos() as u64, Relaxed);
    PERF[k * 2 + 1].fetch_add(1, Relaxed);
    if phase_id == phase::FRAME_BEGIN {
        // (HAIRPERF, the user: "does the log tell you anything about traversal stutters") the game's frame time, start to start:
        // the longest and the frames over 50 ms since the last report
        static LAST: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);
        if let Ok(mut last) = LAST.lock() {
            if let Some(before) = last.replace(started) {
                let gap = started.duration_since(before).as_micros() as u64;
                FRAME_LONGEST.fetch_max(gap, Relaxed);
                if gap > 50_000 {
                    FRAME_HITCHES.fetch_add(1, Relaxed);
                }
            }
        }
        perf_report();
    }
}

/// (HAIRPERF) The game's longest frame (us) and its frames over 50 ms since the last report.
static FRAME_LONGEST: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static FRAME_HITCHES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// (TITLEPERF, the user: "quitting ... back to the title screen ... makes the title screen and menu extremely laggy") Once a
/// second of real time while no world is loaded (once every 10 s in the world, to compare): the game's frames, our time a frame
/// in each of our places, the process's CPU use and the adapter's video memory.
fn perf_report() {
    use std::sync::atomic::Ordering::Relaxed;
    static AT: std::sync::Mutex<Option<(std::time::Instant, u64)>> = std::sync::Mutex::new(None);
    let Ok(mut at) = AT.lock() else { return };
    let cpu_now = process_cpu_ns();
    let Some((since, cpu_before)) = *at else {
        *at = Some((std::time::Instant::now(), cpu_now));
        return;
    };
    let world = game::player_mut().is_some();
    let secs = since.elapsed().as_secs_f64();
    if secs < if world { 10.0 } else { 1.0 } {
        return;
    }
    *at = Some((std::time::Instant::now(), cpu_now));
    let take = |k: usize| (PERF[k * 2].swap(0, Relaxed), PERF[k * 2 + 1].swap(0, Relaxed));
    let phases = [take(0), take(1), take(2), take(3)];
    let frames = phases[0].1.max(1);
    let ms = |ns: u64| ns as f64 / 1e6 / frames as f64;
    let ((present_ns, presents), (survey_ns, submissions)) = kh_er_render::hook_perf();
    let tl = crate::titleload::take_perf();
    let tl = format!("titleload parts: slot watch {:.2}, state wait {:.2}, title_prepare {:.2}, steps in all {:.2}", ms(tl[0]), ms(tl[1]), ms(tl[2]), ms(tl[3]));
    let vram = kh_er_render::video_memory().map_or("unknown".to_string(), |(used, budget)| format!("{:.2} of {:.2} GB", used as f64 / 1e9, budget as f64 / 1e9));
    erlog!(
        "perf ({}): {:.1} game frames/s, {:.1} presents/s, longest frame {:.1} ms, {} frame(s) over 50 ms | our ms a frame: frame begin {:.2} (slowest: {}), pre-physics {:.2}, post-physics {:.2}, other phases {:.2} ({} calls), Present {:.2}, the submissions' survey {:.2} ({} submissions) | process CPU {:.0}% of one core | video memory {vram} | {tl}",
        if world { "in the world" } else { "NO WORLD" },
        phases[0].1 as f64 / secs,
        presents as f64 / secs,
        FRAME_LONGEST.swap(0, Relaxed) as f64 / 1000.0,
        FRAME_HITCHES.swap(0, Relaxed),
        ms(phases[0].0),
        sub_report(frames, phases[0].0),
        ms(phases[1].0),
        ms(phases[2].0),
        ms(phases[3].0),
        phases[3].1,
        ms(present_ns),
        ms(survey_ns),
        submissions,
        cpu_now.saturating_sub(cpu_before) as f64 / 1e9 / secs * 100.0
    );
}

/// The process's CPU time so far (kernel + user, ns).
fn process_cpu_ns() -> u64 {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
    let zero = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
    let (mut a, mut b, mut kernel, mut user) = (zero, zero, zero, zero);
    // SAFETY: the pseudo handle of our own process; plain out parameters.
    if unsafe { GetProcessTimes(GetCurrentProcess(), &mut a, &mut b, &mut kernel, &mut user) } == 0 {
        return 0;
    }
    let t = |f: FILETIME| ((f.dwHighDateTime as u64) << 32 | f.dwLowDateTime as u64) * 100;
    t(kernel) + t(user)
}

/// (TITLEPERF) The frame-begin systems timed one by one (ns since the last report), with their names.
static SUB: [std::sync::atomic::AtomicU64; 15] = [const { std::sync::atomic::AtomicU64::new(0) }; 15];
const SUB_NAMES: [&str; 15] = ["save.set_dir", "savegate", "slots", "gracelock+telescope", "erkeyui", "slots.aids", "pause", "title", "tutorial", "probe", "newgame", "titleload", "fx.trace", "track_player", "the tick's work"];

fn sub_timed<R>(k: usize, f: impl FnOnce() -> R) -> R {
    let started = std::time::Instant::now();
    let r = f();
    SUB[k].fetch_add(started.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
    r
}

/// The three slowest frame-begin systems since the last report, ms a frame, and what none of them took (`total` ns).
fn sub_report(frames: u64, total: u64) -> String {
    let mut all: Vec<(u64, &str)> = SUB.iter().zip(SUB_NAMES).map(|(t, n)| (t.swap(0, std::sync::atomic::Ordering::Relaxed), n)).collect();
    let rest = total.saturating_sub(all.iter().map(|(ns, _)| ns).sum());
    all.sort_by(|a, b| b.0.cmp(&a.0));
    let ms = |ns: u64| ns as f64 / 1e6 / frames.max(1) as f64;
    let top: Vec<String> = all.iter().take(3).map(|(ns, n)| format!("{n} {:.2}", ms(*ns))).collect();
    format!("{}, the rest {:.2}", top.join(", "), ms(rest))
}

fn run_phase(phase_id: u32, dt: f32) {
    let mut guard = STATE.lock().unwrap();
    let Some(state) = guard.as_mut() else { return };
    if phase_id == phase::FRAME_BEGIN {
        sub_timed(13, game::track_player);
        state.frames += 1;
        state.frames_in_tick += 1;
        state.since_tick += dt;
        state.clock += dt;
        state.tick = state.since_tick >= 1.0;
        // (IMPL_ER8) the control file is also looked at once a second of REAL time: while `menu.pause` holds the game's
        // speed at 0 the tick above never comes, and a test could not reach the open menu through the file.
        let real_due = {
            static AT: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);
            let mut at = AT.lock().unwrap_or_else(|e| e.into_inner());
            let due = at.is_none_or(|t| t.elapsed().as_secs_f32() >= 1.0);
            if due || state.tick {
                *at = Some(std::time::Instant::now());
            }
            due
        };
        if (state.tick || real_due) && state.control.poll() {
            if let Some(notice) = state.control.source_notice() {
                erlog!("{notice}"); // (INI) kh-er.ini made (or removed) while the game runs
            }
            erlog!("control: {}", state.control.dump());
            kh_er_render::set_options(render_options(&state.control));
        }
        sub_timed(0, || crate::save::set_dir(&state.root, &state.control)); // (IMPL_ER13)
        sub_timed(1, || crate::savegate::frame(&state.control, dt)); // (savegate)
        sub_timed(2, || crate::slots::frame()); // (IMPL_ER13) a save into a slot is followed to its end
        sub_timed(3, || crate::gracelock::frame(&state.control)); // (SV) the game's grace actions switched off at their source
        sub_timed(3, || crate::telescope::frame(&state.control)); // (TELESCOPE) the game's Birdseye Telescopes switched off at their source
        sub_timed(4, || crate::erkeyui::frame(&state.control)); // (HUD2, ERUI) Elden Ring's banners and item-get UI off (hud.er_banners / hud.er_item_ui = 1: shown)
        sub_timed(5, || crate::slots::aids(&state.control)); // (IMPL_ER13) dbg.slots / dbg.saveto
        sub_timed(6, || crate::pause::frame(&state.control, state.tick)); // (IMPL_ER11)
        sub_timed(7, || crate::title::watch(state.control.on("dbg.title.watch"))); // (IMPL_ER12)
        sub_timed(8, || crate::tutorial::frame(&state.control)); // (IMPL_ER13)
        // (IMPL_ER12) the running scene's models: ONLY with `cut.actor = 1` (OFF by default: the scene's Tarnished is not
        // one of these models, see IMPL_ER12_STATE.md; the look at a new model costs tens of milliseconds)
        if state.control.on("cut.actor") {
            crate::remo::refresh();
        }
        sub_timed(9, || crate::probe::frame(&state.control)); // (IMPL_ER16) dbg.census / dbg.peek / dbg.poke / dbg.hold
        sub_timed(10, || crate::newgame::frame(&state.control, &state.root)); // (IMPL_ER11)
        sub_timed(11, || crate::titleload::frame(&state.control, &state.root)); // (IMPL_ER15)
        sub_timed(12, || crate::fx::trace_frame(&state.control, &crate::log::dir())); // (IMPL_ER14) snd.trace / snd.capture
        let tick_started = std::time::Instant::now();
        if state.tick {
            // (IMPL_ER17) `r.shadow_real = 1`: his real shadow in the game's cascades (the capsule stand-in stays the fallback)
            if let Some(n) = state.control.i64("r.shadow_bias") {
                kh_er_render::debug_shadow_bias(n as i32);
            }
            kh_er_render::hybrid::KH_TEST.store(state.control.i64("r.hybrid_kh_test").map_or(0, |v| v.clamp(0, 1) as u32), std::sync::atomic::Ordering::Relaxed); // (RSF2 round 15, testing aid)
            kh_er_render::set_real_shadow(state.control.i64("r.shadow_real").is_none_or(|v| v != 0)); // (RSF2 round 15) ON by default
            // (IMPL_ER53) `r.early_depth = 0`: his depth only after the game's deferred stage (as before IMPL_ER53)
            kh_er_render::set_early_depth(state.control.i64("r.early_depth").is_none_or(|v| v != 0));
            // (RSF2 round 14) `r.effects_depth` (default 1): the KH characters in the game's half-size effect depth
            kh_er_render::set_party_pin_test(state.control.f32("r.party_pin_test").unwrap_or(0.0)); // (RSF2 round 16, testing aid, off by default)
            // (ZFIN) Zettaflare's finale on an Elden Ring character: `r.finale` 0 off, 1 auto (default), 2 the white-out only, 3 the
            // mask (debug); `r.finale_white` the white (x the effects' scale, default 4); `r.finale_fade` (s, default 0.3) the fade of
            // the last white shape once the crumbled body is hidden; `r.finale_over` (words, comma-separated; default
            // crate::zfinale::OVER_DEFAULT) the effect materials drawn over the silhouette (ZETTA's crumble fragments)
            kh_er_render::finale::set_mode(state.control.i64("r.finale").map_or(1, |v| v.clamp(0, 3) as u32), state.control.f32("r.finale_white").unwrap_or(0.0), state.control.f32("r.finale_fade").unwrap_or(-1.0));
            kh_er_render::finale::set_over(state.control.str("r.finale_over").unwrap_or(crate::zfinale::OVER_DEFAULT).split(',').map(str::to_string).collect());
            // (ZFIN round 9) `r.finale_display` (default 1): the beam's colour over the silhouette at the game's Present, in the test
            // level's display colours; 0 = in the late pass (through the ACES fit's inverse) as before
            kh_er_render::finale::set_display(state.control.flag("r.finale_display") != Some(false));
            // (ZDRAW) `r.zdraw` (default 1): Zettaflare's target drawn from its own model (0: the screen route only)
            crate::zdraw::set_on(state.control.flag("r.zdraw") != Some(false));
            // (RSF round 18, the user: "lights rendered through Donald"; "whatever fixes you made ... apply them to Donald and Goofy")
            // `r.effects_depth` ON by default: every KH character's depth (Sora, Donald, Goofy alike) in the game's half-size effect
            // depth, so its fire, embers, glows and light sprites behind them are hidden; 0 = off as before
            kh_er_render::set_effects_depth(state.control.i64("r.effects_depth").map_or(1, |v| v.clamp(0, 2) as u32));
            // (IMPL_ER56) `r.early_stencil = 0`: no stencil of his; `r.stencil_ref = N`: that value instead of the ground's
            // (IMPL_ER65) `r.late_sora = 0`: no late re-composite; `r.late_sora_far = M`: where what lies behind him is farther than M
            // metres (0 = everywhere he is)
            kh_er_render::set_late_sora(state.control.i64("r.late_sora").is_some_and(|v| v != 0), state.control.f32("r.late_sora_far").unwrap_or(100.0));
            kh_er_render::set_early_stencil(state.control.i64("r.early_stencil").is_none_or(|v| v != 0), state.control.i64("r.stencil_ref").map_or(-1, |v| v as i32));
            // (ER88) `r.taa_char = 0`: his stencil is the ground's again (IMPL_ER56) instead of the characters' (0x50, the game's TAA
            // class for characters: crisp in motion)
            kh_er_render::set_taa_char(state.control.i64("r.taa_char").is_none_or(|v| v != 0));
            // (ER88) `r.velocity_own = 0`: the camera's part of his motion vectors from the game's previous view-projection only (as
            // before); default: from our own record of the camera he was drawn with (needs the pack's skin_v88 shaders)
            kh_er_render::taa::set_velocity_own(state.control.i64("r.velocity_own").is_none_or(|v| v != 0));
            // (RSF2) `r.level_open = 0`: the light level from the shadow at his feet, as before (every KH character)
            kh_er_render::light::set_open_level(state.control.i64("r.level_open").is_none_or(|v| v != 0));
            kh_er_render::camfade::set_inside_out(state.control.i64("r.camfade_inside").is_none_or(|v| v != 0)); // (RSF2)
            // (IMPL_ER80) `r.glow_mask = 0`: the glow of what lies behind him is blurred over him again (as before IMPL_ER80)
            kh_er_render::set_glow_mask(state.control.i64("r.glow_mask").is_none_or(|v| v != 0));
            // (IMPL_ER80) `r.late_sora_taa = 0`: the late re-composite (r.late_sora 1) after the game's TAA, as IMPL_ER65 (its
            // hair flicker); default: before the TAA, at the velocity split
            kh_er_render::set_late_sora_taa(state.control.i64("r.late_sora_taa").is_none_or(|v| v != 0));
            // (IMPL_ER80b) `r.shaft_mask = 0`: the game's light shafts are added over him again (as before IMPL_ER80b)
            kh_er_render::set_shaft_mask(state.control.i64("r.shaft_mask").is_none_or(|v| v != 0));
            // (ER87) `r.hair = 1`: the hair path (ER87 / ER91's core, fringe and 4x passes) when the pack has the outfit's hair
            // shaders. (HAIRPERF, the user 2026-10-27, after `r.hair = 0` in a close-up: "it looked fine") DEFAULT 0: the hair
            // drawn as KH3 draws it, once, its masks' DitherTemporalAA (the translation's udiv restored, sora_material_pack.js)
            // smoothed by the game's TAA. MEASURED in the close-up: the hair path ~12-17 ms of GPU (the game 25 fps), KH3's 1.8 ms
            // (58 fps). A change reloads the drawn outfit.
            kh_er_render::hair::set_on(state.control.i64("r.hair").is_some_and(|v| v != 0));
            // (ER87) `r.hair_late = 0`: the hair's soft fringe drawn right after him instead of at the velocity split (before the
            // game's TAA, after its forward stage, whose sky / far field would otherwise cover the fringe)
            kh_er_render::hair::set_late(state.control.i64("r.hair_late").is_none_or(|v| v != 0));
            // (ER87) `r.hair_late_far = M` (default 4): the late fringe only while he is nearer the camera than M metres
            kh_er_render::hair::set_late_far(state.control.f32("r.hair_late_far").unwrap_or(4.0));
            // (HAIRPERF) `r.hair_point_px = P` (default 6; 0 = never): ER91's hair coverage at one point a pixel while a centimetre
            // of him covers P pixels or more (a close-up: the four sub-pixel points cost ~10 ms of GPU there, for nothing)
            kh_er_render::hair::set_point_px(state.control.f32("r.hair_point_px").unwrap_or(6.0));
            // (CAPPERF) `r.hybrid_lod_px = P` (default 1500000; 0 = never): the hybrid's capture at its level of detail (a coarser
            // shade-ratio lattice, one KH-map search per sun filter) while the KH characters' boxes cover P pixels or more (a
            // close-up: the capture alone cost ~12 ms of GPU there); off below 0.8 P
            kh_er_render::hybrid::set_lod_px(state.control.f32("r.hybrid_lod_px").unwrap_or(kh_er_render::hybrid::LOD_PX_DEFAULT));
            // (ER91) `r.hair91 = 0`: ER87's hair files instead of ER91's (the strand mask's sampler fixed, the continuous
            // supersampled coverage); default 1 when the pack has the outfit's `*_h91` / `*_h91f`. A change reloads the outfit.
            kh_er_render::hair::set_v91(state.control.i64("r.hair91").is_none_or(|v| v != 0));
            // (ER91) `r.hair_msaa = 0`: no 4x hair pass at the split (the fringe there as before); `r.hair_msaa_far = M`
            // (default 8): the 4x pass only while he is nearer the camera than M metres
            kh_er_render::hair91::set_on(state.control.i64("r.hair_msaa").is_none_or(|v| v != 0));
            kh_er_render::hair91::set_far(state.control.f32("r.hair_msaa_far").unwrap_or(8.0));
            // (IMPL_ER78) THE HYBRID: `r.hybrid` (default 1, control.rs DEFAULTS) and its tuning keys; `r.hybrid_dump` (a CHANGE
            // after the payload started) = one dump of the game's G-buffer / light targets around him into the log folder
            kh_er_render::set_hybrid(hybrid_settings(&state.control));
            // (IMPL_ER86) THE STEADY LIGHT: `r.light_stable` (default 1) and its tuning keys (stable_settings above)
            kh_er_render::stable::set_settings(stable_settings(&state.control));
            {
                static SEEN: std::sync::Mutex<Option<Option<String>>> = std::sync::Mutex::new(None);
                let now = state.control.str("r.hybrid_dump").map(str::to_string);
                if let Ok(mut seen) = SEEN.lock() {
                    if seen.as_ref().is_some_and(|before| *before != now) && now.as_deref().is_some_and(|v| v != "0") {
                        kh_er_render::hybrid_dump();
                    }
                    *seen = Some(now);
                }
            }
            {
                static SEEN: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
                let now = state.control.str("r.shadow_dbg").map(str::to_string);
                if let Ok(mut seen) = SEEN.lock() {
                    if *seen != now {
                        if let Some(n) = now.as_deref().and_then(|v| v.parse::<u32>().ok()) {
                            kh_er_render::debug_real_shadow(n);
                        }
                        *seen = now;
                    }
                }
            }
            erlog!("{}", kh_er_render::status());
            erlog!("{}", crate::savegate::summary()); // (savegate)
            erlog!("{}", crate::tutorial::summary()); // (IMPL_ER13)
        }
        SUB[14].fetch_add(tick_started.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
        return;
    }
    crate::probe::phase(); // (IMPL_ER16) dbg.hold
    // (IMPL_ER16) the cutscene's figure of the player: looked for and held hidden twice a frame (MEASURED: 31 us a look; the
    // game never set a cleared flag again in 14 looks a frame over a whole scene)
    if phase_id == phase::PRE_PHYSICS || phase_id == phase::POST_PHYSICS {
        crate::figure::phase(&state.control, state.tick && phase_id == phase::POST_PHYSICS);
    }
    // (LANT) nothing of the hidden Tarnished is drawn while a KH character is shown in his place (his Lantern's model; its
    // light is kept): tarnfx.rs. Out of Sora's mode it only gives back what it held.
    if phase_id == phase::PRE_PHYSICS && (state.control.str("mode") != Some("sora") || game::player_age() >= 30) {
        crate::tarnfx::frame(&state.control, state.control.str("mode") == Some("sora"), state.control.flag("sora.hide") != Some(false), state.tick);
    }
    // (IMPL_ER17) the sites of grace's own flame is not drawn (once a frame, in the world)
    if phase_id == phase::PRE_PHYSICS && state.control.str("mode") == Some("sora") && game::player_age() >= 30 {
        crate::graceflame::frame(&state.control, state.tick);
        crate::gracehide::frame(&state.control); // (SV) the graces' mound models never drawn
        crate::stain::frame(&state.control); // (IMPL_ER17) the empty bloodstain of Elden Ring's own death
    }
    // (IMPL_ER11, EXPERIMENT `cut.hide = 1`) in a real-time cutscene the player's model is hidden at the model level in
    // EVERY phase the loader forwards (the scene's own acts may write its display state after the physics phases)
    if state.control.on("cut.hide") && crate::hud::in_cutscene() {
        if let Some(player) = game::player_mut() {
            let n = passive::apply(player, &Switches { render: Some(false), parts: Some(false), transparency: Some(0.0), ..Switches::default() });
            if n > 0 && state.tick {
                erlog!("cutscene: phase {phase_id}: {n} display value(s) of the player's model had been set again by the game");
            }
        }
    }
    // (IMPL_ER12, EXPERIMENT `cut.viewer = all | k`) the cutscene's own models hidden by their display flag
    {
        let key = state.control.str("cut.viewer").map(str::trim).unwrap_or("");
        let n = if key.is_empty() { if state.control.on("cut.actor") { crate::remo::hide_player_models(crate::hud::in_cutscene()) } else { 0 } } else { crate::remo::hide_models(key.parse::<usize>().ok(), key != "0off") };
        if n > 0 && state.tick {
            erlog!("cutscene: {n} model(s) of the scene are held hidden (cut.viewer = {key})");
        }
    }
    // (IMPL_ER12, EXPERIMENT `cut.scale = f`) the player's model scale: does the figure a cutscene draws follow it?
    if let Some(f) = state.control.f32("cut.scale") {
        if let Some(player) = game::player_mut() {
            let ctrl = player.chr_ins.chr_ctrl.as_mut();
            ctrl.scale_size_x = f;
            ctrl.scale_size_y = f;
            ctrl.scale_size_z = f;
        }
    }
    hud(state);
    crate::hud::er_phase(&state.control); // (hud) Elden Ring's HUD under `hud.er = 0`, the menu's pause, the HUD going with Sora
    crate::save::er_phase(); // (save)
    if phase_id == phase::POST_PHYSICS {
        world_time(state);
    }
    let mut mode = state.control.str("mode").unwrap_or("probe").to_string();
    // A dead or dying player is the game's. With Sora running he stays hidden and Sora goes on
    // being simulated (without input) and drawn until the game takes the player object away.
    let really_dead = game::player_mut().is_some_and(|p| !game::player_alive(p));
    // (IMPL_ER10) A DEATH OF SORA'S THAT WAS HANDED TO THE GAME stays the game's until it has put its player
    // somewhere (a respawn replaces the player object; THE TUTORIAL'S SCRIPTED DEATH DOES NOT: the same body is alive
    // again at 1 HP, plays its death, is warped by the cutscene): nothing is written to the body meanwhile, and a
    // NEW Sora starts where the game puts it (he then waits for the ground there: sora.rs `follow_phase`).
    let sent = game::player_mut().is_some() && state.sora.as_ref().is_some_and(|h| h.death_sent());
    if sent && !really_dead && phase_id == phase::PRE_PHYSICS {
        if let Some(player) = game::player_mut() {
            let at = game::v3(&player.chr_ins.modules.physics.position);
            let moved = state.dead_body.map_or(Vec3::ZERO, |before| at - before);
            state.dead_body = Some(at);
            state.dead_alive_for += dt;
            let warped = moved.length() > 4.0 && !game::origin_shift(moved);
            if warped || state.dead_alive_for > 45.0 {
                erlog!("death: the game {} after the death it was handed: A NEW SORA STARTS THERE", if warped { format!("put its player somewhere else (moved by ({:.2}, {:.2}, {:.2}))", moved.x, moved.y, moved.z) } else { "has had its player alive again for 45 s".to_string() });
                if let Some(mut host) = state.sora.take() {
                    host.stop(player);
                }
                state.dead_body = None;
                state.dead_alive_for = 0.0;
                state.dead_told = false;
                kh_er_render::clear();
                return;
            }
        }
    }
    if !sent {
        state.dead_body = None;
        state.dead_alive_for = 0.0;
    }
    let dead = really_dead || sent;
    if dead && mode == "sora" && state.sora.is_some() {
        if !state.dead_told {
            // (DTH) a scripted death keeps the game's no-death switch until its scene (sora.rs `death_dead_frame`)
            let holds = state.sora.as_ref().is_some_and(|h| h.death_holds_body());
            erlog!("the player died: nothing more is written to him; Sora stays (no input) until the respawn{}", if holds { " (a scripted death: the body is held at the event's HP until the game's scene)" } else { "" });
            if !holds {
                crate::sora::no_dead_release(); // (IMPL_ER23A)
            }
            state.dead_told = true;
            // (IMPL_ER15) ... nor after the game-over menu's Retry (Elden Ring's own respawn, the world kept)
            let scripted = state.sora.as_ref().is_some_and(|h| h.scripted_sent()); // (DTH) the kind latched at the leader's KO
            if !crate::hud::scripted_death() && !scripted && !crate::hud::take_retry_respawn() {
                crate::savegate::death_of_the_games(); // (savegate)
            } // `save.points_only`: the last save is loaded again after it
        }
        if let Some(player) = game::player_mut() {
            passive::apply(player, &Switches { render: Some(false), ..Switches::default() });
        }
        let host = state.sora.as_mut().unwrap();
        match phase_id {
            phase::PRE_PHYSICS => {
                // (prize) the wallet's dead frame: with the mirror the game's counter is held at 0
                if let Some(player) = game::player_mut() {
                    host.prize_dead(player, &state.control, dt);
                    host.death_dead_frame(player, &state.control, dt); // (DTH) the scripted death's black, its switch, its fallback
                }
                host.dead_phase(dt, &state.control, true)
            }
            phase::POST_PHYSICS => {
                host.dead_phase(dt, &state.control, false);
                report(state, dt);
            }
            _ => {}
        }
        return;
    }
    state.dead_told = false;
    if dead {
        if state.original.is_some() {
            erlog!("the player died: releasing him");
        }
        mode = "probe".to_string();
    }
    if mode != "passive" && mode != "sora" && (state.original.is_some() || state.sora.is_some()) {
        release(state);
    }
    match (mode.as_str(), phase_id) {
        ("passive", phase::PRE_PHYSICS) => passive_pre(state, dt),
        ("passive", phase::POST_PHYSICS) => {
            passive_post(state, dt);
            report(state, dt);
        }
        ("cast", phase::PRE_PHYSICS) => cast_probe(state),
        ("sora", phase::PRE_PHYSICS) => sora_phase(state, dt, true),
        ("sora", phase::POST_PHYSICS) => {
            sora_phase(state, dt, false);
            report(state, dt);
        }
        (_, phase::POST_PHYSICS) => report(state, dt),
        _ => {}
    }
}

/// `hud.hide = 1`: Elden Ring's own HUD (bars, equipment, compass) is hidden: the game writes the
/// state again whenever it likes, so it is re-applied in every phase we run in. Without the key
/// (the default) nothing is touched; when the key goes away the game's default state is put back once.
fn hud(state: &mut State) {
    use eldenring::cs::{CSFeManHudState, CSFeManImp};
    use fromsoftware_shared::FromStatic;
    let hide = state.control.flag("hud.hide") == Some(true);
    if !hide && !state.hud_hidden {
        return;
    }
    // SAFETY: game thread (a task phase); the singleton lives as long as the game.
    let Ok(fe) = (unsafe { CSFeManImp::instance_mut() }) else { return };
    if hide {
        if fe.hud_state != CSFeManHudState::HideAll {
            fe.hud_state = CSFeManHudState::HideAll;
        }
        if !state.hud_hidden {
            erlog!("hud: Elden Ring's HUD is held hidden (hud.hide = 1)");
        }
        state.hud_hidden = true;
    } else {
        fe.hud_state = CSFeManHudState::Default;
        state.hud_hidden = false;
        erlog!("hud: Elden Ring's HUD is given back");
    }
}

/// The world clock (testing aid; the mod save only): `time.set = H` or `H:M` asks the game for
/// that time of day once per change of the value (the SDK's `WorldAreaTime::request_time`: the
/// game's own target-time fields, as its "pass time" at a site of grace uses them);
/// `time.rate = x` writes the clock's passage multiplier while the key is there. The clock is
/// logged once a second.
fn world_time(state: &mut State) {
    use eldenring::cs::WorldAreaTime;
    use fromsoftware_shared::FromStatic;
    // SAFETY: game thread (a task phase); the singleton lives as long as the game.
    let Ok(time) = (unsafe { WorldAreaTime::instance_mut() }) else { return };
    let wanted = state.control.str("time.set").map(str::to_string);
    if wanted != state.time_set {
        if let Some(text) = wanted.as_deref() {
            let mut parts = text.split(':').map(|p| p.trim().parse::<f32>().ok());
            if let Some(Some(hours)) = parts.next() {
                let minutes = parts.next().flatten().unwrap_or(hours.fract() * 60.0);
                let (h, m) = ((hours.floor() as u32) % 24, (minutes.floor() as u32) % 60);
                time.request_time(h, m, 0);
                erlog!("time: asked the game for {h:02}:{m:02} (it was {:02}:{:02})", time.clock.hours(), time.clock.minutes());
            }
        }
        state.time_set = wanted;
    }
    if let Some(rate) = state.control.f32("time.rate") {
        time.time_passage_multiplier = rate;
    }
    if state.tick {
        erlog!("time: {:02}:{:02}:{:02} (target {:02}:{:02}, passage x{:.1})", time.clock.hours(), time.clock.minutes(), time.clock.seconds(), time.target_hour, time.target_minute, time.time_passage_multiplier);
    }
}

/// A jump of the player's position that we did not write: the physics origin moved (or the game
/// teleported him). Returns the jump.
fn origin_shift(state: &mut State, now: Vec3, what: &str) -> Option<Vec3> {
    let last = state.last_seen?;
    let delta = now - last;
    if delta.length() > 4.0 {
        erlog!("position jumped by ({:.3}, {:.3}, {:.3}) before {what}: physics origin shift or a teleport by the game", delta.x, delta.y, delta.z);
        return Some(delta);
    }
    None
}

fn passive_pre(state: &mut State, dt: f32) {
    let Some(player) = game::player_mut() else { return };
    if state.original.is_none() {
        let original = Original::capture(player);
        erlog!("player taken: original values: {}", original.describe());
        state.original = Some(original);
    }
    // Keys absent from the control file go back to the value the game had.
    let switches = state.original.as_ref().map(|o| o.as_switches()).unwrap_or_default().overlaid(&Switches::from_control(&state.control));
    state.probe.resets += passive::apply(player, &switches);

    let c = &state.control;
    if !c.on("tp") {
        state.probe.anchor = None;
        state.probe.written_pre = None;
        state.probe.written_last = None;
        return;
    }
    let now = game::v3(&player.chr_ins.modules.physics.position);
    if let Some(shift) = origin_shift(state, now, "PrePhysics") {
        if let Some(anchor) = state.probe.anchor.as_mut() {
            *anchor += shift;
        }
        state.probe.written_last = None;
    }
    let c = &state.control;
    if state.probe.anchor.is_none() || state.probe.anchor_generation != c.generation && c.on("tp.reanchor") {
        state.probe.anchor = Some(now);
        erlog!("teleport anchor ({:.3}, {:.3}, {:.3})", now.x, now.y, now.z);
    }
    state.probe.anchor_generation = c.generation;
    if let Some(last) = state.probe.written_last {
        let d = now - last;
        let (h, v) = (Vec3::new(d.x, 0.0, d.z).length(), d.y.abs());
        state.probe.fight_pre = (state.probe.fight_pre.0.max(h), state.probe.fight_pre.1.max(v));
    }
    let anchor = state.probe.anchor.unwrap();
    let (radius, period) = (c.f32("tp.radius").unwrap_or(0.0), c.f32("tp.period").unwrap_or(4.0));
    let angle = state.clock / period.max(0.1) * std::f32::consts::TAU;
    let target = anchor
        + Vec3::new(c.f32("tp.dx").unwrap_or(0.0), c.f32("tp.dy").unwrap_or(0.0), c.f32("tp.dz").unwrap_or(0.0))
        + Vec3::new(angle.cos(), 0.0, angle.sin()) * radius;
    state.probe.spin += c.f32("tp.yaw_rate").unwrap_or(0.0).to_radians() * dt;
    if c.flag("tp.pre") != Some(false) {
        write_position(player, target, c, state.probe.spin);
        state.probe.written_pre = Some(target);
        state.probe.written_last = Some(target);
        state.last_seen = Some(target);
    }
}

fn write_position(player: &mut eldenring::cs::PlayerIns, target: Vec3, c: &Control, spin: f32) {
    let chr = &mut player.chr_ins;
    {
        let physics = chr.modules.physics.as_mut();
        physics.position = game::hp(target);
        if c.on("tp.last") {
            physics.last_update_position = game::hp(target);
        }
        if c.on("tp.req") {
            physics.chr_proxy_pos_update_requested = true;
        }
        if c.f32("tp.yaw_rate").is_some() {
            let q = Quat::from_rotation_y(spin);
            physics.orientation = game::quaternion(q);
            if c.on("tp.interp") {
                physics.interpolated_orientation = game::quaternion(q);
            }
        }
    }
    if c.on("tp.sync") {
        chr.chr_ctrl.chr_proxy_flags.set_position_sync_requested(true);
        if c.f32("tp.yaw_rate").is_some() {
            chr.chr_ctrl.chr_proxy_flags.set_rotation_sync_requested(true);
        }
    }
}

fn passive_post(state: &mut State, _dt: f32) {
    let Some(player) = game::player_mut() else { return };
    // Keys absent from the control file go back to the value the game had.
    let switches = state.original.as_ref().map(|o| o.as_switches()).unwrap_or_default().overlaid(&Switches::from_control(&state.control));
    state.probe.resets += passive::apply(player, &switches);
    let now = game::v3(&player.chr_ins.modules.physics.position);
    if let Some(written) = state.probe.written_pre {
        let d = now - written;
        let (h, v) = (Vec3::new(d.x, 0.0, d.z).length(), d.y.abs());
        state.probe.fight_post = (state.probe.fight_post.0.max(h), state.probe.fight_post.1.max(v));
    }
    let c = &state.control;
    if c.on("tp") && c.on("tp.post") {
        if let Some(target) = state.probe.written_pre {
            write_position(player, target, c, state.probe.spin);
            state.probe.written_last = Some(target);
            state.last_seen = Some(target);
            return;
        }
    }
    if c.on("tp") {
        state.probe.written_last = Some(now);
    }
    state.last_seen = Some(now);
}

/// Drawing of the conventions and the once-a-second log lines.
fn report(state: &mut State, dt: f32) {
    let tick = state.tick;
    let Some(player) = game::player_mut() else {
        if tick {
            if state.had_player {
                erlog!("player gone");
                state.had_player = false;
                state.original = None;
                state.probe = Experiment::default();
                state.last_seen = None;
            }
            state.since_tick = 0.0;
            state.frames_in_tick = 0;
            state.tick = false;
        }
        return;
    };
    if !state.had_player {
        erlog!("player present");
        state.had_player = true;
    }
    let physics = &player.chr_ins.modules.physics;
    let pos = game::v3(&physics.position);
    let q = game::quat(&physics.orientation);
    let model = &player.chr_ins.chr_ctrl.model_matrix;
    let model_pos = Vec3::new(model.3 .0, model.3 .1, model.3 .2);
    if state.control.str("mode").unwrap_or("probe") != "passive" {
        if origin_shift(state, pos, "PostPhysics").is_some() {}
        state.last_seen = Some(pos);
    }

    state.seen_min = state.seen_min.min(pos);
    state.seen_max = state.seen_max.max(pos);
    {
        let ta = &player.chr_ins.modules.time_act;
        let anim = ta.anim_queue[(ta.read_idx as usize) % 10].anim_id;
        if !state.anims.contains(&anim) && state.anims.len() < 12 {
            state.anims.push(anim);
        }
    }

    if state.control.flag("draw.axes") != Some(false) {
        if let Some(mut draw) = Draw::get() {
            // A cross of the world axes in front of the camera side of the player, at chest height:
            // +X red, +Y green, +Z blue. White: the orientation's -Z (the facing, see the state note);
            // magenta: its +Z.
            let base = pos + Vec3::new(0.0, 1.2, 0.0) + q * Vec3::X * 1.0;
            for (axis, colour) in [(Vec3::X, game::RED), (Vec3::Y, game::GREEN), (Vec3::Z, game::BLUE)] {
                draw.colour(colour);
                draw.line(base, base + axis);
            }
            let chest = pos + Vec3::new(0.0, 1.9, 0.0);
            draw.colour(game::WHITE);
            draw.line(chest, chest + q * Vec3::NEG_Z * 1.5);
            draw.colour(game::MAGENTA);
            draw.line(chest, chest + q * Vec3::Z * 0.5);
            // The capsule of the physics module: position = feet, hit_height tall, hit_radius wide.
            draw.colour(game::YELLOW);
            let (h, r) = (physics.hit_height, physics.hit_radius);
            draw.capsule(pos + Vec3::Y * (h * 0.5), r, h * 0.5);
            draw.colour(game::CYAN);
            draw.cross(model_pos, 0.3);
        }
    }

    if tick {
        let fps = state.frames_in_tick as f32 / state.since_tick;
        let fall = &player.chr_ins.modules.fall;
        let data = &player.chr_ins.modules.data;
        erlog!(
            "pos ({:.3}, {:.3}, {:.3}) q*-Z ({:.3}, {:.3}, {:.3}) model-pos ({:.3}, {:.3}, {:.3}) ground {}/{} falling {} fall_timer {:.2} grav_dis {} c4_nograv {} motion {:.2} slide {}/{} hp {} dt {dt:.5} fps {fps:.1}",
            pos.x, pos.y, pos.z,
            (q * Vec3::NEG_Z).x, (q * Vec3::NEG_Z).y, (q * Vec3::NEG_Z).z,
            model_pos.x - pos.x, model_pos.y - pos.y, model_pos.z - pos.z,
            physics.standing_on_solid_ground, physics.touching_solid_ground, physics.is_falling, fall.fall_timer,
            physics.gravity_disabled, player.chr_ins.chr_flags1c4.no_gravity(), physics.motion_multiplier,
            physics.slide_info.enabled, physics.slide_info.is_sliding, data.hp,
        );
        let extent = state.seen_max - state.seen_min;
        erlog!("  this second: moved over ({:.3}, {:.3}, {:.3}); animations {:?}", extent.x, extent.y, extent.z, state.anims);
        state.seen_min = Vec3::splat(f32::MAX);
        state.seen_max = Vec3::splat(f32::MIN);
        state.anims.clear();
        if state.control.on("log.camera") {
            if let Some(cam) = game::camera() {
                erlog!(
                    "  camera right ({:.3}, {:.3}, {:.3}) up ({:.3}, {:.3}, {:.3}) forward ({:.3}, {:.3}, {:.3}) pos ({:.3}, {:.3}, {:.3}) fov {:.4} aspect {:.4} near {:.3} far {:.1}",
                    cam.right.x, cam.right.y, cam.right.z, cam.up.x, cam.up.y, cam.up.z, cam.forward.x, cam.forward.y, cam.forward.z,
                    cam.position.x, cam.position.y, cam.position.z, cam.fov, cam.aspect, cam.near, cam.far,
                );
            }
        }
        if state.original.is_some() {
            let e = &state.probe;
            erlog!(
                "  passive: game moved him pre->post by h {:.4} v {:.4}; post->pre by h {:.4} v {:.4}; values the game had reset {}",
                e.fight_post.0, e.fight_post.1, e.fight_pre.0, e.fight_pre.1, e.resets,
            );
            state.probe.fight_post = (0.0, 0.0);
            state.probe.fight_pre = (0.0, 0.0);
            state.probe.resets = 0;
        }
        state.since_tick = 0.0;
        state.frames_in_tick = 0;
        state.tick = false;
    }
}

/// `mode = cast`: what the Havok sphere cast answers around the player (logged once a second,
/// drawn every frame). Nothing is changed.
fn cast_probe(state: &mut State) {
    use crate::havok::{self, CastHit};
    if !havok::init() {
        return;
    }
    let Some(player) = game::player_mut() else { return };
    let pos = game::v3(&player.chr_ins.modules.physics.position);
    let q = game::quat(&player.chr_ins.modules.physics.orientation);
    let facing = q * crate::coords::MODEL_FORWARD;
    let c = &state.control;
    let radius = c.f32("cast.radius").unwrap_or(0.3);
    let filter = c.str("cast.filter").and_then(|s| u32::from_str_radix(s.trim_start_matches("0x"), 16).ok()).unwrap_or(havok::FILTER_CAMERA);
    let ahead = c.f32("cast.ahead").unwrap_or(2.0);
    // (name, origin, delta, radius)
    let casts: [(&str, Vec3, Vec3, f32); 6] = [
        ("down from +2 m", pos + Vec3::Y * 2.0, Vec3::NEG_Y * 4.0, radius),
        ("down, starting 0.1 m above the ground (overlapping)", pos + Vec3::Y * 0.1, Vec3::NEG_Y * 1.0, radius),
        ("down, tiny sphere", pos + Vec3::Y * 2.0, Vec3::NEG_Y * 4.0, 0.005),
        ("forward at chest height", pos + Vec3::Y * 1.0, facing * 10.0, radius),
        ("down ahead", pos + facing * ahead + Vec3::Y * 2.0, Vec3::NEG_Y * 6.0, radius),
        ("up from the overlap", pos + Vec3::Y * 0.1, Vec3::Y * 1.0, radius),
    ];
    let mut hits: Vec<CastHit> = Vec::new();
    let mut draw = Draw::get();
    for (name, origin, delta, r) in casts {
        havok::cast_sphere(origin, delta, r, filter, false, &mut hits);
        hits.sort_by(|a, b| a.fraction.total_cmp(&b.fraction));
        if let Some(draw) = draw.as_mut() {
            draw.colour(game::ORANGE);
            draw.line(origin, origin + delta);
            if let Some(h) = hits.first() {
                draw.colour(game::RED);
                draw.cross(h.position, 0.1);
                draw.colour(game::CYAN);
                draw.line(h.position, h.position + h.normal * 0.6);
            }
        }
        if state.tick {
            erlog!("cast {name}: origin ({:.3}, {:.3}, {:.3}) delta ({:.2}, {:.2}, {:.2}) r {r}: {} contacts", origin.x, origin.y, origin.z, delta.x, delta.y, delta.z, hits.len());
            for h in hits.iter().take(4) {
                let centre = origin + delta * h.fraction;
                erlog!(
                    "    fraction {:.5} pos ({:.3}, {:.3}, {:.3}) normal ({:.3}, {:.3}, {:.3}) |centre-pos| {:.4} body {} filter {:#x} owner {}",
                    h.fraction, h.position.x, h.position.y, h.position.z, h.normal.x, h.normal.y, h.normal.z, (centre - h.position).length(), h.body, h.filter, h.owner
                );
            }
        }
    }
}

/// mode = sora: the passive, hidden player driven by the simulation of Sora.
fn sora_phase(state: &mut State, dt: f32, pre: bool) {
    let Some(player) = game::player_mut() else {
        // Loading screen, respawn: Sora starts again where the game puts the player.
        crate::sora::no_dead_release(); // (IMPL_ER23A) the game's no-death switch is never left on without a Sora
        if state.sora.take().is_some() {
            erlog!("sora: the player is gone; stopped until he is back");
            state.original = None;
            kh_er_render::clear();
        }
        return;
    };
    if state.original.is_none() {
        let original = Original::capture(player);
        erlog!("player taken: original values: {}", original.describe());
        state.original = Some(original);
    }
    let hidden = state.control.flag("sora.hide") != Some(false);
    // (IMPL_ER10) in a real-time cutscene the game moves and animates its player itself: only the hiding stays
    let cutscene = state.sora.as_ref().is_some_and(|h| h.game_owns_body());
    // (IMPL_ER17) an action of the body in which the game moves it (a door pushed open, a ladder): its own movement and root
    // motion are let through, but ITS GRAVITY STAYS OFF (MEASURED: released altogether the body dropped a few cm from where Sora
    // had held it, went into its fall animation 4050 and the action was cancelled)
    let action = state.sora.as_ref().is_some_and(|h| h.body_action());
    // (IMPL_ER38, LADDER2) WHILE THE GAME LETS ITS BODY GO OFF ITS LADDER (sora.rs `body_on_ladder`: the let-go after Y at a
    // ladder) its gravity switches are LEFT AS THE GAME SETS THEM (its ladder graph drops it); `ladder.gravity = 0`: held off
    // as for the other actions.
    let ladder = state.sora.as_ref().is_some_and(|h| h.body_on_ladder()) && state.control.flag("ladder.gravity") != Some(false);
    let passive = if action { Switches { dbg_no_move: Some(false), dbg_no_action: Some(false), grav_disabled: if ladder { None } else { Some(true) }, fall_no_motion: if ladder { None } else { Some(true) }, fall_timer_zero: Some(true), slide: Some(false), render: if hidden { Some(false) } else { None }, ..Switches::default() } } else if cutscene { Switches { dbg_no_move: Some(false), dbg_no_action: Some(false), render: if hidden { Some(false) } else { None }, ..Switches::default() } } else { Switches::passive(hidden) };
    // (IMPL_ER13) !!! THE GREYED-OUT PROMPT ("they all failed, the prompt was greyed out"): MEASURED on a new character in
    // the chapel: the hidden body had been in Elden Ring's FALL animation (4050 -> 4000, the loop) for 53 s: it enters it
    // when Sora leaves the ground (a jump, a step down: the body is carried through the air), and with its gravity
    // switched off it NEVER LANDS: Elden Ring keeps it "in the air", where it greys every prompt and takes no action
    // button. With gravity given back for a moment the body landed (animation 0) and the prompt was white at once.
    // RULE: THE BODY'S GRAVITY IS OFF ONLY WHILE SORA IS IN THE AIR; while he stands on a floor the game's own gravity
    // holds its body on the ground under him (we still write its position every frame), so its fall ends when his does.
    // `body.gravity = 0`: the old behaviour (always off).
    let passive = if !cutscene && !action && state.control.flag("body.gravity") != Some(false) && state.sora.as_ref().is_some_and(|h| h.on_floor()) { Switches { grav_disabled: Some(false), ..passive } } else { passive };
    let switches = passive.overlaid(&Switches::from_control(&state.control));
    state.probe.resets += passive::apply(player, &switches);
    // THE INVARIANT: a prompt of the game's is up, Sora stands, and the body plays something that is not its idle
    // animation for 2 s: said loudly (once every 10 s), because that is the state in which the game refuses the button.
    {
        static BAD: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let ta = &player.chr_ins.modules.time_act;
        let anim = ta.anim_queue[(ta.read_idx as usize) % 10].anim_id;
        let bad = !cutscene && anim != 0 && state.sora.as_ref().is_some_and(|h| h.on_floor() && h.prompt_up());
        let n = if bad { BAD.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1 } else { BAD.swap(0, std::sync::atomic::Ordering::Relaxed) * 0 };
        if n > 0 && n % 1200 == 240 {
            erlog!("!!! INTERACTION: Elden Ring's prompt is up and Sora stands on a floor, but the hidden body has been playing animation {anim} (not its idle) for {:.0} s: the game greys its prompts and refuses the action button in that state (gravity off {}, on solid ground {} / touching {})", n as f32 / 120.0, player.chr_ins.modules.physics.gravity_disabled, player.chr_ins.modules.physics.standing_on_solid_ground, player.chr_ins.modules.physics.touching_solid_ground);
        }
    }
    if state.sora.is_none() {
        if !pre {
            return;
        }
        match crate::sora::SoraHost::start(player, &state.root, &state.control) {
            Some(host) => state.sora = Some(host),
            None => return,
        }
    }
    let host = state.sora.as_mut().unwrap();
    if pre {
        host.pre_physics(player, dt, &state.control);
    } else {
        host.post_physics(player, &state.control, state.tick);
    }
}
