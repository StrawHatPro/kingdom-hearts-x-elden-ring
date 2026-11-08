//! (AUDIT-A) The recorder's own smoke test: Second Form with the Kingdom Key, offer -> StyleChange -> three attacks ->
//! the form's finish -> end, through the real core, effect side and audio side. Writes
//! build/sora_tmp/weapons/AUDIT-A/logs/smoke_l1.report.txt. Skipped without the pack.

use glam::Vec3;
use kh_core::{PadButton, StateId};
use kh_host::audit_a::{coverage, write_report, Session};

#[test]
fn the_recorder_sees_second_forms_notifies() {
    let Some(mut s) = Session::new("WEP_KEYBLADE_SO_00", &[], "w_so010", Some(Vec3::new(250.0, 0.0, 0.0)), "smoke_l1") else { return };
    s.idle(5);
    s.rig.sora.debug_add_style(100.0);
    s.idle(15);
    s.step(&[PadButton::SITUATION]);
    assert!(s.until(400, |s| s.rig.out.state != StateId::Stylechange && s.rig.sora.combat().form.cmd != 0), "Second Form starts");
    for _ in 0..3 {
        s.step(&[PadButton::ATTACK]);
        s.idle(20);
    }
    s.idle(60);
    s.finish();
    let text = write_report(&s.rows, "smoke_l1");
    let cov = coverage(&s.rows);
    assert!(!cov.is_empty(), "the clips fired notifies:\n{text}");
    assert!(cov.iter().any(|c| c.emitted), "the seam emitted some of them:\n{text}");
}
