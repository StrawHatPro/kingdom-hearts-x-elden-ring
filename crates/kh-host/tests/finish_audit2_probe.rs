//! (FINISH-AUDIT2) PROBE: Sonic Rave (Kingdom Key Second Form, finish level 2): per step, Sora's actor, his posed kh_sk /
//! center / feet in the world and every live effect system's node, to find why its effects sit below his body.
//! Writes build/sora_tmp/weapons/FINISH-AUDIT2/work/probe_sonic_rave.txt. Skipped without the pack.

use std::fmt::Write as _;

use glam::Vec3;
use kh_core::{PadButton, StateId};
use kh_host::audit_a::Session;

fn press(s: &mut Session, b: PadButton) {
    s.step(&[b]);
    s.step(&[]);
}

#[test]
fn probe_sonic_rave() {
    let Some(mut s) = Session::new("WEP_KEYBLADE_SO_00", &["w_so010"], "w_so010", Some(Vec3::new(300.0, 0.0, 0.0)), "fa2_probe") else { return };
    s.idle(5);
    s.rig.sora.debug_add_style(60.0);
    s.idle(2);
    s.rig.sora.debug_add_style(40.0);
    s.idle(15);
    press(&mut s, PadButton::SITUATION);
    assert!(s.until(400, |s| s.rig.out.state == StateId::Stylechange));
    assert!(s.until(400, |s| s.rig.out.state != StateId::Stylechange));
    assert!(s.until(200, |s| s.rig.out.state == StateId::Idle));
    s.idle(5);
    s.rig.sora.debug_add_style(100.0);
    s.idle(20);
    assert_eq!(s.rig.sora.combat().form.finish_cmd, 0xAC);
    let mut t = String::new();
    for _ in 0..60 {
        press(&mut s, PadButton::SITUATION);
        if s.rig.out.state == StateId::Stylefinish {
            break;
        }
    }
    let mut down = false;
    for _ in 0..400 {
        let extra = s.rig.sora.combat().situation.extra.is_some();
        down = extra && !down;
        let st = if down { s.step(&[PadButton::SITUATION]) } else { s.step(&[]) };
        let out = &s.rig.out;
        let bone = |n: &str| s.audio.tap.bone_world(n).map(|b| b.translation);
        let f = |v: Option<Vec3>| v.map_or("-".to_string(), |v| format!("({:.0},{:.0},{:.0})", v.x, v.y, v.z));
        let row = s.rows.last().unwrap();
        let _ = writeln!(t, "step {} {:?} {} @{:.3} actor {} kh_sk {} center {} Lfoot {} Rfoot {}", out.step, st, row.clip, row.time, f(Some(out.position)), f(bone("kh_sk")), f(bone("center")), f(bone("L_ashi2")), f(bone("R_ashi2")));
        for l in s.fx.world.live() {
            if let Some((x, nodes)) = s.fx.world.latest_nodes(l.id) {
                let n: Vec<String> = nodes.iter().flatten().take(3).map(|p| f(Some(*p))).collect();
                let _ = writeln!(t, "    fx {} sys {} nodes {}", l.name, f(Some(x.position)), n.join(" "));
            }
        }
        if st != StateId::Stylefinish && s.rig.sora.finish().is_none() {
            break;
        }
    }
    let path = kh_host::audit_a::project().join("build/sora_tmp/weapons/FINISH-AUDIT2/work/probe_sonic_rave.txt");
    std::fs::write(path, t).unwrap();
}
