//! (AUDIT-A SOUND) THE WEAPONS' AND PAWNS' OWN POINTS FOR THE AUDIO SIDE. KH3 plays the AttachSound / PlaySound / PlayVoice
//! notifies of a form weapon's or finisher pawn's own clip on THAT mesh: at its `AttachPointName` (a bone: the tower's
//! se02007_026 on `bigD_01`), at its component root (no attach point: the tower's se00001_034), at a socket
//! (`m_SocketName`: the tower's voices on `base`). The core names such a place as the effects do (kh-core forms/ship.rs
//! `wnotify_pass`, shield.rs `pawn_notifies`):
//! - `kb:<model>:<mount>:<bone>[:flags]`: a bone / socket of a form weapon or pawn (kh-host vfx/kbweapon.rs; the mount
//!   `tower` = FINISH_WA's tower actor, `accompany` = a finish's accompany pawn, `root` = his component);
//! - `pawn:<row>`: a shield finisher pawn's notify socket (kh-core `shield::pawn_socket`: Pegasus, the Chariot, the pan).
//!
//! Only the host's effect side knows those meshes (`EffectHost::socket_point`, the weapon rigs posed under Sora's clip, the
//! tower's own actor). A host hands the points of the names [`HostAudio::weapon_anchors`](super::HostAudio::weapon_anchors)
//! asks for to [`HostAudio::anchors`](super::HostAudio::anchors) before its step; the audio side places (and follows) the
//! sounds there. A name without a point (a host without an effect side): a `kb:` anchor sounds at its mount (one of his
//! sockets, else his origin), a `pawn:` anchor at its socket's track in his actor frame (the place these had before).

use std::collections::HashMap;

use glam::Vec3;
use kh_core::Attach;

use super::pose::PoseTap;
use crate::seam::{AudioCommand, EffectCommand};

/// A weapon's / pawn's own point (`kb:` or `pawn:`): what [`Points`] resolves.
pub fn is_weapon_anchor(name: &str) -> bool {
    name.starts_with("kb:") || name.starts_with("pawn:")
}

/// The weapon / pawn anchor of an attach (`Attach::Sora` on such a name).
pub fn anchor_of(at: &Attach) -> Option<&'static str> {
    match at {
        Attach::Sora { bone: Some(bone), .. } if is_weapon_anchor(bone) => Some(bone),
        _ => None,
    }
}

/// The names the step's sound plays and effect spawns (sounds carried by particle systems) hang on, plus those of `live`
/// attaches (followed every step), each once.
pub fn wanted<'a>(commands: &[AudioCommand], effects: &[EffectCommand], live: impl Iterator<Item = &'a Attach>) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    let mut add = |at: &Attach| {
        if let Some(name) = anchor_of(at) {
            if !out.contains(&name) {
                out.push(name);
            }
        }
    };
    for c in commands {
        if let AudioCommand::Play(_, play) = c {
            add(&play.at);
        }
    }
    for c in effects {
        if let EffectCommand::Spawn(_, spawn) = c {
            add(&spawn.at);
        }
    }
    for at in live {
        add(at);
    }
    out
}

/// The points of one step: the host's answers, and the clip time a `pawn:` socket's fallback track is read at.
pub struct Points<'a> {
    pub points: &'a [(&'static str, Vec3)],
    /// Sora's clip time of the step (his heaviest sample's: the pawns' clips run in step with his).
    pub time: f32,
}

impl Points<'_> {
    /// Where an attach on a weapon / pawn anchor is now; None: not such an attach. `offset` is added in world axes (the
    /// rows that use these anchors carry none).
    pub fn place(&self, at: &Attach, tap: &PoseTap, sockets: &HashMap<String, kh_audio::pack::Socket>, host: &dyn Fn(&Attach) -> Vec3) -> Option<Vec3> {
        let Attach::Sora { bone: Some(name), offset } = at else { return None };
        if !is_weapon_anchor(name) {
            return None;
        }
        if let Some(&(_, p)) = self.points.iter().find(|p| p.0 == *name) {
            return Some(p + *offset);
        }
        if name.starts_with("pawn:") {
            let p = kh_core::combat::forms::shield::pawn_socket(name, self.time).map_or(Vec3::ZERO, |p| p.0);
            return Some(host(&Attach::Sora { bone: None, offset: p + *offset }));
        }
        let mount = name.split(':').nth(2).unwrap_or("");
        Some(tap.point(mount, *offset, sockets).unwrap_or_else(|| host(&Attach::Sora { bone: None, offset: *offset })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_weapon_anchor_is_placed_at_the_hosts_point_else_at_its_mount() {
        let tap = PoseTap::default();
        let sockets = HashMap::new();
        let host = |at: &Attach| match at {
            Attach::Sora { bone: None, offset } => Vec3::new(1000.0, 0.0, 0.0) + *offset,
            _ => Vec3::splat(-1.0),
        };
        let given = [("kb:w_so073:tower:bigD_01", Vec3::new(5.0, 6.0, 7.0))];
        let p = Points { points: &given, time: 0.0 };
        let at = |bone| Attach::Sora { bone: Some(bone), offset: Vec3::ZERO };
        assert_eq!(p.place(&at("kb:w_so073:tower:bigD_01"), &tap, &sockets, &host), Some(Vec3::new(5.0, 6.0, 7.0)));
        // no point given: the mount (no such socket of his here: his origin)
        assert_eq!(p.place(&at("kb:w_so073:tower:base"), &tap, &sockets, &host), Some(Vec3::new(1000.0, 0.0, 0.0)));
        // not a weapon anchor: the caller's own rules
        assert_eq!(p.place(&at("R_wep_skt"), &tap, &sockets, &host), None);
        assert_eq!(p.place(&Attach::SORA, &tap, &sockets, &host), None);
        let wanted = wanted(&[], &[], [at("kb:w_so073:tower:bigD_01"), at("R_wep_skt"), at("kb:w_so073:tower:bigD_01"), at("pawn:x")].iter());
        assert_eq!(wanted, ["kb:w_so073:tower:bigD_01", "pawn:x"]);
    }
}
