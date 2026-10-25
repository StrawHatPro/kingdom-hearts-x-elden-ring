//! (FACE) LIP-SYNC FROM A VOICE LINE'S LOUDNESS, IN KH3'S OWN WAY OF AUTHORING LIPS.
//!
//! KH3 lip-syncs a battle line in the body clip that says it: `TresAnimNotify_FaceAnimLip` plays of the types
//! `MOUTH_OPEN_01` .. `MOUTH_OPEN_05` (one-frame mouth poses: rows NO_50 .. NO_54, BA_50 .. BA_54) at the syllables, with
//! the asset's own blend-in (0), and a lip play of no name (type NORMAL_AUTO: the face's own lip loop, NO_0) to close
//! it, blend 0.1 .. 0.2 (Donald's 1301 M_1301: PlayVoice 0.44, MOUTH_OPEN_03 0.47, close 0.64 / 0.1, MOUTH_OPEN_01 0.76,
//! close 0.89 / 0.1). A line that no clip authored (a voice group the code plays, a battle shout over another clip) gets
//! the same kind of track here, made from the line's loudness (`lip.json` of the audio pack, tools/face_lip_pack.js):
//! an open of the level of the syllable's loudness, a close at each pause.
//!
//! The track is a function of the line's own time: it plays the same at any frame rate (a host fires the keys whose
//! time it passed, as a clip's notifies).

/// The mouth types of KH3's lip plays (`ETresFaceAnimType` MOUTH_OPEN_01 .. 05), widest last.
pub const OPEN: [&str; 5] = ["MOUTH_OPEN_01", "MOUTH_OPEN_02", "MOUTH_OPEN_03", "MOUTH_OPEN_04", "MOUTH_OPEN_05"];

/// One lip play: at `time` (s of the line), an open of `OPEN[i]`, or a close (`None`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LipKey {
    pub time: f32,
    pub open: Option<u8>,
}

/// The track's numbers (OUR DESIGN, tuned on Donald's and Goofy's battle lines against KH3's authored lips: their
/// opens are 0.1 - 0.3 s apart, a close lasts 0.1 s or more).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LipParams {
    /// Under this loudness (of the line's peak) the mouth is closed.
    pub open_level: f32,
    /// From these loudnesses: MOUTH_OPEN_02, _03, _05 (below the first: _01).
    pub levels: [f32; 3],
    /// A pose is held at least this long before the next one (s).
    pub min_hold: f32,
    /// A pause must last this long to close the mouth (s).
    pub min_close: f32,
    /// A loudness this far below the syllable's peak is a new syllable (the mouth narrows one level).
    pub syllable_drop: f32,
    /// The mouth leads the sound by this much (s): it is open when the syllable is heard.
    pub lead: f32,
}

pub const PARAMS: LipParams = LipParams { open_level: 0.16, levels: [0.33, 0.52, 0.75], min_hold: 0.066, min_close: 0.05, syllable_drop: 0.7, lead: 0.033 };

fn level(p: &LipParams, s: f32) -> Option<u8> {
    if s < p.open_level {
        None
    } else if s >= p.levels[2] {
        Some(4)
    } else if s >= p.levels[1] {
        Some(2)
    } else if s >= p.levels[0] {
        Some(1)
    } else {
        Some(0)
    }
}

/// The lip track of a line from its loudness (`loud`: 0..255 of the line's peak, `rate` samples per second), with
/// [`PARAMS`]. Empty for a silent line.
pub fn lip_track(loud: &[u8], rate: f32) -> Vec<LipKey> {
    lip_track_with(loud, rate, &PARAMS)
}

pub fn lip_track_with(loud: &[u8], rate: f32, p: &LipParams) -> Vec<LipKey> {
    let mut keys = Vec::new();
    if loud.is_empty() || rate <= 0.0 {
        return keys;
    }
    let v = |k: isize| -> f32 { loud.get(k.clamp(0, loud.len() as isize - 1) as usize).map_or(0.0, |b| *b as f32 / 255.0) };
    // a light smoothing: one sample of noise does not move the mouth
    let s: Vec<f32> = (0..loud.len() as isize).map(|k| (v(k - 1) + 2.0 * v(k) + v(k + 1)) * 0.25).collect();
    let close_frames = (p.min_close * rate).ceil().max(1.0) as usize;
    let hold_frames = (p.min_hold * rate).ceil().max(1.0) as usize;
    let mut current: Option<u8> = None;
    let mut since = f32::NEG_INFINITY;
    let mut peak = 0.0f32;
    let push = |keys: &mut Vec<LipKey>, t: f32, open: Option<u8>| {
        let mut time = (t - p.lead).max(0.0);
        // keys stay in time order, an open is held its minimum (a lead clamped at 0 never squeezes two keys together)
        if let Some(k) = keys.last() {
            time = time.max(k.time + if k.open.is_some() { p.min_hold } else { 0.0 });
        }
        keys.push(LipKey { time, open });
    };
    for k in 0..s.len() {
        let t = k as f32 / rate;
        let want = level(p, s[k]);
        let held = t - since;
        match (current, want) {
            (None, Some(_)) => {
                // a syllable's start: its loudest moment in the next hold decides how wide (the onset is still rising)
                let loudest = s[k..(k + hold_frames).min(s.len())].iter().cloned().fold(0.0f32, f32::max);
                let l = level(p, loudest).unwrap_or(0);
                push(&mut keys, t, Some(l));
                current = Some(l);
                since = t;
                peak = loudest;
            }
            (Some(_), None) => {
                // a pause: only when it lasts (a one-sample dip between two syllables keeps the mouth open)
                let pause = (k..(k + close_frames).min(s.len())).all(|j| level(p, s[j]).is_none());
                if pause && held >= p.min_hold {
                    push(&mut keys, t, None);
                    current = None;
                    since = t;
                }
            }
            (Some(c), Some(l)) => {
                peak = peak.max(s[k]);
                if held < p.min_hold {
                    continue;
                }
                if l != c {
                    push(&mut keys, t, Some(l));
                    current = Some(l);
                    since = t;
                    peak = s[k];
                } else if s[k] < peak * p.syllable_drop {
                    // the loudness fell from this syllable's peak: the next syllable starts narrower
                    let narrower = if c == 4 { 2 } else { c.saturating_sub(1) };
                    if narrower != c {
                        push(&mut keys, t, Some(narrower));
                        current = Some(narrower);
                        since = t;
                    }
                    peak = s[k];
                }
            }
            (None, None) => {}
        }
    }
    if current.is_some() {
        push(&mut keys, s.len() as f32 / rate, None);
    }
    keys
}

/// The key a line plays at `time`: the last key at or before it (None = before the first).
pub fn key_at(track: &[LipKey], time: f32) -> Option<LipKey> {
    track.iter().take_while(|k| k.time <= time).last().copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 60.0;

    /// A line of `parts` (seconds, loudness 0..1).
    fn line(parts: &[(f32, f32)]) -> Vec<u8> {
        parts.iter().flat_map(|&(secs, l)| std::iter::repeat((l * 255.0) as u8).take((secs * RATE).round() as usize)).collect()
    }

    #[test]
    fn a_silent_line_moves_nothing() {
        assert!(lip_track(&line(&[(1.0, 0.0)]), RATE).is_empty());
        assert!(lip_track(&[], RATE).is_empty());
        assert!(lip_track(&line(&[(1.0, 0.1)]), RATE).is_empty(), "breath under the open level");
    }

    #[test]
    fn one_word_opens_at_its_start_and_closes_at_its_end() {
        // 0.2 s silence, 0.5 s loud, 0.3 s silence
        let t = lip_track(&line(&[(0.2, 0.0), (0.5, 1.0), (0.3, 0.0)]), RATE);
        assert_eq!(t.first().map(|k| k.open), Some(Some(4)), "loud: the widest mouth {t:?}");
        assert!((t[0].time - (0.2 - PARAMS.lead - 1.0 / RATE)).abs() < 0.02, "opens just before the sound: {}", t[0].time);
        let close = t.iter().find(|k| k.open.is_none()).unwrap();
        assert!((close.time - 0.7).abs() < 0.06, "closes at the word's end: {}", close.time);
        assert_eq!(t.last().unwrap().open, None, "ends closed");
    }

    #[test]
    fn two_syllables_with_a_pause_close_between() {
        let t = lip_track(&line(&[(0.25, 0.8), (0.12, 0.0), (0.25, 0.4)]), RATE);
        let opens: Vec<Option<u8>> = t.iter().map(|k| k.open).collect();
        assert_eq!(opens.iter().filter(|o| o.is_none()).count(), 2, "a close in the pause and one at the end: {t:?}");
        assert_eq!(opens[0], Some(4));
        assert!(opens.contains(&Some(1)), "the quiet syllable is a narrow mouth: {opens:?}");
    }

    #[test]
    fn a_one_sample_dip_keeps_the_mouth_open() {
        let mut l = line(&[(0.4, 0.9)]);
        l[12] = 0;
        let t = lip_track(&l, RATE);
        assert_eq!(t.iter().filter(|k| k.open.is_none()).count(), 1, "only the end closes: {t:?}");
    }

    #[test]
    fn poses_are_held_at_least_min_hold_and_keys_are_in_order() {
        // a loudness that changes every sample
        let l: Vec<u8> = (0..120).map(|k| if k % 2 == 0 { 250 } else { 90 }).collect();
        let t = lip_track(&l, RATE);
        for w in t.windows(2) {
            assert!(w[1].time >= w[0].time);
            if w[0].open.is_some() {
                assert!(w[1].time - w[0].time >= PARAMS.min_hold - 1e-4, "{w:?}");
            }
        }
    }

    #[test]
    fn a_long_vowel_with_syllable_dips_narrows_and_opens_again() {
        let t = lip_track(&line(&[(0.2, 1.0), (0.1, 0.55), (0.2, 1.0)]), RATE);
        let opens: Vec<Option<u8>> = t.iter().map(|k| k.open).collect();
        assert!(opens.windows(2).any(|w| w == [Some(4), Some(2)] || w == [Some(4), Some(1)]), "the dip narrows: {opens:?}");
        assert!(opens.iter().filter(|o| **o == Some(4)).count() >= 2, "and opens wide again: {opens:?}");
    }

    #[test]
    fn key_at_is_the_last_key_passed() {
        let t = vec![LipKey { time: 0.1, open: Some(2) }, LipKey { time: 0.3, open: None }];
        assert_eq!(key_at(&t, 0.05), None);
        assert_eq!(key_at(&t, 0.2).unwrap().open, Some(2));
        assert_eq!(key_at(&t, 0.3).unwrap().open, None);
    }
}
