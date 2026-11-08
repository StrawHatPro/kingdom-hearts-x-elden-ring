//! (AUDIT-B W-VFX) DRILL PUNCH'S ROPE, END TO END: the real core plays FINISH_DR (Favorite Deputy, form 0x84) -> the effect
//! seam -> `kh_host::vfx::EffectHost` with the live pack: the drill's own M_6421 / M_6422 / M_6423 rows spawn
//! vw_so053_bam_lin0 on himo01..15 with their EndSocketName (`kb:w_so053:R_ude2_wep1_skt:himo<k>:tohimo<k+1>`), the host
//! resolves both sockets on w_so053 every step (KH3's TypeDataSQEX_Thunder tick 0x144250690) and kh-vfx draws each as a
//! strip of smi_lne_00k between them. The trace goes to build/sora_tmp/weapons/AUDIT-B/W-VFX/traces/. Skipped without the pack.

use std::fmt::Write as _;

use glam::Vec3;
use kh_core::{PadButton, StateId};
use kh_host::audit_a::{self, Session};

fn traces() -> std::path::PathBuf {
    let dir = audit_a::project().join("build").join("sora_tmp").join("weapons").join("AUDIT-B").join("W-VFX").join("traces");
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn transform(s: &mut Session, cmd: i32) {
    s.rig.sora.debug_add_style(100.0);
    s.idle(20);
    assert!(s.rig.sora.combat().situation.has(cmd), "{cmd:#x} offered");
    s.step(&[PadButton::SITUATION]);
    for _ in 0..600 {
        if s.step(&[]) != StateId::Stylechange && s.rig.sora.combat().form.cmd == cmd {
            return;
        }
    }
    panic!("STYLECHANGE to {cmd:#x} did not end");
}

fn combo(s: &mut Session) {
    for k in 0..1200 {
        let state = if k % 2 == 0 { s.step(&[PadButton::ATTACK]) } else { s.step(&[]) };
        if k > 30 && matches!(state, StateId::Idle | StateId::Run) {
            break;
        }
    }
    s.idle(30);
}

#[test]
fn drill_punch_finish_draws_the_rope_between_the_drills_himo_sockets() {
    let Some(mut s) = Session::new("WEP_KEYBLADE_SO_03", &["w_so050", "w_so052", "w_so051", "w_so053"], "w_so050", Some(Vec3::new(200.0, 0.0, 0.0)), "auditb_wvfx_thunder") else {
        eprintln!("SKIP: no pack");
        return;
    };
    let pack = audit_a::project().join("build").join("sora_pack");
    s.fx.load_xsi_root(&pack).expect("the XS effect root");
    s.idle(5);
    transform(&mut s, 0x81);
    s.idle(30);
    combo(&mut s);
    transform(&mut s, 0x84);
    s.idle(30);
    combo(&mut s);
    let o = s.rig.out.clone();
    let a = o.yaw.to_radians();
    let at = o.position + Vec3::new(a.cos(), a.sin(), 0.0) * 600.0;
    if let Some(d) = s.rig.dummy.as_mut() {
        d.info.position = Vec3::new(at.x, at.y, 0.0);
        d.info.center = Vec3::new(at.x, at.y, 90.0);
    }
    s.idle(10);
    assert_eq!(s.rig.sora.combat().situation.slot_b.map(|e| e.kind), Some(0x9E), "FINISH_DR on offer");
    s.step(&[PadButton::SITUATION]);
    let camera = kh_vfx::Camera { position: s.rig.out.position + Vec3::new(-400.0, 0.0, 150.0), forward: Vec3::X, right: Vec3::Y, up: Vec3::Z };
    let mut text = String::from("# step clip time | rope strips (smi_lne_00k ribbons): count, chain links, length cm, half width cm, colour\n");
    let (mut most, mut steps_with, mut chained, mut longest) = (0usize, 0usize, 0usize, 0.0f32);
    for _ in 0..1200 {
        let state = s.step(&[]);
        let (step, clip, time) = (s.rig.out.step, s.rig.out.anim_ticked.body_samples()[0].0, s.rig.out.anim_ticked.time);
        let (list, materials, _) = s.fx.draw(1.0, &camera);
        let rope: Vec<&kh_vfx::Ribbon> = list.ribbons.iter().filter(|r| materials.get(r.material as usize).is_some_and(|m| m.rsplit(':').next() == Some("smi_lne_00k"))).collect();
        if let Some(row) = s.rows.last() {
            for e in row.effects.iter().filter(|e| e.name.contains("bam_lin")) {
                let _ = writeln!(text, "step {step} spawn {} at {} drawn {} {}", e.name, e.attach, e.drawn, e.notes.join("; "));
            }
        }
        if !rope.is_empty() {
            steps_with += 1;
            most = most.max(rope.len());
            let ends: Vec<(Vec3, Vec3)> = rope.iter().map(|r| {
                let mid = |p: &kh_vfx::RibbonPoint| (p.a + p.b) * 0.5;
                (mid(&r.points[0]), mid(r.points.last().unwrap()))
            }).collect();
            // the segments form ONE chain: a segment's end is another one's start (himo k -> k + 1 -> k + 2 ...)
            let links = ends.iter().filter(|(_, e)| ends.iter().any(|(b, _)| b.distance(*e) < 0.01)).count();
            if rope.len() == 15 && links == 14 {
                chained += 1;
            }
            let length: f32 = ends.iter().map(|(a, b)| a.distance(*b)).sum();
            longest = longest.max(length);
            let half = (rope[0].points[0].a - rope[0].points[0].b).length() * 0.5;
            let _ = writeln!(text, "step {step} {clip} {time:.3} | {} strips, {links} links, rope {length:.1} cm, half width {half:.3}, colour {:?}", rope.len(), rope[0].points[0].color);
        }
        if !matches!(state, StateId::Stylefinish | StateId::StylefinishDr) && s.rig.sora.combat().form.cmd == 0 {
            break;
        }
    }
    let _ = writeln!(text, "# {steps_with} steps with the rope, at most {most} strips, {chained} steps with one unbroken chain, longest rope {longest:.1} cm");
    std::fs::write(traces().join("drill_rope.txt"), &text).ok();
    assert!(steps_with > 20, "the rope is drawn in M_6421 .. M_6423 ({steps_with} steps)\n{text}");
    // 15 segments himo01 .. himo16 (the rows on himo01..15 with their EndSocketName); 30 while M_6421's set plays out its
    // LOOPEND tail beside M_6423's
    assert!(most == 15 || most == 30, "{most} strips");
    assert!(chained * 2 > steps_with, "the segments join into one rope on most steps ({chained} of {steps_with})");
    assert!(longest > 50.0, "the rope reaches out with the warhead ({longest:.1} cm)");
}
