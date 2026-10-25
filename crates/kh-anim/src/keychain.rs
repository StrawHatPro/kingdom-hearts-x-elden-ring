//! The Kingdom Key's keychain: a second [`Bonamik`] instance on the weapon's own skeleton
//! (`kh_sk > wep > chain1..chain7`) with the weapon's own asset `w_so010_Bonamik`
//! (`build/sora_pack/weapon/data/bonamik.json`: 3 collider spheres and 2 guard capsules on `wep`,
//! kinematic `chain1`, 6 dynamic nodes `chain2..7` in group KEY, cones 75 / 70 degrees).
//! docs/sora-vfx-spec.md 2.7; reference: `tools/lib/bonamik/weapon.js`.
//!
//! The asset has the class, version and schema of Sora's, and reaches no branch the solver
//! leaves untranscribed, so the solver is used unchanged.
//!
//! WHAT MOVES THE CHAIN. The weapon is an actor of its own, snapped to Sora's socket
//! `R_wep_skt`; its anim instance plays `ResetPose` (the reference pose) outside the two weapon
//! clips, so the chain bones never move by animation. All motion enters through the weapon mesh's
//! ComponentToWorld, which the solver reads at every evaluation (LOCAL_FORCE mode: the nodes are
//! carried with the component and `-lfCoeff x` the world travel is re-applied as lag, clamped by
//! `maxLF`), plus world gravity (0, 0, -981).
//!
//! NOTIFIES. `TresAnimNotifyState_Bonamik_CharaWind` begin / end call the KBD component with
//! "children" set (0x140562220 -> 0x143b5ef60 -> 0x143b53410, decompiled C): the wind of a clip
//! played on Sora also reaches the KBD components attached below his mesh, i.e. the weapon's.
//! [`Keychain::step`] therefore mirrors Sora's active character wind when its solver is given.
//! `ChangeGroupParams` (0x143b5f410) has no such flag and stays on Sora.
//!
//! ROUTED NOTIFIES (integration pass, 2026-10-12). Sora's own Animator runs only clip 0033's
//! Bonamik notifies, so the wind above reaches the chain for that clip alone. The host therefore
//! hands this instance the Bonamik notify states that are ACTIVE in the clip Sora plays
//! ([`Keychain::route`]), as the game's "children" calls do:
//! - `TresAnimNotifyState_Bonamik_CharaWind`: `set_chara_wind` with the notify's own properties.
//!   The weapon's core resolves the group names itself: a wind that names `KEY` (13 of Sora's
//!   clips, 0851S .. 0987S) blows on the chain, one that names only groups of Sora's asset (JCT,
//!   SLEEVE ...) finds none here, one that names no group blows on everything. The scale stays 1
//!   (the begin's value): the per-tick scale is the clip's float curve `Wind`, which the pack
//!   carries for no clip (clip 0033's constant 2 arrives through the mirror above).
//! - `TresAnimNotifyState_Bonamik_SpeedRate` (constructor 0x140559D00: begin rate 1.0, blend 0,
//!   restore the original rate at the end, end rate 1.0, end blend 0, `m_ApplyToChildren` true):
//!   begin = `SetSpeedRate(m_BeginSpeedRate, m_BeginBlendTime)`; end = `SetSpeedRate(the rate
//!   before the begin, or m_EndSpeedRate, m_EndBlendTime)`. The notify's Begin / End bodies were
//!   not read: this is what the properties and the setter (0x143B071F0) say.
//!
//! PORT DECISIONS: jitter zeroed (as for Sora); [`Keychain::reset`] asks for a soft reset (what
//! the solver's own teleport check does) and is meant to be called when the weapon is shown —
//! whether the game resets there was not determined; teleport distance 200 cm (class default,
//! see the pack's `user_data.TeleportDistance_evidence`).

use std::path::Path;

use crate::bonamik::notify::CharaWind;
use crate::bonamik::{self, Bonamik, BonamikHost, BonamikOptions};
use crate::json::Json;
use crate::pack::Skeleton;
use crate::Trs;

pub struct Keychain {
    skeleton: Skeleton,
    solver: Bonamik,
    /// The weapon's animated component-space pose: `ResetPose` = the mesh reference pose.
    reference: Vec<Trs>,
    /// The component-space pose of the latest step (the chain bones simulated).
    pose: Vec<Trs>,
    /// The bones the solver may write.
    driven: Vec<usize>,
    wind_mirrored: bool,
    /// The routed clip notifies in force: the key the host gave each (so that a new notify of
    /// the same kind replaces the old one) and, for the speed rate, how its end restores.
    routed_wind: Option<u64>,
    routed_speed: Option<(u64, SpeedRate, f32)>,
}

/// The properties of a `TresAnimNotifyState_Bonamik_SpeedRate`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpeedRate {
    pub begin_rate: f32,
    pub begin_blend: f32,
    pub restore_original: bool,
    pub end_rate: f32,
    pub end_blend: f32,
    pub apply_to_children: bool,
}

impl Default for SpeedRate {
    /// The class constructor 0x140559D00.
    fn default() -> SpeedRate {
        SpeedRate { begin_rate: 1.0, begin_blend: 0.0, restore_original: true, end_rate: 1.0, end_blend: 0.0, apply_to_children: true }
    }
}

fn prop<'a>(props: &'a [(&'a str, &'a str)], name: &str) -> Option<Json> {
    props.iter().find(|p| p.0 == name).and_then(|p| Json::parse(p.1).ok())
}

/// A chara-wind notify from its serialized properties (name, JSON text), as the generated
/// animation tables carry them (`kh_core::anim::NotifyDef::props`).
pub fn chara_wind_from_props(props: &[(&str, &str)]) -> Option<CharaWind> {
    let number = |name: &str| prop(props, name).and_then(|j| j.as_f64());
    let space = prop(props, "m_DirectionSpace");
    let direction_space = match space.as_ref().map(|s| (s.as_f64(), s.as_str())) {
        Some((Some(n), _)) => n as i32,
        Some((_, Some(s))) if s.contains("ModelSpace") => 1,
        Some((_, Some(s))) if s.contains("WorldSpace") => 2,
        _ => 0,
    };
    let v = prop(props, "m_DirectionVec")?;
    let direction_vec = [v.get("x").as_f64().unwrap_or(0.0), v.get("y").as_f64().unwrap_or(0.0), v.get("z").as_f64().unwrap_or(0.0)];
    let groups = prop(props, "m_Groups").map_or(Vec::new(), |g| g.items().iter().filter_map(|n| n.as_str()).map(str::to_string).collect());
    Some(CharaWind { direction_space, direction_vec, base_strength: number("m_BaseStrength")?, wave_amplitude: number("m_WaveAmplitude").unwrap_or(0.0), wave_period: number("m_WavePeriod").unwrap_or(0.0), groups })
}

/// A speed-rate notify from its serialized properties (the class defaults for what is absent).
pub fn speed_rate_from_props(props: &[(&str, &str)]) -> SpeedRate {
    let d = SpeedRate::default();
    let number = |name: &str, default: f32| prop(props, name).and_then(|j| j.as_f64()).map_or(default, |n| n as f32);
    let flag = |name: &str, default: bool| prop(props, name).map_or(default, |j| j.as_bool().unwrap_or(default));
    SpeedRate {
        begin_rate: number("m_BeginSpeedRate", d.begin_rate),
        begin_blend: number("m_BeginBlendTime", d.begin_blend),
        restore_original: flag("m_RestoreOriginalSpeedRate", d.restore_original),
        end_rate: number("m_EndSpeedRate", d.end_rate),
        end_blend: number("m_EndBlendTime", d.end_blend),
        apply_to_children: flag("m_ApplyToChildren", d.apply_to_children),
    }
}

impl Keychain {
    /// From the weapon pack directory (`build/sora_pack/weapon`): `manifest.json` (skeleton) and
    /// `data/bonamik.json`.
    pub fn load(weapon_dir: &Path) -> Result<Keychain, String> {
        let read = |path: std::path::PathBuf| -> Result<Json, String> {
            let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            Json::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
        };
        let manifest = read(weapon_dir.join("manifest.json"))?;
        let data = read(weapon_dir.join("data").join("bonamik.json"))?;
        Keychain::new(&manifest, &data)
    }

    pub fn new(manifest: &Json, data: &Json) -> Result<Keychain, String> {
        let skeleton = Skeleton::from_manifest(manifest)?;
        let reference = bonamik::reference_pose(&skeleton);
        let solver = Bonamik::with_reference_pose(data, &skeleton, &reference, BonamikOptions::default())?;
        let driven = solver.driven_bones();
        Ok(Keychain { skeleton, solver, pose: reference.clone(), reference, driven, wind_mirrored: false, routed_wind: None, routed_speed: None })
    }

    /// THE ROUTED NOTIFIES of the clip Sora plays, once per step BEFORE [`Keychain::step`]: the
    /// chara-wind notify state and the speed-rate notify state that are active now, each with a
    /// key that names the notify instance (the host's: play id and notify index), or `None`.
    /// A change of key is the old notify's End followed by the new one's Begin.
    pub fn route(&mut self, wind: Option<(u64, &CharaWind)>, speed: Option<(u64, SpeedRate)>) {
        if self.routed_wind != wind.map(|w| w.0) {
            if self.routed_wind.take().is_some() {
                self.solver.clear_chara_wind();
            }
            if let Some((key, props)) = wind {
                self.solver.set_chara_wind(props);
                self.routed_wind = Some(key);
            }
        }
        let speed = speed.filter(|s| s.1.apply_to_children);
        if self.routed_speed.map(|s| s.0) != speed.map(|s| s.0) {
            if let Some((_, old, original)) = self.routed_speed.take() {
                self.solver.set_speed_rate(if old.restore_original { original } else { old.end_rate }, old.end_blend);
            }
            if let Some((key, props)) = speed {
                let original = self.solver.speed_rate();
                self.solver.set_speed_rate(props.begin_rate, props.begin_blend);
                self.routed_speed = Some((key, props, original));
            }
        }
    }

    /// The routed notifies in force: (a wind is set, the speed-rate notify's begin rate).
    pub fn routed(&self) -> (bool, Option<f32>) {
        (self.routed_wind.is_some(), self.routed_speed.map(|s| s.1.begin_rate))
    }

    /// One simulation step. `weapon_to_world` = the weapon mesh's ComponentToWorld (Sora's
    /// `R_buki` in world space with the socket's relative transform; KH3 units). `sora` = Sora's
    /// solver, for the character wind his clips' notify states set. Returns the weapon's
    /// component-space pose.
    pub fn step(&mut self, dt: f32, weapon_to_world: &Trs, sora: Option<&Bonamik>) -> &[Trs] {
        match sora.and_then(|s| s.chara_wind_all_groups()) {
            Some(wind) => {
                self.solver.set_chara_wind_all_groups(&wind);
                self.wind_mirrored = true;
            }
            None if self.wind_mirrored => {
                self.solver.clear_chara_wind_all_groups();
                self.wind_mirrored = false;
                // A routed wind that the mirror overwrote is set again by the next `route`.
                self.routed_wind = None;
            }
            None => {}
        }
        self.pose.copy_from_slice(&self.reference);
        self.solver.frame(dt, &mut self.pose, weapon_to_world, &BonamikHost::default());
        &self.pose
    }

    /// The weapon became visible: a soft reset on the next evaluation (the nodes are re-attached
    /// rigidly to the chain root with zero local force, as after a teleport).
    pub fn reset(&mut self) {
        self.solver.request_reset(false);
    }

    /// The component-space pose of the latest step.
    pub fn pose(&self) -> &[Trs] {
        &self.pose
    }

    /// The weapon's animated (rigid) component-space pose.
    pub fn reference(&self) -> &[Trs] {
        &self.reference
    }

    pub fn skeleton(&self) -> &Skeleton {
        &self.skeleton
    }

    /// The bones the solver may write (`chain1..chain7`).
    pub fn driven_bones(&self) -> &[usize] {
        &self.driven
    }

    /// Solver evaluations so far (30 per second).
    pub fn evaluations(&self) -> u32 {
        self.solver.stats().steps
    }

    pub fn solver(&self) -> &Bonamik {
        &self.solver
    }
}
