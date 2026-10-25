//! (ER79) WHICH OF AN OUTFIT'S OWN CLIPS ARE PLAYED. An outfit (KH1, KH2 ...) plays its own clip where it has one
//! (`Animator::set_own_clips`) and KH3 Sora's, retargeted onto its skeleton by the renderer, everywhere else. A per-outfit
//! list of SLOTS takes KH3 Sora's clips back for those slots even where the outfit has its own (the user, 2026-10-15:
//! "for KH1 Sora, can he use KH3 Sora's standing pose adjusted for his skeleton?"). The hosts read the list from a key
//! (kh-er: `boc.<outfit>.use_kh3`); the slot names are this table's.
//!
//! The clips are named as in Sora's pack (`build/sora_pack/manifest.json` logical_animations); a slot lists every clip
//! of that role, also those no outfit has (then it changes nothing).

use std::collections::BTreeSet;

/// The slots: (name, the pack's clips of it).
pub const SLOTS: &[(&str, &[&str])] = &[
    // standing in the field: IdleN, IdleBreakN; TurnN's samples (turn in place)
    ("idle", &["0001", "0004", "0074", "0074L", "0074R", "0074BL", "0074BR"]),
    // the battle stand: IdleB, IdleLockon, IdleBreakW, BattleStart, BattleEnd
    ("battle_stand", &["1015", "1018", "1004", "1014", "1028"]),
    // run / walk to stand: WalkEnd R/L (N, W, B), RunEnd R/L (N, W, B)
    ("stop", &["0005", "0006", "1005", "1006", "1019", "1020", "0011", "0012", "1011", "1012", "1025", "1026"]),
    // landing to stand: Land / LandB, LightLand (B), LightLand R / L (B)
    ("land", &["0034", "0120", "0109", "0110"]),
    // stand to run: RunStartN at -180 / -90 / 0 / 90 / 180
    ("run_start", &["0007", "0007L", "0007R", "0007BL", "0007BR"]),
    ("walk", &["0002"]),
    ("run", &["0003", "0003L", "0003R"]),
    ("jump", &["0031", "0032", "0033"]),
    // the face slots' clips an outfit may have of its own (eyes and mouth)
    ("face", &["0000EM", "0006EM", "0020EM", "0021EM", "0050M", "0051M", "0052M"]),
];

/// The KH1 outfit's default (the user's request: KH3 Sora's standing pose, field and battle; with KH3's run / walk stops
/// and landings, which end in that pose: KH1's own end in KH1's stand, and the blend into KH3's moved a hand up to 4.3 cm
/// in one frame, against 1.2 cm with KH3's (offline, IMPL_ER79_STATE.md))
pub const KH1_DEFAULT: &str = "idle,battle_stand,stop,land";

/// The clips a list names: slot names of [`SLOTS`] and / or clip names, separated by commas, spaces or `+`; `all` = every
/// slot; `0`, `none`, `off` or an empty list = none. Unknown words are kept as clip names (a clip the pack lacks changes
/// nothing).
pub fn kh3_clips(list: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for word in list.split(|c: char| c == ',' || c == '+' || c.is_whitespace()).map(str::trim).filter(|w| !w.is_empty()) {
        let lower = word.to_ascii_lowercase();
        if matches!(lower.as_str(), "0" | "none" | "off" | "-") {
            continue;
        }
        if lower == "all" {
            out.extend(SLOTS.iter().flat_map(|(_, clips)| clips.iter().map(|c| c.to_string())));
            continue;
        }
        match SLOTS.iter().find(|(name, _)| *name == lower) {
            Some((_, clips)) => out.extend(clips.iter().map(|c| c.to_string())),
            None => {
                out.insert(word.to_string());
            }
        }
    }
    out
}

/// The default list of an outfit (`boc.<id>.use_kh3` absent): KH1's stand; the others none.
pub fn default_for(outfit: &str) -> &'static str {
    if outfit == "kh1" {
        KH1_DEFAULT
    } else {
        ""
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_and_clip_names_parse() {
        let s = kh3_clips("idle,battle_stand");
        assert!(s.contains("0001") && s.contains("1015") && s.contains("0004"));
        assert!(kh3_clips(KH1_DEFAULT).contains("0011") && kh3_clips(KH1_DEFAULT).contains("0034") && !kh3_clips(KH1_DEFAULT).contains("0003"));
        assert!(!s.contains("0011") && !s.contains("0003"));
        assert!(kh3_clips("0").is_empty() && kh3_clips("").is_empty() && kh3_clips("none").is_empty());
        let raw = kh3_clips("stop + 0034");
        assert!(raw.contains("0011") && raw.contains("0034") && !raw.contains("0001"));
        assert!(kh3_clips("all").contains("0052M"));
        assert_eq!(default_for("kh1"), KH1_DEFAULT);
        assert_eq!(default_for("kh2"), "");
    }

    #[test]
    fn no_clip_is_in_two_slots() {
        let mut seen = BTreeSet::new();
        for (_, clips) in SLOTS {
            for c in *clips {
                assert!(seen.insert(*c), "{c} in two slots");
            }
        }
    }
}
