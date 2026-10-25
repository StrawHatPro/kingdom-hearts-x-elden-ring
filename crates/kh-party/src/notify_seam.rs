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
#[allow(dead_code, clippy::all)]
pub(crate) mod full {
    pub mod types {
        #![allow(dead_code, clippy::all)]
        include!(concat!(env!("KH_PARTY_GEN_DIR"), "/party_anim_types.rs"));
    }
    pub mod ex001 {
        #![allow(dead_code, unused_imports, clippy::all)]
        use super::types::*;
        include!(concat!(env!("KH_PARTY_GEN_DIR"), "/party_anim_n_ex001.rs"));
    }
    pub mod ex002 {
        #![allow(dead_code, unused_imports, clippy::all)]
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
        NotifyData::Effect { entries } => entries.iter().filter_map(|e| Some(SeamNotify::Effect { effect: e.effect?, socket: e.socket, group_id: e.group_id })).collect(),
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
