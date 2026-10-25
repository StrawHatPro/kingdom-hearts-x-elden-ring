//! (ER77, the user: "They should replace ALL voice lines KH3 Sora uses only when playing as KH1 sora") AN OUTFIT'S OWN
//! VOICE: while the host says an outfit with a voice table is worn (`Engine::set_voice_outfit`), EVERY voice line the
//! engine would play (a voice notify: direct, random list or voice group; a code voice "group:N"; a bare voice asset name;
//! `play_voice_group`) goes through the outfit's table instead: the KH3 line is told by keys, the first key with a row
//! names the event, the event's clips are played (random without an immediate repeat, or by combo step), and an event
//! without clips is SILENT. KH3's voice is never played while such an outfit is worn. Sound effects, footsteps and every
//! other non-voice sound are untouched; other outfits are untouched.
//!
//! DATA: `PACK/audio/outfits/<outfit>/voice.json` (tools/sora_outfit_voice.js from sheets/port/sora-outfit-voice.json):
//! the clips (mono 48 kHz s16, level-matched to KH3's battle voice) and the rules. `Pack::load_outfit_voices` adds each
//! outfit's clips to the pack as ONE BANK of sounds made from a KH3 voice sound (the template: Battle_Sora #128): the
//! same category (Vo_BattlePC), priority, start macro (106: one battle voice at a time), bus, aux send, distance curves
//! and the same bank number, so the clips cut each other as KH3's lines do. The asset side (volume, attenuation = 3D)
//! is the replaced KH3 line's, so the clip is placed, panned and attenuated as that line would have been.
//!
//! KEYS (the most specific first): `group:<voice group>:<unit>#<element>` (the magic state's tag on the shared cast lines,
//! kh-core fire_magic.rs), `group:<voice group>:<unit>`, `anim:<logical animation>` (the animation whose notify fired),
//! `asset:<KH3 voice asset name>` (every voice asset has one: the fallback).

use std::collections::HashMap;

use kh_anim::json::Json;

use crate::pack::{Bank, Material, Pack, Selection, Sequence, SeqCmd, SoundDef, Track};

/// How an event picks its clip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    /// At random, never the clip it played last.
    Random,
    /// By combo step: the first voice of a combo plays the first clip, the second the second ... (a gap of more than
    /// `combo_step_gap` or any other voice starts again at the first).
    Step,
}

/// One event of the table.
#[derive(Clone, Debug, PartialEq)]
pub struct Rule {
    pub id: String,
    /// Sound indices of the outfit's bank; empty = silent.
    pub clips: Vec<u32>,
    pub pick: Pick,
    /// mapped / stand_in / user / silent (for the log).
    pub kind: String,
}

/// One outfit's voice.
#[derive(Clone, Debug, PartialEq)]
pub struct OutfitVoice {
    pub outfit: String,
    /// Its bank in `Pack::banks`.
    pub bank: u32,
    /// Clip ids by sound index.
    pub clips: Vec<String>,
    pub rules: Vec<Rule>,
    /// Key -> rule index.
    pub by_key: HashMap<String, usize>,
    /// Seconds after which a combo starts again at its first clip.
    pub combo_step_gap: f32,
}

impl OutfitVoice {
    /// The rule of the first key that has one.
    pub fn rule_for<'a>(&'a self, keys: &[String]) -> Option<(&'a Rule, &'a str)> {
        keys.iter().find_map(|k| self.by_key.get_key_value(k).map(|(key, &i)| (&self.rules[i], key.as_str())))
    }
}

/// The keys of a voice line, most specific first. `name`: the seam name ("sora_magic:20#fire", "sora_item",
/// "/Game/Sound/VOICE/..."); `kh3_asset`: the KH3 voice asset it resolved to; `anim`: the animation whose notify fired;
/// `voice_group_no`: the pawn's number (for a bare group name).
pub fn voice_keys(name: &str, is_group: &dyn Fn(&str) -> bool, kh3_asset: Option<&str>, anim: Option<&str>, voice_group_no: i32) -> Vec<String> {
    let mut keys = Vec::new();
    let (base, tag) = match name.split_once('#') {
        Some((b, t)) => (b, Some(t)),
        None => (name, None),
    };
    let group = match base.split_once(':').and_then(|(g, n)| Some((g, n.parse::<i32>().ok()?))) {
        Some((g, n)) if is_group(g) => Some((g, n)),
        _ if is_group(base) => Some((base, voice_group_no)),
        _ => None,
    };
    if let Some((g, n)) = group {
        if let Some(tag) = tag {
            keys.push(format!("group:{g}:{n}#{tag}"));
        }
        keys.push(format!("group:{g}:{n}"));
    }
    if let Some(anim) = anim.filter(|a| !a.is_empty()) {
        keys.push(format!("anim:{anim}"));
    }
    if let Some(asset) = kh3_asset {
        let name = crate::pack::asset_key(asset).rsplit('/').next().unwrap_or(asset);
        keys.push(format!("asset:{name}"));
    }
    keys
}

/// The picker's memory (per engine).
#[derive(Clone, Debug, Default)]
pub struct Picker {
    last: HashMap<(String, String), u32>,
    step: usize,
    last_step_at: f64,
}

impl Picker {
    /// The clip (sound index) `rule` plays now. `now`: the engine clock, s.
    pub fn pick(&mut self, outfit: &OutfitVoice, rule: &Rule, now: f64, rng: &mut crate::rng::XorShift) -> Option<u32> {
        if rule.clips.is_empty() {
            return None;
        }
        match rule.pick {
            Pick::Step => {
                if now - self.last_step_at > outfit.combo_step_gap as f64 {
                    self.step = 0;
                }
                let clip = rule.clips[self.step % rule.clips.len()];
                self.step += 1;
                self.last_step_at = now;
                Some(clip)
            }
            Pick::Random => {
                // any other voice starts the combo steps again
                self.step = 0;
                let key = (outfit.outfit.clone(), rule.id.clone());
                let last = self.last.get(&key).copied();
                let choices: Vec<u32> = rule.clips.iter().copied().filter(|c| rule.clips.len() == 1 || Some(*c) != last).collect();
                let clip = choices[((rng.next() >> 1) as usize) % choices.len()];
                self.last.insert(key, clip);
                Some(clip)
            }
        }
    }
}

impl Pack {
    /// Reads `<pack>/outfits/*/voice.json` and adds each outfit's clips as a bank (see the module). Returns what it did,
    /// for the host's log. Call before the pack is shared (`Arc`).
    pub fn load_outfit_voices(&mut self) -> Vec<String> {
        let mut said = Vec::new();
        let Ok(dirs) = std::fs::read_dir(self.dir.join("outfits")) else { return said };
        let mut files: Vec<_> = dirs.filter_map(|d| d.ok()).map(|d| d.path().join("voice.json")).filter(|p| p.is_file()).collect();
        files.sort();
        for file in files {
            match std::fs::read_to_string(&file).map_err(|e| e.to_string()).and_then(|t| Json::parse(t.trim_start_matches('\u{feff}'))).and_then(|j| self.add_outfit_voice(&j)) {
                Ok(text) => said.push(text),
                Err(e) => said.push(format!("{}: NOT READ ({e})", file.display())),
            }
        }
        said
    }

    /// Adds one outfit's voice (the JSON of `voice.json`).
    pub fn add_outfit_voice(&mut self, j: &Json) -> Result<String, String> {
        if !j.get("format").as_str().unwrap_or("").starts_with("kh-outfit-voice/") {
            return Err("not an outfit voice table".to_string());
        }
        let outfit = j.get("outfit").as_str().ok_or("no outfit")?.to_string();
        let template_bank = j.get("template_bank").as_str().ok_or("no template_bank")?;
        let &tb = self.bank_by_name.get(template_bank).ok_or_else(|| format!("template bank {template_bank} is not in the pack"))?;
        let template_index = j.get("template_sound").as_usize().unwrap_or(0) as u32;
        let template = self.sound(tb, template_index).cloned().ok_or_else(|| format!("template sound {template_bank}#{template_index} is not in the pack"))?;
        let first = template.sequences.first().cloned().unwrap_or(Sequence { commands: Vec::new(), lowpass: None, aux_send_db: None });
        let track_commands = template.tracks.values().next().map(|t| t.commands.clone()).unwrap_or_default();
        let name = format!("OUTFIT/{outfit}/voice");
        let mut bank = Bank { name: name.clone(), number: self.banks[tb as usize].number, sounds: HashMap::new(), materials: HashMap::new() };
        let mut clips = Vec::new();
        let mut index_of = HashMap::new();
        for (i, c) in j.get("clips").items().iter().enumerate() {
            let i = i as u32;
            let id = c.get("id").as_str().ok_or("a clip without id")?.to_string();
            bank.materials.insert(i, Material { file: c.get("file").as_str().ok_or("a clip without file")?.to_string(), channels: c.get("channels").as_usize().unwrap_or(1) as u32, sample_rate: c.get("sample_rate").as_usize().unwrap_or(48_000) as u32, samples: c.get("samples").as_usize().unwrap_or(0) as u32, loop_points: None });
            let mut tracks = HashMap::new();
            tracks.insert(0, Track { commands: track_commands.clone(), material: i });
            bank.sounds.insert(
                i,
                SoundDef {
                    index: i,
                    // own sound numbers (the macro's "same sound" test), far from KH3's
                    number: 60_000 + i,
                    volume: template.volume * c.get("volume").as_f32().unwrap_or(1.0),
                    selection: Selection::All,
                    sequences: vec![Sequence { commands: vec![SeqCmd::KeyOn { track: 0, looped: false }, SeqCmd::End], lowpass: first.lowpass.clone(), aux_send_db: first.aux_send_db.clone() }],
                    tracks,
                    ..template.clone()
                },
            );
            index_of.insert(id.clone(), i);
            clips.push(id);
        }
        let mut rules = Vec::new();
        let mut by_key = HashMap::new();
        for r in j.get("rules").items() {
            let id = r.get("id").as_str().unwrap_or("").to_string();
            let mut indices = Vec::new();
            for c in r.get("clips").items() {
                let c = c.as_str().unwrap_or("");
                indices.push(*index_of.get(c).ok_or_else(|| format!("rule {id}: unknown clip {c}"))?);
            }
            let pick = if r.get("pick").as_str() == Some("step") { Pick::Step } else { Pick::Random };
            for k in r.get("match").items() {
                if let Some(k) = k.as_str() {
                    by_key.insert(k.to_string(), rules.len());
                }
            }
            rules.push(Rule { id, clips: indices, pick, kind: r.get("kind").as_str().unwrap_or("mapped").to_string() });
        }
        let at = self.banks.len() as u32;
        self.bank_by_name.insert(name, at);
        self.banks.push(bank);
        let text = format!("outfit {outfit}: {} clips, {} events, {} keys (bank {at})", clips.len(), rules.len(), by_key.len());
        self.outfit_voices.push(OutfitVoice { outfit, bank: at, clips, rules, by_key, combo_step_gap: j.get("combo_step_gap_s").as_f32().unwrap_or(1.5) });
        Ok(text)
    }

    /// The voice of an outfit, when it has one.
    pub fn outfit_voice(&self, outfit: &str) -> Option<&OutfitVoice> {
        self.outfit_voices.iter().find(|o| o.outfit == outfit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_go_from_the_most_specific() {
        let is_group = |g: &str| g == "sora_magic" || g == "sora_item";
        assert_eq!(voice_keys("sora_magic:20#fire", &is_group, Some("/Game/Sound/VOICE/Localization/ja/battle/PC/magic-tujyo-1.magic-tujyo-1"), Some("MagicFire"), 0), ["group:sora_magic:20#fire", "group:sora_magic:20", "anim:MagicFire", "asset:magic-tujyo-1"]);
        assert_eq!(voice_keys("sora_item", &is_group, Some("/Game/Sound/VOICE/Localization/en/battle/PC/item"), None, 3), ["group:sora_item:3", "asset:item"]);
        assert_eq!(voice_keys("/Game/Sound/VOICE/Localization/ja/battle/PC/bt1010510sr0.bt1010510sr0", &is_group, Some("/Game/Sound/VOICE/Localization/en/battle/PC/bt1010510sr0"), Some("1301"), 0), ["anim:1301", "asset:bt1010510sr0"]);
    }

    #[test]
    fn random_never_repeats_and_steps_count_up() {
        let outfit = OutfitVoice { outfit: "kh1".into(), bank: 0, clips: vec![], rules: vec![], by_key: HashMap::new(), combo_step_gap: 1.5 };
        let hurt = Rule { id: "hurt".into(), clips: vec![4, 5, 6], pick: Pick::Random, kind: "mapped".into() };
        let combo = Rule { id: "combo".into(), clips: vec![0, 1, 2], pick: Pick::Step, kind: "mapped".into() };
        let mut picker = Picker::default();
        let mut rng = crate::rng::XorShift::new(7);
        let mut last = None;
        for _ in 0..200 {
            let c = picker.pick(&outfit, &hurt, 0.0, &mut rng).unwrap();
            assert!(hurt.clips.contains(&c) && Some(c) != last);
            last = Some(c);
        }
        let steps: Vec<u32> = [10.0, 10.4, 10.8, 11.2, 13.0].iter().map(|t| picker.pick(&outfit, &combo, *t, &mut rng).unwrap()).collect();
        assert_eq!(steps, [0, 1, 2, 0, 0], "counts up, wraps, and starts again after a gap");
        picker.pick(&outfit, &combo, 14.0, &mut rng);
        picker.pick(&outfit, &hurt, 14.1, &mut rng);
        assert_eq!(picker.pick(&outfit, &combo, 14.2, &mut rng), Some(0), "another voice starts the steps again");
        let silent = Rule { id: "s".into(), clips: vec![], pick: Pick::Random, kind: "silent".into() };
        assert_eq!(picker.pick(&outfit, &silent, 0.0, &mut rng), None);
    }
}
