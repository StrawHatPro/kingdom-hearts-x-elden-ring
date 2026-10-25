//! Which sound a trigger means (docs/sora-audio-spec.md sections 4.1, 4.3 and 5): the tables of
//! the pack's `triggers`, indexed for the questions a host asks.
//!
//! The simulation (`kh-core`) tells its host WHAT happened through the audio seam as a NAME: a
//! sound asset path, a weapon sound alias (`SWING3`, `HIT4`), an automatic motion sound (`Run`,
//! `LandNormal`, `step`), a voice asset or a voice group. `Routes::resolve` turns that name into
//! the sound asset to start, with the things only the audio side knows: the surface under him,
//! the weapon's sets, the voice language, the pawn's voice group number.

use std::collections::HashMap;

use crate::autose::Motion;
use crate::pack::Pack;
use crate::rng::XorShift;

/// The voice language. Voice assets are always REFERENCED under `/Localization/ja/`; the English
/// build overlays its files at those paths (spec 5.2). The pack holds both under their real
/// folders, and a voice is resolved BY NAME per language: seven cues of `Battle_Sora` have a
/// different sound index in en and ja, and asset volumes differ on some.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    En,
    Ja,
}

impl Language {
    pub fn folder(self) -> &'static str {
        match self {
            Language::En => "en",
            Language::Ja => "ja",
        }
    }

    pub fn parse(text: &str) -> Option<Language> {
        match text {
            "en" => Some(Language::En),
            "ja" => Some(Language::Ja),
            _ => None,
        }
    }
}

/// A voice asset path in `language`.
pub fn localized(asset: &str, language: Language) -> String {
    let asset = crate::pack::asset_key(asset);
    for folder in ["/Localization/ja/", "/Localization/en/"] {
        if let Some(at) = asset.find(folder) {
            return format!("{}/Localization/{}/{}", &asset[..at], language.folder(), &asset[at + folder.len()..]);
        }
    }
    asset.to_string()
}

/// A `TresAnimNotify_PlayVoice` with `m_RandomVoiceAssets` (spec 5.1).
#[derive(Clone, Debug, PartialEq)]
pub struct RandomVoice {
    /// The trigger id: the notify object the pick memory lives on.
    pub id: String,
    pub logical: Vec<String>,
    pub time: f32,
    /// (asset as referenced, weight)
    pub entries: Vec<(String, u32)>,
}

/// A sound a particle system carries (spec 5.5).
#[derive(Clone, Debug, PartialEq)]
pub struct EffectSe {
    pub asset: String,
    /// `FadeOutTime` when the effect ends; `None` with `bNoSeFadeAtTheEffectEnd` (the sound
    /// plays out).
    pub fade_out: Option<f32>,
    /// `bPlayAtEnd`: played when the effect ends.
    pub at_end: bool,
}

/// What the audio side knows when a name arrives.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Context<'a> {
    /// `SurfaceType` of the physical material under the pawn.
    pub surface: u32,
    pub language: Language,
    /// The pawn's voice group number (int at pawn+0x4D8; the states set it, spec 5.2).
    pub voice_group_no: i32,
    /// The animation whose notifies fired this step (logical name, time), when the host knows.
    pub anim: Option<(&'a str, f32)>,
}

/// What a name resolves to.
#[derive(Clone, Debug, PartialEq)]
pub enum Resolved {
    /// Start this asset. `voice`: it is a voice line (attached at the head, volume clamped).
    Asset { path: String, voice: bool },
    /// A weighted random voice list: `Routes::random_voices[index]`.
    RandomVoice { index: usize },
    /// A known trigger that plays nothing here, and why.
    Silent(&'static str),
    Unknown,
}

/// The lookup tables.
#[derive(Clone, Debug, Default)]
pub struct Routes {
    /// Kingdom Key swing set: SWINGn -> asset (`enumset_swing_w_so010`; 11..13 are empty).
    pub swing: HashMap<u32, String>,
    /// Kingdom Key hit set: HITn -> asset (`enumset_hit_w_so010`).
    pub hit: HashMap<u32, String>,
    /// `FsTable_Sora.PerSurfaceInfos[surface].SoundAssetPaths[type - 1]`.
    pub per_surface: HashMap<(u32, Motion), String>,
    /// The aux records (slide, railslide, sliding, step) per surface.
    pub aux: HashMap<(u32, String), String>,
    /// `NonSurfaceInfos` (the cloth rustles).
    pub non_surface: HashMap<Motion, String>,
    /// `p_ex001_VoiceGroupSet`: group -> (unit number, asset).
    pub voice_groups: HashMap<String, Vec<(i32, String)>>,
    pub random_voices: Vec<RandomVoice>,
    /// Logical animation -> the `ChangeAutoSeEnable` windows (start, end, setting).
    pub enable_windows: HashMap<String, Vec<(f32, f32, bool)>>,
    /// Effect name (the particle system's asset name) -> its sounds.
    pub effects: HashMap<String, Vec<EffectSe>>,
    /// The PlayAutoSe notifies that play a record of the AUX surface table ("slide", "step",
    /// "railslide"): (logical animations, time, record). The core's notify event carries only
    /// the source type "AuxSurfaceTable"; the animation and its time tell which notify it was.
    pub aux_notifies: Vec<(Vec<String>, f32, String)>,
    /// A prize orb's sounds: "<class>.get" (`TresPrizeBase.m_SoundGet`: collected) and
    /// "<class>.bound" (`m_SoundBound`: a bounce on the ground) -> asset (rows `code.prize`).
    pub prizes: HashMap<String, String>,
    /// Asset name ("se00000_054") -> its path, for callers that name a sound by its asset name
    /// alone. Voices are entered under their /ja/ reference path.
    pub by_name: HashMap<String, String>,
    /// (party stream E) Whose routes these are: `None` Sora, `Some("n_ex001")` Donald ... (`Routes::for_owner`).
    pub owner: Option<String>,
    /// (party stream E) Every weapon's swing / hit set of a party member, by weapon model ("w_do010"); the
    /// equipped one is copied into `swing` / `hit` by `select_weapon`.
    pub weapon_sets: HashMap<String, (HashMap<u32, String>, HashMap<u32, String>)>,
    /// (party stream E) The weapon whose sets are in `swing` / `hit`.
    pub weapon: Option<String>,
}

fn alias_number(name: &str, prefix: &str) -> Option<u32> {
    name.strip_prefix(prefix)?.parse().ok()
}

impl Routes {
    /// Sora's routes (the rows without an owner, and the shared code routes).
    pub fn new(pack: &Pack) -> Routes {
        Routes::build(pack, None)
    }

    /// (party stream E) A party member's routes: the rows of `owner` ("n_ex001", "n_ex002": their weapon sets, their
    /// footstep table, their voice group set, their clips' notify lists) plus the shared code routes (effect-attached
    /// sounds, prizes, names). The default weapon (the lowest model number) is selected.
    pub fn for_owner(pack: &Pack, owner: &str) -> Routes {
        let mut r = Routes::build(pack, Some(owner));
        if let Some(first) = r.weapon_sets.keys().min().cloned() {
            r.select_weapon(&first);
        }
        r
    }

    /// (party stream E) Uses `model`'s swing / hit sets for the aliases from now on. False: no such weapon.
    pub fn select_weapon(&mut self, model: &str) -> bool {
        if self.weapon.as_deref() == Some(model) {
            return true;
        }
        let Some((swing, hit)) = self.weapon_sets.get(model).cloned() else { return false };
        self.swing = swing;
        self.hit = hit;
        self.weapon = Some(model.to_string());
        true
    }

    fn build(pack: &Pack, owner: Option<&str>) -> Routes {
        let mut r = Routes { owner: owner.map(str::to_string), ..Routes::default() };
        for path in pack.assets.keys().filter(|p| !p.contains("/Localization/en/")) {
            if let Some(name) = path.rsplit('/').next() {
                r.by_name.entry(name.to_string()).or_insert_with(|| path.clone());
            }
        }
        for t in &pack.triggers {
            let first = t.sounds.first();
            // (party stream E) the per-character rows belong to their owner only; code routes are shared
            if !t.route.starts_with("code.") && t.owner.as_deref() != owner {
                continue;
            }
            match t.route.as_str() {
                "weapon.swing_set" | "weapon.hit_set" => {
                    if let Some((sound, n)) = first.and_then(|s| Some((s, s.alias_index?))) {
                        let swing = t.route == "weapon.swing_set";
                        match &t.weapon {
                            Some(model) => {
                                let sets = r.weapon_sets.entry(model.clone()).or_default();
                                if swing { &mut sets.0 } else { &mut sets.1 }.insert(n, sound.asset.clone());
                            }
                            None => {
                                if swing { &mut r.swing } else { &mut r.hit }.insert(n, sound.asset.clone());
                            }
                        }
                    }
                }
                "autose.per_surface" => {
                    if let (Some(surface), Some(motion), Some(sound)) = (t.surface, t.slot.as_deref().and_then(Motion::from_name), first) {
                        r.per_surface.insert((surface, motion), sound.asset.clone());
                    }
                }
                "autose.aux_surface" => {
                    if let (Some(surface), Some(slot), Some(sound)) = (t.surface, &t.slot, first) {
                        r.aux.insert((surface, slot.clone()), sound.asset.clone());
                    }
                }
                "autose.non_surface" => {
                    // The pack lists the cloth rows of the other costume tables under the same
                    // ids (banks 00907, 00910, 00911); `FsTable_Sora` is bank 00901 (spec 4.1).
                    if let (Some(motion), Some(sound)) = (t.slot.as_deref().and_then(Motion::from_name), first) {
                        if sound.asset.contains("/SE/cloth/00901/") || !r.non_surface.contains_key(&motion) {
                            r.non_surface.insert(motion, sound.asset.clone());
                        }
                    }
                }
                "voice.group" => {
                    for s in &t.sounds {
                        if let (Some(group), Some(no)) = (&s.group, s.group_no) {
                            r.voice_groups.entry(group.clone()).or_default().push((no, s.asset.clone()));
                        }
                    }
                }
                "notify.voice_random" => r.random_voices.push(RandomVoice { id: t.id.clone(), logical: t.logical.clone(), time: t.time.unwrap_or(0.0), entries: t.sounds.iter().map(|s| (s.asset.clone(), s.weight.unwrap_or(0))).collect() }),
                "notify.auto_se_enable" => {
                    if let (Some(start), Some(setting)) = (t.time, t.enable_setting) {
                        for logical in &t.logical {
                            r.enable_windows.entry(logical.clone()).or_default().push((start, start + t.duration.unwrap_or(0.0), setting));
                        }
                    }
                }
                "notify.auto_se" => {
                    if let Some(record) = t.slot.as_ref().filter(|_| t.trigger.contains("PlayAutoSe")) {
                        r.aux_notifies.push((t.logical.clone(), t.time.unwrap_or(0.0), record.clone()));
                    }
                }
                "code.prize" => {
                    // "PrizeHpS: collected" / "PrizeHpS: bounce on the ground".
                    if let (Some((class, what)), Some(sound)) = (t.trigger.split_once(':'), first) {
                        let suffix = if what.contains("bounce") { "bound" } else { "get" };
                        r.prizes.insert(format!("{}.{suffix}", class.trim()), sound.asset.clone());
                    }
                }
                route if route.starts_with("code.effect_se") => {
                    // "Effect <name> starts".
                    let name = t.trigger.strip_prefix("Effect ").and_then(|rest| rest.split_whitespace().next());
                    if let (Some(name), Some(sound)) = (name, first) {
                        // Effect names are matched without case (the game's asset names are
                        // case-insensitive: "VW_SO010_APP0" is `vw_so010_app0`).
                        r.effects.entry(name.to_ascii_lowercase()).or_default().push(EffectSe { asset: sound.asset.clone(), fade_out: if t.no_fade_at_end { None } else { Some(t.fade_out.unwrap_or(0.0)) }, at_end: t.play_at_end });
                    }
                }
                _ => {}
            }
        }
        r
    }

    /// The table entry of a motion sound on `surface`. No fallback: an empty entry is silence
    /// (lookup 0x145811C50).
    pub fn motion_asset(&self, motion: Motion, surface: u32) -> Option<&str> {
        if motion.per_surface() { self.per_surface.get(&(surface, motion)) } else { self.non_surface.get(&motion) }.map(String::as_str)
    }

    /// The enable flag of the detectors at `time` of `anim` (spec 4.3): inside a window its
    /// setting, else the component's own state, which is ASSUMED enabled (the three glide-loop
    /// windows that ENABLE imply something disables it around them; that code was not found).
    pub fn auto_se_enabled(&self, anim: &str, time: f32) -> bool {
        self.enable_windows.get(anim).and_then(|w| w.iter().find(|(a, b, _)| time >= *a && time <= *b)).map_or(true, |w| w.2)
    }

    /// The sounds of an effect, by its particle system's name or path
    /// ("/Game/Effects/.../vs_com_grd00.vs_com_grd00" -> "vs_com_grd00").
    pub fn effect_sounds(&self, effect: &str) -> &[EffectSe] {
        let name = effect.rsplit(|c| c == '/' || c == '.').next().unwrap_or(effect);
        self.effects.get(&name.to_ascii_lowercase()).map_or(&[], Vec::as_slice)
    }

    /// The unit of a voice group whose number equals the pawn's (no match plays nothing:
    /// `m_bPlayOnGroupNotFound` is false on all of Sora's groups).
    pub fn voice_group_asset(&self, group: &str, number: i32) -> Option<&str> {
        self.voice_groups.get(group)?.iter().find(|(no, _)| *no == number).map(|(_, a)| a.as_str())
    }

    /// The weighted random list a voice name stands for, when the name is the first entry of one
    /// (the seam carries the first entry; the animation and its time tell which notify it was).
    fn random_list(&self, name: &str, anim: Option<(&str, f32)>) -> Option<usize> {
        let key = crate::pack::asset_key(name);
        let mut candidates = self.random_voices.iter().enumerate().filter(|(_, v)| v.entries.first().is_some_and(|e| e.0 == key));
        match anim {
            Some((anim, time)) => candidates.filter(|(_, v)| v.logical.iter().any(|l| l == anim)).min_by(|a, b| (a.1.time - time).abs().total_cmp(&(b.1.time - time).abs())).map(|(i, _)| i),
            None => candidates.next().map(|(i, _)| i),
        }
    }

    /// The asset a seam name means.
    pub fn resolve(&self, pack: &Pack, name: &str, cx: &Context) -> Resolved {
        // (ER77) "sora_magic:20#fire": the magic state's element tag on a shared cast line (read by an outfit's voice
        // table, outfit.rs); the line itself is the name without it.
        let name = if name.starts_with("/Game/") { name } else { name.split('#').next().unwrap_or(name) };
        // (party stream E) a `TresActorVoice` (TresAnimNotify_PlayActorVoice.m_ActorVoice, by path or name): the sound
        // of its first sound section, as a voice line.
        if name.contains("ActorVoice") || name.starts_with("av_") {
            let key = name.rsplit('/').next().unwrap_or(name);
            if let Some(first) = pack.actor_voices.get(key.split('.').next().unwrap_or(key)).and_then(|l| l.first()) {
                return Resolved::Asset { path: localized(first, cx.language), voice: true };
            }
        }
        if name.starts_with("/Game/") {
            if name.contains("/VOICE/") {
                if let Some(index) = self.random_list(name, cx.anim) {
                    return Resolved::RandomVoice { index };
                }
                return Resolved::Asset { path: localized(name, cx.language), voice: true };
            }
            return Resolved::Asset { path: crate::pack::asset_key(name).to_string(), voice: false };
        }
        if let Some(n) = alias_number(name, "SWING") {
            // 11..13 are empty in the Kingdom Key's set: silent with this weapon (spec 5.3).
            return self.swing.get(&n).map_or(Resolved::Silent("no entry in the weapon's swing set"), |a| Resolved::Asset { path: a.clone(), voice: false });
        }
        if let Some(n) = alias_number(name, "HIT") {
            return self.hit.get(&n).map_or(Resolved::Silent("no entry in the weapon's hit set"), |a| Resolved::Asset { path: a.clone(), voice: false });
        }
        if let Some(motion) = Motion::from_name(name) {
            return self.motion_asset(motion, cx.surface).map_or(Resolved::Silent("no table entry for this surface"), |a| Resolved::Asset { path: a.to_string(), voice: false });
        }
        if name == "AuxSurfaceTable" {
            let Some((anim, time)) = cx.anim else { return Resolved::Silent("an aux-table motion sound without its animation") };
            let record = self.aux_notifies.iter().filter(|n| n.0.iter().any(|l| l == anim)).min_by(|a, b| (a.1 - time).abs().total_cmp(&(b.1 - time).abs())).map(|n| n.2.clone());
            return match record {
                Some(record) => self.aux.get(&(cx.surface, record)).map_or(Resolved::Silent("no aux record for this surface"), |a| Resolved::Asset { path: a.clone(), voice: false }),
                None => Resolved::Unknown,
            };
        }
        if self.aux.keys().any(|(_, slot)| slot == name) {
            return self.aux.get(&(cx.surface, name.to_string())).map_or(Resolved::Silent("no aux record for this surface"), |a| Resolved::Asset { path: a.clone(), voice: false });
        }
        // "group:N": a voice group with the unit number given by the caller (the magic states
        // name the cast voice so: "sora_magic:20").
        if let Some((group, number)) = name.split_once(':').and_then(|(g, n)| Some((g, n.parse::<i32>().ok()?))) {
            if self.voice_groups.contains_key(group) {
                return self.voice_group_asset(group, number).map_or(Resolved::Silent("the voice group has no unit with this number"), |a| Resolved::Asset { path: localized(a, cx.language), voice: true });
            }
        }
        if self.voice_groups.contains_key(name) {
            return self.voice_group_asset(name, cx.voice_group_no).map_or(Resolved::Silent("no unit with the pawn's voice group number"), |a| Resolved::Asset { path: localized(a, cx.language), voice: true });
        }
        let _ = pack;
        // (IMPL_ER19) "effect:<particle system>": THE SOUND AN EFFECT CARRIES (spec 5.5, rows code.effect_se*), asked
        // for by name through the audio seam by a part of a host that draws that effect itself, outside the core's effect
        // seam (the save point's heal burst: kh_host::savepoint). Its first sound that plays at the effect's start.
        if let Some(effect) = name.strip_prefix("effect:") {
            return match self.effect_sounds(effect).iter().find(|se| !se.at_end) {
                Some(se) => Resolved::Asset { path: se.asset.clone(), voice: false },
                None => Resolved::Silent("the effect carries no sound"),
            };
        }
        if let Some(path) = self.prizes.get(name) {
            return Resolved::Asset { path: path.clone(), voice: false };
        }
        if let Some(path) = self.by_name.get(name) {
            let voice = path.contains("/VOICE/");
            return Resolved::Asset { path: if voice { localized(path, cx.language) } else { path.clone() }, voice };
        }
        Resolved::Unknown
    }
}

/// `TresAnimNotify_PlayVoice`'s pick (0x14056CAE0): weighted, the last picked entry left out:
/// `r = (xorshift >> 1) % total`. A one-entry list therefore plays once and then never again.
pub fn pick_voice(entries: &[(String, u32)], last: Option<usize>, rng: &mut XorShift) -> Option<usize> {
    let total: u32 = entries.iter().enumerate().filter(|(i, _)| Some(*i) != last).map(|(_, e)| e.1).sum();
    if total == 0 {
        return None;
    }
    let mut r = (rng.next() >> 1) % total;
    for (i, e) in entries.iter().enumerate() {
        if Some(i) == last {
            continue;
        }
        if r < e.1 {
            return Some(i);
        }
        r -= e.1;
    }
    None
}
