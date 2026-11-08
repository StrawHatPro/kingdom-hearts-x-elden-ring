//! (AUDIT-A STAFF) Mirage Staff (Ever After, `form.WA` CHANGE_WA 0x88), driven through the recorder (`kh_host::audit_a`:
//! the real core, the real effect side with the keyblade root, the real audio side with the pack's routes and Ever After's
//! swing / hit sets): offer -> StyleChange10 -> the COMBO_WA combo (0x82 0x83 0x85 0x86 0x87) -> the dodge's mirror images
//! (Avatar Shift, four) -> the held beams with the images (0x80 x n, 0x81: Avatar Barrage) -> FINISH_WA (the tower, Sora on
//! X_link01_skt, the tower's own A417 notifies) -> the end of the form. Every sound / voice / weapon-SE / effect notify of
//! every clip Sora played, and of the tower's own clip, must be emitted by the seam AND resolved by the host (an effect drawn,
//! a sound asset played), judged with KH3's effect-group rule. Report: build/sora_tmp/weapons/AUDIT-A/logs/
//! audit_a_staff.report.txt (build/sora_tmp/weapons/AUDIT-A/STAFF.md). Skipped without the pack.

use glam::Vec3;
use kh_core::{PadButton, StateId};
use kh_host::audit_a::{self, Covered, Session, Want};

/// Ever After's models (the keyblade w_so070, the staff w_so072, the deform unit w_so071, the tower w_so073).
const EVER_AFTER: &[&str] = &["w_so070", "w_so072", "w_so071", "w_so073"];

struct Run {
    s: Session,
    marks: Vec<(u64, String)>,
    groups: Vec<(u64, i32)>,
}

impl Run {
    fn step(&mut self, held: &[PadButton]) -> StateId {
        let state = self.s.step(held);
        self.groups.push((self.s.rig.out.step, self.s.rig.sora.effect_group()));
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

    fn group_at(&self, step: u64) -> i32 {
        self.groups.iter().find(|(s, _)| *s == step).map_or(0, |g| g.1)
    }

    fn history(&self) -> Vec<u8> {
        self.s.rig.sora.combat().history.clone()
    }
}

/// A covered notify, judged with the effect-group rule: (pass, line).
fn judged(r: &Run, c: &Covered) -> (bool, String) {
    let base = format!("step {:5} {} @{:.3} {} {}", c.step, c.clip, c.time, c.want.kind(), c.want.seam_name());
    if let Want::Effect { group, effect, .. } = &c.want {
        let current = r.group_at(c.step);
        if !kh_core::notify::effect_group_passes(*group, *effect != "key_trail0", current) {
            return (!c.emitted, format!("{base} group {group} (play group {current}): not due -> {}", if c.emitted { "EMITTED ANYWAY" } else { "not emitted (right)" }));
        }
        return (c.ok, format!("{base} group {group} (play group {current}) -> {} {}", if c.emitted { "emitted" } else { "NOT EMITTED" }, c.resolved));
    }
    (c.ok, format!("{base} -> {} {}", if c.emitted { "emitted" } else { "NOT EMITTED" }, c.resolved))
}

/// (AUDIT-A STAFF) A `wnotify` row of the staff's sheet (the staff's / transform unit's / tower's OWN clip notify, AUDIT-B's
/// seam), judged against the session: in the step whose notify window of Sora's `clip` passes the row's time, the seam
/// spawned its effect on its `kb:` anchor and the effect side drew it, or played its sound and the audio side resolved it.
/// None: Sora never played that clip in this session.
fn judged_wnotify(s: &Session, row: &kh_core::combat::forms::staff::FormStaffRow) -> Option<(bool, String)> {
    let t = row.values.first().copied().unwrap_or(0.0);
    let anchor = row.flags.first().copied().unwrap_or("");
    let plays: Vec<&audit_a::StepRow> = s.rows.iter().filter(|r| r.clip == row.clip && r.form_cmd == 0x88).collect();
    if plays.is_empty() {
        return None;
    }
    let due: Vec<&audit_a::StepRow> = plays.iter().copied().filter(|r| (t > r.window.0 && t <= r.window.1) || (r.window.0 < 0.0 && t == 0.0)).collect();
    let base = format!("{} {}@{t:.4} {anchor}", row.id, row.clip);
    if due.is_empty() {
        let first = plays.first().map(|r| format!("first step {} window {:?}", r.step, r.window)).unwrap_or_default();
        return Some((false, format!("{base} -> NO STEP passes its time ({first})")));
    }
    let mut lines = Vec::new();
    let mut ok = true;
    for r in due {
        if row.code == 0 {
            let name = row.effects.first().copied().unwrap_or("").to_lowercase();
            let hit = r.effects.iter().find(|e| e.stop.is_none() && e.name == name && e.attach.contains(anchor));
            match hit {
                Some(e) if e.drawn > 0 => lines.push(format!("step {} emitted drawn {} at actor {:?} (anchor point {:?})", r.step, e.drawn, e.at_actor, e.socket_actor)),
                Some(e) => {
                    ok = false;
                    lines.push(format!("step {} emitted, NOT DRAWN {:?}", r.step, e.notes));
                }
                None => {
                    ok = false;
                    lines.push(format!("step {} NOT EMITTED", r.step));
                }
            }
        } else {
            let name = row.sounds.first().copied().unwrap_or("");
            let emitted = r.sounds.iter().any(|x| x.stop.is_none() && x.name == name);
            let short = name.rsplit(['/', '.']).next().unwrap_or(name);
            let heard = r.audio.iter().find(|a| (a.0 == "play" || a.0 == "random-voice") && (a.1.contains(short) || a.1.contains(name)));
            match (emitted, heard) {
                (true, Some(a)) => lines.push(format!("step {} emitted, {} {} {}", r.step, a.0, a.2, a.3)),
                (true, None) => {
                    ok = false;
                    lines.push(format!("step {} emitted, NOT RESOLVED {:?}", r.step, r.audio));
                }
                (false, _) => {
                    ok = false;
                    lines.push(format!("step {} NOT EMITTED", r.step));
                }
            }
        }
    }
    Some((ok, format!("{base} -> {}", lines.join("; "))))
}

/// The own-clip rows of the session, every one PASS (rows of clips Sora did not play are listed as such: the test that
/// plays them judges them).
fn wnotify_report(s: &Session, text: &mut String, failed: &mut Vec<String>) -> usize {
    text.push_str("# the staff's / transform unit's / tower's own clip notifies (wnotify rows, AUDIT-B's seam)\n");
    let mut judged = 0;
    for row in kh_core::combat::forms::staff::FORM_STAFF.iter().filter(|r| r.what == "wnotify") {
        match judged_wnotify(s, row) {
            Some((ok, line)) => {
                judged += 1;
                text.push_str(&format!("{} {line}\n", if ok { "PASS" } else { "FAIL" }));
                if !ok {
                    failed.push(line);
                }
            }
            None => text.push_str(&format!("---- {} {}: clip not played in this session\n", row.id, row.clip)),
        }
    }
    judged
}

#[test]
fn mirage_staff_offer_to_end_every_notify() {
    let Some(s) = Session::new("WEP_KEYBLADE_SO_04", EVER_AFTER, "w_so070", Some(Vec3::new(200.0, 0.0, 0.0)), "audit_a_staff") else { return };
    let mut r = Run { s, marks: Vec::new(), groups: Vec::new() };
    r.idle(5);
    // ---- the offer and the transformation (StyleChange10, wear form 2)
    r.s.rig.sora.debug_add_style(100.0);
    r.idle(15);
    assert_eq!(r.s.rig.sora.combat().situation.list.first().map(|e| e.kind), Some(0x88), "CHANGE_WA offered");
    r.mark("offer: Triangle on CHANGE_WA");
    r.step(&[PadButton::SITUATION]);
    for _ in 0..600 {
        if r.step(&[]) != StateId::Stylechange && r.s.rig.sora.combat().form.cmd == 0x88 {
            break;
        }
    }
    let f = r.s.rig.sora.combat().form;
    assert_eq!((f.cmd, f.wear_form), (0x88, 2), "Mirage Staff, wear form 2");
    r.mark("Mirage Staff on");
    r.idle(10);
    // ---- the combo: 0x82 0x83 0x85 0x86 (the braid) 0x87
    r.mark("combo");
    let first = r.history().len();
    for k in 0..900 {
        let done = r.history()[first..].contains(&0x87);
        let state = if k % 2 == 0 && !done { r.step(&[PadButton::ATTACK]) } else { r.step(&[]) };
        if k > 30 && matches!(state, StateId::Idle | StateId::Run | StateId::Jump) {
            break;
        }
    }
    r.idle(30);
    assert_eq!(r.history()[first..], [0x82, 0x83, 0x85, 0x86, 0x87], "the ground combo");
    // ---- the mirror images: the dodge without the stick in battle (Avatar Shift), four of them
    r.s.rig.sora.force_battle(true);
    r.mark("images (dodge x4)");
    for _ in 0..4 {
        r.step(&[PadButton::GUARD]);
        r.idle(45);
    }
    assert_eq!(r.s.rig.sora.combat().staff.avatars.list.len(), 4, "four images");
    // ---- the held beams with the images (Avatar Barrage): 0x80 x n, then 0x81
    r.mark("held beams");
    let first = r.history().len();
    for k in 0..900 {
        let state = r.step(&[PadButton::ATTACK]);
        if k > 30 && matches!(state, StateId::Idle | StateId::Run) {
            break;
        }
    }
    let kinds = r.history()[first..].to_vec();
    assert!(kinds.len() >= 2 && kinds.last() == Some(&0x81) && kinds[..kinds.len() - 1].iter().all(|k| *k == 0x80), "the beam chain: {kinds:x?}");
    r.idle(40);
    // ---- FINISH_WA
    assert_eq!(r.s.rig.sora.combat().situation.slot_b.map(|e| e.kind), Some(0xA2), "FINISH_WA on offer");
    r.mark("FINISH_WA");
    r.step(&[PadButton::SITUATION]);
    for _ in 0..1500 {
        let state = r.step(&[]);
        if state != StateId::Stylefinish && r.s.rig.sora.combat().form.cmd == 0 {
            break;
        }
    }
    r.mark("form ended");
    assert_eq!(r.s.rig.sora.combat().form.cmd, 0, "the finish ends the form");
    r.idle(60);
    r.s.finish();
    // ---- the report
    let cov = audit_a::coverage(&r.s.rows);
    let mut text = String::new();
    for (step, label) in &r.marks {
        text.push_str(&format!("# mark step {step}: {label}\n"));
    }
    let mut failed = Vec::new();
    text.push_str("# notifies judged with the effect-group rule\n");
    for c in &cov {
        let (ok, line) = judged(&r, c);
        text.push_str(&format!("{} {line}\n", if ok { "PASS" } else { "FAIL" }));
        if !ok {
            failed.push(line);
        }
    }
    let own = wnotify_report(&r.s, &mut text, &mut failed);
    // the staff's A416 ChangeAttachParent: the hosts hang it on kh_root for its window ((AUDIT-A PLACEMENT) the generic reader
    // kh-core combat::forms::placement::part_parent, which replaced staff::part_parent)
    let braid: Vec<(u64, Option<&str>)> = r.s.rows.iter().filter(|x| x.clip == "A416" && x.form_cmd == 0x88).map(|x| (x.step, kh_core::combat::forms::placement::part_parent(x.form_cmd, "w_so072", x.clip, x.time))).collect();
    let on = braid.iter().filter(|b| b.1 == Some("kh_root")).count();
    text.push_str(&format!("# the staff's A416 TresAnimNotifyState_ChangeAttachParent (kh_root, 0 .. 3.1 s): {on} of {} steps of A416 on kh_root\n", braid.len()));
    if braid.is_empty() || on == 0 {
        failed.push("A416: the staff is not re-parented to kh_root".to_string());
    }
    text.push_str(&audit_a::report(&r.s.rows));
    std::fs::write(audit_a::path_in_logs("audit_a_staff.report.txt"), &text).ok();
    assert!(!cov.is_empty() && own >= 28, "{own} own-clip rows judged");
    assert!(failed.is_empty(), "{} of {} notifies failed:\n{}", failed.len(), cov.len() + own, failed.join("\n"));
}

/// The dive (0x84, M_A418: the target well below, Decide `dz < -100 && dz < -dist`) and its staff SE se02007_038 (w_so072's
/// own A418), in a session of its own (the form's 20 s are spent by the first one).
#[test]
fn mirage_staff_the_dive_and_its_own_sound() {
    let Some(s) = Session::new("WEP_KEYBLADE_SO_04", EVER_AFTER, "w_so070", Some(Vec3::new(200.0, 0.0, 0.0)), "audit_a_staff_dive") else { return };
    let mut r = Run { s, marks: Vec::new(), groups: Vec::new() };
    r.idle(5);
    r.s.rig.sora.debug_add_style(100.0);
    r.idle(15);
    r.step(&[PadButton::SITUATION]);
    for _ in 0..600 {
        if r.step(&[]) != StateId::Stylechange && r.s.rig.sora.combat().form.cmd == 0x88 {
            break;
        }
    }
    r.idle(20);
    // the dummy right beside him, then a jump: at the top the target is below by more than its distance
    let out = r.s.rig.out.clone();
    let at = Vec3::new(out.position.x, out.position.y, 0.0) + Vec3::new(out.yaw.to_radians().cos(), out.yaw.to_radians().sin(), 0.0) * 60.0;
    if let Some(d) = r.s.rig.dummy.as_mut() {
        d.info.position = at;
        d.info.center = at + Vec3::Z * 90.0;
    }
    r.idle(5);
    r.mark("jump + attack: the dive");
    let first = r.history().len();
    r.step(&[PadButton::JUMP]);
    for _ in 0..24 {
        r.step(&[PadButton::JUMP]);
    }
    for k in 0..300 {
        let done = r.history()[first..].contains(&0x84);
        let state = if k % 2 == 0 && !done { r.step(&[PadButton::ATTACK]) } else { r.step(&[]) };
        if done && k > 60 && matches!(state, StateId::Idle | StateId::Run) {
            break;
        }
    }
    r.idle(30);
    r.s.finish();
    assert!(r.history()[first..].contains(&0x84), "the dive: {:x?}", &r.history()[first..]);
    let mut text = String::new();
    for (step, label) in &r.marks {
        text.push_str(&format!("# mark step {step}: {label}\n"));
    }
    let mut failed = Vec::new();
    for c in &audit_a::coverage(&r.s.rows) {
        let (ok, line) = judged(&r, c);
        text.push_str(&format!("{} {line}\n", if ok { "PASS" } else { "FAIL" }));
        if !ok {
            failed.push(line);
        }
    }
    wnotify_report(&r.s, &mut text, &mut failed);
    let dive = kh_core::combat::forms::staff::FORM_STAFF.iter().find(|x| x.id == "wnotify.w_so072.A418.0").expect("the A418 row");
    let (ok, line) = judged_wnotify(&r.s, dive).expect("A418 played");
    text.push_str(&audit_a::report(&r.s.rows));
    std::fs::write(audit_a::path_in_logs("audit_a_staff_dive.report.txt"), &text).ok();
    assert!(ok, "{line}");
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// (AUDIT-A STAFF) The mirror image IS KH3's particle: TresProjWA_Avater's only component is ParticleComp =
/// vw_so072_mrg_sora1 (Blueprints/Projectile/WA/TresProjWA_Avater: no mesh of its own), whose SQEX_SkeletalMesh emitters
/// draw Sora's body p_ex001_0 held at A411's 0.42 s (bStopAnimation) and the staff w_so072_0 in its R_wep7_skt, with
/// smt_so070_bnsn_00a. The effect side must give both meshes every step of its life.
#[test]
fn the_mirror_image_draws_soras_body_and_the_staff() {
    use kh_vfx::{Camera, DrawList, EffectDef, SpawnOptions, World, Xform};
    let path = audit_a::project().join("build/sora_pack/keyblades/vfx/effects/vw_so072_mrg_sora1.json");
    let Ok(text) = std::fs::read_to_string(&path) else { return };
    let def = EffectDef::parse(&text).expect("the image's effect");
    let mut world = World::new();
    world.register(def);
    world.spawn("vw_so072_mrg_sora1", Xform::at(Vec3::ZERO), SpawnOptions::default()).expect("spawned");
    let (mut body, mut staff, mut lines) = (0, 0, Vec::new());
    for k in 0..120 {
        world.step(1.0 / 60.0);
        let mut list = DrawList::default();
        world.draw(1.0, &Camera::DEFAULT, &mut list);
        for s in &list.skeletal {
            if k % 30 == 0 {
                lines.push(format!("step {k}: {} parent {:?} clip {:?} material {:?} params {:?} transform {:?}", s.mesh, s.parent, s.clip_at(&|_| Some(1.033)), world.material_name(s.material), s.params, s.transform));
            }
        }
        body += usize::from(list.skeletal.iter().any(|s| s.mesh == "p_ex001_0"));
        staff += usize::from(list.skeletal.iter().any(|s| s.mesh == "w_so072_0"));
    }
    std::fs::write(audit_a::path_in_logs("staff_image_probe.txt"), format!("body {body} / staff {staff} of 120 steps\n{}\n", lines.join("\n"))).ok();
    assert!(body > 0 && staff > 0, "the image's body {body} / staff {staff} steps: {lines:?}");
}
