//! (F-SECOND) EVERY EFFECT ANCIENT LIGHT SPAWNS ENDS WITH THE FORM. Starlight's Second Form S, finish level 3 (0xAE,
//! STYLEFINISH slot 155, clip M_1435), driven end to end: kh-core through the effect seam into the host's effect side
//! with the main pack. The in-game round (build/sora_tmp/weapons/ROUND/RESULTS.md, FAIL 6) saw the colour bands and
//! sparkles stay: `vw_so150_udis_pil1`, whose child emitters `c_red` .. `c_blue` loop for ever (EmitterLoops 0, generated
//! once) and hold their nested `vw_so150_udis_pil0`. KH3 loop-ends it at the next animation change (its row has
//! `WhenChangeAnimationType` END and end type LOOPEND: the play API's `Set` 0x141770D30 -> `EnableLoopEnd` 0x1442626D0);
//! the host does the same for every effect `EffectDef::endless` names, which now counts such children.
//! Skipped, saying so, without the pack.

use std::path::PathBuf;

use glam::Vec3;
use kh_core::{Input, PadButton, PadButtons, PadState, StateId};
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
    /// Every effect file name the session spawned (lower case).
    spawned: Vec<String>,
}

impl Run {
    fn step(&mut self, held: &[PadButton]) -> StateId {
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
        self.rig.out.state
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.step(&[]);
        }
    }

    fn live(&self) -> Vec<String> {
        self.host.world.live().iter().map(|l| l.name.to_lowercase()).collect()
    }
}

#[test]
fn every_effect_of_ancient_light_ends_with_the_form() {
    let pack = project().join("build").join("sora_pack");
    let loadout = project().join("sheets").join("port").join("sora-loadout-99.json");
    if !pack.join("vfx").join("effects").join("vw_so150_udis_pil1.json").is_file() {
        eprintln!("SKIPPED: no effect pack under {}", pack.display());
        return;
    }
    let mut rig = kh_host::rig::Rig::new(&pack, &loadout, Some(Vec3::new(300.0, 0.0, 0.0))).expect("Sora's rig");
    rig.sora.status_mut().weapon = "WEP_KEYBLADE_SO_17".to_string();
    let mut host = EffectHost::load(&pack).expect("the main effect pack");
    // Starlight in his hand: its own effects pass the other-keyblade rule (kh-er sora/kbdraw.rs: the models' prefixes).
    host.set_keyblade(None, None, &["vw_so150".to_string()]);
    let mut r = Run { rig, host, held: PadButtons::NONE, spawned: Vec::new() };
    r.idle(5);
    // Second Form S, two more full gauges: Ancient Light in the finish slot.
    r.rig.sora.debug_add_style(100.0);
    r.idle(15);
    r.step(&[PadButton::SITUATION]);
    r.idle(200);
    r.rig.sora.debug_add_style(100.0);
    r.idle(3);
    r.rig.sora.debug_add_style(100.0);
    r.idle(20);
    assert_eq!((r.rig.sora.combat().form.cmd, r.rig.sora.combat().form.finish_cmd), (0x95, 0xAE), "Ancient Light offered");
    r.step(&[PadButton::SITUATION]);
    let mut finished = false;
    let mut pil1_alive_in_clip = false;
    for _ in 0..600 {
        let state = r.step(&[]);
        pil1_alive_in_clip |= r.live().iter().any(|n| n == "vw_so150_udis_pil1");
        if state != StateId::Stylefinish && r.rig.sora.combat().form.cmd == 0 {
            finished = true;
            break;
        }
    }
    assert!(finished, "the finish ran and the form ended");
    assert!(pil1_alive_in_clip, "the pillars' bands played");
    for name in ["vw_so150_wrp0", "vw_so150_udis_pil1", "vw_so150_udis_sph1", "vw_so150_udis_fir0"] {
        assert!(r.spawned.iter().any(|s| s == name), "{name} spawned by M_1435: {:?}", r.spawned);
    }
    // The form has ended: within 3 s nothing the finish spawned is left (nor anything nested in it).
    r.idle(180);
    let left: Vec<String> = r.live().into_iter().filter(|n| n.starts_with("vw_so150") || r.spawned.contains(n)).collect();
    assert!(left.is_empty(), "effects of the finish still live after the form: {left:?} (stats {:?})", r.host.stats());
}
