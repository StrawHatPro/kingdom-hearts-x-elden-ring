//! (KEYCHAIN-DUAL) OFFLINE TESTS: every Keyblade part a form holds has its own keychain (its own Bonamik on its own skeleton),
//! stepped by the shared [`super::PartChains`] (the player's, and each Sora copy's `Rig::chains`), placed by the renderer's own
//! `place_part` (what `draw_equipped_parts` / `step_parts` call) on Sora's skeleton from the pack: Double Form's Oathkeeper
//! and Oblivion, Light Form's Oathkeeper, Dark Form's Oblivion, Storm Flag's banner (w_so093, five 7-node rows); a copy's hand
//! keyblade by [`super::HandChain`].
//!
//! With `KEYCHAIN_DUAL_SHOTS=1` the runs are written as bone tracks (build/sora_tmp/weapons/KEYCHAIN-DUAL/shots/*.json) for the
//! offline pictures (build/sora_tmp/weapons/KEYCHAIN-DUAL/tools/plot.js).

use std::path::PathBuf;
use std::sync::OnceLock;

use glam::{Quat, Vec3};
use kh_anim::Trs;
use kh_er_render::keyblade::{FormPose, Table};
use kh_er_render::FormWeapon;

use super::{ChainIn, HandChain, PartChains};

/// No Animator, no clip.
const NO: ChainIn<'static> = ChainIn { animator: None, clip: (None, 0.0) };

const STEP: f32 = crate::sora::STEP;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The pack's keyblades table (None without the pack: the tests are skipped).
pub(super) fn table() -> Option<&'static Table> {
    static T: OnceLock<Option<Table>> = OnceLock::new();
    T.get_or_init(|| Table::read(&root().join("build/sora_pack"))).as_ref()
}

/// Sora's skeleton and his reference pose (component space).
fn sora() -> Option<&'static (kh_anim::Skeleton, Vec<Trs>)> {
    static S: OnceLock<Option<(kh_anim::Skeleton, Vec<Trs>)>> = OnceLock::new();
    S.get_or_init(|| {
        let pack = kh_anim::Pack::load(&root().join("build/sora_pack")).ok()?;
        let reference = kh_anim::bonamik::reference_pose(&pack.skeleton);
        Some((pack.skeleton, reference))
    })
    .as_ref()
}

fn ready() -> Option<(&'static Table, &'static (kh_anim::Skeleton, Vec<Trs>))> {
    let r = table().zip(sora());
    if r.is_none() {
        eprintln!("SKIPPED: no pack at {}", root().join("build/sora_pack").display());
    }
    r
}

/// Sora's component in the world at time `t` of a motion (GAME space, cm, Z up).
#[derive(Clone, Copy)]
enum Motion {
    /// standing still
    Idle,
    /// a swing: a 270-degree turn in 0.4 s, then held
    Swing,
    /// a run along +X at 600 cm/s for 1 s, then a stop
    Run,
}

fn to_world(m: Motion, t: f32) -> Trs {
    let (translation, yaw) = match m {
        Motion::Idle => (Vec3::ZERO, 0.0),
        Motion::Swing => (Vec3::ZERO, (t / 0.4).min(1.0) * 1.5 * std::f32::consts::PI),
        Motion::Run => (Vec3::X * 600.0 * t.min(1.0), 0.0),
    };
    Trs { translation, rotation: Quat::from_rotation_z(yaw), scale: Vec3::ONE }
}

/// The parts of a form placed on Sora's reference pose at `to_world` (keyblade.rs `place_part`, as `draw_equipped_parts`).
fn parts(t: &Table, sk: &(kh_anim::Skeleton, Vec<Trs>), keyblade: &str, form: &str, to_world: &Trs) -> Vec<FormWeapon> {
    parts_at(t, sk, keyblade, form, to_world, (None, 0.0))
}

/// [`parts`] with the parts playing their own clip of the logical name `clip.0` at `clip.1` (s).
fn parts_at(t: &Table, sk: &(kh_anim::Skeleton, Vec<Trs>), keyblade: &str, form: &str, to_world: &Trs, clip: (Option<&str>, f32)) -> Vec<FormWeapon> {
    let index_of = |b: &str| sk.0.index_of(b);
    let socket = crate::sora::weapon_socket();
    let pose = FormPose { index_of: &index_of, component: &sk.1, to_world, anim: clip.0, time: clip.1, hand_socket: &socket };
    t.form(keyblade, form, "").map(|f| f.weapons.iter().filter_map(|p| kh_er_render::keyblade::place_part(t, p, &pose)).collect()).unwrap_or_default()
}

/// One model's track of a run: per step its bones in the world, simulated and rigid (its clip's pose, no keychain).
#[derive(Default)]
struct Track {
    model: String,
    parents: Vec<Option<usize>>,
    sim: Vec<Vec<Vec3>>,
    rigid: Vec<Vec<Vec3>>,
}

impl Track {
    /// The largest distance of a bone from its rigid place over the steps from `from`.
    fn deviation(&self, from: usize) -> f32 {
        self.sim.iter().zip(&self.rigid).skip(from).flat_map(|(s, r)| s.iter().zip(r).map(|(a, b)| a.distance(*b))).fold(0.0, f32::max)
    }
    /// The mean of the last row of bones (the chain's / banner's ends) relative to the weapon's root at step `i`.
    fn tips(&self, i: usize) -> Vec3 {
        let leaves: Vec<usize> = (0..self.parents.len()).filter(|b| !self.parents.contains(&Some(*b))).collect();
        let s = &self.sim[i];
        leaves.iter().map(|&b| s[b] - s[0]).sum::<Vec3>() / leaves.len().max(1) as f32
    }
    fn finite(&self) -> bool {
        self.sim.iter().flatten().all(|p| p.is_finite())
    }
}

/// `steps` steps of `chains` under a motion: every part of the form, its simulated and rigid bones recorded.
fn run(chains: &mut PartChains, keyblade: &str, form: &str, motion: Motion, steps: usize) -> Vec<Track> {
    run_clip(chains, keyblade, form, motion, steps, None)
}

/// [`run`] with the parts playing their own clip of the logical name `clip` from its start (its notify states routed).
fn run_clip(chains: &mut PartChains, keyblade: &str, form: &str, motion: Motion, steps: usize, clip: Option<&str>) -> Vec<Track> {
    let Some((t, sk)) = ready() else { return Vec::new() };
    let mut tracks: Vec<Track> = Vec::new();
    for i in 0..steps {
        let w = to_world(motion, i as f32 * STEP);
        let at = (clip, i as f32 * STEP);
        let mut list = parts_at(t, sk, keyblade, form, &w, at);
        let rigid = list.clone();
        chains.step(STEP, &list, &ChainIn { animator: None, clip: at });
        chains.apply(&mut list, 1.0);
        for (f, r) in list.iter().zip(&rigid) {
            let model = f.model.trim_start_matches(kh_er_render::keyblade::PREFIX).to_string();
            let k = match tracks.iter().position(|x| x.model == model) {
                Some(k) => k,
                None => {
                    let parents = t.model(&model).and_then(|m| m.skeleton.clone()).map(|s| s.bones.iter().map(|b| b.parent).collect()).unwrap_or_default();
                    tracks.push(Track { model, parents, ..Track::default() });
                    tracks.len() - 1
                }
            };
            tracks[k].sim.push(f.component.iter().map(|b| b.then(&f.to_world).translation).collect());
            tracks[k].rigid.push(r.component.iter().map(|b| b.then(&r.to_world).translation).collect());
        }
    }
    tracks
}

/// (KEYCHAIN_DUAL_SHOTS=1) a run's tracks for the pictures.
fn shot(name: &str, tracks: &[Track]) {
    if std::env::var("KEYCHAIN_DUAL_SHOTS").is_err() {
        return;
    }
    let dir = root().join("build/sora_tmp/weapons/KEYCHAIN-DUAL/shots");
    let _ = std::fs::create_dir_all(&dir);
    let pts = |v: &Vec<Vec<Vec3>>| serde_json::Value::Array(v.iter().map(|f| serde_json::Value::Array(f.iter().map(|p| serde_json::json!([p.x, p.y, p.z])).collect())).collect());
    let json = serde_json::json!({ "name": name, "step": STEP, "tracks": tracks.iter().map(|t| serde_json::json!({ "model": t.model, "parents": t.parents, "sim": pts(&t.sim), "rigid": pts(&t.rigid) })).collect::<Vec<_>>() });
    let _ = std::fs::write(dir.join(format!("{name}.json")), serde_json::to_string(&json).unwrap_or_default());
}

fn track<'a>(tracks: &'a [Track], model: &str) -> &'a Track {
    tracks.iter().find(|t| t.model == model).unwrap_or_else(|| panic!("{model} is a part of the form"))
}

/// Double Form (Oathkeeper's form 18): Oathkeeper in his right hand and Oblivion in his left, EACH with its own keychain that
/// swings out under a turn and trails a run.
#[test]
fn keychain_dual_double_form_both_keyblades_swing_and_trail() {
    if ready().is_none() {
        return;
    }
    let mut chains = PartChains::default();
    let swing = run(&mut chains, "w_so200", "18", Motion::Swing, 60);
    shot("double_swing", &swing);
    let mut live = chains.live();
    live.sort();
    assert_eq!(live, ["w_so200", "w_so210"], "both Keyblades of Double Form simulate their keychains");
    for model in ["w_so200", "w_so210"] {
        let t = track(&swing, model);
        assert!(t.finite(), "{model}: finite");
        let d = t.deviation(1);
        eprintln!("{model}: swing deviation {d:.1} cm");
        assert!(d > 3.0 && d < 150.0, "{model}: its keychain swings out of its rigid place under the turn ({d:.1} cm)");
    }
    let mut idle = PartChains::default();
    let still = run(&mut idle, "w_so200", "18", Motion::Idle, 120);
    let mut moving = PartChains::default();
    let runs = run(&mut moving, "w_so200", "18", Motion::Run, 50);
    shot("double_run", &runs);
    for model in ["w_so200", "w_so210"] {
        let (a, b) = (track(&still, model).tips(119), track(&runs, model).tips(49));
        eprintln!("{model}: tips idle {a:?}, running {b:?}");
        assert!(b.x < a.x - 1.0, "{model}: its keychain trails the run (-X) ({:.1} cm)", b.x - a.x);
    }
}

/// The same form's parts drawn for a Sora copy use their own sims: a copy's steps never move the player's keychains.
#[test]
fn keychain_dual_copy_has_its_own_part_chains() {
    if ready().is_none() {
        return;
    }
    let mut alone = PartChains::default();
    let reference = run(&mut alone, "w_so210", "18", Motion::Swing, 40);
    let (mut player, mut copy) = (PartChains::default(), PartChains::default());
    let Some((t, sk)) = ready() else { return };
    let mut last = Vec::new();
    for i in 0..40 {
        let time = i as f32 * STEP;
        let mut mine = parts(t, sk, "w_so210", "18", &to_world(Motion::Swing, time));
        player.step(STEP, &mine, &NO);
        // the copy (Oblivion's Double Form, as the leader's clone) runs in the meantime
        let mut his = parts(t, sk, "w_so210", "18", &to_world(Motion::Run, time));
        copy.step(STEP, &his, &NO);
        copy.apply(&mut his, 1.0);
        player.apply(&mut mine, 1.0);
        last = vec![mine, his];
    }
    let mine = &last[0];
    for f in mine {
        let model = f.model.trim_start_matches(kh_er_render::keyblade::PREFIX);
        let alone_end = track(&reference, model).sim.last().cloned().unwrap_or_default();
        let now: Vec<Vec3> = f.component.iter().map(|b| b.then(&f.to_world).translation).collect();
        assert_eq!(now, alone_end, "{model}: the player's keychain is the same with or without a copy stepping");
    }
    let mut live = copy.live();
    live.sort();
    assert_eq!(live, ["w_so200", "w_so210"], "the copy's Double Form simulates both Keyblades");
    let copy_runs = run(&mut PartChains::default(), "w_so210", "18", Motion::Run, 40);
    shot("copy_double_run", &copy_runs);
    for model in ["w_so200", "w_so210"] {
        assert!(track(&copy_runs, model).deviation(1) > 1.0, "{model}: the copy's keychain moves");
    }
}

/// Light Form (Oathkeeper alone, form 16) and Dark Form (Oblivion alone, form 17): the one Keyblade part has its keychain.
#[test]
fn keychain_dual_light_and_dark_form_keychains() {
    if ready().is_none() {
        return;
    }
    for (keyblade, form, model, name) in [("w_so200", "16", "w_so200", "light_swing"), ("w_so210", "17", "w_so210", "dark_swing")] {
        let mut chains = PartChains::default();
        let swing = run(&mut chains, keyblade, form, Motion::Swing, 60);
        shot(name, &swing);
        assert_eq!(chains.live(), [model], "{name}: the form's Keyblade simulates its keychain");
        let d = track(&swing, model).deviation(1);
        assert!(d > 3.0 && d < 150.0, "{name}: it swings ({d:.1} cm)");
    }
}

/// Storm Flag (Wheel of Fate's form 0F): the banner (w_so093's five rows A..E, 6 dynamic nodes each, lateral and bending links)
/// hangs under gravity standing still, settles, and trails a run.
#[test]
fn keychain_dual_storm_flag_banner_hangs_and_trails() {
    if ready().is_none() {
        return;
    }
    let mut chains = PartChains::default();
    let idle = run(&mut chains, "w_so090", "0F", Motion::Idle, 180);
    shot("flag_idle", &idle);
    assert_eq!(chains.live(), ["w_so093"], "the flag simulates its banner");
    let flag = track(&idle, "w_so093");
    assert!(flag.finite(), "the banner stays finite");
    let d = flag.deviation(1);
    eprintln!("flag: idle deviation {d:.1} cm, tips {:?} -> {:?}", flag.tips(0), flag.tips(179));
    assert!(d > 3.0, "the banner moves from its authored shape under gravity ({d:.1} cm)");
    assert!(flag.tips(179).z < flag.tips(0).z, "the banner sags under gravity");
    let settle = flag.sim[178].iter().zip(&flag.sim[179]).map(|(a, b)| a.distance(*b)).fold(0.0, f32::max);
    // (the second node of each row stays on its 80-degree cone in a two-state cycle: the JS reference evaluator, tools/flag_ref.js, does the same)
    assert!(settle < 3.0, "it comes to rest hanging ({settle:.2} cm a step)");
    let mut moving = PartChains::default();
    let runs = run(&mut moving, "w_so090", "0F", Motion::Run, 60);
    shot("flag_run", &runs);
    let mut swinging = PartChains::default();
    shot("flag_swing", &run(&mut swinging, "w_so090", "0F", Motion::Swing, 60));
    let (a, b) = (flag.tips(179), track(&runs, "w_so093").tips(59));
    eprintln!("flag: tips idle {a:?}, running {b:?}");
    assert!(b.x < a.x - 2.0, "the banner trails the run ({:.1} cm)", b.x - a.x);
}

/// A copy's hand keyblade has ITS keychain (not the Kingdom Key's): Oblivion's 9 bones, swinging; the Kingdom Key: None (the
/// rig's own).
#[test]
fn keychain_dual_copy_hand_keyblade_own_keychain() {
    let Some((_, sk)) = ready() else { return };
    let mut hand = HandChain::default();
    let Some(r_buki) = sk.0.index_of("R_buki") else { panic!("R_buki") };
    let mut poses = Vec::new();
    for i in 0..40 {
        let w = crate::sora::weapon_socket().then(&sk.1[r_buki]).then(&to_world(Motion::Swing, i as f32 * STEP));
        poses.push(hand.step("w_so210", STEP, &w, &NO, i == 0).expect("Oblivion is not the Kingdom Key"));
    }
    assert_eq!(poses[0].len(), 9, "Oblivion's own skeleton");
    let moved = poses.iter().map(|p| p.iter().zip(&poses[0]).map(|(a, b)| a.translation.distance(b.translation)).fold(0.0, f32::max)).fold(0.0, f32::max);
    assert!(moved > 1.0, "its keychain swings ({moved:.1} cm)");
    assert!(hand.step(kh_er_render::keyblade::KINGDOM_KEY, STEP, &Trs::IDENTITY, &NO, false).is_none(), "the Kingdom Key: the rig's own keychain");
}

/// The drawn list takes the poses between the two latest steps; a part away for a step is reset when it is back (its form's
/// end and start, a switch); a part with no keychain (Storm Flag's transform unit w_so091) is left as drawn.
#[test]
fn keychain_dual_apply_interpolates_and_resets() {
    let Some((t, sk)) = ready() else { return };
    let mut chains = PartChains::default();
    for i in 0..10 {
        chains.step(STEP, &parts(t, sk, "w_so200", "18", &to_world(Motion::Swing, i as f32 * STEP)), &NO);
    }
    let drawn = parts(t, sk, "w_so200", "18", &to_world(Motion::Swing, 9.0 * STEP));
    let (mut at0, mut at1) = (drawn.clone(), drawn.clone());
    chains.apply(&mut at0, 0.0);
    chains.apply(&mut at1, 1.0);
    assert_ne!(at0[0].component, at1[0].component, "two different steps");
    let sim = chains.sims.iter().find(|s| s.model == "w_so200").expect("Oathkeeper's sim");
    let near = |a: &[Trs], b: &[Trs]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.translation.distance(y.translation) < 1.0e-3 && x.rotation.dot(y.rotation).abs() > 0.99999);
    assert!(near(&at0[0].component, &sim.before), "alpha 0: the step before");
    assert!(near(&at1[0].component, &sim.now), "alpha 1: the latest step");
    // the form ends for a step, then starts again: reset (no stretch from the old place)
    chains.step(STEP, &[], &NO);
    assert!(chains.live().is_empty(), "nothing live without the parts");
    chains.step(STEP, &parts(t, sk, "w_so200", "18", &to_world(Motion::Idle, 0.0)), &NO);
    let sim = chains.sims.iter().find(|s| s.model == "w_so200").expect("Oathkeeper's sim");
    assert_eq!(sim.before, sim.now, "a part back is drawn from its new place");
    // a model without a keychain is drawn as given
    let mut flag = parts(t, sk, "w_so090", "0F", &Trs::IDENTITY);
    flag.extend(t.form("w_so090", "0F", "").into_iter().flat_map(|f| f.transform.clone()).filter_map(|p| {
        let index_of = |b: &str| sk.0.index_of(b);
        let socket = crate::sora::weapon_socket();
        kh_er_render::keyblade::place_part(t, &p, &FormPose { index_of: &index_of, component: &sk.1, to_world: &Trs::IDENTITY, anim: None, time: 0.0, hand_socket: &socket })
    }));
    let before: Vec<Vec<Trs>> = flag.iter().map(|f| f.component.clone()).collect();
    chains.step(STEP, &flag, &NO);
    chains.apply(&mut flag, 0.5);
    for (f, b) in flag.iter().zip(&before) {
        if f.model.ends_with("w_so091") {
            assert_eq!(&f.component, b, "w_so091 has no keychain: as drawn");
        }
    }
}

/// Storm Flag's finisher clip F420 (the weapon's own clip of M_F420) blows its own CharaWind on the banner (model space
/// (0.4, 0.8, 1), 1500 + 800 wave over 1 s, 0 .. 7.2 s): routed to the flag's keychain, it holds the banner out and moving
/// where gravity alone lets it hang still.
#[test]
fn keychain_dual_storm_flag_finisher_wind() {
    if ready().is_none() {
        return;
    }
    let mut chains = PartChains::default();
    let blown = run_clip(&mut chains, "w_so090", "0F", Motion::Idle, 180, Some("M_F420"));
    shot("flag_f420_wind", &blown);
    let own: Vec<String> = chains.sims.iter().filter_map(|s| s.chain.as_ref()).flat_map(|c| c.own_states()).map(str::to_string).collect();
    assert_eq!(own, ["TresAnimNotifyState_Bonamik_CharaWind"], "F420's wind is in force on the banner");
    let mut still = PartChains::default();
    let hung = run_clip(&mut still, "w_so090", "0F", Motion::Idle, 180, Some("M_F420_none"));
    let (b, h) = (track(&blown, "w_so093"), track(&hung, "w_so093"));
    assert!(b.finite(), "finite in the wind");
    let moving = (150..180).map(|i| b.sim[i].iter().zip(&b.sim[i - 1]).map(|(x, y)| x.distance(*y)).fold(0.0, f32::max)).fold(0.0, f32::max);
    let apart = b.sim[179].iter().zip(&h.sim[179]).map(|(x, y)| x.distance(*y)).fold(0.0, f32::max);
    eprintln!("flag F420: tips {:?} (gravity alone {:?}), still moving {moving:.1} cm a step, {apart:.1} cm from the hanging banner", b.tips(179), h.tips(179));
    assert!(apart > 10.0, "the wind holds the banner away from where it hangs ({apart:.1} cm)");
}

/// (KEYCHAIN-DUAL follow-up 1) THE HAND KEYBLADES' OWN CLIP 1814 (SoKcIrradiate, every hand Keyblade's, the Kingdom Key's
/// included): while Sora plays it (0 .. 2 s) its SpeedRate (0.2, blend 0.2 s) and ChangeGroupParams states are in force on
/// the Keyblade's own keychain, and they end with it (the speed rate restored).
#[test]
fn keychain_dual_hand_keyblades_own_clip_1814() {
    let Some((t, sk)) = ready() else { return };
    let Some(r_buki) = sk.0.index_of("R_buki") else { panic!("R_buki") };
    let w = crate::sora::weapon_socket().then(&sk.1[r_buki]);
    for model in ["w_so010", "w_so080", "w_so200", "w_so210"] {
        let mut c = kh_er_render::keyblade::keychain_in(t, model).expect("a keychain").expect("it reads");
        for i in 0..30 {
            super::step_keychain(&mut c, model, STEP, None, &w, &ChainIn { animator: None, clip: (Some("1814"), 0.5 + i as f32 * STEP) });
        }
        let mut own: Vec<&str> = c.own_states();
        own.sort_unstable();
        assert_eq!(own, ["TresAnimNotifyState_Bonamik_ChangeGroupParams", "TresAnimNotifyState_Bonamik_SpeedRate"], "{model}: 1814's states in force");
        let slow = c.solver().speed_rate();
        assert!(slow < 0.5, "{model}: the chain's time runs at 1814's rate ({slow})");
        for i in 0..30 {
            super::step_keychain(&mut c, model, STEP, None, &w, &ChainIn { animator: None, clip: (Some("1814"), 2.1 + i as f32 * STEP) });
        }
        assert!(c.own_states().is_empty(), "{model}: 1814's states end with their window");
        assert!(c.solver().speed_rate() > 0.99, "{model}: the original rate is back ({})", c.solver().speed_rate());
    }
    // through the hand chain (the player's and every copy's): the same, by the logical name
    let mut hand = HandChain::default();
    hand.step("w_so200", STEP, &w, &ChainIn { animator: None, clip: (Some("SoKcIrradiate"), 0.5) }, true);
    assert_eq!(hand.chain.as_ref().map(|c| c.own_states().len()), Some(2), "Oathkeeper in his hand: 1814's states by its logical name");
}
