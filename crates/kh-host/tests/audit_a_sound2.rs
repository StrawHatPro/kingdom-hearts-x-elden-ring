//! (AUDIT-A SOUND) SOUND-2: the second round of AUDIT-A's sound inbox (build/sora_tmp/weapons/AUDIT-A/SOUND_INBOX.md, 2026-11-05).
//! - ARROWGUN: the bullets' hit SE se02002_039 requests that left no audio line: KH3's SEAD start macro 1 (the same sound
//!   started <= 0.067 s ago: the new one is cancelled; bank SE/weapon/02002/se02002 sound 39 `macro_start` 1). Each such
//!   request now leaves a `silent` line naming the rule, and the recorder judges every seam play (`audit_a::judge_sounds`).
//! - STAFF (and every weapon / pawn-mounted sound): a form weapon's / pawn's own clip sound plays on its anchor
//!   (`kb:<model>:<mount>:<bone>` / `pawn:<row>`), placed by the audio side at the effect side's point of it
//!   (kh-host audio/anchors.rs). The tower's se02007_026 at its bone bigD_01, se00001_034 at its root, the voices at `base`.
//! - ARROWGUN-2's weapon-clip sounds resolve by name to KH3's assets.
//!
//! Reports: build/sora_tmp/weapons/AUDIT-A/logs/sound2_*.report.txt. Skipped without the pack.

use std::fmt::Write as _;
use std::sync::Arc;

use glam::Vec3;
use kh_audio::routes::{Context, Resolved};
use kh_audio::{Engine, Language, Listener, Pack, SeamPlay};
use kh_core::{PadButton, StateId};
use kh_host::audit_a::{self, Session};
use kh_host::vfx::Scene;

fn pack() -> Option<Pack> {
    let root = audit_a::project().join("build").join("sora_pack");
    let dir = root.join("audio");
    if !dir.join("manifest.json").is_file() {
        eprintln!("SKIPPED: no audio pack at {}", dir.display());
        return None;
    }
    let mut pack = Pack::load(&dir).expect("the audio pack");
    for extra in [root.join("xsi").join("audio"), root.join("keyblades").join("audio")] {
        pack.merge_party_root(&extra).expect("an audio root merges");
    }
    Some(pack)
}

/// KH3's rule, at the engine: se02002_039 (bank se02002 sound 39, start macro 1). One request per simulation step at most,
/// as a host gives them: [requests of the step, then one engine update]. The instance's age counts from its first update
/// (spec 2.2 A1), so a request N steps after a play sees an age of (N - 1) / 60 s: N <= 5 is cancelled (4 / 60 = 0.0667 <=
/// 0.067), N = 6 plays.
#[test]
fn the_bullet_hit_se_follows_kh3s_sead_retrigger_rule() {
    let Some(pack) = pack() else { return };
    let pack = Arc::new(pack);
    let asset = pack.asset("/Game/Sound/SE/weapon/02002/se02002_039").expect("se02002_039 packed");
    let def = pack.sound(asset.bank, asset.index).expect("its sound");
    assert_eq!((def.number, def.macro_start), (39, 1), "KH3's bank data: se02002.sab sound 39, start macro 1");
    let mut engine = Engine::new(pack.clone(), 1, Language::En);
    let listener = Listener::default();
    let mut handle = 0;
    let mut request = |engine: &mut Engine| {
        handle += 1;
        engine.seam_play(handle, &SeamPlay { name: "se02002_039", position: Vec3::new(300.0, 0.0, 90.0), volume: 1.0, pitch: 1.0, looping: false, spatial: true, anim: None }).is_some()
    };
    let update = |engine: &mut Engine| {
        engine.update(1.0 / 60.0, &listener, &|_| None);
    };
    // step 0: two bullets hit in the same step -> one plays, the second is cancelled
    let mut got = vec![(0, request(&mut engine)), (0, request(&mut engine))];
    update(&mut engine);
    // steps 1 .. 6: one request each at steps 5 and 6 (5 / 6 steps after the play)
    for step in 1..=6 {
        if step >= 5 {
            got.push((step, request(&mut engine)));
        }
        update(&mut engine);
    }
    assert_eq!(got, [(0, true), (0, false), (5, false), (6, true)], "plays per request (step, played)");
    let log = engine.take_log();
    let silent: Vec<&str> = log.iter().filter(|l| l.kind == "silent" && l.cause == "seam se02002_039").map(|l| l.note.as_str()).collect();
    assert_eq!(silent.len(), 2, "one silent line per cancelled request: {log:?}");
    for note in &silent {
        assert!(note.contains("SEAD start macro 1") && note.contains("the same sound, started <= 0.067 s ago, count >= 1: the new one is cancelled"), "{note}");
        assert!(audit_a::silent_by_kh3_rule(note), "the recorder takes it for KH3's rule: {note}");
    }
    assert_eq!(log.iter().filter(|l| l.kind == "play" && l.cause == "seam se02002_039").count(), 2);
}

/// Shooting Star's Double Arrowguns: COMBO_DW near the dummy (the ring of 8 bullets, many hits in a few steps). Every
/// se02002_039 request of the seam leaves its own audio line: played, or silent by SEAD macro 1.
#[test]
fn every_arrowgun_bullet_hit_request_has_its_own_audio_line() {
    let Some(mut s) = Session::new("WEP_KEYBLADE_SO_02", &["w_so020", "w_so021", "w_so022", "w_so023", "w_so024"], "w_so020", Some(Vec3::new(250.0, 0.0, 0.0)), "sound2_dw_hits") else { return };
    s.idle(5);
    s.rig.sora.debug_add_style(100.0);
    s.idle(15);
    for _ in 0..40 {
        s.step(&[PadButton::SITUATION]);
        if s.step(&[]) == StateId::Stylechange {
            break;
        }
    }
    for _ in 0..600 {
        if s.step(&[]) != StateId::Stylechange && s.rig.sora.combat().form.cmd == 0x80 {
            break;
        }
    }
    assert_eq!(s.rig.sora.combat().form.cmd, 0x80, "Double Arrowguns");
    s.idle(20);
    let first = s.rig.sora.combat().history.len();
    let mut attacking = false;
    for k in 0..900 {
        let press = k % 2 == 0 && (k < 2 || attacking);
        let state = if press { s.step(&[PadButton::ATTACK]) } else { s.step(&[]) };
        attacking = matches!(state, StateId::ComboDw | StateId::Attack);
        if k > 30 && !attacking && s.rig.sora.combat().history.len() > first {
            break;
        }
    }
    s.idle(60);
    s.finish();
    let judged: Vec<audit_a::SoundJudged> = audit_a::judge_sounds(&s.rows).into_iter().filter(|j| j.name.contains("se02002_039")).collect();
    let mut text = String::new();
    let (requests, played, silent) = judged.iter().fold((0, 0, 0), |a, j| (a.0 + j.requests, a.1 + j.played, a.2 + j.kh3_silent.len()));
    let _ = writeln!(text, "# se02002_039 (the bullets' hit SE): {requests} requests, {played} played, {silent} silent by KH3's SEAD rule, {} groups", judged.len());
    for j in &judged {
        let _ = writeln!(text, "{} step {:5} {}: {} request(s), {} played, silent [{}]{}", if j.ok { "PASS" } else { "FAIL" }, j.step, j.clip, j.requests, j.played, j.kh3_silent.join("; "), if j.other.is_empty() { String::new() } else { format!(", other {:?}", j.other) });
    }
    text.push_str(&audit_a::report(&s.rows));
    std::fs::write(audit_a::path_in_logs("sound2_dw_hits.report.txt"), &text).ok();
    assert!(requests > 0, "the bullets hit the dummy");
    let bad: Vec<String> = judged.iter().filter(|j| !j.ok).map(|j| format!("step {} {} req {} played {} silent {:?} other {:?}", j.step, j.name, j.requests, j.played, j.kh3_silent, j.other)).collect();
    assert!(bad.is_empty(), "requests without their own line:\n{}", bad.join("\n"));
    assert_eq!(requests, played + silent);
    assert!(judged.iter().flat_map(|j| &j.kh3_silent).all(|n| n.contains("SEAD start macro 1")), "the only silence is macro 1");
}

/// ARROWGUN-2's weapon-clip sounds (rows `wnotify.w_so021.2401.2` ... of sheets/port/sora-form-arrowgun.json, by their
/// short name as `wnotify_pass` sends them) resolve through Sora's routes, with the roots kh-er merges, to KH3's own asset
/// (`/Game/Sound/SE/weapon/<bank>/<name>`), packed and playable.
#[test]
fn the_arrowgun_weapon_clip_sounds_resolve_by_name() {
    let Some(pack) = pack() else { return };
    let routes = kh_audio::Routes::new(&pack);
    let mut checked = Vec::new();
    for row in kh_core::combat::forms::arrowgun::FORM_ARROWGUN.iter().filter(|r| r.what == "wnotify" && r.code != 0) {
        let name = row.logical; // (the sound of a code-1 row: arrowgun.rs `on_notifies`)
        let cx = Context { surface: 0, language: Language::En, voice_group_no: 0, anim: Some((row.clip, row.values.first().copied().unwrap_or(0.0))) };
        let Resolved::Asset { path, voice: false } = routes.resolve(&pack, name, &cx) else { panic!("{}: {name} does not resolve to an SE asset", row.id) };
        let bank = &name[2..7];
        assert_eq!(path.trim_end_matches(&format!(".{name}")), format!("/Game/Sound/SE/weapon/{bank}/{name}"), "{}: KH3's own asset", row.id);
        let asset = pack.asset(&path).unwrap_or_else(|| panic!("{path} packed"));
        let def = pack.sound(asset.bank, asset.index).unwrap_or_else(|| panic!("{path}: its bank sound"));
        let b = pack.bank(asset.bank);
        assert!(def.tracks.values().any(|t| b.materials.get(&t.material).is_some_and(|m| pack.dir.join(&m.file).is_file() || std::path::Path::new(&m.file).is_file())), "{path}: a material file");
        checked.push(format!("{} {} {name} -> {path} = {}#{}", row.id, row.clip, b.name, asset.index));
    }
    std::fs::write(audit_a::path_in_logs("sound2_arrowgun_clip_sounds.report.txt"), checked.join("\n") + "\n").ok();
    for want in ["se02002_023", "se02002_024", "se02002_020", "se02002_021", "se02002_025", "se02002_022", "se02004_011", "se02004_012", "se02004_014"] {
        assert!(checked.iter().any(|c| c.contains(&format!(" {want} "))), "{want} is a row of the sheet and resolves: {checked:#?}");
    }
}

/// Mirage Staff FINISH_WA: the tower's own sounds sound AT THE TOWER. Through the recorder (real core, effect side, audio
/// side): se02007_026 at the tower's bone bigD_01, se00001_034 at its root, the voices at its socket `base`, each where the
/// effect side puts that point of the tower (the same resolution its effects use), and followed while they last; not at Sora
/// (who stands 25 m up the tower).
#[test]
fn the_towers_own_sounds_sound_at_the_tower() {
    let Some(mut s) = Session::new("WEP_KEYBLADE_SO_04", &["w_so070", "w_so072", "w_so071", "w_so073"], "w_so070", Some(Vec3::new(200.0, 0.0, 0.0)), "sound2_tower") else { return };
    let history = |s: &Session| s.rig.sora.combat().history.clone();
    s.idle(5);
    s.rig.sora.debug_add_style(100.0);
    s.idle(15);
    s.step(&[PadButton::SITUATION]);
    for _ in 0..600 {
        if s.step(&[]) != StateId::Stylechange && s.rig.sora.combat().form.cmd == 0x88 {
            break;
        }
    }
    assert_eq!(s.rig.sora.combat().form.cmd, 0x88, "Mirage Staff");
    s.idle(10);
    let first = history(&s).len();
    for k in 0..900 {
        let done = history(&s)[first..].contains(&0x87);
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
    // the rows and the anchors they name (sheets/port/sora-form-staff.json)
    let rows: Vec<(&str, &str, &str)> = kh_core::combat::forms::staff::FORM_STAFF
        .iter()
        .filter(|r| r.what == "wnotify" && r.code != 0 && r.flags.first().is_some_and(|a| a.starts_with("kb:w_so073:tower:")))
        .map(|r| (r.id, r.sounds.first().copied().unwrap_or(""), r.flags[0]))
        .collect();
    assert_eq!(rows.len(), 4, "the tower's 4 sound rows: {rows:?}");
    let mut text = String::new();
    let mut started: Vec<(&str, f32)> = Vec::new(); // (row id, distance of the start from its point)
    let mut followed: Vec<(&str, f32)> = Vec::new(); // the largest distance of a live sound from its point
    let mut moved = 0.0f32; // the largest distance of a start from where the port put it before
    for _ in 0..1500 {
        let state = s.step(&[]);
        let row = s.rows.last().expect("a row").clone();
        let out = s.rig.out.clone();
        let tower = kh_core::combat::forms::staff::tower(s.rig.sora.finish(), s.rig.sora.combat());
        // the effect side's point of each anchor this step (a fresh tap of this step's pose, as kh-er's effect step has it)
        let mut tap = kh_host::audio::PoseTap::default();
        tap.record(out.step, &out, s.rig.animator.skeleton(), &s.rig.pose, &kh_host::rig::component_to_world(&out));
        let target = |_: kh_core::TargetId| None;
        let scene = Scene { out: &out, sora: &s.rig.sora, tap: &tap, target: &target, ground: Some(0.0), eye: out.position };
        for (id, name, anchor) in &rows {
            let want = s.fx.socket_point(&scene, anchor);
            if let Some(x) = row.sounds.iter().find(|x| x.stop.is_none() && x.name == *name && x.attach.contains(anchor)) {
                let at = x.placed.expect("the audio side placed it");
                let d = at.distance(want);
                let root = tower.map(|t| t.position);
                let _ = writeln!(text, "step {:5} {} {id} {name} on {anchor}: placed {at:?}, the tower's point {want:?} (d {d:.2} cm); tower root {root:?}; Sora {:?} (d {:.0} cm)", row.step, row.clip, out.position, at.distance(out.position));
                started.push((id, d));
                // where the port put it before (`Attach::Sora { bone: Some("tower") }`: no such bone -> 1 m above his origin)
                moved = moved.max(at.distance(kh_host::seam::on_actor(out.position, out.yaw, true, Vec3::ZERO)));
                if anchor.ends_with(':') {
                    let t = tower.expect("the tower stands when its clip starts");
                    assert!(at.distance(t.position) < 1.0, "{id}: the root sound at the tower's root {:?}, got {at:?}", t.position);
                }
            }
            // a live 3D sound follows its point (the engine's position of the sound this step)
            let cause = format!("seam {}", name.rsplit('/').next().unwrap_or(name));
            if tower.is_some() {
                for a in s.audio.engine.active().iter().filter(|a| a.cause == cause && !a.stopping) {
                    if let Some(p) = a.position {
                        let d = p.distance(want);
                        match followed.iter_mut().find(|f| f.0 == *id) {
                            Some(f) => f.1 = f.1.max(d),
                            None => followed.push((id, d)),
                        }
                    }
                }
            }
        }
        if state != StateId::Stylefinish && s.rig.sora.combat().form.cmd == 0 {
            break;
        }
    }
    s.idle(30);
    s.finish();
    // KH3's own assets: the two AttachSounds have NO AttenuationSettings (assets_converted/packages/.../se02007_026.uasset.json,
    // se00001_034: 2D, heard alike wherever they hang); the voices have npc_attenuation (3D, at `base`)
    let pack = s.audio.engine.pack().clone();
    let spatial = |path: &str| pack.asset(path).map(|a| a.attenuation);
    let (se26, se34, v40, v70) = (
        spatial("/Game/Sound/SE/weapon/02007/se02007_026"),
        spatial("/Game/Sound/SE/common/00001_action/se00001_034"),
        spatial("/Game/Sound/VOICE/Localization/en/battle/PC/bt1010740sr0"),
        spatial("/Game/Sound/VOICE/Localization/en/battle/PC/bt1010770sr0"),
    );
    let _ = writeln!(text, "# 3D by KH3's asset (AttenuationSettings): se02007_026 {se26:?}, se00001_034 {se34:?}, bt1010740 {v40:?}, bt1010770 {v70:?}");
    let _ = writeln!(text, "# followed (largest distance of a live 3D sound from its point while the tower stands): {followed:?}");
    let _ = writeln!(text, "# the largest distance of a start from where the port put it before (1 m above Sora): {moved:.0} cm");
    text.push_str(&audit_a::report(&s.rows));
    std::fs::write(audit_a::path_in_logs("sound2_tower.report.txt"), &text).ok();
    for (id, _, _) in &rows {
        assert!(started.iter().any(|x| x.0 == *id), "{id}: started in the finish ({started:?})");
    }
    assert!(started.iter().all(|x| x.1 < 1.0), "every start at its point of the tower: {started:?}");
    assert_eq!((se26, se34, v40, v70), (Some(false), Some(false), Some(true), Some(true)), "KH3's attenuation of the four");
    assert!(followed.iter().any(|x| x.0.ends_with(".voice3")), "the 3D voice was followed: {followed:?}");
    assert!(followed.iter().all(|x| x.1 < 1.0), "every live 3D sound follows its point: {followed:?}");
    assert!(moved > 300.0, "the tower's points are not where the port put these before: {moved} cm");
}
