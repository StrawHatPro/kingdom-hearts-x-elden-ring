//! The playback model against a small synthetic pack (the manifest shape of
//! tools/sora_audio_pack.js): docs/sora-audio-spec.md sections 2 and 3. Each test names the
//! section it checks. Tests against the real pack are in tests/pack.rs.

use std::sync::Arc;

use glam::Vec3;
use kh_anim::json::Json;
use kh_audio::driver::{attenuation, curve};
use kh_audio::{Driver, Listener, Note, Pack, PlayOutcome, PlayParams, VoiceCommand, RULES};

const DT: f32 = 1.0 / 60.0;

fn seq(n: usize, group: u32, weight: u32, commands: &str) -> String {
    format!(r#"{{"sequence":{n},"group":{group},"weight":{weight},"zero_ones":[{{"name":"zeroone_distance_rate","target":"lowpass","points":[{{"x":0,"y":1,"curve":1}},{{"x":1,"y":0.3,"curve":0}}]}},{{"name":"zeroone_distance_rate","target":"auxSendDb","points":[{{"x":0,"y":-12,"curve":0}},{{"x":1,"y":12,"curve":0}}]}}],"commands":[{commands}]}}"#)
}

fn key_on(track: u32, looped: u32) -> String {
    format!(r#"{{"name":"keyOn","track":{track},"loopFlag":{looped}}},{{"name":"end"}}"#)
}

fn track(material: u32, commands: &str) -> String {
    format!(r#"{{"type":"MaterialTrack","material":{material},"commands":[{commands}{}{{"name":"end"}}]}}"#, if commands.is_empty() { "" } else { "," })
}

#[allow(clippy::too_many_arguments)]
fn sound(index: u32, kind: &str, category: (u32, &str), priority: i32, volume: f32, macro_start: u32, selection: &str, sequences: &[String], tracks: &[(u32, String)]) -> String {
    let tracks: Vec<String> = tracks.iter().map(|(k, t)| format!(r#""{k}":{t}"#)).collect();
    format!(
        r#""{index}":{{"index":{index},"number":{index},"type":"{kind}","category":{{"number":{},"name":"{}"}},"priority":{priority},"volume":{volume},"main_bus":{{"name":"3DSE"}},"aux_sends":[{{"volume":0.25}}],"flags":0,"macro_start":{macro_start},"macro_stop":0,"flags2":4,"release_seconds":0,"selection":{selection},"sequences":[{}],"tracks":{{{}}}}}"#,
        category.0,
        category.1,
        sequences.join(","),
        tracks.join(",")
    )
}

fn groups(list: &[(u32, usize)]) -> String {
    let g: Vec<String> = list.iter().enumerate().map(|(i, (weight, member))| format!(r#"{{"group":{i},"weight":{weight},"members":[{member}]}}"#)).collect();
    format!("[{}]", g.join(","))
}

const FOOT: (u32, &str) = (30, "SE_footstep");
const ACTION: (u32, &str) = (21, "SE_action");
const VOICE: (u32, &str) = (103, "Vo_BattlePC");

/// Macro 106 and 110 exactly as the game's configuration has them (manifest `config.macros`).
const MACROS: &str = r#"
{"number":1,"commands":[{"name":"filterSoundId","cmp":"==","relativeToSelf":true,"value":0},{"name":"filterElapsed","cmp":"<=","mode":0,"value":0.067},{"name":"cancelSelf","ifCount":">= 1","arg":0},{"name":"end"}]},
{"number":7,"commands":[{"name":"filterSoundId","cmp":"==","relativeToSelf":true,"value":0},{"name":"filterElapsed","cmp":"<=","mode":0,"value":0.533},{"name":"cancelSelf","ifCount":">= 1","arg":0},{"name":"end"}]},
{"number":2,"commands":[{"name":"filterSoundId","cmp":"==","relativeToSelf":true,"value":0},{"name":"filterElapsed","cmp":">=","mode":0,"value":0.067},{"name":"stopOthers","ifCount":">= 1","arg":0,"f1":0.05,"target":"all in set","fadeSeconds":0.05},{"name":"resetSet"},{"name":"filterSoundId","cmp":"==","relativeToSelf":true,"value":0},{"name":"filterElapsed","cmp":"<=","mode":0,"value":0.067},{"name":"cancelSelf","ifCount":">= 1","arg":0},{"name":"end"}]},
{"number":106,"commands":[{"name":"filterBankNumber","cmp":"==","relativeToSelf":true,"value":0},{"name":"filterSoundId","cmp":"==","relativeToSelf":true,"value":0},{"name":"cancelSelf","ifCount":">= 1","arg":0},{"name":"resetSet"},{"name":"filterBankNumber","cmp":"==","relativeToSelf":true,"value":0},{"name":"filterSoundId","cmp":"!=","relativeToSelf":true,"value":0},{"name":"filterPriority","cmp":"<","relativeToSelf":true,"value":0},{"name":"cancelSelf","ifCount":">= 1","arg":0},{"name":"resetSet"},{"name":"filterBankNumber","cmp":"==","relativeToSelf":true,"value":0},{"name":"filterSoundId","cmp":"!=","relativeToSelf":true,"value":0},{"name":"filterPriority","cmp":">=","relativeToSelf":true,"value":0},{"name":"stopOthers","ifCount":">= 1","arg":0,"f1":0.066,"target":"all in set","fadeSeconds":0.066},{"name":"resetSet"},{"name":"filterBankNumber","cmp":"==","relativeToSelf":true,"value":1000},{"name":"filterPriority","cmp":">=","relativeToSelf":true,"value":64},{"name":"stopOthers","ifCount":">= 1","arg":0,"f1":0.066,"target":"all in set","fadeSeconds":0.066},{"name":"resetSet"},{"name":"filterBankNumber","cmp":"==","relativeToSelf":true,"value":1000},{"name":"filterPriority","cmp":"<","relativeToSelf":true,"value":64},{"name":"cancelSelf","ifCount":">= 1","arg":0},{"name":"resetSet"},{"name":"filterBankNumber","cmp":"==","relativeToSelf":true,"value":2000},{"name":"cancelSelf","ifCount":">= 1","arg":0},{"name":"end"}]},
{"number":110,"commands":[{"name":"filterBankNumber","cmp":"==","relativeToSelf":true,"value":0},{"name":"filterMacro","cmp":"==","relativeToSelf":true,"value":0},{"name":"cancelSelf","ifCount":">= 1","arg":0},{"name":"resetSet"},{"name":"filterBankNumber","cmp":"==","relativeToSelf":true,"value":0},{"name":"filterMacro","cmp":"!=","relativeToSelf":true,"value":0},{"name":"filterPriority","cmp":"<","relativeToSelf":true,"value":0},{"name":"cancelSelf","ifCount":">= 1","arg":0},{"name":"resetSet"},{"name":"filterBankNumber","cmp":"==","relativeToSelf":true,"value":0},{"name":"filterMacro","cmp":"!=","relativeToSelf":true,"value":0},{"name":"filterPriority","cmp":">=","relativeToSelf":true,"value":0},{"name":"stopOthers","ifCount":">= 1","arg":0,"f1":0.066,"target":"all in set","fadeSeconds":0.066},{"name":"end"}]}
"#;

/// The SE pool here holds 4 sounds (the game's holds 52), so that a test can fill it.
fn pack() -> Arc<Pack> {
    let one = |track_commands: &str| (vec![seq(0, 0, 0, &key_on(0, 0))], vec![(0u32, track(0, track_commands))]);
    let variations = |n: usize| -> (Vec<String>, Vec<(u32, String)>) { ((0..n).map(|i| seq(i, i as u32, 0, &key_on(i as u32, 0))).collect(), (0..n).map(|i| (i as u32, track(i as u32, ""))).collect()) };
    let (order_seqs, order_tracks) = variations(3);
    let (mut random_seqs, random_tracks) = variations(3);
    for (i, weight) in [50, 30, 20].into_iter().enumerate() {
        random_seqs[i] = seq(i, i as u32, weight, &key_on(i as u32, 0));
    }
    let (plain_seqs, plain_tracks) = one("");
    let (fade_seqs, fade_tracks) = one(r#"{"name":"volume","target":0,"ms":0,"curve":0},{"name":"volume","target":1,"ms":100,"curve":3},{"name":"randomPitch","min":-0.1,"max":0.1,"mode":1},{"name":"randomVolume","min":0.2,"max":1,"mode":2}"#);
    let timed = vec![seq(0, 0, 0, r#"{"name":"wait","ms":100},{"name":"keyOn","track":0,"loopFlag":0},{"name":"wait","ms":50},{"name":"keyOn","track":1,"loopFlag":0},{"name":"end"}"#)];
    let foot = [
        sound(1, "OrderSound", FOOT, 0, 0.6, 0, &format!(r#"{{"rule":"order","always":[],"rotate":{}}}"#, groups(&[(0, 0), (0, 1), (0, 2)])), &order_seqs, &order_tracks),
        sound(2, "RandomSound", FOOT, 0, 1.0, 0, &format!(r#"{{"rule":"random","always":[],"choose":{},"noRepeat":true}}"#, groups(&[(50, 0), (30, 1), (20, 2)])), &random_seqs, &random_tracks),
        sound(3, "Sound", ACTION, 128, 1.0, 1, r#"{"rule":"all"}"#, &plain_seqs, &plain_tracks),
        sound(4, "Sound", ACTION, 128, 1.0, 0, r#"{"rule":"all"}"#, &[seq(0, 0, 0, &key_on(0, 1))], &[(0, track(9, ""))]),
        sound(5, "Sound", ACTION, 128, 1.0, 0, r#"{"rule":"all"}"#, &fade_seqs, &fade_tracks),
        sound(6, "Sound", ACTION, 128, 1.0, 0, r#"{"rule":"all"}"#, &timed, &[(0, track(0, "")), (1, track(1, ""))]),
        sound(7, "Sound", ACTION, 64, 1.0, 0, r#"{"rule":"all"}"#, &plain_seqs, &plain_tracks),
        sound(8, "Sound", ACTION, 128, 1.0, 7, r#"{"rule":"all"}"#, &plain_seqs, &plain_tracks),
        sound(9, "Sound", ACTION, 128, 1.0, 2, r#"{"rule":"all"}"#, &[seq(0, 0, 0, &key_on(0, 1))], &[(0, track(9, ""))]),
    ];
    let voice = |index: u32, priority: i32, macro_start: u32| sound(index, "Sound", VOICE, priority, 1.0, macro_start, r#"{"rule":"all"}"#, &[seq(0, 0, 0, &key_on(0, 0))], &[(0, track(8, ""))]);
    let materials = |n: u32| -> String {
        let mut m: Vec<String> = (0..n).map(|i| format!(r#""{i}":{{"file":"wav/m{i:03}.wav","channels":1,"sample_rate":48000,"samples":4800,"loop":null}}"#)).collect();
        // A long line (1 s) and a loop (0.5 s, loop 0.25..0.5).
        m.push(r#""8":{"file":"wav/m008.wav","channels":1,"sample_rate":48000,"samples":48000,"loop":null}"#.to_string());
        m.push(r#""9":{"file":"wav/m009.wav","channels":1,"sample_rate":48000,"samples":24000,"loop":{"start":12000,"end":24000}}"#.to_string());
        m.join(",")
    };
    let bank = |name: &str, number: i32, sounds: &[String]| format!(r#""{name}":{{"bank_number":{number},"sounds":{{{}}},"materials":{{{}}}}}"#, sounds.join(","), materials(4));
    let manifest = format!(
        r#"{{"format":"kh3-sora-audio-pack/1","config":{{"categories":[
            {{"number":3,"name":"VOICE","max_sounds":12,"shares_pool_of":null}},
            {{"number":10,"name":"SE","max_sounds":4,"shares_pool_of":null}},
            {{"number":21,"name":"SE_action","max_sounds":0,"shares_pool_of":10}},
            {{"number":30,"name":"SE_footstep","max_sounds":0,"shares_pool_of":10}},
            {{"number":103,"name":"Vo_BattlePC","max_sounds":0,"shares_pool_of":3}}],
          "macros":[{MACROS}]}},
          "banks":{{{},{},{},{}}},
          "sound_assets":{{}},"triggers":[]}}"#,
        bank("SE/foot", 10010, &foot),
        bank("VOICE/Battle_Sora", 63000, &[voice(1, 128, 106), voice(2, 128, 106), voice(3, 64, 106), voice(4, 128, 110), voice(5, 128, 110), voice(6, 200, 106)]),
        bank("VOICE/Battle_Other", 62000, &[voice(1, 60, 106), voice(2, 128, 106)]),
        bank("VOICE/Battle_Third", 61000, &[voice(1, 128, 106)]),
    );
    let tables = r#"{"events":[
        {"name":"STATE_SLOW_START","commands":[{"op":"CategorySetVolume","category":"SE_footstep","volume":0.8,"fade":0.2,"layer":5},{"op":"CategorySetPitch","category":"SE_footstep","pitch":0.3,"fade":0.2},{"op":"CategorySetVolume","category":"Vo_BattlePC","volume":0,"fade":0.2,"layer":5}]},
        {"name":"STATE_SLOW_END","commands":[{"op":"CategorySetVolume","category":"SE_footstep","volume":1,"fade":0.2,"layer":5},{"op":"CategorySetPitch","category":"SE_footstep","pitch":1,"fade":0.2},{"op":"CategorySetVolume","category":"Vo_BattlePC","volume":1,"fade":0.2,"layer":5}]},
        {"name":"STATE_MENU_OPEN","commands":[{"op":"CategoryPause","category":"SE_action","fade":0.02}]},
        {"name":"STATE_MENU_CLOSE","commands":[{"op":"CategoryResume","category":"SE_action","fade":0.2}]},
        {"name":"CMD_STOP_RT_SOUNDS","commands":[{"op":"CategoryStop","category":"Vo_BattlePC","fade":0.5},{"op":"CategoryStop","category":"SE_footstep","fade":0.02}]}],
        "static_se":[]}"#;
    Arc::new(Pack::from_json(&Json::parse(&manifest).expect("manifest"), Some(&Json::parse(tables).expect("tables"))).expect("pack"))
}

const FOOT_BANK: u32 = 0;
const SORA: u32 = 1;
const OTHER: u32 = 2;
const THIRD: u32 = 3;

fn driver() -> Driver {
    let pack = pack();
    assert_eq!(pack.banks[FOOT_BANK as usize].name, "SE/foot");
    assert_eq!((pack.banks[SORA as usize].number, pack.banks[OTHER as usize].number, pack.banks[THIRD as usize].number), (63000, 62000, 61000));
    Driver::new(pack, 1)
}

fn flat() -> PlayParams {
    PlayParams::default()
}

fn listener() -> Listener {
    Listener::default()
}

/// The materials started by one update.
fn started(commands: &[VoiceCommand]) -> Vec<u32> {
    commands.iter().filter_map(|c| if let VoiceCommand::Start { material, .. } = c { Some(*material) } else { None }).collect()
}

fn play_and_start(d: &mut Driver, bank: u32, index: u32) -> Vec<u32> {
    assert!(matches!(d.play(bank, index, &flat()), PlayOutcome::Started(_)));
    started(&d.update(DT, &listener()))
}

#[test]
fn order_is_a_round_robin_whose_cursor_is_shared_by_all_instances() {
    // Spec 2.2: the cursor is a byte in the loaded bank, starts at group 0, advances by one per
    // start, whatever instance started it.
    let mut d = driver();
    let mut picks = Vec::new();
    for _ in 0..7 {
        picks.extend(play_and_start(&mut d, FOOT_BANK, 1));
    }
    assert_eq!(picks, [0, 1, 2, 0, 1, 2, 0]);
    // Two starts within one update still take consecutive variations.
    d.play(FOOT_BANK, 1, &flat());
    d.play(FOOT_BANK, 1, &flat());
    assert_eq!(started(&d.update(DT, &listener())), [1, 2]);
    // The cursor belongs to the bank's sound, not to the driver's RNG: another seed, same order.
    let mut other = Driver::new(pack(), 987_654);
    assert_eq!(play_and_start(&mut other, FOOT_BANK, 1), [0]);
}

#[test]
fn random_follows_the_weights_and_never_repeats() {
    // Spec 2.2: r = rand() % 101 against the running weight sum; the previous pick again ->
    // the heaviest other group.
    let mut d = driver();
    let mut counts = [0u32; 3];
    let mut last = None;
    let n = 6000;
    for _ in 0..n {
        let pick = play_and_start(&mut d, FOOT_BANK, 2);
        assert_eq!(pick.len(), 1);
        assert_ne!(Some(pick[0]), last, "an immediate repeat");
        last = Some(pick[0]);
        counts[pick[0] as usize] += 1;
        // Let it play out (0.1 s) so that the pool never fills.
        for _ in 0..8 {
            d.update(DT, &listener());
        }
    }
    // The stationary distribution of "roll 51 / 30 / 20 of 101, a repeat becomes the heaviest
    // other" (0 -> 1, 1 -> 0, 2 -> 0): p0 = 0.5385, p1 = 0.3884 ... worked out numerically here.
    let roll = [51.0 / 101.0, 30.0 / 101.0, 20.0 / 101.0];
    let mut p = [1.0f64 / 3.0; 3];
    for _ in 0..200 {
        let mut q = [0.0f64; 3];
        for from in 0..3 {
            for to in 0..3 {
                let to_real = if to == from { if from == 0 { 1 } else { 0 } } else { to };
                q[to_real] += p[from] * roll[to];
            }
        }
        p = q;
    }
    for k in 0..3 {
        let got = counts[k] as f64 / n as f64;
        assert!((got - p[k]).abs() < 0.03, "group {k}: {got:.3} vs {:.3} ({counts:?})", p[k]);
    }
    assert!(counts[0] > counts[1] && counts[1] > counts[2]);
}

#[test]
fn macro_1_cancels_a_repeat_of_the_same_sound_within_its_window() {
    // Spec 2.1: 1 / 3 / 5 / 7 = the same sound started at most 0.067 / .. / 0.533 s ago.
    let mut d = driver();
    assert!(matches!(d.play(FOOT_BANK, 3, &flat()), PlayOutcome::Started(_)));
    assert_eq!(d.play(FOOT_BANK, 3, &flat()), PlayOutcome::Cancelled { by_macro: 1 });
    for _ in 0..4 {
        d.update(DT, &listener());
    }
    // 3 steps old (0.05 s): still inside 0.067 s.
    assert_eq!(d.play(FOOT_BANK, 3, &flat()), PlayOutcome::Cancelled { by_macro: 1 });
    d.update(DT, &listener());
    d.update(DT, &listener());
    // 5 steps old (0.083 s): allowed again. Another sound was never affected.
    assert!(matches!(d.play(FOOT_BANK, 3, &flat()), PlayOutcome::Started(_)));
    assert!(matches!(d.play(FOOT_BANK, 7, &flat()), PlayOutcome::Started(_)));

    // Macro 7: 0.533 s.
    let mut d = driver();
    d.play(FOOT_BANK, 8, &flat());
    let mut allowed_at = None;
    for step in 1..60 {
        d.update(DT, &listener());
        if matches!(d.play(FOOT_BANK, 8, &flat()), PlayOutcome::Started(_)) {
            allowed_at = Some(step);
            break;
        }
    }
    // The first sound (0.1 s long) has ended by then, so nothing is left to compare with: a
    // one-shot shorter than the window can simply be started again once it is over.
    assert_eq!(allowed_at, Some(7));
}

#[test]
fn macro_2_restarts_an_older_copy_and_refuses_a_younger_one() {
    // Spec 2.1: copies older than 0.067 s are stopped with a 0.05 s fade; a younger copy
    // cancels the new sound.
    let mut d = driver();
    let PlayOutcome::Started(first) = d.play(FOOT_BANK, 9, &flat()) else { panic!() };
    assert_eq!(d.play(FOOT_BANK, 9, &flat()), PlayOutcome::Cancelled { by_macro: 2 });
    for _ in 0..7 {
        d.update(DT, &listener());
    }
    let PlayOutcome::Started(second) = d.play(FOOT_BANK, 9, &flat()) else { panic!("an older copy does not cancel") };
    assert!(d.take_notes().iter().any(|n| matches!(n, Note::MacroStop { key, by_macro: 2, .. } if *key == first)));
    // The old loop fades for 0.05 s (3 steps), then its voice is stopped.
    let mut stops = 0;
    for _ in 0..5 {
        stops += d.update(DT, &listener()).iter().filter(|c| matches!(c, VoiceCommand::Stop { .. })).count();
    }
    assert_eq!(stops, 1);
    assert!(!d.is_playing(first) && d.is_playing(second));
}

#[test]
fn battle_voices_are_arbitrated_by_bank_and_priority() {
    // Spec 2.1, macros 106 / 110.
    let mut d = driver();
    let PlayOutcome::Started(line1) = d.play(SORA, 1, &flat()) else { panic!() };
    d.update(DT, &listener());
    // The same line is playing -> the new one is cancelled.
    assert_eq!(d.play(SORA, 1, &flat()), PlayOutcome::Cancelled { by_macro: 106 });
    // A line of lower priority than the playing one -> cancelled.
    assert_eq!(d.play(SORA, 3, &flat()), PlayOutcome::Cancelled { by_macro: 106 });
    // Another line of the same priority -> the playing line is stopped (0.066 s) and it plays.
    let PlayOutcome::Started(line2) = d.play(SORA, 2, &flat()) else { panic!("an equal-priority line replaces") };
    for _ in 0..6 {
        d.update(DT, &listener());
    }
    assert!(!d.is_playing(line1) && d.is_playing(line2));
    // A higher-priority line replaces; the lower one cannot come back while it plays.
    let PlayOutcome::Started(shout) = d.play(SORA, 6, &flat()) else { panic!() };
    assert_eq!(d.play(SORA, 2, &flat()), PlayOutcome::Cancelled { by_macro: 106 });
    for _ in 0..6 {
        d.update(DT, &listener());
    }
    assert!(!d.is_playing(line2) && d.is_playing(shout));

    // Macro 110: a line with the same macro is playing -> cancelled; lines of the other macro
    // are arbitrated by priority.
    let mut d = driver();
    d.play(SORA, 4, &flat());
    d.update(DT, &listener());
    assert_eq!(d.play(SORA, 5, &flat()), PlayOutcome::Cancelled { by_macro: 110 });
    assert_eq!(d.play(SORA, 4, &flat()), PlayOutcome::Cancelled { by_macro: 110 });

    // The bank numbered 1000 LOWER: arbitrated by a priority difference of 64.
    let mut d = driver();
    let PlayOutcome::Started(other_quiet) = d.play(OTHER, 1, &flat()) else { panic!() };
    d.update(DT, &listener());
    // 128 - 60 >= 64: the other character's line is stopped.
    assert!(matches!(d.play(SORA, 1, &flat()), PlayOutcome::Started(_)));
    for _ in 0..6 {
        d.update(DT, &listener());
    }
    assert!(!d.is_playing(other_quiet));
    let mut d = driver();
    d.play(OTHER, 2, &flat());
    d.update(DT, &listener());
    // 128 - 128 < 64: the new line is cancelled.
    assert_eq!(d.play(SORA, 1, &flat()), PlayOutcome::Cancelled { by_macro: 106 });
    // The bank numbered 2000 lower playing anything -> cancelled.
    let mut d = driver();
    d.play(THIRD, 1, &flat());
    d.update(DT, &listener());
    assert_eq!(d.play(SORA, 1, &flat()), PlayOutcome::Cancelled { by_macro: 106 });
    // And a bank that is none of these does not interfere at all.
    let mut d = driver();
    d.play(SORA, 1, &flat());
    d.update(DT, &listener());
    assert!(matches!(d.play(OTHER, 2, &flat()), PlayOutcome::Started(_)));
}

#[test]
fn a_full_pool_steals_the_lowest_priority_oldest_first_and_drops_what_is_lower() {
    // Spec 2.1 item 2. The synthetic SE pool holds 4 (the game's 52); footsteps have priority 0.
    let mut d = driver();
    let PlayOutcome::Started(step1) = d.play(FOOT_BANK, 1, &flat()) else { panic!() };
    let PlayOutcome::Started(step2) = d.play(FOOT_BANK, 1, &flat()) else { panic!() };
    let PlayOutcome::Started(weapon) = d.play(FOOT_BANK, 7, &flat()) else { panic!() };
    let PlayOutcome::Started(loop1) = d.play(FOOT_BANK, 4, &flat()) else { panic!() };
    d.update(DT, &listener());
    assert_eq!(d.live_count(), 4);
    // Full. An action sound (128) takes the place of the OLDEST footstep, at once.
    let PlayOutcome::Started(action) = d.play(FOOT_BANK, 5, &flat()) else { panic!("a higher priority steals") };
    assert!(!d.is_playing(step1) && d.is_playing(step2));
    let commands = d.update(DT, &listener());
    assert_eq!(commands.iter().filter(|c| matches!(c, VoiceCommand::Stop { .. })).count(), 1, "{commands:?}");
    assert_eq!(started(&commands).len(), 1, "the new one starts on the next update");
    // Then the other footstep goes, then the priority-64 sound.
    assert!(matches!(d.play(FOOT_BANK, 4, &flat()), PlayOutcome::Started(_)));
    assert!(!d.is_playing(step2) && d.is_playing(weapon));
    assert!(matches!(d.play(FOOT_BANK, 4, &flat()), PlayOutcome::Started(_)));
    assert!(!d.is_playing(weapon));
    // Now everything in the pool is 128: a footstep (0) is dropped, a 64 is dropped ...
    assert_eq!(d.play(FOOT_BANK, 1, &flat()), PlayOutcome::Dropped);
    assert_eq!(d.play(FOOT_BANK, 7, &flat()), PlayOutcome::Dropped);
    // ... and an equal priority steals the oldest of them.
    assert!(matches!(d.play(FOOT_BANK, 4, &flat()), PlayOutcome::Started(_)));
    assert!(!d.is_playing(loop1) && d.is_playing(action));
    // A dropped footstep did not advance nothing it should not: voices have their own pool.
    assert!(matches!(d.play(SORA, 1, &flat()), PlayOutcome::Started(_)));
    assert_eq!(d.live_count(), 5);
}

#[test]
fn the_3d_shape_is_full_to_400_cm_and_linear_to_zero_at_4000() {
    // Spec 3.3.
    assert_eq!(attenuation(0.0), (1.0, 0.0));
    assert_eq!(attenuation(400.0), (1.0, 0.0));
    assert_eq!(attenuation(2200.0), (0.5, 0.5));
    assert_eq!(attenuation(4000.0), (0.0, 1.0));
    assert_eq!(attenuation(1.0e6), (0.0, 1.0));
    assert_eq!((RULES.attenuation_inner_radius, RULES.attenuation_max_distance), (400.0, 4000.0));

    // Through the driver: volume = sound volume x 3D volume; low-pass 1.0 -> 0.3 with curve 1 on
    // the first segment; reverb send -12 dB -> +12 dB times the aux send volume; pan from the
    // listener's axes.
    let mut d = driver();
    let l = Listener { position: Vec3::ZERO, forward: Vec3::X, right: Vec3::Y };
    let at = |d: &mut Driver, position: Vec3| -> kh_audio::VoiceParams {
        d.play(FOOT_BANK, 7, &PlayParams { position: Some(position), ..flat() });
        let commands = d.update(DT, &l);
        let Some(VoiceCommand::Start { params, .. }) = commands.iter().find(|c| matches!(c, VoiceCommand::Start { .. })) else { panic!("{commands:?}") };
        for _ in 0..8 {
            d.update(DT, &l);
        }
        *params
    };
    let near = at(&mut d, Vec3::new(300.0, 0.0, 0.0));
    assert_eq!((near.volume, near.lowpass, near.pan), (1.0, 1.0, 0.0));
    assert!((near.reverb_send - 0.25 * 10f32.powf(-12.0 / 20.0)).abs() < 1.0e-4, "{}", near.reverb_send);
    let half = at(&mut d, Vec3::new(0.0, 2200.0, 0.0));
    assert!((half.volume - 0.5).abs() < 1.0e-6 && (half.pan - 0.5).abs() < 1.0e-6, "{half:?}");
    // Curve 1 at t = 0.5: 1 - (1 - 0.25)^3 = 0.578125 of the way from 1.0 to 0.3.
    assert!((half.lowpass - (1.0 - 0.7 * 0.578_125)).abs() < 1.0e-5, "{}", half.lowpass);
    assert!((half.reverb_send - 0.25).abs() < 1.0e-5);
    let far = at(&mut d, Vec3::new(-5000.0, 0.0, 0.0));
    assert_eq!(far.volume, 0.0);
    assert!((far.lowpass - 0.3).abs() < 1.0e-6 && (far.pan.abs() - 1.0).abs() < 1.0e-6);
    let left = at(&mut d, Vec3::new(0.0, -100.0, 0.0));
    assert!((left.pan + 0.5).abs() < 1.0e-6);
    // A 2D sound is not attenuated.
    d.play(FOOT_BANK, 7, &flat());
    assert!(matches!(d.update(DT, &l)[..], [VoiceCommand::Start { params, .. }] if params.volume == 1.0 && params.lowpass == 1.0));

    // A sound that moves (attached to a bone) is updated.
    let mut d = driver();
    let PlayOutcome::Started(key) = d.play(FOOT_BANK, 4, &PlayParams { position: Some(Vec3::ZERO), ..flat() }) else { panic!() };
    d.update(DT, &l);
    d.set_position(key, Vec3::new(2200.0, 0.0, 0.0));
    assert!(matches!(d.update(DT, &l)[..], [VoiceCommand::Update { params, .. }] if (params.volume - 0.5).abs() < 1.0e-6));
}

#[test]
fn slow_motion_sets_the_se_categories_to_volume_0_8_and_pitch_0_3() {
    // Spec 3.4: STATE_SLOW_START, 0.2 s fade on the "slow" layer; battle voices volume 0.
    assert_eq!((RULES.event_slow_volume, RULES.event_slow_pitch), (0.8, 0.3));
    let mut d = driver();
    d.play(FOOT_BANK, 4, &flat());
    let PlayOutcome::Started(step) = d.play(FOOT_BANK, 1, &PlayParams { volume: 1.0, ..flat() }) else { panic!() };
    let _ = step;
    d.update(DT, &listener());
    assert!(d.event("STATE_SLOW_START"));
    assert!(!d.event("NO_SUCH_EVENT"));
    // Half way through the fade.
    for _ in 0..6 {
        d.update(DT, &listener());
    }
    let (volume, pitch) = d.category_state(30);
    assert!((volume - 0.9).abs() < 0.01 && (pitch - 0.65).abs() < 0.02, "{volume} {pitch}");
    for _ in 0..7 {
        d.update(DT, &listener());
    }
    assert_eq!(d.category_state(30), (0.8, 0.3));
    assert_eq!(d.category_state(103).0, 0.0);
    // Not named by the event: untouched.
    assert_eq!(d.category_state(21), (1.0, 1.0));
    // A footstep started now: volume 0.6 (sound) x 0.8, pitch 0.3; and it lasts 1 / 0.3 as long.
    d.play(FOOT_BANK, 1, &flat());
    let commands = d.update(DT, &listener());
    let Some(VoiceCommand::Start { params, .. }) = commands.iter().find(|c| matches!(c, VoiceCommand::Start { .. })) else { panic!() };
    assert!((params.volume - 0.48).abs() < 1.0e-6 && (params.pitch - 0.3).abs() < 1.0e-6);
    let before = d.live_count();
    for _ in 0..18 {
        d.update(DT, &listener());
    }
    assert_eq!(d.live_count(), before, "0.1 s of material at pitch 0.3 lasts 0.33 s");
    for _ in 0..4 {
        d.update(DT, &listener());
    }
    assert_eq!(d.live_count(), before - 1);
    assert!(d.event("STATE_SLOW_END"));
    for _ in 0..13 {
        d.update(DT, &listener());
    }
    assert_eq!(d.category_state(30), (1.0, 1.0));
}

#[test]
fn menu_pause_freezes_and_resumes_and_stop_rt_sounds_stops_with_a_fade() {
    // Spec 3.4.
    let mut d = driver();
    d.play(FOOT_BANK, 7, &flat());
    d.update(DT, &listener());
    d.update(DT, &listener());
    assert!(d.event("STATE_MENU_OPEN"));
    let mut paused = 0;
    for _ in 0..30 {
        paused += d.update(DT, &listener()).iter().filter(|c| matches!(c, VoiceCommand::Pause { paused: true, .. })).count();
    }
    // 0.1 s of material would long be over: it is held.
    assert_eq!((paused, d.live_count()), (1, 1));
    assert!(d.event("STATE_MENU_CLOSE"));
    let resumed = d.update(DT, &listener()).iter().filter(|c| matches!(c, VoiceCommand::Pause { paused: false, .. })).count();
    assert_eq!(resumed, 1);
    for _ in 0..6 {
        d.update(DT, &listener());
    }
    assert_eq!(d.live_count(), 0);

    let mut d = driver();
    let PlayOutcome::Started(voice) = d.play(SORA, 1, &flat()) else { panic!() };
    let PlayOutcome::Started(step) = d.play(FOOT_BANK, 1, &flat()) else { panic!() };
    let PlayOutcome::Started(action) = d.play(FOOT_BANK, 4, &flat()) else { panic!() };
    d.update(DT, &listener());
    assert!(d.event("CMD_STOP_RT_SOUNDS"));
    // Footsteps 0.02 s, voices 0.5 s; the action loop is not named by this synthetic event.
    d.update(DT, &listener());
    d.update(DT, &listener());
    d.update(DT, &listener());
    assert!(!d.is_playing(step) && d.is_playing(voice));
    for _ in 0..30 {
        d.update(DT, &listener());
    }
    assert!(!d.is_playing(voice) && d.is_playing(action));
}

#[test]
fn sequences_wait_and_key_on_with_the_overshoot_and_tracks_fade_and_randomise() {
    // Spec 2.3. Sound 6: wait 100 ms, keyOn track 0, wait 50 ms, keyOn track 1 (which releases
    // track 0), end.
    let mut d = driver();
    d.play(FOOT_BANK, 6, &flat());
    let mut log = Vec::new();
    for step in 0..20 {
        for c in d.update(DT, &listener()) {
            match c {
                VoiceCommand::Start { material, offset, .. } => log.push((step, "start", material, offset.round() as i64)),
                VoiceCommand::Stop { .. } => log.push((step, "stop", 0, 0)),
                _ => {}
            }
        }
    }
    // Step 0 is time 0; step 6 ends at 0.1 s exactly (float: the key-on may land on step 6 or 7).
    assert_eq!(log.len(), 3, "{log:?}");
    let (s0, s1) = (log[0].0, log[2].0);
    assert!(log[0].1 == "start" && log[0].2 == 0 && (6..=7).contains(&s0), "{log:?}");
    assert!(log[1].1 == "stop" && log[2].1 == "start" && log[2].2 == 1 && (3..=4).contains(&(s1 - s0)), "{log:?}");
    // The overshoot is the start offset: both key-ons happen inside a step, never before their time.
    let frames_per_step = 800.0;
    assert!((log[0].3 as f64) <= frames_per_step && (log[2].3 as f64) <= frames_per_step, "{log:?}");
    assert!((log[2].3 - log[0].3 - ((s1 - s0) as i64 * 800 - 2400)).abs() <= 1, "the second key-on is 50 ms after the first: {log:?}");

    // Sound 5: volume 0 -> 1 over 100 ms with curve 3 (t^2), random pitch -0.1..+0.1 added,
    // random volume 0.2..1 multiplied: drawn once.
    let mut pitches = Vec::new();
    let mut scales = Vec::new();
    for seed in 1..40 {
        let mut d = Driver::new(pack(), (seed as u32).wrapping_mul(2_654_435_761));
        d.play(FOOT_BANK, 5, &flat());
        let mut volumes = Vec::new();
        let mut pitch = 0.0;
        for _ in 0..5 {
            for c in d.update(DT, &listener()) {
                match c {
                    VoiceCommand::Start { params, .. } | VoiceCommand::Update { params, .. } => {
                        volumes.push(params.volume);
                        if pitch != 0.0 {
                            assert_eq!(pitch, params.pitch, "the random pitch is drawn once");
                        }
                        pitch = params.pitch;
                    }
                    _ => {}
                }
            }
        }
        assert!((0.9..=1.1).contains(&pitch), "{pitch}");
        pitches.push(pitch);
        // t^2 at steps 0..4 of a fade whose clock runs at the sound's pitch (1.0 here).
        assert_eq!(volumes.len(), 5);
        assert_eq!(volumes[0], 0.0);
        let scale = volumes[3] / curve(3, 3.0 * DT / 0.1);
        assert!((0.2..=1.0).contains(&scale), "{scale}");
        assert!((volumes[2] - scale * curve(3, 2.0 * DT / 0.1)).abs() < 1.0e-5 && volumes[4] > volumes[3]);
        scales.push(scale);
    }
    let spread = |v: &[f32]| v.iter().cloned().fold(f32::MIN, f32::max) - v.iter().cloned().fold(f32::MAX, f32::min);
    assert!(spread(&pitches) > 0.12 && spread(&scales) > 0.5, "{pitches:?} {scales:?}");
}

#[test]
fn a_one_shot_ends_with_its_material_and_a_loop_runs_until_stopped() {
    // Spec 2.5.
    let mut d = driver();
    let PlayOutcome::Started(shot) = d.play(FOOT_BANK, 7, &flat()) else { panic!() };
    let PlayOutcome::Started(looped) = d.play(FOOT_BANK, 4, &flat()) else { panic!() };
    let first = d.update(DT, &listener());
    assert!(first.iter().any(|c| matches!(c, VoiceCommand::Start { material: 9, looped: true, .. })));
    assert!(first.iter().any(|c| matches!(c, VoiceCommand::Start { material: 0, looped: false, .. })));
    for _ in 0..5 {
        d.update(DT, &listener());
    }
    assert!(d.is_playing(shot));
    d.update(DT, &listener());
    d.update(DT, &listener());
    // 0.1 s = 6 steps: over, without a Stop command (the voice ends by itself).
    assert!(!d.is_playing(shot));
    for _ in 0..600 {
        assert!(d.update(DT, &listener()).is_empty());
    }
    assert!(d.is_playing(looped));
    // Stop with a fade: the volume goes down, then the voice is stopped.
    d.stop(looped, Some(0.1));
    let mut volumes = Vec::new();
    let mut stopped = false;
    for _ in 0..8 {
        for c in d.update(DT, &listener()) {
            match c {
                VoiceCommand::Update { params, .. } => volumes.push(params.volume),
                VoiceCommand::Stop { .. } => stopped = true,
                _ => {}
            }
        }
    }
    assert!(volumes.len() >= 4 && volumes.windows(2).all(|w| w[1] < w[0]) && stopped && !d.is_playing(looped), "{volumes:?}");
}

#[test]
fn the_fade_curves_are_the_six_of_the_sheet() {
    // `curve.shapes`: t, 1-(1-t^2)^3, 1-(1-t)^2, t^2, 2^t-1, 2-2^(1-t).
    let t = 0.3f32;
    let expected = [t, 1.0 - (1.0 - t * t).powi(3), 1.0 - (1.0 - t) * (1.0 - t), t * t, 2f32.powf(t) - 1.0, 2.0 - 2f32.powf(1.0 - t)];
    for (n, e) in expected.into_iter().enumerate() {
        assert!((curve(n as u8, t) - e).abs() < 1.0e-6);
        assert_eq!((curve(n as u8, 0.0), curve(n as u8, 1.0)), (0.0, 1.0));
    }
}

#[test]
fn the_volume_and_pitch_are_clamped() {
    // Spec 2.4: volume 0..1, pitch 1/512..2.
    let mut d = driver();
    d.play(FOOT_BANK, 7, &PlayParams { volume: 5.0, pitch: 9.0, ..flat() });
    assert!(matches!(d.update(DT, &listener())[..], [VoiceCommand::Start { params, .. }] if params.volume == 1.0 && params.pitch == 2.0));
    d.play(FOOT_BANK, 7, &PlayParams { volume: 0.5, pitch: 0.0001, ..flat() });
    assert!(d.update(DT, &listener()).iter().any(|c| matches!(c, VoiceCommand::Start { params, .. } if params.volume == 0.5 && params.pitch == 1.0 / 512.0)));
}

#[test]
fn a_sound_to_the_listeners_right_pans_right_whatever_way_the_listener_faces() {
    // (ER62) The pan follows the listener's own axes, not the world's: a listener at (500, 500, 0)
    // facing +Y (its right is then -X, core axes being left-handed: X forward, Y right, Z up).
    let l = Listener { position: Vec3::new(500.0, 500.0, 0.0), forward: Vec3::Y, right: Vec3::NEG_X };
    let right = kh_audio::pan_of(&l, l.position + l.right * 300.0 + l.forward * 50.0);
    let left = kh_audio::pan_of(&l, l.position - l.right * 300.0);
    let ahead = kh_audio::pan_of(&l, l.position + l.forward * 300.0);
    let behind = kh_audio::pan_of(&l, l.position - l.forward * 300.0);
    assert!(right > 0.3 && right < 0.5, "{right}");
    assert!((left + 0.5).abs() < 1.0e-6, "{left}");
    assert!(ahead.abs() < 1.0e-6 && (behind.abs() - 1.0).abs() < 1.0e-6);
    // Through the mixer's pan law: louder in the right channel.
    let params = kh_audio::VoiceParams { volume: 1.0, pitch: 1.0, pan: right, pan_depth: 1.0, lowpass: 1.0, reverb_send: 0.0 };
    let [gl, gr] = kh_audio::mixer::gains(&params, false);
    assert!(gr > gl * 2.0, "{gl} {gr}");
    // And through the driver with that listener.
    let mut d = driver();
    d.play(FOOT_BANK, 7, &PlayParams { position: Some(l.position + l.right * 300.0), ..flat() });
    let commands = d.update(DT, &l);
    let Some(VoiceCommand::Start { params, .. }) = commands.iter().find(|c| matches!(c, VoiceCommand::Start { .. })) else { panic!("{commands:?}") };
    assert!((params.pan - 0.5).abs() < 1.0e-6, "{params:?}");
}
