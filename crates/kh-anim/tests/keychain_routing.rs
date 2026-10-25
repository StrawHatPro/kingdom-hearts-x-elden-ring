//! The Bonamik notify states of Sora's clips that are routed to the weapon's keychain instance
//! (integration pass, 2026-10-12; crates/kh-anim/src/keychain.rs "ROUTED NOTIFIES"): a chara wind
//! that names group KEY blows the chain, one that names only Sora's groups does not, and the
//! speed-rate notify slows the chain's simulated time and restores it. On the real weapon pack
//! (build/sora_pack/weapon); without it the test says so.

use std::path::PathBuf;

use glam::{Quat, Vec3};
use kh_anim::keychain::{chara_wind_from_props, speed_rate_from_props, SpeedRate};
use kh_anim::{Keychain, Trs};

fn chain() -> Option<Keychain> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/weapon");
    if !dir.join("data/bonamik.json").is_file() {
        eprintln!("SKIPPED: no weapon pack at {}", dir.display());
        return None;
    }
    Some(Keychain::load(&dir).expect("the keychain sets up"))
}

/// The weapon held still, blade up (the pose of tests/keychain.rs at rest).
fn held() -> Trs {
    let q = Quat::from_rotation_y((-90.0f32).to_radians());
    Trs { translation: Vec3::new(0.0, 0.0, 160.0) - q * Vec3::new(60.0, 0.0, 0.0), rotation: q, scale: Vec3::ONE }
}

fn tip(chain: &Keychain) -> Vec3 {
    held().transform_point(chain.pose().last().unwrap().translation)
}

/// The properties as the generated tables carry them (clip 0851S's wind, shortened group list).
const WIND_KEY: [(&str, &str); 6] = [("m_Groups", "[\"Hair\",\"KEY\",\"SLEEVE\"]"), ("m_DirectionSpace", "\"ESQEX_Bonamik_EmissiveDirection_WorldSpace\""), ("m_DirectionVec", "{\"x\":0,\"y\":1,\"z\":0}"), ("m_BaseStrength", "600"), ("m_WaveAmplitude", "4"), ("m_WavePeriod", "0.03")];
const WIND_OTHER: [(&str, &str); 6] = [("m_Groups", "[\"JCT\",\"SLEEVE\"]"), ("m_DirectionSpace", "\"ESQEX_Bonamik_EmissiveDirection_WorldSpace\""), ("m_DirectionVec", "{\"x\":0,\"y\":1,\"z\":0}"), ("m_BaseStrength", "600"), ("m_WaveAmplitude", "4"), ("m_WavePeriod", "0.03")];

#[test]
fn properties_parse_with_the_class_defaults() {
    let wind = chara_wind_from_props(&WIND_KEY).unwrap();
    assert_eq!((wind.direction_space, wind.direction_vec, wind.base_strength, wind.groups.len()), (2, [0.0, 1.0, 0.0], 600.0, 3));
    assert!(chara_wind_from_props(&[("m_BaseStrength", "1")]).is_none(), "no direction: not a wind");
    // Only m_BeginSpeedRate is serialized in Sora's clips: the rest is the constructor's.
    let speed = speed_rate_from_props(&[("m_BeginSpeedRate", "0.10000000149011612")]);
    assert_eq!(speed, SpeedRate { begin_rate: 0.1, ..SpeedRate::default() });
    assert!(speed.restore_original && speed.apply_to_children && speed.end_blend == 0.0);
    assert!(!speed_rate_from_props(&[("m_ApplyToChildren", "false")]).apply_to_children);
}

#[test]
fn a_wind_that_names_key_blows_the_chain_and_others_do_not() {
    let (Some(mut still), Some(mut blown), Some(mut other)) = (chain(), chain(), chain()) else { return };
    let world = held();
    // Settle all three for 3 s.
    for _ in 0..180 {
        still.step(1.0 / 60.0, &world, None);
        blown.step(1.0 / 60.0, &world, None);
        other.step(1.0 / 60.0, &world, None);
    }
    let rest = tip(&still);
    assert!((tip(&blown) - rest).length() < 1.0e-3);
    let (key, sleeve) = (chara_wind_from_props(&WIND_KEY).unwrap(), chara_wind_from_props(&WIND_OTHER).unwrap());
    let mut pushed = 0.0f32;
    for _ in 0..120 {
        still.step(1.0 / 60.0, &world, None);
        blown.route(Some((1, &key)), None);
        blown.step(1.0 / 60.0, &world, None);
        other.route(Some((1, &sleeve)), None);
        other.step(1.0 / 60.0, &world, None);
        pushed = pushed.max((tip(&blown) - tip(&still)).y);
    }
    assert_eq!(blown.routed(), (true, None));
    assert!(pushed > 2.0, "the KEY wind (world +Y, strength 600) moved the chain's tip {pushed} cm");
    assert!((tip(&other) - tip(&still)).length() < 1.0e-3, "a wind on JCT / SLEEVE finds no group on the weapon");
    // The notify ends: the wind is cleared and the chain hangs again.
    for _ in 0..240 {
        still.step(1.0 / 60.0, &world, None);
        blown.route(None, None);
        blown.step(1.0 / 60.0, &world, None);
    }
    assert_eq!(blown.routed(), (false, None));
    assert!((tip(&blown) - tip(&still)).length() < 1.0, "back at rest: {:?} vs {:?}", tip(&blown), tip(&still));
}

#[test]
fn the_speed_rate_notify_slows_the_chain_and_restores() {
    let Some(mut chain) = chain() else { return };
    assert_eq!(chain.solver().speed_rate(), 1.0);
    let slow = speed_rate_from_props(&[("m_BeginSpeedRate", "0.10000000149011612")]);
    chain.route(None, Some((7, slow)));
    assert_eq!((chain.solver().speed_rate(), chain.routed()), (0.1, (false, Some(0.1))));
    // The same notify instance on later steps changes nothing; another instance replaces it.
    chain.route(None, Some((7, slow)));
    chain.route(None, Some((8, SpeedRate { begin_rate: 0.8, ..SpeedRate::default() })));
    assert_eq!(chain.solver().speed_rate(), 0.8);
    // End: m_RestoreOriginalSpeedRate (the default) gives back the rate from before the begin.
    chain.route(None, None);
    assert_eq!((chain.solver().speed_rate(), chain.routed()), (1.0, (false, None)));
    // Without the restore flag the end sets m_EndSpeedRate; a blend time ramps instead of jumping.
    chain.route(None, Some((9, SpeedRate { begin_rate: 0.5, begin_blend: 1.0, restore_original: false, end_rate: 0.25, ..SpeedRate::default() })));
    assert_eq!(chain.solver().speed_rate(), 1.0, "a blended begin starts from the rate in force");
    let world = held();
    for _ in 0..90 {
        chain.step(1.0 / 60.0, &world, None);
    }
    assert!((chain.solver().speed_rate() - 0.5).abs() < 1.0e-4, "after the 1 s blend: {}", chain.solver().speed_rate());
    chain.route(None, None);
    assert_eq!(chain.solver().speed_rate(), 0.25);
    // A notify that does not apply to children is not routed.
    chain.route(None, Some((10, SpeedRate { begin_rate: 0.1, apply_to_children: false, ..SpeedRate::default() })));
    assert_eq!(chain.solver().speed_rate(), 0.25);
}
