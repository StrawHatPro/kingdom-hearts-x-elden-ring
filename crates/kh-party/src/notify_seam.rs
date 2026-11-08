//! B's adapter from its clip notifies to stream E's neutral `SeamNotify` (contract E.3): the
//! pawn's fired notifies (`anim::FiredNotify`, B's runtime `ClipNotify` with class + window only)
//! are looked up in stream A's full sequence tables (`build/gen/rust/party_anim_n_ex00x.rs`,
//! `SEQUENCES[i].notifies` in A's `party_anim_types.rs` types, kh_core's field set), and their
//! seam part goes to `kh_core::host::party::PartyNotify::dispatch` in the pose tick (pawn.rs `step`).
//! Mapping (E.3): Sound / WeaponSe / AutoSe / Voice one each; one `Effect` per AttachEffect row with
//! an effect; `TresAnimNotify_PlayActorVoice` -> `ActorVoice` from its `m_ActorVoice` prop. Without
//! A's tables (cfg `kh_party_anim_full` off) nothing is dispatched.

use crate::anim::FiredNotify;
use crate::ids::Chara;
use kh_core::anim::NotifyPhase;
use kh_core::host::party::{PartyNotifyEvent, SeamNotify};

#[cfg(kh_party_anim_full)]
pub mod full {
    pub mod types {
        include!(concat!(env!("KH_PARTY_GEN_DIR"), "/party_anim_types.rs"));
    }
    pub mod ex001 {
        use super::types::*;
        include!(concat!(env!("KH_PARTY_GEN_DIR"), "/party_anim_n_ex001.rs"));
    }
    pub mod ex002 {
        use super::types::*;
        include!(concat!(env!("KH_PARTY_GEN_DIR"), "/party_anim_n_ex002.rs"));
    }
}

/// The seam requests of one fired notify of `sequence` (A's sequence name), with the sequence key
/// (its index in the member's table) for `PartyNotifyEvent::sequence`.
#[cfg(kh_party_anim_full)]
pub fn seam_of(chara: Chara, sequence: &str, fired: &FiredNotify, ordinal: usize) -> (u32, Vec<SeamNotify>) {
    use full::types::{NotifyData, SequenceData};
    let seqs: &[SequenceData] = match chara {
        Chara::Donald => &full::ex001::SEQUENCES[..],
        Chara::Goofy => &full::ex002::SEQUENCES[..],
    };
    let Some((key, seq)) = seqs.iter().enumerate().find(|(_, s)| s.name == sequence) else { return (u32::MAX, Vec::new()) };
    let n = fired.notify;
    // the same class at the same trigger time; else the `ordinal`-th notify of that class
    let same: Vec<_> = seq.notifies.iter().filter(|d| d.class.game_name() == n.class).collect();
    let Some(def) = same.iter().find(|d| (d.time - n.start).abs() < 2.0e-3).or_else(|| same.get(ordinal)).copied() else { return (key as u32, Vec::new()) };
    let out = match def.data {
        NotifyData::Sound { sound, attach_point, volume, pitch, fade_out, loop_end_sound, loop_end_pitch, .. } => vec![SeamNotify::Sound { sound, attach_point, volume, pitch, fade_out, loop_end_sound, loop_end_pitch }],
        NotifyData::WeaponSe { alias, socket, follow, volume, pitch, fade_out, .. } => vec![SeamNotify::WeaponSe { alias, socket, follow, volume, pitch, fade_out }],
        NotifyData::AutoSe { motion, source, fade_out } => vec![SeamNotify::AutoSe { name: motion.or(source).unwrap_or(""), fade_out }],
        NotifyData::Voice { group, asset, random, fade_out } => vec![SeamNotify::Voice { name: asset.or(group).or(random.first().map(|r| r.0)).unwrap_or(""), fade_out }],
        // (THE USER 2026-11-02: "this effect is not aligned properly") each row keeps its own place on its socket (KH3's
        // LocationOffset / RotationOffset / ScaleOffset): the shield bash's VN_slg_bsh0 is a metre ahead of vfx_root
        NotifyData::Effect { entries } => entries.iter().filter_map(|e| Some(SeamNotify::Effect { effect: e.effect?, socket: e.socket, group_id: e.group_id, offset: e.offset.into(), rotation: e.rotation.into(), scale: e.scale[0] })).collect(),
        NotifyData::None if n.class == "TresAnimNotify_PlayActorVoice" => def
            .props
            .iter()
            .find(|(k, _)| *k == "m_ActorVoice")
            .map(|(_, v)| vec![SeamNotify::ActorVoice { asset: v.trim_matches('"') }])
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    (key as u32, out)
}

/// (AUD) The hit sound of the attack window for `row` in A's sequence `sequence` (TresAnimNotifyState_Attack: its
/// m_HitSEAsset, else its m_HitSE alias "HIT1" ..): what a friend's melee hit plays on contact (hitfx.rs).
#[cfg(kh_party_anim_full)]
pub fn attack_hit_se(chara: Chara, sequence: &str, row: &str) -> Option<&'static str> {
    use full::types::{NotifyData, SequenceData};
    let seqs: &[SequenceData] = match chara {
        Chara::Donald => &full::ex001::SEQUENCES[..],
        Chara::Goofy => &full::ex002::SEQUENCES[..],
    };
    let seq = seqs.iter().find(|s| s.name == sequence)?;
    seq.notifies.iter().find_map(|d| match d.data {
        NotifyData::Attack { row: r, hit_se, hit_se_asset, .. } if r == row => hit_se_asset.or(hit_se),
        _ => None,
    })
}

/// (AUD) Without A's full tables: no window sound (hitfx.rs plays the weapon set's HIT1).
#[cfg(not(kh_party_anim_full))]
pub fn attack_hit_se(_chara: Chara, _sequence: &str, _row: &str) -> Option<&'static str> {
    None
}

#[cfg(not(kh_party_anim_full))]
pub fn seam_of(_chara: Chara, _sequence: &str, _fired: &FiredNotify, _ordinal: usize) -> (u32, Vec<SeamNotify>) {
    (u32::MAX, Vec::new())
}

/// The `PartyNotifyEvent`s of one tick's fired notifies (Tick phases are skipped: `dispatch` ignores them).
pub fn events_of(chara: Chara, sequence: &str, clip_notifies: &[crate::anim::ClipNotify], fired: &[FiredNotify], out: &mut Vec<PartyNotifyEvent>) {
    out.clear();
    for f in fired {
        if f.phase == NotifyPhase::Tick {
            continue;
        }
        let i = f.index as usize;
        let ordinal = clip_notifies[..i.min(clip_notifies.len())].iter().filter(|c| c.class == f.notify.class).count();
        let (key, seam) = seam_of(chara, sequence, f, ordinal);
        for data in seam {
            out.push(PartyNotifyEvent { sequence: key, index: f.index, state: f.notify.is_state(), phase: f.phase, data });
        }
    }
}

#[cfg(all(test, kh_party_anim_full))]
mod tests {
    use super::*;
    use glam::Vec3;

    /// The effect rows (name tail, socket, offset, rotation, scale) a clip's AttachEffect notifies hand the seam.
    fn rows(clip: &str) -> Vec<(&'static str, &'static str, Vec3, Vec3, f32)> {
        let c = crate::tables::clip(Chara::Goofy, clip);
        let fired: Vec<FiredNotify> = c.notifies.iter().enumerate().filter(|(_, n)| n.class.contains("AttachEffect")).map(|(i, n)| FiredNotify { index: i as u16, phase: NotifyPhase::Begin, notify: n }).collect();
        let mut out = Vec::new();
        events_of(Chara::Goofy, c.sequence, c.notifies, &fired, &mut out);
        out.iter()
            .filter_map(|e| match e.data {
                SeamNotify::Effect { effect, socket, offset, rotation, scale, .. } => Some((effect.rsplit('.').next().unwrap_or(effect), socket, offset, rotation, scale)),
                _ => None,
            })
            .collect()
    }

    /// (THE USER 2026-11-02, Goofy's lunge: "this effect is not aligned properly") KH3 puts the shield bash's hexagon
    /// VN_slg_bsh0 on vfx_root at LocationOffset (100, 0, 80): a metre ahead of his feet at shield height. The seam got
    /// the socket only, so it sat on his root, low and behind the lunge. Every row keeps its own place now: the ground
    /// and the air bash (the combo's Shove 3 and Counter Bash play the ground one), and the heavy bash's weapon rows.
    #[test]
    fn the_shield_bash_hexagon_keeps_its_place_a_metre_ahead_of_his_root() {
        for clip in ["SGuardShieldBash", "SGuardAirShieldBash"] {
            let r = rows(clip);
            assert!(r.contains(&("VN_slg_bsh0", "vfx_root", Vec3::new(100.0, 0.0, 80.0), Vec3::ZERO, 1.0)), "{clip}: {r:?}");
        }
        let heavy = rows("SGuardHeavyBash");
        assert!(heavy.contains(&("VN_bsh_inl1", "vfx_top", Vec3::ZERO, Vec3::new(180.0, 0.0, 0.0), 1.0)), "{heavy:?}");
        assert!(heavy.contains(&("VN_slg_hvy01", "vfx_top", Vec3::new(0.0, 0.0, -85.0), Vec3::new(90.0, 0.0, 0.0), 1.0)), "{heavy:?}");
        // the counter Shield Shove's OWNER row names "vfx_top", a socket only his shield has: KH3 puts it on his mesh
        // (+Y ahead), so (0, 180, 80) yaw 90 is 1.8 m ahead, 80 cm up, facing ahead; on no socket in his own frame
        let shove = rows("SGuardShieldBash2");
        assert!(shove.contains(&("VN_slg_hvy00", "None", Vec3::new(180.0, 0.0, 80.0), Vec3::ZERO, 1.0)), "{shove:?}");
    }
}
