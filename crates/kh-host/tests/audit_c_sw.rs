//! (AUDIT-C SW) Ultimate Form (Ultima Weapon, `form.SW` CHANGE_SW 0x87), driven through the recorder (`kh_host::audit_a`:
//! the real core, the real effect side with the keyblade root, the real audio side with the pack's routes and the form
//! weapon's swing / hit sets): offer -> StyleChange9 (the ChangeSW blast, w_so191) -> the COMBO_SW chain 0xC7 .. 0xCC on
//! the ground and in the air -> the Reflec guard (9279) and the air one (9280) with a guarded hit -> Counter Blade (0x25,
//! 9341 / BladeCounter) -> FINISH_SW (M_9419, the blade storm) -> the end of the form. Every sound / voice / weapon-SE /
//! effect notify of every clip Sora played must be emitted by the seam AND resolved by the host (an effect drawn, a sound
//! asset played), judged with KH3's effect-group rule. Report: build/sora_tmp/weapons/AUDIT-C/SW/logs/audit_c_sw.report.txt
//! (build/sora_tmp/weapons/AUDIT-C/SW/PART.md). Skipped without the pack.

use glam::Vec3;
use kh_core::enums::ETresDamageKind;
use kh_core::states::HitOnPlayer;
use kh_core::{PadButton, StateId, TargetId};
use kh_host::audit_a::{self, Covered, Session, Want};

/// Ultima Weapon's models: the keyblade w_so190, the transform unit w_so191, the form blade w_so192.
const ULTIMA: &[&str] = &["w_so190", "w_so191", "w_so192"];

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

    /// Attack every other step until the chain ends (a jump first for the air chain): the kinds played.
    fn combo(&mut self, air: bool) -> Vec<u8> {
        let first = self.history().len();
        if air {
            self.step(&[PadButton::JUMP]);
            self.idle(12);
        }
        for k in 0..1800 {
            let state = if k % 2 == 0 { self.step(&[PadButton::ATTACK]) } else { self.step(&[]) };
            if k > 30 && matches!(state, StateId::Idle | StateId::Run) {
                break;
            }
        }
        self.idle(30);
        self.history()[first..].to_vec()
    }

    /// A hit from the front (the dummy's side) on Sora.
    fn hit_from_front(&mut self) {
        let own = self.s.rig.sora.output().position;
        self.s.rig.sora.take_hit(HitOnPlayer { direction: Vec3::new(-1.0, 0.0, 0.0), causer_yaw: 180.0, ..HitOnPlayer::new(ETresDamageKind::Small, 30.0, TargetId(1), own + Vec3::new(150.0, 0.0, 0.0)) });
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

fn logs_dir() -> std::path::PathBuf {
    let d = audit_a::project().join("build/sora_tmp/weapons/AUDIT-C/SW/logs");
    std::fs::create_dir_all(&d).ok();
    d
}

#[test]
fn ultimate_form_offer_to_end_every_notify() {
    let Some(s) = Session::new("WEP_KEYBLADE_SO_14", ULTIMA, "w_so190", Some(Vec3::new(200.0, 0.0, 0.0)), "audit_c_sw") else { return };
    let mut r = Run { s, marks: Vec::new(), groups: Vec::new() };
    // kh-er reads the XS root too (fx.rs): FINISH_SW's vw_so192_trf_* are packed there
    r.s.load_xsi_root();
    r.idle(5);
    // ---- the offer and the transformation (StyleChange9, wear form 6)
    r.s.rig.sora.debug_add_style(100.0);
    r.idle(15);
    assert_eq!(r.s.rig.sora.combat().situation.list.first().map(|e| e.kind), Some(0x87), "CHANGE_SW offered");
    r.mark("offer: Triangle on CHANGE_SW");
    r.step(&[PadButton::SITUATION]);
    for _ in 0..600 {
        if r.step(&[]) != StateId::Stylechange && r.s.rig.sora.combat().form.cmd == 0x87 {
            break;
        }
    }
    let f = r.s.rig.sora.combat().form;
    assert_eq!((f.cmd, f.wear_form), (0x87, 6), "Ultimate Form, wear form 6 (Final)");
    assert_eq!(r.s.rig.sora.status().attack_model, "w_so192", "the form blade's swing / hit sets");
    r.mark("Ultimate Form on");
    r.idle(20);
    // the aura rows of ead_so192_aura0 (IdleB 9015 names it 0 .. 2.67 s): both feet and the blade, alive in the host
    let live: Vec<String> = r.s.fx.world.live().iter().map(|l| l.name.to_lowercase()).collect();
    assert!(live.iter().filter(|n| n.starts_with("vw_so192_foot_aura0")).count() >= 2 && live.iter().any(|n| n.starts_with("vw_so192_bld_aur0")), "the aura rows alive: {live:?}");
    // ---- the ground chain
    r.mark("ground combo");
    assert_eq!(r.combo(false), [0xC7, 0xC8, 0xC9, 0xCA, 0xCB, 0xCC], "COMBO_SW on the ground");
    // ---- the air chain (every kind is in air mode: the same chain)
    r.mark("air combo");
    let air = r.combo(true);
    assert!(air.starts_with(&[0xC7, 0xC8]), "COMBO_SW in the air: {air:x?}");
    r.idle(40);
    // ---- the Reflec guard, a guarded hit, Counter Blade
    r.s.rig.sora.force_battle(true);
    r.mark("Reflec guard + hit + Counter Blade");
    r.step(&[PadButton::GUARD]);
    // past the Reflec clip's vw_so192_grd0 (0.09 s) before the hit (a hit entry plays the clip from 0.1667 s, anim vt+0xF0)
    r.idle(8);
    let hp = r.s.rig.sora.status().hp;
    r.hit_from_front();
    r.idle(2);
    assert_eq!(r.s.rig.sora.status().hp, hp, "guarded");
    let first = r.history().len();
    r.step(&[PadButton::ATTACK]);
    r.idle(90);
    assert_eq!(r.history()[first..].first(), Some(&0x25), "Counter Blade (forced kind 0x25)");
    // ---- the air Reflec (AirReflec 9280)
    r.mark("AirReflec");
    r.step(&[PadButton::JUMP]);
    r.idle(12);
    r.step(&[PadButton::GUARD]);
    r.idle(80);
    // ---- the teleport toward a far target (12 m, inside the doubled soft range): the vanish (JumpNormalStart) with the
    // Ultima warp's aura group 15, the reappearance 150 cm short of it with group 16 (ead_pc_aura_com0)
    r.mark("teleport (target at 12 m)");
    let far = r.s.rig.sora.output().position + Vec3::new(1200.0, 0.0, 0.0);
    if let Some(d) = r.s.rig.dummy.as_mut() {
        (d.info.position, d.info.center) = (Vec3::new(far.x, far.y, 0.0), Vec3::new(far.x, far.y, 90.0));
    }
    r.idle(30);
    let from = r.s.rows.len();
    r.step(&[PadButton::ATTACK]);
    r.idle(40);
    let spawned: Vec<String> = r.s.rows[from..].iter().flat_map(|row| &row.effects).filter(|e| e.drawn > 0).map(|e| e.name.clone()).collect();
    for name in ["ws_so192_wrp_in01", "ws_so192_wrp_in03", "ws_so192_wrp_in04", "ws_so192_wrp_out01", "ws_so192_wrp_out03", "ws_so192_wrp_out04"] {
        assert!(spawned.iter().any(|s| s == name), "{name} drawn at the teleport: {spawned:?}");
    }
    r.idle(120);
    // ---- FINISH_SW
    r.idle(20);
    assert_eq!(r.s.rig.sora.combat().situation.slot_b.map(|e| e.kind), Some(0xA1), "FINISH_SW on offer");
    r.mark("FINISH_SW");
    r.step(&[PadButton::SITUATION]);
    for _ in 0..1500 {
        let state = r.step(&[]);
        if state != StateId::Stylefinish && r.s.rig.sora.combat().form.cmd == 0 {
            break;
        }
    }
    r.mark("form ended");
    assert_eq!(r.s.rig.sora.combat().form.cmd, 0, "the finish ends the form");
    r.idle(90);
    r.s.finish();
    // ---- the transform unit w_so191's own 9401 (wnotify rows) and the blade / feet aura rows of ead_so192_aura0
    let drawn = |name: &str| r.s.rows.iter().flat_map(|row| &row.effects).filter(|e| e.name.to_lowercase().contains(name)).map(|e| e.drawn).max().unwrap_or(0);
    for name in ["vw_so191_chg_ora0", "vw_so191_chg_trf0", "vw_so191_chg_kaz0", "vw_so191_chg_lst0", "ead_so192_aura0"] {
        assert!(drawn(name) > 0, "{name} drawn");
    }
    assert!(r.s.rows.iter().flat_map(|row| &row.sounds).any(|s| s.name == "se02019_011"), "w_so191 9401: AttachSound se02019_011");
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
    text.push_str(&audit_a::report(&r.s.rows));
    std::fs::write(logs_dir().join("audit_c_sw.report.txt"), &text).ok();
    assert!(!cov.is_empty());
    assert!(failed.is_empty(), "{} of {} notifies failed:\n{}", failed.len(), cov.len(), failed.join("\n"));
}
