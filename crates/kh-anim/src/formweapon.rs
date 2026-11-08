//! (XW) A WEAPON WITH ITS OWN SKELETON AND CLIP: the XS moves' form weapons (Hyper Hammer, Blizzard Blades, Ultima). KH3
//! gives each weapon actor its own anim instance, which plays the row of its anim set that has the logical name Sora plays
//! (`M_3416` on the Hyper Hammer = its clip 3416: the hammer's root and parts scale while Giant Stamp runs), else its
//! `m_DefaultAnimName` (`ResetPose`). build/sora_tmp/er/xw/tools/xw_form_weapons.js writes, per weapon, the skeleton
//! (`skeleton_xw.json`, the manifest format of [`Skeleton::from_manifest`]) and that clip (`anims_xw/<clip>.glb`, the
//! pack's conventions). Hosts (kh-app xsi_weapons.rs, kh-er-render xsiweapon.rs) sample it at the time of Sora's clip
//! and skin the weapon with the component-space result, as the Kingdom Key is skinned with its keychain's pose.
//!
//! Sampling is [`crate::pose::Animator::sample`]'s (translation linear, rotation `key_rotation`, scale `key_scale`, a bone
//! without a track at its reference pose, held at the clip's ends); the root's tracks apply (the weapon clips have no root
//! motion: the Hyper Hammer's grows from its root `kh_sk`).

use std::path::Path;

use crate::blend::{key_rotation, key_scale};
use crate::json::Json;
use crate::pack::{Clip, Skeleton};
use crate::pose::component_space;
use crate::Trs;

/// A form weapon's skeleton and the clip it plays for one move.
#[derive(Clone, Debug)]
pub struct WeaponRig {
    pub skeleton: Skeleton,
    pub clip: Clip,
    /// Per track of the clip: its bone in the skeleton (None: a track of a bone this mesh does not have).
    bound: Vec<Option<usize>>,
}

impl WeaponRig {
    /// From the skeleton file and the clip file (the paths weapons_xw.json names).
    pub fn load(skeleton: &Path, clip: &Path) -> Result<WeaponRig, String> {
        let text = std::fs::read_to_string(skeleton).map_err(|e| format!("{}: {e}", skeleton.display()))?;
        let json = Json::parse(text.trim_start_matches('\u{feff}')).map_err(|e| format!("{}: {e}", skeleton.display()))?;
        let skeleton = Skeleton::from_manifest(&json)?;
        let name = clip.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let clip = Clip::load(clip, &name)?;
        Ok(WeaponRig::new(skeleton, clip))
    }

    pub fn new(skeleton: Skeleton, clip: Clip) -> WeaponRig {
        let bound = clip.tracks.iter().map(|t| skeleton.index_of(&t.bone)).collect();
        WeaponRig { skeleton, clip, bound }
    }

    /// The clip's length (its last key), seconds.
    pub fn length(&self) -> f32 {
        self.clip.tracks.iter().flat_map(|t| [t.translation.times.last(), t.rotation.times.last(), t.scale.times.last()]).flatten().fold(0.0, |a: f32, b| a.max(*b))
    }

    /// The local pose at `time` (seconds).
    pub fn local(&self, time: f32) -> Vec<Trs> {
        let mut pose = self.skeleton.reference_pose();
        for (track, bone) in self.clip.tracks.iter().zip(&self.bound) {
            let Some(i) = *bone else { continue };
            if let Some((a, b, f)) = track.translation.around(time) {
                pose[i].translation = a.lerp(b, f);
            }
            if let Some((a, b, f)) = track.rotation.around(time) {
                pose[i].rotation = if f == 0.0 { a } else { key_rotation(a, b, f) };
            }
            if let Some((a, b, f)) = track.scale.around(time) {
                pose[i].scale = if f == 0.0 { a } else { key_scale(a, b, f) };
            }
        }
        pose
    }

    /// Every bone relative to the weapon's component (its socket on Sora) at `time`: what the skin is made from.
    pub fn component(&self, time: f32) -> Vec<Trs> {
        component_space(&self.skeleton, &self.local(time))
    }

    /// The reference pose relative to the component (the bind pose).
    pub fn reference(&self) -> Vec<Trs> {
        component_space(&self.skeleton, &self.skeleton.reference_pose())
    }
}

/// (DRILL-IDLE) A WEAPON'S DEFAULT CLIP, KH3's UTresAnimInstance play by name (0x14056ECE0): when the weapon's anim set has
/// no row of the name Sora plays (param_2), and `m_DefaultAnimName` (+0x420) is set and found, the instance plays THAT row
/// instead, its loop bit (mode bit 0) = `m_bDefaultAnimAlwaysLoopPlay` (+0x428); with `m_bDefaultAnimCheckSame` (+0x42C)
/// a default play that is already running (the slot instance's +0x18 bit 0, set by vt+0x6A0 for a default play) is kept, not
/// restarted. The values are each weapon's anim blueprint CDO (PACK/keyblades/anim_instances.json, tools/sora_keyblades_pack.js
/// phase `anim_instances`): Drill Punch's w_so053 plays its `ResetPose` 0000 LOOPED, which is what folds its rope (himo01's
/// X scale 0.01, himo16's 100) and spins its drill (dorill_all, one turn per 0.567 s). Without it the weapon showed its bind
/// pose: the rope stretched out 15 links and the drill head hanging 1.5 m away from the gauntlet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DefaultPlay {
    /// `m_bDefaultAnimAlwaysLoopPlay`
    pub looping: bool,
    /// `m_bDefaultAnimCheckSame`
    pub check_same: bool,
}

/// (DRILL-IDLE) The clock of a weapon's default clip ([`DefaultPlay`]), driven by what Sora plays: a weapon actor's anim
/// instance is told the name of each of his play calls; the default clip starts at the first call the weapon has no row
/// of, runs on its own clock (looping or held at its end) and restarts at each later such call unless `check_same`. A call
/// is seen here as a change of his clip's name (a loop of his that wraps is one call). Repeated calls with the same name and
/// time change nothing, so every pass of a frame may ask.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DefaultClock {
    last: Option<(String, f32)>,
    playing: bool,
    clock: f32,
}

impl DefaultClock {
    /// Sora plays `anim` at `time` (s) and the weapon has (`own_row`) or lacks its row of it; the default clip's time when the
    /// weapon plays its default, else None.
    pub fn advance(&mut self, anim: &str, time: f32, own_row: bool, play: DefaultPlay) -> Option<f32> {
        let new_call = self.last.as_ref().is_none_or(|(name, _)| name != anim);
        let step = match &self.last {
            // the same play: his clock moved on (or wrapped: the part of his new cycle)
            Some((name, before)) if name == anim => if time >= *before { time - before } else { time },
            // a new play call: as long as his new clip has run
            _ => time.max(0.0),
        };
        self.last = Some((anim.to_string(), time));
        if own_row {
            self.playing = false;
            return None;
        }
        if !self.playing || (new_call && !play.check_same) {
            self.playing = true;
            self.clock = time.max(0.0);
        } else {
            self.clock += step;
        }
        Some(self.clock)
    }

    /// The time to sample a default clip of `length` s at (looped, else held at its end).
    pub fn sample_time(clock: f32, length: f32, play: DefaultPlay) -> f32 {
        if play.looping && length > 0.0 {
            clock.rem_euclid(length)
        } else {
            clock.clamp(0.0, length.max(0.0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn staged(model: &str, clip: &str) -> Option<WeaponRig> {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../pack/xsi/weapons").join(model);
        let dir = if dir.is_dir() { dir } else { PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/er/xw/pack/xsi/weapons").join(model) };
        WeaponRig::load(&dir.join("skeleton_xw.json"), &dir.join("anims_xw").join(format!("{clip}.glb"))).ok()
    }

    /// (XW) the Hyper Hammer's own clip 3416 moves its parts (skipped where the staged files are not on this machine): at the
    /// start the reference pose; while Giant Stamp runs the root kh_sk grows to x2 (the hammer is giant), BH_Brim / Cactus1
    /// to x2.33 (with the root's x2 on top), and all of it is back at the end
    #[test]
    fn the_hyper_hammer_grows_with_its_own_clip() {
        let Some(rig) = staged("w_so052", "3416") else { return };
        assert!((rig.length() - 3.1).abs() < 0.02, "{}", rig.length());
        let root = rig.skeleton.index_of("kh_sk").unwrap();
        let brim = rig.skeleton.index_of("BH_Brim").unwrap();
        let reference = rig.reference();
        let max_root = (0..=93).map(|f| rig.local(f as f32 / 30.0)[root].scale.x).fold(0.0, f32::max);
        assert!((max_root - 2.0).abs() < 1.0e-3, "{max_root}");
        let max_brim = (0..=93).map(|f| rig.component(f as f32 / 30.0)[brim].scale.x).fold(0.0, f32::max);
        assert!(max_brim > 3.0, "the brim grows on top of its root: {max_brim}");
        // ends: the reference pose (the hammer as Sora holds it before and after)
        for t in [0.0, 3.1] {
            let c = rig.component(t);
            for (a, b) in c.iter().zip(&reference) {
                assert!((a.translation - b.translation).length() < 0.05 && (a.scale - b.scale).length() < 1.0e-3, "t {t}");
            }
        }
    }

    /// (XW) a weapon without a row for the move holds its ResetPose: Ultima's (the Kingdom Key set's 0000, the keychain
    /// tracks dropped) and the Blizzard Blades' are their bind pose
    #[test]
    fn a_weapon_without_its_own_row_holds_its_reset_pose() {
        for (model, clip) in [("w_so192", "0000"), ("w_so084", "0000"), ("w_so086", "0000")] {
            let Some(rig) = staged(model, clip) else { return };
            let (a, b) = (rig.component(0.1), rig.reference());
            assert_eq!(a.len(), b.len());
            for (x, y) in a.iter().zip(&b) {
                assert!((x.translation - y.translation).length() < 0.01 && x.rotation.dot(y.rotation).abs() > 0.9999, "{model}");
            }
        }
    }

    /// (DRILL-IDLE) the default clip's clock (KH3 0x14056ECE0): it starts at the first play the weapon has no row of, runs on
    /// across Sora's loop wraps, keeps running into his next row-less play with check-same (restarts without), stops while
    /// he plays one of its rows, and samples looped or held
    #[test]
    fn the_default_clip_runs_on_its_own_clock() {
        let same = DefaultPlay { looping: true, check_same: true };
        let fresh = DefaultPlay { looping: true, check_same: false };
        let mut c = DefaultClock::default();
        assert_eq!(c.advance("6001", 0.1, false, same), Some(0.1));
        assert_eq!(c.advance("6001", 0.1, false, same), Some(0.1), "the same call again changes nothing");
        assert!((c.advance("6001", 0.5, false, same).unwrap() - 0.5).abs() < 1.0e-6);
        // his idle wraps: the clock goes on
        assert!((c.advance("6001", 0.05, false, same).unwrap() - 0.55).abs() < 1.0e-6);
        // his walk (no row either): check-same keeps the running default
        assert!((c.advance("6010", 0.02, false, same).unwrap() - 0.57).abs() < 1.0e-6);
        // a row of its own: no default; the next row-less play starts it again
        assert_eq!(c.advance("M_6411", 0.3, true, same), None);
        assert_eq!(c.advance("6001", 0.0, false, same), Some(0.0));
        // without check-same a new row-less call restarts it
        let mut d = DefaultClock::default();
        d.advance("6001", 0.4, false, fresh);
        assert_eq!(d.advance("6010", 0.01, false, fresh), Some(0.01));
        // looped / held
        assert!((DefaultClock::sample_time(1.2, 0.5, same) - 0.2).abs() < 1.0e-5);
        assert!((DefaultClock::sample_time(1.2, 0.5, DefaultPlay::default()) - 0.5).abs() < 1.0e-6);
    }
}
