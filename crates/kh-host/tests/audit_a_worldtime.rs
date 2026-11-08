//! (AUDIT-A WORLDTIME) KH3'S WORLD TIME PER FINISHER, END TO END: the recorder (`kh_host::audit_a`: the real core with the
//! level-99 loadout and the live pack, its camera layer on = kh-camera `form_bp` as kh-er cam.rs steps it) with its world
//! clock (`Session::world` = `kh_host::worldtime::WorldClock`, stepped in kh-er's order: before each step from the core's
//! per-type table, the blueprint's `SetGlobalGameSpeed` of the step before). For every slow window it logs where it began
//! and in which step / clip / clip time it ended, and checks KH3's rule:
//! - WorldSlow (0x14140E500 -> FUN_1405DA460(pawn, 1, 0x419C, 0.1)): ENEMY / GIMMICK at 0.1, Sora (PLAYER), his party
//!   (FRIEND) and the camera at 1.0, the global speed untouched;
//! - Counter Shield's hit-stops (tate_finish: B418 x0.1 in 0.08 s, back in 0.2 s; B426 x0.05 in 0.2 s, back in 0.01 s): the
//!   GLOBAL speed (Sora included);
//! - the windows: STYLECHANGE (transformation .. its end), Stun Impact (.. M_1421's first hit window), Sonic Rave (.. the
//!   first dash M_1425), Last Arcanum (.. M_1427's first hit window: count == 1), Ancient Light (.. its fifth window), the shield's grown
//!   attacks 0x55 / 0x56 (ComboSH Begin .. End), the Ultimate Finish (.. M_B425), FINISH_BZ (.. Big Bang's shot),
//!   FINISH_WA (.. the tower's first attack window, 6.2 s of its A417).
//!
//! Logs: build/sora_tmp/weapons/AUDIT-A/logs/worldtime_<name>.log. Skipped without the pack.

use glam::{Vec2, Vec3};
use kh_core::enums::ETresDamageKind;
use kh_core::states::HitOnPlayer;
use kh_core::{PadButton, StateId, TargetId};
use kh_host::audit_a::{path_in_logs, Session};
use kh_host::worldtime::WorldRates;

const DT: f32 = 1.0 / 60.0;

fn press(s: &mut Session, b: PadButton) {
    s.step(&[b]);
    s.step(&[]);
}

fn session(weapon: &str, models: &[&str], sound: &str, log: &str) -> Option<Session> {
    let mut s = Session::new(weapon, models, sound, Some(Vec3::new(250.0, 0.0, 0.0)), log)?;
    s.cams_on();
    s.world_on();
    s.idle(5);
    Some(s)
}

/// One stretch of steps at the same slow.
#[derive(Debug)]
struct Window {
    from: u64,
    to: u64,
    /// The step's state / clip / clip time where it began and where it ended (the last step of the window is the step in
    /// which the core turned it off: the rates of a row are the table as the step before left it).
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

fn windows(s: &Session, rate: impl Fn(&WorldRates) -> f32) -> Vec<Window> {
    let mut out: Vec<Window> = Vec::new();
    let mut open = false;
    for &(step, state, clip, time, r) in &s.world_rows {
        let v = rate(&r);
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

/// The log, and the rule every session holds: while the enemies are slowed Sora, his party and the camera are not.
fn report(s: &mut Session, name: &str) -> (Vec<Window>, Vec<Window>) {
    let enemy = windows(s, |r| r.enemy_type);
    let global = windows(s, |r| r.global);
    let mut text = String::new();
    for w in &enemy {
        text.push_str(&w.line("WorldSlow (ENEMY / GIMMICK)"));
        text.push('\n');
    }
    for w in &global {
        text.push_str(&w.line("global game speed (everything)"));
        text.push('\n');
    }
    for l in s.world.take_log() {
        text.push_str(&format!("clock: {l}\n"));
    }
    std::fs::write(path_in_logs(&format!("worldtime_{name}.log")), &text).ok();
    eprintln!("{name}:\n{text}");
    for &(step, _, _, _, r) in &s.world_rows {
        if r.enemy_type < 1.0 {
            assert!((r.enemy_type - 0.1).abs() < 1e-6 && (r.gimmick_type - 0.1).abs() < 1e-6, "step {step}: WorldSlow's 0.1: {r:?}");
            assert!(r.player_type == 1.0 && r.friend_type == 1.0 && r.camera_type == 1.0, "step {step}: Sora / the party / the camera not slowed: {r:?}");
        }
    }
    (enemy, global)
}

fn transform(s: &mut Session) -> i32 {
    s.rig.sora.debug_add_style(60.0);
    s.idle(2);
    s.rig.sora.debug_add_style(40.0);
    s.idle(15);
    press(s, PadButton::SITUATION);
    assert!(s.until(400, |s| s.rig.out.state == StateId::Stylechange), "STYLECHANGE starts");
    assert!(s.until(400, |s| s.rig.out.state != StateId::Stylechange), "STYLECHANGE ends");
    assert!(s.until(200, |s| s.rig.out.state == StateId::Idle), "back in IDLE");
    s.idle(5);
    s.rig.sora.combat().form.cmd
}

fn run_finish(s: &mut Session) {
    for _ in 0..60 {
        press(s, PadButton::SITUATION);
        if s.rig.out.state == StateId::Stylefinish || s.rig.sora.finish().is_some() {
            break;
        }
    }
    let mut down = false;
    for _ in 0..2000 {
        let extra = s.rig.sora.combat().situation.extra.is_some();
        down = extra && !down;
        let state = if down { s.step(&[PadButton::SITUATION]) } else { s.step(&[]) };
        if state != StateId::Stylefinish && s.rig.sora.finish().is_none() {
            break;
        }
    }
}

fn combos(s: &mut Session) {
    for _ in 0..120 {
        press(s, PadButton::ATTACK);
    }
    assert!(s.until(300, |s| s.rig.out.state == StateId::Idle), "the ground combo ends");
    s.idle(5);
    press(s, PadButton::JUMP);
    s.idle(10);
    for _ in 0..60 {
        press(s, PadButton::ATTACK);
    }
    assert!(s.until(400, |s| s.rig.out.state == StateId::Idle), "the air combo ends");
    s.idle(5);
}

/// Second Form: Stun Impact, Sonic Rave, then the level-3 finish (Last Arcanum with the Kingdom Key, Ancient Light with
/// Starlight), each after its own transformation.
fn second_form(weapon: &str, models: &[&str], sound: &str, name: &str) -> Option<Vec<Window>> {
    let mut s = session(weapon, models, sound, &format!("worldtime_{name}"))?;
    transform(&mut s);
    run_finish(&mut s);
    assert!(s.until(300, |s| s.rig.sora.combat().form.cmd == 0), "the form ends with Stun Impact");
    s.idle(120);
    transform(&mut s);
    combos(&mut s);
    if s.rig.sora.combat().form.finish_cmd != 0xAC {
        s.rig.sora.debug_add_style(100.0);
        s.idle(20);
    }
    run_finish(&mut s);
    assert!(s.until(300, |s| s.rig.sora.combat().form.cmd == 0), "the form ends with Sonic Rave");
    s.idle(120);
    transform(&mut s);
    s.rig.sora.debug_add_style(100.0);
    s.idle(3);
    s.rig.sora.debug_add_style(100.0);
    s.idle(20);
    run_finish(&mut s);
    assert!(s.until(300, |s| s.rig.sora.combat().form.cmd == 0), "the form ends with its level-3 finish");
    s.idle(120);
    let (enemy, global) = report(&mut s, name);
    assert!(global.is_empty(), "{name}: no SetGlobalGameSpeed in Second Form: {global:?}");
    Some(enemy)
}

/// Every transformation slows the world from STYLECHANGE's Begin to its end.
fn transformations(enemy: &[Window]) -> usize {
    enemy.iter().filter(|w| w.begin.0 == StateId::Stylechange).count()
}

#[test]
fn second_form_kingdom_key_stun_impact_sonic_rave_last_arcanum() {
    let Some(enemy) = second_form("WEP_KEYBLADE_SO_00", &[], "w_so010", "second_l1") else { return };
    assert_eq!(transformations(&enemy), 3, "the three transformations: {enemy:?}");
    let finishes: Vec<&Window> = enemy.iter().filter(|w| w.begin.0 == StateId::Stylefinish).collect();
    assert_eq!(finishes.len(), 3, "{enemy:?}");
    // Stun Impact: off when M_1421's first hit window opens (slot 152 sub 1 `swing_started`)
    assert_eq!(finishes[0].end.1, "1421", "{:?}", finishes[0]);
    assert!((finishes[0].end.2 - 0.7333).abs() < 1.5 * DT, "at M_1421's TresAnimNotifyState_Attack (0.733 s): {:?}", finishes[0]);
    // Sonic Rave: off as M_1425 (the first dash) starts (slot 153 sub 1 -> 5)
    assert!(matches!(finishes[1].end.1, "1424" | "1425"), "{:?}", finishes[1]);
    // Last Arcanum: off when count == 1 (slot 154 sub 1; the TresAtk begin handler 0x1413DD7D0 counts each window first): M_1427's
    // first hit window, 0.300 s
    assert!(finishes[2].end.1 == "1427" && (finishes[2].end.2 - 0.3).abs() < 1.5 * DT, "{:?}", finishes[2]);
}

#[test]
fn second_form_s_starlight_ancient_light() {
    let Some(enemy) = second_form("WEP_KEYBLADE_SO_17", &["w_so150"], "w_so150", "second_l2") else { return };
    assert_eq!(transformations(&enemy), 3, "{enemy:?}");
    let last = enemy.iter().rfind(|w| w.begin.0 == StateId::Stylefinish).expect("Ancient Light's slow");
    // Ancient Light (slot 155): off when the fifth hit window has begun (+0x12C > 4), in M_1435
    assert_eq!(last.end.1, "1435", "{last:?}");
    assert!((last.end.2 - 1.6).abs() < 1.5 * DT, "M_1435's fifth Attack window begins at 1.6 s: {last:?}");
}

/// A hit from 150 cm in front of him (audit_a_shield.rs's).
fn hit_in_front(own: Vec3, yaw: f32) -> HitOnPlayer {
    let (s, c) = yaw.to_radians().sin_cos();
    let ahead = Vec3::new(c, s, 0.0);
    HitOnPlayer { direction: -ahead, causer_yaw: yaw + 180.0, ..HitOnPlayer::new(ETresDamageKind::Small, 30.0, TargetId(1), own + ahead * 150.0) }
}

fn block(s: &mut Session) {
    let (own, yaw) = (s.rig.out.position, s.rig.out.yaw);
    s.rig.sora.take_hit(hit_in_front(own, yaw));
    for _ in 0..110 {
        s.step(&[PadButton::GUARD]);
    }
}

fn grown_attack(s: &mut Session) -> u8 {
    assert!(s.until(200, |s| matches!(s.rig.out.state, StateId::Idle | StateId::Run)), "back on his feet");
    let first = s.rig.sora.combat().history.len();
    s.step(&[PadButton::ATTACK]);
    s.step(&[]);
    s.idle(260);
    // the grown kind among what the press started
    s.rig.sora.combat().history[first.min(s.rig.sora.combat().history.len())..].iter().copied().find(|k| matches!(k, 0x55 | 0x56)).unwrap_or(0)
}

/// Counter Shield: the transformation, the grown attacks 0x55 (B418) / 0x56 (B426) with their hit-stops, the Ultimate Finish.
#[test]
fn counter_shield_grown_attacks_hit_stops_and_ultimate_finish() {
    const HERO: &[&str] = &["w_so030", "w_so036", "w_so035", "w_so032", "w_so031", "w_so033", "w_so037", "w_so034"];
    let Some(mut s) = session("WEP_KEYBLADE_SO_01", HERO, "w_so030", "worldtime_sh") else { return };
    s.rig.sora.debug_add_style(100.0);
    s.idle(15);
    s.step(&[PadButton::SITUATION]);
    assert!(s.until(400, |s| s.rig.out.state != StateId::Stylechange && s.rig.sora.combat().form.cmd == 0x89), "Counter Shield");
    s.idle(30);
    // the combo first, as audit_a_shield.rs's session (the guard's block then grows the weapon)
    for k in 0..400 {
        if k % 2 == 0 {
            s.step(&[PadButton::ATTACK]);
        } else {
            s.step(&[]);
        }
        if k > 40 && matches!(s.rig.out.state, StateId::Idle | StateId::Run) {
            break;
        }
    }
    s.idle(90);
    for _ in 0..40 {
        s.step(&[PadButton::GUARD]);
    }
    block(&mut s);
    s.idle(30);
    let a = grown_attack(&mut s);
    for _ in 0..40 {
        s.step(&[PadButton::GUARD]);
    }
    block(&mut s);
    block(&mut s);
    s.stick = Vec2::new(0.0, 1.0);
    for _ in 0..40 {
        s.step(&[PadButton::GUARD]);
    }
    s.stick = Vec2::ZERO;
    s.idle(60);
    let b = grown_attack(&mut s);
    assert_eq!((a, b), (0x55, 0x56), "the grown attacks");
    s.step(&[PadButton::SITUATION]);
    for k in 0..3000 {
        if s.rig.sora.combat().situation.extra.is_some() && k % 2 == 0 {
            s.step(&[PadButton::SITUATION]);
        } else {
            s.step(&[]);
        }
        if k > 30 && s.rig.sora.finish().is_none() && !matches!(s.rig.out.state, StateId::Stylefinish | StateId::StylefinishSh) {
            break;
        }
    }
    s.idle(120);
    assert_eq!(s.rig.sora.combat().form.cmd, 0, "the finish ends the form");
    let (enemy, global) = report(&mut s, "sh");
    assert_eq!(transformations(&enemy), 1, "{enemy:?}");
    // the grown attacks: ComboSH Begin (PlayAttack) .. End, in B418 / B426
    let grown: Vec<&Window> = enemy.iter().filter(|w| w.begin.0 == StateId::ComboSh).collect();
    assert_eq!(grown.len(), 2, "{enemy:?}");
    assert_eq!((grown[0].begin.1, grown[1].begin.1), ("B418", "B426"), "{grown:?}");
    // the Ultimate Finish: STYLEFINISH_SH from its Begin until M_B425 starts (Tick sub 3)
    let finish = enemy.iter().find(|w| w.begin.0 == StateId::StylefinishSh).expect("FINISH_SH's slow");
    assert_eq!(finish.end.0, StateId::StylefinishSh);
    assert!(s.world_rows.iter().any(|r| r.0 == finish.to + 1 && r.2 == "B425"), "off as M_B425 starts: {finish:?}");
    // the hit-stops, both in their clips (B418's BpEvent ID 2 at 0.867 s .. ID 3's end at 0.933 s of clip time; B426's ID 4
    // 2.222 .. 2.255 s: build/gen/rust/sora_anim.rs). Sora's clip runs on the slowed clock too, so the real time stretches.
    assert_eq!(global.len(), 2, "{global:?}");
    assert_eq!(global[0].begin.1, "B418");
    assert!((global[0].lowest - 0.1).abs() < 1e-4, "B418 reaches x0.1: {global:?}");
    let at = |step: u64| s.world_rows.iter().find(|r| r.0 == step).map(|r| r.4.global).unwrap_or(1.0);
    // B418's ramp: the first step 1 - 0.9 / 0.08 / 60, x0.1 five steps on; back x1 over 0.2 s (12 steps) after ID 3's end
    assert!((at(global[0].from) - (1.0 - 0.9 / 0.08 * DT)).abs() < 1e-4, "{}", at(global[0].from));
    assert!((at(global[0].from + 4) - 0.1).abs() < 1e-4);
    assert!((at(global[0].to) - (1.0 - 0.9 / 0.2 * DT)).abs() < 1e-3, "the last step of B418's ramp back: {}", at(global[0].to));
    // B426: x0.05 over 0.2 s asked at ID 4's begin, ResetGlobalGameSpeed(1, 0.01) at its end 0.033 s of (slowed) clip time on:
    // the ramp is cut after three steps at 1 - 0.95 / 0.2 x 3 / 60 = x0.7625 and is back the step after (KH3's data: the
    // window is shorter than the ramp)
    assert_eq!(global[1].begin.1, "B426");
    assert!((global[1].begin.2 - 2.2216).abs() < 2.0 * DT, "at ID 4's begin: {global:?}");
    assert!((global[1].lowest - (1.0 - 0.95 / 0.2 * 3.0 * DT)).abs() < 1e-3, "{global:?}");
    assert_eq!(global[1].to - global[1].from, 2, "{global:?}");
    // ... and both while the grown attack's WorldSlow holds: the enemies at 0.1 x the hit-stop
    for (g, w) in global.iter().zip(&grown) {
        assert!(g.from >= w.from && g.from <= w.to, "the hit-stop inside the grown attack: {g:?} {w:?}");
    }
}

/// Shooting Star's Magic Launcher: FINISH_BZ, slowed from STYLEFINISH's Begin until Big Bang's shot (0x1413DD180).
#[test]
fn shooting_star_finish_bz() {
    let Some(mut s) = session("WEP_KEYBLADE_SO_02", &["w_so020", "w_so021", "w_so022", "w_so023", "w_so024"], "w_so020", "worldtime_bz") else { return };
    for cmd in [0x80, 0x83] {
        s.rig.sora.debug_add_style(100.0);
        s.idle(15);
        for _ in 0..40 {
            s.step(&[PadButton::SITUATION]);
            if s.step(&[]) == StateId::Stylechange {
                break;
            }
        }
        for _ in 0..600 {
            if s.step(&[]) != StateId::Stylechange && s.rig.sora.combat().form.cmd == cmd {
                break;
            }
        }
        assert_eq!(s.rig.sora.combat().form.cmd, cmd);
        s.idle(30);
    }
    assert_eq!(s.rig.sora.combat().situation.slot_b.map(|e| e.kind), Some(0x9D), "FINISH_BZ on offer");
    s.step(&[PadButton::SITUATION]);
    let mut shot = None;
    for _ in 0..900 {
        let state = s.step(&[]);
        // Big Bang is the finish's only bullet (0x1413DD180 -> arrowgun.rs `fire`)
        if shot.is_none() && state == StateId::Stylefinish && !s.rig.sora.combat().arrowgun.list.is_empty() {
            shot = Some(s.rig.out.step);
        }
        if state != StateId::Stylefinish && s.rig.sora.combat().form.cmd == 0 {
            break;
        }
    }
    s.idle(60);
    let (enemy, global) = report(&mut s, "bz");
    assert!(global.is_empty(), "{global:?}");
    assert_eq!(transformations(&enemy), 2, "{enemy:?}");
    let f = enemy.iter().find(|w| w.begin.0 == StateId::Stylefinish).expect("FINISH_BZ's slow");
    assert_eq!(f.end.1, "5417", "{f:?}");
    assert_eq!(Some(f.to), shot, "off in the step of Big Bang's shot: {f:?}");
}

/// Mirage Staff: FINISH_WA, slowed until the tower's first attack window (6.2 s of its A417), not its first tick.
#[test]
fn mirage_staff_finish_wa_until_6_2_s() {
    let Some(mut s) = session("WEP_KEYBLADE_SO_04", &["w_so070", "w_so072", "w_so071", "w_so073"], "w_so070", "worldtime_wa") else { return };
    s.rig.sora.debug_add_style(100.0);
    s.idle(15);
    s.step(&[PadButton::SITUATION]);
    for _ in 0..600 {
        if s.step(&[]) != StateId::Stylechange && s.rig.sora.combat().form.cmd == 0x88 {
            break;
        }
    }
    assert_eq!(s.rig.sora.combat().form.cmd, 0x88);
    s.idle(10);
    let first = s.rig.sora.combat().history.len();
    for k in 0..900 {
        let done = s.rig.sora.combat().history[first..].contains(&0x87);
        let state = if k % 2 == 0 && !done { s.step(&[PadButton::ATTACK]) } else { s.step(&[]) };
        if k > 30 && matches!(state, StateId::Idle | StateId::Run | StateId::Jump) {
            break;
        }
    }
    s.idle(30);
    s.rig.sora.force_battle(true);
    for _ in 0..4 {
        s.step(&[PadButton::GUARD]);
        s.idle(45);
    }
    for k in 0..900 {
        let state = s.step(&[PadButton::ATTACK]);
        if k > 30 && matches!(state, StateId::Idle | StateId::Run) {
            break;
        }
    }
    s.idle(40);
    assert_eq!(s.rig.sora.combat().situation.slot_b.map(|e| e.kind), Some(0xA2), "FINISH_WA on offer");
    s.step(&[PadButton::SITUATION]);
    let mut tower: Vec<(u64, f32)> = Vec::new();
    for _ in 0..1500 {
        let state = s.step(&[]);
        if let Some(t) = kh_core::combat::forms::staff::tower(s.rig.sora.finish(), s.rig.sora.combat()) {
            tower.push((s.rig.out.step, t.time));
        }
        if state != StateId::Stylefinish && s.rig.sora.combat().form.cmd == 0 {
            break;
        }
    }
    s.idle(60);
    let (enemy, global) = report(&mut s, "wa");
    assert!(global.is_empty(), "{global:?}");
    let f = enemy.iter().find(|w| w.begin.0 == StateId::Stylefinish).expect("FINISH_WA's slow");
    let at = tower.iter().find(|t| t.0 == f.to).map(|t| t.1).expect("the tower runs when the slow ends");
    eprintln!("FINISH_WA: the slow ends at the tower's {at:.3} s");
    assert!((at - 6.2).abs() < 2.0 * DT, "the world slow ends at the tower's 6.2 s: {at} ({f:?})");
}
