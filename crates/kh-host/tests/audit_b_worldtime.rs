//! (AUDIT-B W-HANDOVER) KH3'S WORLD TIME AND DARK SCREEN FOR AUDIT-B'S FORMS, END TO END: the recorder (`kh_host::audit_a`:
//! the real core, the level-99 loadout, the live pack, its camera layer) with AUDIT-A's world clock (`Session::world` =
//! `kh_host::worldtime::WorldClock`, kh-er's order). Per keyblade: both transformations and the finisher; every slow window is
//! logged (where it began, in which step / state / clip / clip time it ended) and checked against KH3's rule:
//! - WorldSlow 0x14140E500 (on: FUN_1405DA460(pawn, 1, 0x419C, 0.1)): ENEMY / GIMMICK x0.1; Sora (PLAYER), the party (FRIEND) and the
//!   camera x1; the global speed untouched (no AUDIT-B blueprint calls SetGlobalGameSpeed; the attack rows have no hit-stop);
//! - STYLECHANGE: Begin 0x1413CBA70 on .. Tick 0x1413D3AC0 / End 0x1413CE760 off; no dark screen (it never calls the command begin
//!   event, slot 103);
//! - every finish: StyleFinish Begin 0x1413CBF70 -> the command begin event (slot 103 0x14150A1F0: WorldSlow on + the dark screen
//!   0x14140DF80(ctrl, 1, 1.0, 1.0)) .. the command end event (slot 111 0x14150C440: WorldSlow off + the dark screen off), with the
//!   finish's own earlier WorldSlow off:
//!   - FINISH_DR 0x9E: Drill Punch slot 148 0x1413DF1F0 and Clock Drill's StyleFinishDR Tick 0x1413D4990 never call it: slowed
//!     for the whole finish;
//!   - FINISH_YO 0x9C: slot 149 0x1413E0CD0, at the catch of M_4420 (sub 1 -> 5);
//!   - FINISH_SK 0x9F: StyleFinishSK Tick 0x1413D6F30 sub 3, at the Chandelier's done flag (+0xB48) in M_7423;
//!   - FINISH_GM 0xA0: StyleFinishGM Tick 0x1413D5660 sub 2, at the end of M_8416 (M_8417 starts);
//!   - FINISH_FL 0xA6: StyleFinishFL Tick 0x1413D5060 sub 2, the tick after the Kraken starts (+0xB48 = 1), in M_F420.
//! - the dark screen stays on until the end event in every finish.
//!
//! Logs: build/sora_tmp/weapons/AUDIT-B/W-HANDOVER/traces/worldtime_<name>.log. Skipped without the pack.

use glam::Vec3;
use kh_core::{PadButton, StateId};
use kh_host::audit_a::{project, Session};
use kh_host::worldtime::WorldRates;

const DT: f32 = 1.0 / 60.0;

/// A run with the dark screen recorded after every step: (step, state, clip, clip time, Form::dark_screen).
struct Run {
    s: Session,
    dark: Vec<(u64, StateId, &'static str, f32, bool)>,
    marks: Vec<(u64, String)>,
}

impl Run {
    fn new(key: &str, models: &[&str], base: &str, name: &str) -> Option<Run> {
        let mut s = Session::new(key, models, base, Some(Vec3::new(250.0, 0.0, 0.0)), name)?;
        s.cams_on();
        s.world_on();
        let mut r = Run { s, dark: Vec::new(), marks: Vec::new() };
        r.idle(5);
        Some(r)
    }

    fn step(&mut self, held: &[PadButton]) -> StateId {
        let state = self.s.step(held);
        let o = &self.s.rig.out;
        let clip = o.anim.name.sequences().first().copied().unwrap_or("-");
        self.dark.push((o.step, state, clip, o.anim.time, self.s.rig.sora.combat().form.dark_screen));
        state
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.step(&[]);
        }
    }

    fn mark(&mut self, label: &str) {
        self.marks.push((self.s.rig.out.step, label.to_string()));
    }

    /// The dummy `ahead` cm in front of him, `up` cm up.
    fn place(&mut self, ahead: f32, up: f32) {
        let o = self.s.rig.out.clone();
        let a = o.yaw.to_radians();
        let at = o.position + Vec3::new(a.cos(), a.sin(), 0.0) * ahead;
        if let Some(d) = self.s.rig.dummy.as_mut() {
            d.info.position = Vec3::new(at.x, at.y, up);
            d.info.center = Vec3::new(at.x, at.y, 90.0 + up);
        }
    }

    /// The gauge full, Triangle on the offered formchange, the transformation to its end.
    fn transform(&mut self, cmd: i32) {
        self.s.rig.sora.debug_add_style(100.0);
        self.idle(20);
        assert!(self.s.rig.sora.combat().situation.has(cmd), "{cmd:#x} offered");
        self.mark(&format!("transform {cmd:#x}"));
        self.step(&[PadButton::SITUATION]);
        for _ in 0..600 {
            if self.step(&[]) != StateId::Stylechange && self.s.rig.sora.combat().form.cmd == cmd {
                self.idle(30);
                return;
            }
        }
        panic!("STYLECHANGE to {cmd:#x} did not end");
    }

    /// Triangle on the finish `cmd` (the gauge topped up until it is offered), until the finish and the form are over.
    fn finish(&mut self, cmd: i32) {
        for _ in 0..30 {
            if self.s.rig.sora.combat().situation.slot_b.map(|e| e.kind) == Some(cmd) {
                break;
            }
            self.s.rig.sora.debug_add_style(100.0);
            self.idle(10);
        }
        assert_eq!(self.s.rig.sora.combat().situation.slot_b.map(|e| e.kind), Some(cmd), "the finish {cmd:#x} on offer");
        self.mark(&format!("finish {cmd:#x}"));
        self.step(&[PadButton::SITUATION]);
        for _ in 0..1800 {
            let state = self.step(&[]);
            if !matches!(state, StateId::Stylefinish | StateId::StylefinishDr) && self.s.rig.sora.combat().form.cmd == 0 {
                break;
            }
        }
        self.mark("form ended");
        self.idle(120);
    }
}

/// One stretch of steps at the same flag / slow.
#[derive(Debug)]
struct Window {
    from: u64,
    to: u64,
    begin: (StateId, &'static str, f32),
    end: (StateId, &'static str, f32),
    lowest: f32,
}

impl Window {
    fn line(&self, what: &str) -> String {
        format!(
            "{what}: steps {} .. {} ({:.3} s) x{:.3}: began in {:?} {}@{:.3}, ended in {:?} {}@{:.3}",
            self.from,
            self.to,
            (self.to + 1 - self.from) as f32 * DT,
            self.lowest,
            self.begin.0,
            self.begin.1,
            self.begin.2,
            self.end.0,
            self.end.1,
            self.end.2
        )
    }
}

fn windows_of(rows: impl Iterator<Item = (u64, StateId, &'static str, f32, f32)>) -> Vec<Window> {
    let mut out: Vec<Window> = Vec::new();
    let mut open = false;
    for (step, state, clip, time, v) in rows {
        if v < 1.0 {
            if open {
                let w = out.last_mut().expect("open");
                w.to = step;
                w.end = (state, clip, time);
                w.lowest = w.lowest.min(v);
            } else {
                out.push(Window { from: step, to: step, begin: (state, clip, time), end: (state, clip, time), lowest: v });
                open = true;
            }
        } else {
            open = false;
        }
    }
    out
}

fn rate_windows(s: &Session, rate: impl Fn(&WorldRates) -> f32) -> Vec<Window> {
    windows_of(s.world_rows.iter().map(|&(step, state, clip, time, r)| (step, state, clip, time, rate(&r))))
}

/// The dark screen's on windows (the flag as 0 = on, so the same window rule).
fn dark_windows(r: &Run) -> Vec<Window> {
    windows_of(r.dark.iter().map(|&(step, state, clip, time, on)| (step, state, clip, time, if on { 0.0 } else { 1.0 })))
}

fn is_finish(state: StateId) -> bool {
    matches!(state, StateId::Stylefinish | StateId::StylefinishDr)
}

struct Report {
    enemy: Vec<Window>,
    dark: Vec<Window>,
}

/// The log, and the rules every session holds: while the enemies are slowed, Sora, the party and the camera are not; no global
/// speed (no hit-stop); the dark screen only in a finish.
fn report(r: &mut Run, name: &str) -> Report {
    let enemy = rate_windows(&r.s, |w| w.enemy_type);
    let global = rate_windows(&r.s, |w| w.global);
    let dark = dark_windows(r);
    let mut text = String::new();
    for (step, label) in &r.marks {
        text.push_str(&format!("# mark step {step}: {label}\n"));
    }
    for w in &enemy {
        text.push_str(&w.line("WorldSlow (ENEMY / GIMMICK)"));
        text.push('\n');
    }
    for w in &global {
        text.push_str(&w.line("global game speed (everything)"));
        text.push('\n');
    }
    for w in &dark {
        text.push_str(&w.line("dark screen (Form::dark_screen on)"));
        text.push('\n');
    }
    for l in r.s.world.take_log() {
        text.push_str(&format!("clock: {l}\n"));
    }
    let dir = project().join("build").join("sora_tmp").join("weapons").join("AUDIT-B").join("W-HANDOVER").join("traces");
    std::fs::create_dir_all(&dir).ok();
    std::fs::write(dir.join(format!("worldtime_{name}.log")), &text).ok();
    eprintln!("{name}:\n{text}");
    for &(step, _, _, _, w) in &r.s.world_rows {
        if w.enemy_type < 1.0 {
            assert!((w.enemy_type - 0.1).abs() < 1e-6 && (w.gimmick_type - 0.1).abs() < 1e-6, "step {step}: WorldSlow's 0.1: {w:?}");
            assert!(w.player_type == 1.0 && w.friend_type == 1.0 && w.camera_type == 1.0, "step {step}: Sora / the party / the camera not slowed: {w:?}");
        }
    }
    assert!(global.is_empty(), "{name}: no SetGlobalGameSpeed / hit-stop in AUDIT-B's forms: {global:?}");
    for d in &dark {
        assert!(is_finish(d.begin.0), "{name}: the dark screen begins with a finish's command begin event: {d:?}");
    }
    assert!(r.dark.iter().all(|x| !(x.4 && x.1 == StateId::Stylechange)), "{name}: no dark screen in STYLECHANGE");
    Report { enemy, dark }
}

/// Both transformations slow the world from STYLECHANGE's Begin to its end.
fn assert_transformations(rep: &Report, n: usize, name: &str) {
    let t: Vec<&Window> = rep.enemy.iter().filter(|w| w.begin.0 == StateId::Stylechange).collect();
    assert_eq!(t.len(), n, "{name}: the transformations: {:?}", rep.enemy);
    for w in t {
        assert!(w.begin.2 < 2.5 * DT, "{name}: on at STYLECHANGE's Begin: {w:?}");
    }
}

/// The finish's slow and dark windows: (slow, dark).
fn finish_windows<'a>(rep: &'a Report, name: &str) -> (&'a Window, &'a Window) {
    let slow = rep.enemy.iter().find(|w| is_finish(w.begin.0)).unwrap_or_else(|| panic!("{name}: the finish's WorldSlow: {:?}", rep.enemy));
    let dark = rep.dark.first().unwrap_or_else(|| panic!("{name}: the finish's dark screen"));
    assert!(slow.begin.2 < 2.5 * DT, "{name}: WorldSlow on at the finish's Begin (the command begin event): {slow:?}");
    assert!(dark.from.abs_diff(slow.from) <= 1, "{name}: the dark screen on with it (the same event): {dark:?} {slow:?}");
    assert_eq!(rep.dark.len(), 1, "{name}: one dark window: {:?}", rep.dark);
    (slow, dark)
}

/// The dark screen lasts to the finish's end event: its last step is the last step in the finish state.
fn assert_dark_to_the_end(r: &Run, dark: &Window, name: &str) {
    let last = r.dark.iter().filter(|x| is_finish(x.1)).map(|x| x.0).max().expect("finish rows");
    assert!(dark.to + 1 >= last && dark.to <= last + 1, "{name}: the dark screen until the end event (last finish step {last}): {dark:?}");
}

const FD: &[&str] = &["w_so050", "w_so052", "w_so051", "w_so053"];
const CT: &[&str] = &["w_so110", "w_so112", "w_so111", "w_so113", "w_so114"];
const HG: &[&str] = &["w_so060", "w_so061", "w_so062", "w_so063", "w_so064", "w_so065"];
const CS: &[&str] = &["w_so080", "w_so081", "w_so082", "w_so083", "w_so084", "w_so085", "w_so086", "w_so087", "w_so088"];
const NG: &[&str] = &["w_so170", "w_so172", "w_so173", "w_so174"];
const WF: &[&str] = &["w_so090", "w_so091", "w_so092", "w_so093", "w_so094"];

/// FINISH_DR: neither Drill Punch's slot 148 nor the Clock's StyleFinishDR Tick calls WorldSlow: slowed and dark to the end event.
fn drill(key: &str, models: &[&str], base: &str, cmds: [i32; 2], name: &str) {
    let Some(mut r) = Run::new(key, models, base, name) else { return };
    r.transform(cmds[0]);
    r.transform(cmds[1]);
    r.place(600.0, 0.0);
    r.idle(10);
    r.finish(0x9E);
    let rep = report(&mut r, name);
    assert_transformations(&rep, 2, name);
    let (slow, dark) = finish_windows(&rep, name);
    let slowed_rows = r.s.world_rows.iter().filter(|x| is_finish(x.1) && x.0 > slow.from).all(|x| x.4.enemy_type < 1.0);
    assert!(slowed_rows, "{name}: FINISH_DR slowed for the whole finish (no WorldSlow off before the end event): {slow:?}");
    let last = r.s.world_rows.iter().filter(|x| is_finish(x.1)).map(|x| x.0).max().expect("finish rows");
    assert!(slow.to + 1 >= last, "{name}: off with the end event (last finish step {last}): {slow:?}");
    assert_dark_to_the_end(&r, dark, name);
}

#[test]
fn favorite_deputy_hyper_hammer_drill_punch_finish_dr() {
    drill("WEP_KEYBLADE_SO_03", FD, "w_so050", [0x81, 0x84], "fd_drill_punch");
}

#[test]
fn classic_tone_boom_hammer_clock_drill_finish_dr() {
    drill("WEP_KEYBLADE_SO_11", CT, "w_so110", [0x92, 0x93], "ct_clock_drill");
}

/// A finish whose own tick turns WorldSlow off before the end: off in `clip`, the dark screen to the end event.
fn early_off(r: &mut Run, name: &str, transforms: usize, clip: &str) -> (u64, f32) {
    let rep = report(r, name);
    assert_transformations(&rep, transforms, name);
    let (slow, dark) = finish_windows(&rep, name);
    assert!(is_finish(slow.end.0) && slow.end.1 == clip, "{name}: WorldSlow off inside the finish in {clip}: {slow:?}");
    let after = r.s.world_rows.iter().filter(|x| is_finish(x.1) && x.0 > slow.to).count();
    assert!(after > 0, "{name}: the finish runs on at x1 after its WorldSlow off: {slow:?}");
    assert!(dark.to > slow.to, "{name}: the dark screen outlasts the slow (only the end event puts it off): {dark:?}");
    assert_dark_to_the_end(r, dark, name);
    (slow.to, slow.end.2)
}

/// Twin Yo-Yos' FINISH_YO: slot 149 0x1413E0CD0 turns it off at M_4420's catch (sub 1 -> 5).
#[test]
fn happy_gear_agile_claws_twin_yo_yos_finish_yo() {
    let name = "hg_finish_yo";
    let Some(mut r) = Run::new("WEP_KEYBLADE_SO_05", HG, "w_so060", name) else { return };
    r.transform(0x8A);
    r.place(200.0, 0.0);
    r.transform(0x82);
    r.place(250.0, 0.0);
    r.idle(10);
    r.finish(0x9C);
    let (_, t) = early_off(&mut r, name, 2, "4420");
    eprintln!("{name}: off at M_4420 {t:.3}");
}

/// Blizzard Blades' FINISH_SK: StyleFinishSK Tick 0x1413D6F30 sub 3, at the Chandelier's done flag in M_7423.
#[test]
fn crystal_snow_blizzard_claws_blades_finish_sk() {
    let name = "cs_finish_sk";
    let Some(mut r) = Run::new("WEP_KEYBLADE_SO_06", CS, "w_so080", name) else { return };
    r.transform(0x90);
    r.place(200.0, 0.0);
    r.transform(0x85);
    r.place(250.0, 0.0);
    r.idle(10);
    r.finish(0x9F);
    let (_, t) = early_off(&mut r, name, 2, "7423");
    eprintln!("{name}: off at M_7423 {t:.3}");
}

/// Nano Arms' FINISH_GM: StyleFinishGM Tick 0x1413D5660 sub 2 at M_8416's end: the next row plays M_8417.
#[test]
fn nano_gear_nano_arms_finish_gm() {
    let name = "ng_finish_gm";
    let Some(mut r) = Run::new("WEP_KEYBLADE_SO_08", NG, "w_so170", name) else { return };
    r.transform(0x86);
    r.place(250.0, 0.0);
    r.idle(10);
    r.finish(0xA0);
    let (to, _) = early_off(&mut r, name, 1, "8416");
    assert!(r.s.world_rows.iter().any(|x| x.0 == to + 1 && x.2 == "8417"), "FINISH_GM: off as M_8417 starts (step {to})");
}

/// Storm Flag's FINISH_FL (Highwind's chain): StyleFinishFL Tick 0x1413D5060 sub 2, the tick after the Kraken starts, in M_F420.
#[test]
fn wheel_of_fate_highwind_storm_flag_finish_fl() {
    let name = "wf_finish_fl";
    let Some(mut r) = Run::new("WEP_KEYBLADE_SO_09", WF, "w_so090", name) else { return };
    r.transform(0x8C);
    r.place(200.0, 0.0);
    r.transform(0x8D);
    r.place(200.0, 0.0);
    r.idle(10);
    r.finish(0xA6);
    let (_, t) = early_off(&mut r, name, 2, "F420");
    assert!(t < 4.0 * DT, "FINISH_FL: off two ticks into M_F420 (sub 2 sets +0xB48, the next tick WorldSlow off): {t:.3}");
}
