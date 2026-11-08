//! (FINISH-AUDIT2) THE EFFECT PLACEMENT CENSUS: every Keyblade's attacks through the real core and the real effect host,
//! every system of Sora's notifies measured EVERY STEP against KH3's attach object and socket (`EffectHost::
//! measure_placements`: the row's AttachType object read from the notify data, the socket on that object, the same socket
//! resolution and attach-data placement as the drawing). Per Keyblade: a ground combo, an air combo, the four shortcut
//! spells, then its forms (offer -> transformation -> combos -> the second stage where it has one -> the finisher with every
//! extra). Two passes: "before" with the weapon-object rule off (every row on his own socket of that name, as before
//! 2026-11-07), "after" with it. The census files: build/sora_tmp/weapons/FINISH-AUDIT2/census/census_<keyblade>.{before,
//! after}.tsv, the totals in census_summary.txt. The test fails on any fault of the "after" pass. Skipped without the pack.

use std::fmt::Write as _;
use std::sync::atomic::Ordering;

use glam::Vec3;
use kh_core::magic::View;
use kh_core::{Input, PadButton, PadButtons, PadState, StateId};
use kh_host::audit_a::{project, Session, PLACEMENT_TOLERANCE_CM};
use kh_host::vfx::weaponattach::OFF;

fn press(s: &mut Session, b: &[PadButton]) {
    s.step(b);
    s.step(&[]);
}

fn settle(s: &mut Session) {
    s.until(400, |s| matches!(s.rig.out.state, StateId::Idle | StateId::Run));
    s.idle(10);
}

fn combos(s: &mut Session) {
    for _ in 0..40 {
        press(s, &[PadButton::ATTACK]);
    }
    settle(s);
    press(s, &[PadButton::JUMP]);
    s.idle(8);
    for _ in 0..25 {
        press(s, &[PadButton::ATTACK]);
    }
    settle(s);
}

fn spells(s: &mut Session) {
    for face in [PadButton::JUMP, PadButton::SITUATION, PadButton::GUARD, PadButton::ATTACK] {
        s.step(&[PadButton::LeftShoulder]);
        s.step(&[PadButton::LeftShoulder, face]);
        s.step(&[PadButton::LeftShoulder]);
        s.step(&[]);
        s.idle(90);
        settle(s);
    }
}

/// Fills the gauge, takes the offer (Y) and waits out STYLECHANGE; false: no form came.
fn transform(s: &mut Session) -> bool {
    let before = s.rig.sora.combat().form.cmd;
    s.rig.sora.debug_add_style(60.0);
    s.idle(2);
    s.rig.sora.debug_add_style(40.0);
    s.idle(15);
    for _ in 0..4 {
        press(s, &[PadButton::SITUATION]);
        if s.until(30, |s| s.rig.out.state == StateId::Stylechange) {
            break;
        }
        settle(s);
    }
    if !s.until(500, |s| s.rig.out.state != StateId::Stylechange) {
        return false;
    }
    settle(s);
    s.rig.sora.combat().form.cmd != 0 && s.rig.sora.combat().form.cmd != before
}

/// Y until a finisher runs, then Y on every extra until the form ends.
fn finish(s: &mut Session) {
    s.rig.sora.debug_add_style(100.0);
    s.idle(20);
    for _ in 0..60 {
        press(s, &[PadButton::SITUATION]);
        if s.rig.sora.finish().is_some() {
            break;
        }
    }
    let mut down = false;
    for _ in 0..2400 {
        down = !down;
        if down {
            s.step(&[PadButton::SITUATION]);
        } else {
            s.step(&[]);
        }
        if s.rig.sora.finish().is_none() && s.rig.sora.combat().form.cmd == 0 {
            break;
        }
    }
    s.idle(60);
    settle(s);
}

fn models_of(keyblade_model: &str) -> Vec<String> {
    let prefix = &keyblade_model[..keyblade_model.len() - 1];
    let dir = project().join("build/sora_pack/keyblades");
    let mut out: Vec<String> = std::fs::read_dir(dir).into_iter().flatten().flatten().filter_map(|e| e.file_name().into_string().ok()).filter(|n| n.starts_with(prefix)).collect();
    out.sort();
    out
}

fn session(item: &str, model: &str, name: &str) -> Option<Session> {
    let models = models_of(model);
    let refs: Vec<&str> = models.iter().map(String::as_str).collect();
    let mut s = Session::new(item, &refs, model, Some(Vec3::new(300.0, 0.0, 0.0)), name)?;
    s.idle(5);
    combos(&mut s);
    spells(&mut s);
    // the forms: first stage, its combos, the second stage when it has one, the finisher
    if transform(&mut s) {
        combos(&mut s);
        if transform(&mut s) {
            combos(&mut s);
        }
        finish(&mut s);
    }
    s.idle(30);
    Some(s)
}

/// One step with the shotlock's view (a camera 3 m behind him, his facing): the aim locks what it sees.
fn step_view(s: &mut Session, held: &[PadButton], prev: &mut PadButtons) {
    let now = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
    let eye = s.rig.out.position + Vec3::new(-300.0, 0.0, 150.0);
    let input = Input { view: Some(View::from_camera(eye, 0.0, -10.0, 90.0, 1280, 720)), ..Input::default() }.with_pad(PadState::from_held(now, *prev));
    *prev = now;
    s.step_input(&input);
}

/// The Keyblade's shotlock: R1 held (the aim locks the dummy), Attack (the shot), the shotlock to its end.
fn shotlock_session(item: &str, model: &str, name: &str) -> Option<Session> {
    let models = models_of(model);
    let refs: Vec<&str> = models.iter().map(String::as_str).collect();
    let mut s = Session::new(item, &refs, model, Some(Vec3::new(600.0, 0.0, 0.0)), name)?;
    s.idle(5);
    let mut prev = PadButtons::NONE;
    for _ in 0..2 {
        for _ in 0..70 {
            step_view(&mut s, &[PadButton::RightShoulder], &mut prev);
        }
        step_view(&mut s, &[PadButton::RightShoulder, PadButton::ATTACK], &mut prev);
        for _ in 0..40 {
            step_view(&mut s, &[PadButton::ATTACK], &mut prev);
        }
        for _ in 0..600 {
            step_view(&mut s, &[], &mut prev);
        }
        settle(&mut s);
    }
    Some(s)
}

/// Grand magic: each shortcut element cast five times (its grand-magic tally), the gauge filled, Y: Firaza / Blizzaza /
/// Thundaza / Curaza (AwardAtMax's roll offers the element cast most), and the plain -ga casts on the way.
fn grand_session(name: &str) -> Option<Session> {
    let mut s = Session::new("WEP_KEYBLADE_SO_00", &["w_so010"], "w_so010", Some(Vec3::new(300.0, 0.0, 0.0)), name)?;
    s.idle(5);
    for face in [PadButton::JUMP, PadButton::SITUATION, PadButton::GUARD, PadButton::ATTACK] {
        for _ in 0..5 {
            s.step(&[PadButton::LeftShoulder]);
            s.step(&[PadButton::LeftShoulder, face]);
            s.step(&[PadButton::LeftShoulder]);
            s.step(&[]);
            s.idle(60);
            settle(&mut s);
            s.rig.sora.status_mut().mp = s.rig.sora.status().max_mp();
        }
        s.rig.sora.debug_add_style(100.0);
        s.idle(20);
        // the grand magic among the offers (the formchange may stand first): Left Trigger to it, then Y
        for _ in 0..6 {
            match s.rig.sora.combat().situation.current() {
                Some((_, kind)) if (110..=126).contains(&kind) || kind == 231 => break,
                Some(_) => press(&mut s, &[PadButton::SITUATION_NEXT]),
                None => break,
            }
        }
        press(&mut s, &[PadButton::SITUATION]);
        s.idle(240);
        settle(&mut s);
    }
    Some(s)
}

/// The XS finishers (Spiral Rush 0xE3, Zantetsuken 0xE4, Event Horizon 0xE5, Giant Stamp 0xE6: kh-core `debug_xs_finisher`)
/// at the end of a ground combo, with the XS effect root read.
fn xs_session(name: &str) -> Option<Session> {
    let mut s = Session::new("WEP_KEYBLADE_SO_00", &["w_so010"], "w_so010", Some(Vec3::new(300.0, 0.0, 0.0)), name)?;
    s.load_xsi_root();
    s.idle(5);
    for kind in [0xE3u8, 0xE4, 0xE5, 0xE6] {
        s.rig.sora.debug_xs_finisher(kind);
        for _ in 0..30 {
            press(&mut s, &[PadButton::ATTACK]);
        }
        settle(&mut s);
    }
    Some(s)
}

const KEYBLADES: [(&str, &str); 17] = [
    ("WEP_KEYBLADE_SO_00", "w_so010"),
    ("WEP_KEYBLADE_SO_01", "w_so030"),
    ("WEP_KEYBLADE_SO_02", "w_so020"),
    ("WEP_KEYBLADE_SO_03", "w_so050"),
    ("WEP_KEYBLADE_SO_04", "w_so070"),
    ("WEP_KEYBLADE_SO_05", "w_so060"),
    ("WEP_KEYBLADE_SO_06", "w_so080"),
    ("WEP_KEYBLADE_SO_07", "w_so040"),
    ("WEP_KEYBLADE_SO_08", "w_so170"),
    ("WEP_KEYBLADE_SO_09", "w_so090"),
    ("WEP_KEYBLADE_SO_10", "w_so140"),
    ("WEP_KEYBLADE_SO_11", "w_so110"),
    ("WEP_KEYBLADE_SO_12", "w_so200"),
    ("WEP_KEYBLADE_SO_13", "w_so210"),
    ("WEP_KEYBLADE_SO_14", "w_so190"),
    ("WEP_KEYBLADE_SO_17", "w_so150"),
    ("WEP_KEYBLADE_SO_18", "w_so121"),
];

#[test]
fn every_effect_sits_where_kh3_attaches_it() {
    if !project().join("build/sora_pack/vfx").is_dir() {
        return;
    }
    let mut summary = format!("(FINISH-AUDIT2) placement census, tolerance {PLACEMENT_TOLERANCE_CM} cm: keyblade | pass | systems measured (steps) | faults (steps) | distinct faulty rows (clip, effect, socket) | worst cm\n");
    let mut after_faults = Vec::new();
    for (pass, off) in [("before", true), ("after", false)] {
        OFF.store(off, Ordering::Relaxed);
        let mut runs: Vec<(String, Option<Session>)> = Vec::new();
        for (item, model) in KEYBLADES {
            runs.push((format!("moves {model}"), session(item, model, &format!("census_{model}"))));
            runs.push((format!("shotlock {model}"), shotlock_session(item, model, &format!("census_shot_{model}"))));
        }
        runs.push(("grand magic".to_string(), grand_session("census_grand")));
        runs.push(("XS finishers".to_string(), xs_session("census_xs")));
        for (model, s) in runs {
            let Some(mut s) = s else { return };
            s.finish();
            let faults = s.placement_faults();
            let mut rows: Vec<(&str, &str, &str)> = faults.iter().map(|c| (c.1, c.3.effect.as_str(), c.3.sent.as_str())).collect();
            rows.sort_unstable();
            rows.dedup();
            let worst = faults.iter().map(|c| c.3.off_cm()).fold(0.0f32, f32::max);
            let _ = writeln!(summary, "{model} | {pass} | {} | {} | {} | {worst:.0}", s.census.len(), faults.len(), rows.len());
            if !off {
                after_faults.extend(rows.iter().map(|r| format!("{model} {} {} {}", r.0, r.1, r.2)));
            }
        }
    }
    OFF.store(false, Ordering::Relaxed);
    std::fs::write(project().join("build/sora_tmp/weapons/FINISH-AUDIT2/census/census_summary.txt"), &summary).ok();
    assert!(after_faults.is_empty(), "{} faulty rows after the fix:\n{}\n{summary}", after_faults.len(), after_faults.join("\n"));
}
