//! (FX5) NOTHING OF AN ABSENT SORA IS DRAWN: `EffectHost::stop_sora` ends every effect Sora started (his handles below
//! the party's ranges) at once, among them the hero light's steady Keyblade glow (KH3's appear effect `VW_SO010_APP0`
//! with only its "Key" (the Keyblade's own mesh) and "glw_I" emitters, persistent on R_wep_skt: the Keyblade the CV3 run
//! saw lying on the chapel floor after the starter choice on a Donald start); the party's effects and the free ones
//! (save points, prizes) go on. Skipped, saying so, without the pack.

use std::path::PathBuf;

use glam::Vec3;
use kh_core::host::party::{PartyMember, SeamHandles};
use kh_core::{Attach, EffectHandle, EffectId, EffectSpawn};
use kh_host::audio::PoseTap;
use kh_host::seam::EffectCommand;
use kh_host::vfx::{EffectHost, Scene};

fn project() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

fn spawn(name: &'static str, at: Attach, persistent: bool) -> EffectSpawn {
    EffectSpawn { id: EffectId(0), name, at, rotation: Vec3::ZERO, scale: 1.0, persistent }
}

const GLOW: &str = "VW_SO010_APP0#glow";

#[test]
fn fx5_an_absent_soras_effects_end_and_nothing_of_his_is_drawn() {
    let pack = project().join("build").join("sora_pack");
    let loadout = project().join("sheets").join("port").join("sora-loadout-99.json");
    if !pack.join("vfx").join("effects").join("VW_SO010_APP0.json").is_file() {
        eprintln!("SKIPPED: no effect pack under {}", pack.display());
        return;
    }
    let rig = kh_host::rig::Rig::new(&pack, &loadout, None).expect("Sora's rig");
    let mut host = EffectHost::load(&pack).expect("the main effect pack");
    // the hero light's steady glow, as kh-er defines it (kh-er-render pointlight.rs GLOW_EFFECT / GLOW_EMITTERS)
    host.define_variant(GLOW, "VW_SO010_APP0", &["Key", "glw_I"], Some(0.25));
    let tap = PoseTap::default();
    let none = |_: kh_core::TargetId| None;
    let scene = Scene { out: &rig.out, sora: &rig.sora, tap: &tap, target: &none, ground: Some(0.0), eye: Vec3::new(-500.0, 0.0, 200.0) };
    let mut party = SeamHandles::for_member(PartyMember::DONALD);
    let donald_loop = party.effect();
    let commands = vec![
        // Sora's: the glow (persistent, on his weapon socket) and a one-shot
        EffectCommand::Spawn(EffectHandle(3), spawn(GLOW, Attach::Sora { bone: Some("R_wep_skt"), offset: Vec3::ZERO }, true)),
        EffectCommand::Spawn(EffectHandle(4), spawn("vm_cur_hed0", Attach::Sora { bone: None, offset: Vec3::Z * 100.0 }, false)),
        // Donald's persistent one
        EffectCommand::Spawn(donald_loop, spawn("vm_cur_hed0", Attach::World(Vec3::new(300.0, 0.0, 100.0)), true)),
    ];
    host.step(1.0 / 60.0, &commands, &scene);
    let free = host.free_spawn("vm_cur_hed0", Vec3::new(-300.0, 0.0, 0.0), 0.0, 1.0).expect("a free effect (as a save point's)");
    for _ in 0..30 {
        host.step(1.0 / 60.0, &[], &scene);
    }
    let names = |h: &EffectHost| h.world.live().iter().map(|l| l.name.to_string()).collect::<Vec<_>>();
    let before = names(&host);
    assert!(before.iter().any(|n| n.eq_ignore_ascii_case(GLOW)), "the Keyblade glow plays: {before:?}");
    let cam = kh_vfx::Camera { position: Vec3::new(-500.0, 0.0, 200.0), forward: Vec3::X, right: Vec3::Y, up: Vec3::Z };
    let key_meshes = |h: &mut EffectHost| {
        let (list, _, meshes) = h.draw(1.0, &cam);
        let meshes = meshes.to_vec();
        let names: Vec<String> = list.meshes.iter().filter_map(|m| meshes.get(m.mesh as usize).cloned()).collect();
        println!("  mesh particles drawn: {names:?}");
        names.iter().filter(|n| n.to_lowercase().contains("so010") || n.to_lowercase().contains("key")).count()
    };
    let drawn_before = key_meshes(&mut host);
    println!("before: live {before:?}; the Keyblade's mesh particles drawn: {drawn_before}");
    assert!(drawn_before > 0, "the glow draws the Keyblade's mesh (the CV3 floor Keyblade)");

    // Sora leaves the party: kh-er Fx::sora_absent -> stop_sora(true)
    let n = host.stop_sora(true);
    assert!(n >= 2, "his effects end (the glow and the one-shot with its attach rows): {n}");
    host.step(1.0 / 60.0, &[], &scene);
    let after = names(&host);
    let drawn_after = key_meshes(&mut host);
    println!("after stop_sora: live {after:?}; the Keyblade's mesh particles drawn: {drawn_after}");
    assert!(!after.iter().any(|n| n.eq_ignore_ascii_case(GLOW)), "no glow of his is left: {after:?}");
    assert_eq!(drawn_after, 0, "nothing of his is drawn");
    assert!(host.party_loops().first().is_some_and(|l| l.0 == 0 && l.1 >= 1), "Donald's loop goes on: {:?}", host.party_loops());
    assert!(host.free_alive(free), "the free effect (a save point's) goes on");
    // nothing more of his to stop
    assert_eq!(host.stop_sora(true), 0);
}
