//! (party stream E) THE AUDIO LOG OF A SCRIPTED PARTY FIGHT, offline: the requests Donald's and Goofy's states and
//! clips put on the audio seam during a short fight (as kh_core::host::party names them), through `kh_audio::Engine`
//! with the real pack, one engine step per 60 Hz step, written as the test app's `--audio-log` CSV.
//!
//!   cargo run -p kh-audio --example party_fight_log -- [OUT.csv]     (default build/sora_tmp/party/impl_e/fight_audio_log.csv)
//!
//! The script (the seam names are the ones the party code emits; sources in brackets):
//!   step   0  Donald / Goofy BattleStart (PlayVoiceGroup IfIdle, 0x141275210) - one unit each, numbers 1 / 2
//!   step  30  Donald MagicFire clip (1321): its voice notify (random list bt2160120/130/140do0) + the Fire cast
//!             effect vn_ex001_fir_cal00 (its SeAttach sound) + the Fire actor n_ex001_ProjectileMagFire_C spawned
//!   step  60  the Fire actor hits a dummy (m_PawnHitSEAsset se00301_022)
//!   step  90  Donald MagicFire again: Blizzard (vn_ex001_blz_cal00, n_ex001_ProjectileMagBlizzard_C) ...
//!   step 120  ... and its hit
//!   step 150  Donald MagicCast clip (1341) on Sora: voice group number 1 (slot 1326: target uid 1), notify "MagicCast"
//!             -> bt2160150do0; the Cure cast effect vn_ex001_cur_cal00
//!   step 200  Donald walks 1.5 s on surface 10 (rock), then 1.5 s on surface 13 (wood): his footsteps (FsTable_Donald),
//!             from the recorded walk of tests/data (both pawns use BoneSet_Sora1)
//!   step 450  Goofy is hit: the damage clip M_0201's voice notify goofy_damage_random1
//!   step 480  Goofy below a quarter of his HP: QuarterHP (PlayVoiceGroup Replace, 0x1412795F0), number 2
//!   step 520  Donald heals Goofy with a Potion: ItemUse clip notify GiveRecovery with number 1; Goofy RecoveryHP 1
//! The listener stands 4 m behind the party, as the test level's camera.

use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use glam::Vec3;
use kh_audio::engine::PartyVoiceMode;
use kh_audio::{BoneFrame, Engine, Language, Listener, Pack, SeamPlay};

const DT: f32 = 1.0 / 60.0;
const DONALD: u8 = 0;
const GOOFY: u8 = 1;

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| concat!(env!("CARGO_MANIFEST_DIR"), "/../../build/sora_tmp/party/impl_e/fight_audio_log.csv").to_string());
    let pack = Arc::new(Pack::load(Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../build/sora_pack/audio"))).expect("the audio pack"));
    let mut engine = Engine::new(pack, 1, Language::En);
    let listener = Listener { position: Vec3::new(-400.0, 0.0, 170.0), forward: Vec3::X, right: Vec3::Y };
    let donald_at = Vec3::new(0.0, -150.0, 0.0);
    let goofy_at = Vec3::new(0.0, 150.0, 0.0);
    let walk: Vec<(String, f32, BoneFrame)> = include_str!("../tests/data/sora_walk_run_jump.csv")
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .filter_map(|l| {
            let c: Vec<&str> = l.split(',').collect();
            let mut p = [Vec3::ZERO; 14];
            for (k, slot) in p.iter_mut().enumerate() {
                let f = |i: usize| c[4 + 3 * k + i].parse::<f32>().unwrap();
                *slot = Vec3::new(f(0), f(1), f(2)) + donald_at;
            }
            (c[2] == "WalkLoopN").then(|| (c[2].to_string(), c[3].parse().unwrap(), BoneFrame::from_positions(&p, c[1] == "1")))
        })
        .collect();
    let mut file = std::io::BufWriter::new(std::fs::File::create(&out).expect("the log file"));
    writeln!(file, "step,time,kind,cause,sound,category,file,volume,pitch,pan,distance,note").unwrap();
    let mut handle = 0x2000_0000u32;
    let mut next = |m: u8| {
        handle += 1;
        handle + m as u32 * 0x0100_0000
    };
    let at = |m: u8| if m == DONALD { donald_at } else { goofy_at } + Vec3::Z * 100.0;
    let play = |name: &str, m: u8, anim: Option<(&'static str, f32)>| SeamPlay { name: Box::leak(name.to_string().into_boxed_str()), position: at(m), volume: 1.0, pitch: 1.0, looping: false, spatial: true, anim };
    for step in 0..700u32 {
        match step {
            0 => {
                engine.party_seam_play(DONALD, next(DONALD), &play("BattleStart:1", DONALD, None), Some(PartyVoiceMode::IfIdle));
                engine.party_seam_play(GOOFY, next(GOOFY), &play("BattleStart:2", GOOFY, None), Some(PartyVoiceMode::IfIdle));
            }
            30 | 90 => {
                let (cast, actor) = if step == 30 { ("vn_ex001_fir_cal00", "n_ex001_ProjectileMagFire_C") } else { ("vn_ex001_blz_cal00", "n_ex001_ProjectileMagBlizzard_C") };
                engine.party_seam_play(DONALD, next(DONALD), &play("/Game/Sound/VOICE/Localization/ja/battle/NPC/DG/bt2160120do0", DONALD, Some(("MagicFire", 0.2))), None);
                engine.effect_spawned(next(DONALD), cast, at(DONALD));
                engine.effect_spawned(next(DONALD), actor, at(DONALD) + Vec3::X * 60.0);
            }
            60 => {
                engine.actor_hit(Some(next(DONALD)), "n_ex001_ProjectileMagFire_C", false, Vec3::new(500.0, 0.0, 100.0));
            }
            120 => {
                engine.actor_hit(Some(next(DONALD)), "n_ex001_ProjectileMagBlizzard_C", false, Vec3::new(500.0, 0.0, 100.0));
            }
            150 => {
                engine.set_party_voice_group_no(DONALD, 1);
                engine.party_seam_play(DONALD, next(DONALD), &play("MagicCast", DONALD, Some(("MagicCast", 0.3))), None);
                engine.effect_spawned(next(DONALD), "vn_ex001_cur_cal00", at(DONALD));
            }
            450 => {
                engine.party_seam_play(GOOFY, next(GOOFY), &play("/Game/Sound/VOICE/Localization/ja/battle/NPC/DG/goofy_damage_random1", GOOFY, Some(("M_0201", 0.0))), None);
            }
            480 => {
                engine.party_seam_play(GOOFY, next(GOOFY), &play("QuarterHP:2", GOOFY, None), Some(PartyVoiceMode::Replace));
            }
            520 => {
                engine.set_party_voice_group_no(DONALD, 1);
                engine.party_seam_play(DONALD, next(DONALD), &play("GiveRecovery", DONALD, Some(("ItemUse", 0.4))), None);
                engine.party_seam_play(GOOFY, next(GOOFY), &play("RecoveryHP:1", GOOFY, None), Some(PartyVoiceMode::Replace));
            }
            _ => {}
        }
        // Donald's walk: steps 200 .. 440, rock then wood
        if (200..380).contains(&step) {
            let k = (step - 200) as usize % walk.len();
            let (anim, time, frame) = &walk[k];
            let surface = if step < 290 { 10 } else { 13 };
            engine.party_auto_se(DONALD, DT, frame, surface, Some((anim, *time)), &listener, &|_| None);
        }
        engine.update(DT, &listener, &|_| Some(Vec3::ZERO));
        for l in engine.take_log() {
            let csv = |t: &str| if t.contains(',') || t.contains('"') { format!("\"{}\"", t.replace('"', "\"\"")) } else { t.to_string() };
            let (v, p, pan) = l.params.map_or((String::new(), String::new(), String::new()), |q| (format!("{:.4}", q.volume), format!("{:.4}", q.pitch), format!("{:.4}", q.pan)));
            writeln!(file, "{step},{:.4},{},{},{},{},{},{v},{p},{pan},{},{}", step as f32 * DT, l.kind, csv(&l.cause), csv(&l.sound), l.category, csv(&l.file), l.distance.map_or(String::new(), |d| format!("{d:.1}")), csv(&l.note)).unwrap();
        }
    }
    file.flush().unwrap();
    println!("party_fight_log: {out}");
    if !engine.unknown.is_empty() {
        println!("unknown names: {:?}", engine.unknown);
    }
}
