//! (FREEZE-FX, the user 2026-11-07: "make sure frozen effects are applied to anything that gets frozen by blizzard") KH3'S ICE
//! ENCASING ON A CHARACTER for exactly the freeze, through the host's effect side with the main pack: the ice row's
//! `vs_abn_frz_in` (SQEX_Material_Scalar "Ice" 0 -> 1 over 0.3 s, UseLastParameter) leaves "Ice" at 1 on the character after
//! its emitter ends, for as long as the persistent row is not stopped (KH3 keeps the last value on the mesh); the stop ends
//! it, the break's `vs_abn_frz_out0` takes it back to 0. Skipped, saying so, without the pack.

use std::path::PathBuf;

use glam::Vec3;
use kh_core::{Attach, EffectHandle, EffectId, EffectSpawn, Input};
use kh_host::audio::PoseTap;
use kh_host::seam::{AudioQueue, EffectCommand, EffectQueue};
use kh_host::vfx::{EffectHost, Scene};

fn project() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

fn spawn(handle: u32, name: &'static str, persistent: bool) -> EffectCommand {
    EffectCommand::Spawn(EffectHandle(handle), EffectSpawn { id: EffectId(0xC1C), name, at: Attach::Sora { bone: Some("vfx_abn_root"), offset: Vec3::ZERO }, rotation: Vec3::ZERO, scale: 1.0, persistent })
}

#[test]
fn freeze_fx_the_ice_encasing_holds_for_the_whole_freeze_and_the_break_takes_it_back() {
    let pack = project().join("build").join("sora_pack");
    let loadout = project().join("sheets").join("port").join("sora-loadout-99.json");
    if !pack.join("vfx").join("effects").join("vs_abn_frz_in.json").is_file() {
        eprintln!("SKIPPED: no effect pack under {}", pack.display());
        return;
    }
    let mut rig = kh_host::rig::Rig::new(&pack, &loadout, None).expect("Sora's rig");
    let mut host = EffectHost::load(&pack).expect("the main effect pack");
    let camera = kh_vfx::Camera { position: Vec3::new(-400.0, 0.0, 120.0), forward: Vec3::X, right: Vec3::Y, up: Vec3::Z };
    let ice = 0x7C00_0001;
    let mut step = |extra: Vec<EffectCommand>| -> Option<f32> {
        let (mut audio, mut effects) = (AudioQueue::default(), EffectQueue::default());
        rig.step_with(&Input::default(), &mut audio, &mut effects);
        effects.0.extend(extra);
        let tap = PoseTap::default();
        let none = |_: kh_core::TargetId| None;
        let scene = Scene { out: &rig.out, sora: &rig.sora, tap: &tap, target: &none, ground: Some(0.0), eye: Vec3::new(-500.0, 0.0, 200.0) };
        host.step(1.0 / 60.0, &effects.0, &scene);
        let _ = host.draw(1.0, &camera);
        // (KH3 SetFrozen sends the freeze rows to the equipment too: the held weapons take the same drive)
        host.char_drives().iter().find(|d| d.owner == 0 && d.name == "Ice").inspect(|d| assert!(d.equipment, "the weapons take the ice")).map(|d| d.value[0])
    };
    let _ = step(Vec::new());
    let mut frozen = vec![step(vec![spawn(ice, "vs_abn_frz_in", true)])];
    for _ in 0..180 {
        frozen.push(step(Vec::new()));
    }
    // 0 -> 1 over its 0.3 s, then held for the whole 3 s freeze although its emitter ended at 0.3 s
    assert!(frozen[25..].iter().all(|v| v.is_some_and(|x| x > 0.95)), "the encasing holds: {:?}", &frozen[15..40]);
    // the freeze ends: the row stopped, the break's out row
    let mut after = vec![step(vec![EffectCommand::Stop(EffectHandle(ice), false), spawn(0x7C00_0002, "vs_abn_frz_out0", false)])];
    for _ in 0..40 {
        after.push(step(Vec::new()));
    }
    assert!(after[..3].iter().any(|v| v.is_some_and(|x| x > 0.3)), "the break starts from the ice: {:?}", &after[..10]);
    assert!(after[30..].iter().all(|v| v.is_none_or(|x| x < 0.02)), "and takes it back to 0: {:?}", &after[25..]);
}
