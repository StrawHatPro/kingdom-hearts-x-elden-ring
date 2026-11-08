//! (AUDIT-A) THE RANDOM VOICE LISTS OF THE CORE'S OWN CLIPS. A `TresAnimNotify_PlayVoice` with `m_RandomVoiceAssets` plays a
//! weighted draw of its list, never the same entry twice running (0x14056CAE0, `kh_audio::routes::pick_voice`). The audio
//! pack's notify rows (`notify.voice_random`) cover the main anim set and the StyleChange clips only: the replacement anim
//! sets of the formchanges (`p_ex001_AnimSet2` .. `23`, e.g. Double Arrowguns' 2411, the Counter Shield's B412, Mirage
//! Staff's A411) are not in them, so the seam's name of such a notify (its first entry) played that first entry every time.
//! The lists are KH3's own data in the core's clip table (`kh_core::anim::SEQUENCES`, generated from the clips): every list
//! the pack lacks is added to the host's routes under the rows that play the clip (their names as the core plays them) and
//! the clip's own name. Nothing in the pack is changed.

use kh_audio::routes::RandomVoice;
use kh_audio::Routes;
use kh_core::anim::{NotifyData, ANIMS, SEQUENCES};

/// Every random voice list of the core's clips: one per (clip, notify), with the rows that play the clip.
pub fn core_random_voices() -> Vec<RandomVoice> {
    let mut out: Vec<(usize, RandomVoice)> = Vec::new();
    for anim in ANIMS.iter() { // (PACKGEN) a run-time table in the release build (kh-gendata)
        for &(_, index) in anim.samples {
            let Some(seq) = SEQUENCES.get(index) else { continue };
            for (k, n) in seq.notifies.iter().enumerate() {
                let NotifyData::Voice { random, asset: None, group: None, .. } = n.data else { continue };
                if random.is_empty() {
                    continue;
                }
                let key = index * 4096 + k;
                if let Some((_, list)) = out.iter_mut().find(|(i, _)| *i == key) {
                    if !list.logical.iter().any(|l| l == anim.name) {
                        list.logical.push(anim.name.to_string());
                    }
                    continue;
                }
                let entries = random.iter().map(|(asset, weight)| (kh_audio::pack::asset_key(asset).to_string(), u32::try_from(*weight).unwrap_or(0))).collect();
                let mut logical = vec![anim.name.to_string()];
                if anim.name != seq.name {
                    logical.push(seq.name.to_string());
                }
                out.push((key, RandomVoice { id: format!("core.{}.PlayVoice.{k}", seq.name), logical, time: n.time, entries }));
            }
        }
    }
    out.into_iter().map(|(_, l)| l).collect()
}

/// Adds every list of the core's clips the routes lack (`Routes::add_random_voice`); how many were added.
pub fn add_core_random_voices(routes: &mut Routes) -> usize {
    core_random_voices().into_iter().filter(|l| routes.add_random_voice(l.clone())).count()
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_core_lists_name_their_clip_and_rows() {
        let lists = super::core_random_voices();
        assert!(!lists.is_empty());
        for l in &lists {
            assert!(!l.logical.is_empty() && !l.entries.is_empty(), "{l:?}");
            assert!(l.entries.iter().all(|e| !e.0.contains('.')), "asset keys: {l:?}");
        }
    }
}
