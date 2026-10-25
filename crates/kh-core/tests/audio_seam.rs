//! The audio seam's second-pass additions (docs/sora-audio-spec.md 5.1, 5.2, 5.6; port plan 11):
//! `LoopEndSound` at the End of an `AttachSound` notify state, the prize sound of a collected
//! orb, the voice group number tables. (The number's way through a cast is covered by the
//! scripted `--audio-log` runs of the app: build/sora_tmp/IMPL_AUDIO_STATE.md.)

use kh_core::host::audio::{attack_voice_group_no, cure_heal_effect, item_heal_effect, prize_get_sound, ACTOR_MAP_HIT, ACTOR_PAWN_HIT, DEATH_VOICE_GROUP_NO};
use kh_core::host::{EventKind, RecordingHost};
use kh_core::status::{ItemKind, PrizeKind};
use kh_core::test_world::TestWorld;
use kh_core::{AnimId, Input, NotifyData, PlayArgs, Pose, Sora, StatusRequest, Tuning, Vec3, SORA_PARAMS};

const DT: f32 = 1.0 / 60.0;

fn spawn() -> Sora {
    Sora::new(SORA_PARAMS, Pose { position: Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS)), yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT)
}

/// (kind word, name) of the sound events of a step: "play" / "loop" / "stop".
fn sounds(out: &kh_core::Output, live: &mut Vec<(u32, &'static str)>) -> Vec<(&'static str, &'static str)> {
    let mut list = Vec::new();
    for event in &out.events {
        match event.kind {
            EventKind::SoundPlay { handle, play } => {
                live.push((handle.0, play.name));
                list.push((if play.looping { "loop" } else { "play" }, play.name));
            }
            EventKind::SoundStop { handle, .. } => list.push(("stop", live.iter().find(|l| l.0 == handle.0).map_or("?", |l| l.1))),
            _ => {}
        }
    }
    list
}

#[test]
fn an_attach_sound_plays_its_loop_end_sound_when_the_notify_ends() {
    // The one LoopEndSound of Sora's set: the AttachSound of sequence 0049 (se00001_031 with
    // loop end se00001_040; sheet row notify.0049.SAttachSound.1).
    let (anim, loop_sound, end_sound) = AnimId::ALL
        .iter()
        .find_map(|anim| {
            // (Every sample of a blend space: 0049 is the middle sample of DiveFallLoop, whose
            // notifies all fire whatever the weights.)
            let notifies = anim.data().samples.iter().flat_map(|&(_, sequence)| kh_core::anim::SEQUENCES[sequence].notifies.iter());
            notifies.into_iter().find_map(|n| if let NotifyData::Sound { sound, loop_end_sound: Some(end), .. } = n.data { Some((*anim, sound, end)) } else { None })
        })
        .expect("an animation with a LoopEndSound");
    assert!(loop_sound.contains("se00001_031") && end_sound.contains("se00001_040"), "{loop_sound} {end_sound}");
    let world = TestWorld::floor();
    let mut host = RecordingHost::new();
    let mut sora = spawn();
    sora.debug_play_anim(anim, PlayArgs::mode(0, -1.0));
    let (mut live, mut seen) = (Vec::new(), Vec::new());
    for _ in 0..20 {
        let out = host.step(&world, |h| sora.step_hosted(DT, &Input::default(), h));
        seen.extend(sounds(&out, &mut live));
    }
    assert!(seen.contains(&("loop", loop_sound)), "the notify state started its loop: {seen:?}");
    assert!(!seen.iter().any(|s| s.1 == end_sound), "no loop end while the state runs: {seen:?}");
    // Interrupted by another animation: End fires, the loop stops and the loop-end sound plays
    // once, after the stop.
    sora.debug_play_anim(AnimId::IdleN, PlayArgs::mode(1, 0.0));
    seen.clear();
    for _ in 0..30 {
        let out = host.step(&world, |h| sora.step_hosted(DT, &Input::default(), h));
        seen.extend(sounds(&out, &mut live));
    }
    let stop = seen.iter().position(|s| *s == ("stop", loop_sound)).unwrap_or_else(|| panic!("the loop is stopped: {seen:?}"));
    let end = seen.iter().position(|s| *s == ("play", end_sound)).unwrap_or_else(|| panic!("the loop-end sound plays: {seen:?}"));
    assert!(stop < end && seen.iter().filter(|s| s.1 == end_sound).count() == 1, "{seen:?}");
}

#[test]
fn a_collected_prize_plays_its_get_sound() {
    let world = TestWorld::floor();
    let mut host = RecordingHost::new();
    let mut sora = spawn();
    sora.status_request(StatusRequest::Prize(PrizeKind::MpLarge));
    let out = host.step(&world, |h| sora.step_hosted(DT, &Input::default(), h));
    assert_eq!(sounds(&out, &mut Vec::new()), [("play", "PrizeMpL.get")]);
    let names: Vec<_> = PrizeKind::ALL.iter().map(|k| prize_get_sound(*k as u8)).collect();
    assert_eq!(names, ["PrizeHpS.get", "PrizeHpL.get", "PrizeMpS.get", "PrizeMpL.get", "PrizeMunnyS.get", "PrizeMunnyM.get", "PrizeMunnyL.get", "PrizeFocusS.get", "PrizeFocusL.get"]);
}

#[test]
fn the_tables_of_the_audio_seam() {
    // Sheet sora-sounds, rows code.voice_group_no.
    assert_eq!([0x25, 0x4D, 0x4E, 0x5D, 0x67, 0xB6, 0xB7, 0x01].map(attack_voice_group_no), [52, 54, 55, 1, 1, 2, 2, 0]);
    assert_eq!(DEATH_VOICE_GROUP_NO, 1);
    assert_ne!(ACTOR_PAWN_HIT, ACTOR_MAP_HIT);
    assert_eq!([0, 1, 2].map(cure_heal_effect), ["vm_cur_hed0", "vm_cur_hed1", "vm_cur_hed2"]);
    assert_eq!(item_heal_effect(ItemKind::Potion as u32), Some("vs_itm_pot0"));
    assert_eq!(item_heal_effect(ItemKind::Ether as u32), Some("vs_itm_etr0"));
    assert_eq!(item_heal_effect(ItemKind::Tent as u32), None);
}
