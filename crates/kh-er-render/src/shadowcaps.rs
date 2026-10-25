//! (IMPL_ER23A) The Keyblade's part of the stand-in sun shadow (gfx.rs: `er.caps`, capsules in game space, cm).
//!
//! The Kingdom Key's own measures, from the pack's weapon manifest (`build/sora_pack/weapon/manifest.json`, `geometry`:
//! vertex extents of LOD 0 in the frame of the bone `wep`, UE cm, "+X runs from the grip to the tip of the blade"; socket
//! `vfx_top` at x = 79.04): the guard spans x -16 .. 8 and z -14.5 .. 14.5 around the grip, the shaft is 2.6 thick from
//! x 8 to 79, the teeth reach z -13.6 between x 60 and 76. (The keychain, x -56 .. -20, throws no shadow worth a capsule.)

use glam::{Quat, Vec3};
use kh_anim::Trs;

use crate::Weapon;

/// Index of the bone `wep` in the weapon skeleton (root `kh_sk`, `wep`, `chain1` .. `chain7`).
const WEP_BONE: usize = 1;

/// (end a, end b, radius) in the frame of the bone `wep`, cm.
pub const KEYBLADE_CAPSULES: [(Vec3, Vec3, f32); 3] = [
    // the shaft, from the guard to the tip
    (Vec3::new(-14.0, 0.0, 0.0), Vec3::new(77.0, 0.0, 0.0), 3.0),
    // the guard around the grip
    (Vec3::new(-4.0, 0.0, -9.5), Vec3::new(-4.0, 0.0, 9.5), 5.5),
    // the teeth
    (Vec3::new(68.0, 0.0, -3.0), Vec3::new(68.0, 0.0, -8.5), 5.5),
];

/// The Keyblade's capsules in game space. `wep` in the weapon's component: the pose's bone, or (no pose: the reference
/// pose is drawn) the reference one: turned half a turn about X, at the component's origin.
pub fn keyblade_capsules(weapon: &Weapon) -> Vec<(Vec3, Vec3, f32)> {
    let reference = Trs { rotation: Quat::from_xyzw(1.0, 0.0, 0.0, 0.0), ..Trs::IDENTITY };
    let wep = weapon.component.get(WEP_BONE).copied().filter(|t| t.translation.is_finite() && t.rotation.is_finite()).unwrap_or(reference);
    let to_world = wep.then(&weapon.to_world);
    KEYBLADE_CAPSULES.iter().map(|(a, b, radius)| (to_world.transform_point(*a), to_world.transform_point(*b), *radius)).filter(|(a, b, _)| a.is_finite() && b.is_finite()).collect()
}

/// (IMPL_ER44) A point on the blade in game space: `along` cm from the grip toward the tip (the bone `wep`'s +X; the tip
/// socket `vfx_top` is at 79). The Keyblade light sits there.
pub fn blade_point(weapon: &Weapon, along: f32) -> Option<Vec3> {
    let reference = Trs { rotation: Quat::from_xyzw(1.0, 0.0, 0.0, 0.0), ..Trs::IDENTITY };
    let wep = weapon.component.get(WEP_BONE).copied().filter(|t| t.translation.is_finite() && t.rotation.is_finite()).unwrap_or(reference);
    Some(wep.then(&weapon.to_world).transform_point(Vec3::X * along)).filter(|p| p.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The user: "the keyblade doesn't cast a shadow". Before, the capsule ran from the hand to the bone farthest from
    /// it = the keychain's last link; with no chain pose there was none.
    #[test]
    fn the_blade_has_its_capsules_with_and_without_a_chain_pose() {
        let hand = Trs { translation: Vec3::new(1000.0, 2000.0, 300.0), ..Trs::IDENTITY };
        // no pose at all (no keychain file): the three capsules, the shaft 91 cm long from the hand outward along +X
        let caps = keyblade_capsules(&Weapon { to_world: hand, component: Vec::new(), opacity: 1.0 });
        assert_eq!(caps.len(), 3);
        let (a, b, radius) = caps[0];
        assert!((b - a - Vec3::X * 91.0).length() < 1e-3 && radius == 3.0, "{a:?} {b:?}");
        assert!((b - hand.translation - Vec3::X * 77.0).length() < 1e-3, "the tip is 77 cm from the hand");
        // the teeth: on the side the half turn about X puts -Z (= +Z of the component)
        assert!(caps[2].1.z > hand.translation.z + 8.0);
        // a pose: the chain's links dangle 60 cm down from the pommel; the blade's capsules do not follow them
        let mut component = vec![Trs::IDENTITY; 9];
        component[1] = Trs { rotation: Quat::from_xyzw(1.0, 0.0, 0.0, 0.0), ..Trs::IDENTITY };
        for (k, link) in component.iter_mut().enumerate().skip(2) {
            link.translation = Vec3::new(-19.0, 0.0, -8.0 * (k - 1) as f32);
        }
        let posed = keyblade_capsules(&Weapon { to_world: hand, component, opacity: 1.0 });
        assert_eq!(posed, caps);
        // the weapon turned with the hand: the shaft turns with it
        let turned = Trs { rotation: Quat::from_rotation_z(std::f32::consts::FRAC_PI_2), ..hand };
        let caps = keyblade_capsules(&Weapon { to_world: turned, component: Vec::new(), opacity: 1.0 });
        assert!((caps[0].1 - caps[0].0 - Vec3::Y * 91.0).length() < 1e-3);
        // (IMPL_ER44) the Keyblade light's point: on the shaft, 77 cm = the shaft's end
        let w = Weapon { to_world: turned, component: Vec::new(), opacity: 1.0 };
        assert!((blade_point(&w, 77.0).unwrap() - caps[0].1).length() < 1e-3);
        assert!((blade_point(&w, 55.0).unwrap() - (turned.translation + Vec3::Y * 55.0)).length() < 1e-3);
    }
}
