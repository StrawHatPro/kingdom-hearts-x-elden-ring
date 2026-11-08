//! (AUDIT-C ANLOOK) RAGE FORM'S PARTICLES AS KH3 DRAWS THEM, through the real core, the effect seam and the host's effect side
//! with the main pack:
//! - the aura (ead_p_ant_com group 100, vp_ant_bdy_ora00 on R_te / L_te / R_ashi1 / L_ashi1): four tails, each its one
//!   enabled emitter TailPoly_lp0 (TailPoly_lp is disabled in the asset), RISING while he stands still by the tail Wave's
//!   MoveVector (0, 0, 1) cm PER FRAME (VFX-2's trace: displacement = MoveVector x dt x 60.024) plus the +-1 cm / frame random
//!   drift, 40 points 5 cm apart;
//! - the eyes (group 13, from the first Riskcharge): three vp_ant_eye_ora00 with the attach rows' OWN ParticleColor /
//!   ParticleAlpha (KH3's post-spawn 0x141772CD0 -> component +0xB78 / +0xB84): (1, 1, 1, 1) on vfx_center, (0.65, 0.65, 0.65, 1)
//!   on vfx_lit_mune, (0.4, 0.4, 0.4, 1) on vfx_abn_atama (before AUDIT-C all three drew white: EffectSpawn had no colour);
//! - the Riskcharge group (10: vp_ant_rsk_crg_00) with its identity row.
//!
//! Skipped, saying so, without the pack.

use std::path::PathBuf;

use glam::Vec3;
use kh_core::{Input, PadButton, PadButtons, PadState};
use kh_host::audio::PoseTap;
use kh_host::seam::{AudioQueue, EffectCommand, EffectQueue};
use kh_host::vfx::{EffectHost, Scene};

fn project() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

struct Run {
    rig: kh_host::rig::Rig,
    host: EffectHost,
    held: PadButtons,
    colors: Vec<(String, [f32; 4])>,
}

impl Run {
    fn step(&mut self, held: &[PadButton]) {
        let now = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let input = Input::default().with_pad(PadState::from_held(now, self.held));
        self.held = now;
        let (mut audio, mut effects) = (AudioQueue::default(), EffectQueue::default());
        self.rig.step_with(&input, &mut audio, &mut effects);
        // the seam's colour commands, by the effect they colour
        for c in &effects.0 {
            if let EffectCommand::Color(h, rgba) = c {
                let name = effects.0.iter().find_map(|s| match s {
                    EffectCommand::Spawn(sh, sp) if sh == h => Some(sp.name.to_string()),
                    _ => None,
                });
                self.colors.push((name.unwrap_or_default(), *rgba));
            }
        }
        let tap = PoseTap::default();
        let none = |_: kh_core::TargetId| None;
        let scene = Scene { out: &self.rig.out, sora: &self.rig.sora, tap: &tap, target: &none, ground: Some(0.0), eye: Vec3::new(-500.0, 0.0, 200.0) };
        self.host.step(1.0 / 60.0, &effects.0, &scene);
    }

    fn systems(&self, name: &str) -> Vec<kh_vfx::EffectId> {
        self.host.world.live().iter().filter(|l| l.name.eq_ignore_ascii_case(name) && !l.ending).map(|l| l.id).collect()
    }

    /// The aura's ribbons this frame: (points, rise of the tail's highest point over its head point, cm).
    fn aura_ribbons(&mut self) -> Vec<(usize, f32)> {
        let camera = kh_vfx::Camera { position: Vec3::new(-400.0, 0.0, 120.0), forward: Vec3::X, right: Vec3::Y, up: Vec3::Z };
        let (list, names, _) = self.host.draw(1.0, &camera);
        let ours = |m: u32| names.get(m as usize).is_some_and(|n| n.to_lowercase().contains("gmi_ora_00sla1"));
        list.ribbons
            .iter()
            .filter(|r| ours(r.material) && !r.points.is_empty())
            .map(|r| {
                let head = r.points.iter().min_by(|a, b| a.u.total_cmp(&b.u)).map_or(0.0, |p| (p.a.z + p.b.z) * 0.5);
                let top = r.points.iter().map(|p| p.a.z.max(p.b.z)).fold(f32::MIN, f32::max);
                (r.points.len(), top - head)
            })
            .collect()
    }
}

#[test]
fn rage_aura_rises_and_the_eyes_take_their_rows_colours() {
    let pack = project().join("build").join("sora_pack");
    let loadout = project().join("sheets").join("port").join("sora-loadout-99.json");
    if !pack.join("vfx").join("effects").join("vp_ant_bdy_ora00.json").is_file() {
        eprintln!("SKIPPED: no effect pack under {}", pack.display());
        return;
    }
    let rig = kh_host::rig::Rig::new(&pack, &loadout, Some(Vec3::new(300.0, 0.0, 0.0))).expect("Sora's rig");
    let mut host = EffectHost::load(&pack).expect("the main effect pack");
    let _ = host.load_xsi_root(&pack);
    let _ = host.load_kb_root(&pack);
    host.preload();
    let mut r = Run { rig, host, held: PadButtons::NONE, colors: Vec::new() };
    for _ in 0..5 {
        r.step(&[]);
    }
    let max = r.rig.sora.status().max_hp();
    let mut tries = 0;
    while !r.rig.sora.combat().situation.has(0x8B) && tries < 300 {
        r.rig.sora.status_mut().hp = max;
        r.step(&[]);
        r.rig.sora.status_mut().hp = 1;
        r.step(&[]);
        tries += 1;
    }
    assert!(r.rig.sora.combat().situation.has(0x8B), "Rage Form offered");
    for _ in 0..60 {
        r.step(&[]);
    }
    r.step(&[PadButton::SITUATION]);
    for _ in 0..170 {
        r.step(&[]);
    }
    assert_eq!(r.rig.sora.combat().form.cmd, 0x8B, "in Rage Form");
    // THE AURA STANDING STILL: four tails of up to 40 points that rise (VFX-2's per-frame MoveVector).
    let mut best: Vec<(usize, f32)> = Vec::new();
    for _ in 0..180 {
        r.step(&[]);
        let now = r.aura_ribbons();
        if now.iter().map(|x| x.1).sum::<f32>() > best.iter().map(|x| x.1).sum::<f32>() {
            best = now;
        }
    }
    eprintln!("aura ribbons standing (points, rise cm): {best:?}");
    assert_eq!(r.systems("vp_ant_bdy_ora00").len(), 4, "four aura systems");
    assert!(best.len() >= 4, "four aura tails drawn: {best:?}");
    assert!(best.iter().all(|(n, _)| *n <= 41), "a tail has at most 40 points (+ its head): {best:?}");
    assert!(best.iter().filter(|(_, rise)| *rise >= 20.0).count() >= 4, "every tail rises at least 20 cm while he stands: {best:?}");
    // the aura's rows are identity: no colour command for them
    assert!(r.colors.iter().all(|(n, _)| !n.eq_ignore_ascii_case("vp_ant_bdy_ora00")), "{:?}", r.colors);
    // RISKCHARGE: the eyes and the charge group.
    r.rig.sora.debug_add_style(100.0);
    for _ in 0..95 {
        r.step(&[]);
    }
    assert!(r.rig.sora.combat().situation.has(0xB3), "RISKCHARGE offered");
    r.step(&[PadButton::SITUATION]);
    assert_eq!(r.rig.sora.combat().form.rage.riskcharges, 1, "RISKCHARGE taken");
    // (AN: the charge is M_D419's own notify; the eyes start at RISKCHARGE's end, StyleFinish End 0x1413CF044 at n == 1)
    let mut charge_seen = 0;
    let mut charge_white = true;
    for _ in 0..600 {
        r.step(&[]);
        let charge = r.systems("vp_ant_rsk_crg_00");
        charge_seen = charge_seen.max(charge.len());
        charge_white &= charge.iter().all(|id| r.host.world.color(*id).is_none_or(|c| c == [1.0; 4]));
        if !r.systems("vp_ant_eye_ora00").is_empty() {
            break;
        }
    }
    for _ in 0..10 {
        r.step(&[]);
    }
    let eyes = r.systems("vp_ant_eye_ora00");
    assert_eq!(eyes.len(), 3, "the eyes' three rows");
    let mut colours: Vec<[f32; 4]> = eyes.iter().filter_map(|id| r.host.world.color(*id)).collect();
    colours.sort_by(|a, b| b[0].total_cmp(&a[0]));
    eprintln!("eye colours: {colours:?}; seam colour commands: {:?}", r.colors);
    let want = [[1.0, 1.0, 1.0, 1.0], [0.65, 0.65, 0.65, 1.0], [0.4, 0.4, 0.4, 1.0]];
    assert_eq!(colours.len(), 3);
    for (got, want) in colours.iter().zip(want) {
        assert!(got.iter().zip(want).all(|(g, w)| (g - w).abs() < 1.0e-6), "eye colour {got:?}, KH3's row {want:?}");
    }
    eprintln!("vp_ant_rsk_crg_00 systems seen: {charge_seen}");
    assert!(charge_seen > 0, "the first Riskcharge's charge (vp_ant_rsk_crg_00)");
    assert!(charge_white, "the charge's row is identity");
}

/// FINISH_AN's look: what is alive during the finisher and after the form ends (the harness pictures show a black ground).
#[test]
fn rage_finish_effects_and_what_outlives_the_form() {
    let pack = project().join("build").join("sora_pack");
    let loadout = project().join("sheets").join("port").join("sora-loadout-99.json");
    if !pack.join("vfx").join("effects").join("vp_ant_bdy_ora00.json").is_file() {
        eprintln!("SKIPPED: no effect pack under {}", pack.display());
        return;
    }
    let rig = kh_host::rig::Rig::new(&pack, &loadout, Some(Vec3::new(300.0, 0.0, 0.0))).expect("Sora's rig");
    let mut host = EffectHost::load(&pack).expect("the main effect pack");
    let _ = host.load_xsi_root(&pack);
    let _ = host.load_kb_root(&pack);
    host.preload();
    let mut r = Run { rig, host, held: PadButtons::NONE, colors: Vec::new() };
    let max = r.rig.sora.status().max_hp();
    let mut tries = 0;
    while !r.rig.sora.combat().situation.has(0x8B) && tries < 300 {
        r.rig.sora.status_mut().hp = max;
        r.step(&[]);
        r.rig.sora.status_mut().hp = 1;
        r.step(&[]);
        tries += 1;
    }
    for _ in 0..60 {
        r.step(&[]);
    }
    r.step(&[PadButton::SITUATION]);
    for _ in 0..200 {
        r.step(&[]);
    }
    // three Riskcharges, then the finish
    for _ in 0..4 {
        r.rig.sora.debug_add_style(100.0);
        for _ in 0..95 {
            r.step(&[]);
        }
        r.step(&[PadButton::SITUATION]);
        for _ in 0..200 {
            r.step(&[]);
        }
    }
    let names = |r: &Run| {
        let mut v: Vec<String> = r.host.world.live().iter().filter(|l| !l.ending).map(|l| l.name.to_lowercase()).collect();
        v.sort();
        v.dedup();
        v
    };
    eprintln!("form cmd {:#x}, riskcharges {}", r.rig.sora.combat().form.cmd, r.rig.sora.combat().form.rage.riskcharges);
    let mut pool: Vec<(f32, f32, f32)> = Vec::new();
    for k in 0..8 {
        eprintln!("t+{:.1}s form {:#x}: {:?}", k as f32 * 0.5, r.rig.sora.combat().form.cmd, names(&r));
        let camera = kh_vfx::Camera { position: Vec3::new(-400.0, 0.0, 150.0), forward: Vec3::X, right: Vec3::Y, up: Vec3::Z };
        let (list, mats, _) = r.host.draw(1.0, &camera);
        // the finisher's dark pool (vp_ant_fin00 Decal0, gdi_ant_fin_dcl00): 25 m, CustomParam1 (its dissolve) 0 until 2 s, 1 at
        // 3 s, gone at its 5 s duration
        for d in list.decals.iter().filter(|d| mats.get(d.material as usize).is_some_and(|m| m.contains("gdi_ant_fin_dcl00"))) {
            let size = d.transform.matrix3.y_axis.length();
            eprintln!("   pool: half size {size:.0} cm, time {:.2} s, CustomParam1 {:.3}", d.time, d.custom[1]);
            pool.push((d.time, d.custom[1], size));
        }
        for _ in 0..30 {
            r.step(&[]);
        }
    }
    assert_eq!(r.rig.sora.combat().form.cmd, 0, "the form ended after FINISH_AN");
    let left: Vec<String> = names(&r).into_iter().filter(|n| n.starts_with("vp_ant_")).collect();
    assert!(left.is_empty(), "Rage effects left 4 s after the finish: {left:?}");
    assert!(pool.iter().any(|p| p.0 < 2.0 && p.1 < 0.01), "the pool solid before 2 s: {pool:?}");
    assert!(pool.iter().any(|p| p.0 > 3.0 && p.1 > 0.99), "the pool dissolved (CustomParam1 1) after 3 s: {pool:?}");
    assert!(pool.iter().all(|p| (p.2 - 2491.0).abs() < 2.0), "25 m pool (DecalSize 2491 at 0.6 s): {pool:?}");
}
