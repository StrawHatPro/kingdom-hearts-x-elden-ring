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
}
