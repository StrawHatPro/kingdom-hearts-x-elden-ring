//! (F-RAGE fix) THE RAGE FORM PARTICLES, END TO END (the user, in game: "rage form doesn't have the rage form particles"):
//! kh-core's HP-loss offer -> Y -> Rage Form, through the effect seam into the host's effect side with the main pack. The
//! aura of the form (ead_p_ant_com group 100: vp_ant_bdy_ora00 on both hands and feet) is spawned, found in the pack,
//! alive AND DRAWN (its ribbons in the draw list) while Sora stands still and while he attacks; RISKCHARGE's groups
//! (10 .. 9 + n and 13) likewise; everything of the form is gone after it. Skipped, saying so, without the pack.

use std::path::PathBuf;

use glam::Vec3;
use kh_core::{Input, PadButton, PadButtons, PadState};
use kh_host::audio::PoseTap;
use kh_host::seam::{AudioQueue, EffectQueue};
use kh_host::vfx::{EffectHost, Scene};

fn project() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

struct Run {
    rig: kh_host::rig::Rig,
    host: EffectHost,
    held: PadButtons,
    spawned: Vec<String>,
}

impl Run {
    fn step(&mut self, held: &[PadButton]) {
        let now = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let input = Input::default().with_pad(PadState::from_held(now, self.held));
        self.held = now;
        let (mut audio, mut effects) = (AudioQueue::default(), EffectQueue::default());
        self.rig.step_with(&input, &mut audio, &mut effects);
        for c in &effects.0 {
            if let kh_host::seam::EffectCommand::Spawn(_, s) = c {
                let file = s.name.rsplit(['/', '.']).next().unwrap_or(s.name).to_lowercase();
                if !self.spawned.contains(&file) {
                    self.spawned.push(file);
                }
            }
        }
        let tap = PoseTap::default();
        let none = |_: kh_core::TargetId| None;
        let scene = Scene { out: &self.rig.out, sora: &self.rig.sora, tap: &tap, target: &none, ground: Some(0.0), eye: Vec3::new(-500.0, 0.0, 200.0) };
        self.host.step(1.0 / 60.0, &effects.0, &scene);
    }

    fn live(&self, prefix: &str) -> usize {
        self.host.world.live().iter().filter(|l| l.name.to_lowercase().starts_with(prefix)).count()
    }

    /// Ribbons and quads of the frame whose material is one of `materials`.
    fn drawn(&mut self, materials: &[&str]) -> usize {
        let camera = kh_vfx::Camera { position: Vec3::new(-400.0, 0.0, 120.0), forward: Vec3::X, right: Vec3::Y, up: Vec3::Z };
        let (list, names, _) = self.host.draw(1.0, &camera);
        let ours = |m: u32| names.get(m as usize).is_some_and(|n| materials.iter().any(|x| n.to_lowercase().contains(x)));
        list.ribbons.iter().filter(|r| ours(r.material)).count() + list.quads.iter().filter(|q| ours(q.material)).count()
    }
}

#[test]
fn rage_form_shows_its_particles_and_they_end_with_it() {
    let pack = project().join("build").join("sora_pack");
    let loadout = project().join("sheets").join("port").join("sora-loadout-99.json");
    if !pack.join("vfx").join("effects").join("vp_ant_bdy_ora00.json").is_file() {
        eprintln!("SKIPPED: no effect pack under {}", pack.display());
        return;
    }
    let rig = kh_host::rig::Rig::new(&pack, &loadout, Some(Vec3::new(300.0, 0.0, 0.0))).expect("Sora's rig");
    let mut host = EffectHost::load(&pack).expect("the main effect pack");
    // the roots kh-er reads too (kh-er fx.rs): the XS moves' and the keyblades' (Rage's clips name effects of both)
    let _ = host.load_xsi_root(&pack);
    let _ = host.load_kb_root(&pack);
    // (F-RAGE fix) every Rage effect of the main pack is preloaded (its materials made with the pack)
    let preloaded = host.preload();
    assert!(preloaded > 0 && ["vp_ant_bdy_ora00", "vp_ant_eye_ora00", "vp_ant_rsk_crg_00", "vp_ant_fin_tog00"].iter().all(|n| host.world.has(n)), "Rage's effects are preloaded");
    let mut r = Run { rig, host, held: PadButtons::NONE, spawned: Vec::new() };
    for _ in 0..5 {
        r.step(&[]);
    }
    // THE OFFER: losses to 1 HP until the roll (34 %) comes up.
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
    // (the entry appears first: selectable after its appear time)
    for _ in 0..60 {
        r.step(&[]);
    }
    r.step(&[PadButton::SITUATION]);
    for _ in 0..150 {
        r.step(&[]);
    }
    assert_eq!(r.rig.sora.combat().form.cmd, 0x8B, "in Rage Form");
    assert_eq!(r.live("vp_ant_bdy_ora00"), 4, "the aura on both hands and feet is alive; host log: {:?}", r.host.log);
    // DRAWN while he stands still (the tails rise by their Wave drift) ...
    // (after the transformation clip StyleChange13, 2.7 s)
    let mut still = 0;
    for _ in 0..120 {
        r.step(&[]);
        still = still.max(r.drawn(&["gmi_ora_00sla1"]));
    }
    assert_eq!(r.rig.out.anim.name, kh_core::AnimId::from_name("D015").expect("D015"), "Rage Form's own idle (AnimSet13 IdleB)");
    assert!(still >= 4, "the aura draws while he stands: {still} pieces; host log: {:?}", r.host.log);
    // ... and while he attacks.
    let mut moving = 0;
    for k in 0..90 {
        r.step(if k % 20 < 2 { &[PadButton::ATTACK] } else { &[] });
        moving = moving.max(r.drawn(&["gmi_ora_00sla1"]));
    }
    assert!(moving >= 4, "the aura draws while he attacks: {moving}");
    // RISKCHARGE: a full gauge, Y: its charge group (10) and the eyes (13) come with it.
    for _ in 0..60 {
        r.step(&[]);
    }
    r.rig.sora.debug_add_style(100.0);
    for _ in 0..5 {
        r.step(&[]);
    }
    assert!(r.rig.sora.combat().situation.has(0xB3), "RISKCHARGE offered");
    for _ in 0..60 {
        r.step(&[]);
    }
    // (the landing's command cool-down after the air combo runs out first)
    for _ in 0..30 {
        r.step(&[]);
    }
    r.step(&[PadButton::SITUATION]);
    assert_eq!(r.rig.sora.combat().form.rage.riskcharges, 1, "RISKCHARGE taken");
    // (AUDIT-C) the eyes (group 13) start at RISKCHARGE's END (StyleFinish End 0x1413CF044, n == 1), after M_D419
    for _ in 0..30 {
        r.step(&[]);
    }
    for _ in 0..400 {
        if r.spawned.iter().any(|s| s == "vp_ant_eye_ora00") {
            break;
        }
        r.step(&[]);
    }
    for name in ["vp_ant_rsk_crg_00", "vp_ant_eye_ora00"] {
        assert!(r.spawned.iter().any(|s| s == name), "{name} spawned: {:?}", r.spawned);
        assert!(r.live(name) > 0, "{name} alive; host log: {:?}", r.host.log);
    }
    // NOTHING OF RAGE FORM WENT UNFOUND (its clips' notifies included)
    let missing: Vec<&String> = r.host.log.iter().filter(|l| l.contains("vp_ant_") || l.contains("ant_com")).collect();
    assert!(missing.is_empty(), "Rage effects the host could not draw: {missing:?}");
    // THE END: the form's time out; within 3 s nothing of the aura is left.
    r.rig.sora.combat_mut().form.time_left = 0.05;
    for _ in 0..240 {
        r.step(&[]);
    }
    assert_eq!(r.rig.sora.combat().form.cmd, 0, "the form ended");
    let left = r.host.world.live().iter().filter(|l| l.name.to_lowercase().starts_with("vp_ant_") && !l.ending).map(|l| l.name.clone()).collect::<Vec<_>>();
    assert!(left.is_empty(), "Rage effects left after the form: {left:?}");
}
