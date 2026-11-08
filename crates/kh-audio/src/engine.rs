//! The audio side of a host, engine-free: the driver, the trigger routes, the automatic motion
//! sounds, the UI static SEs and the low-HP alarm behind one object. A host (Bevy or not)
//! - forwards the audio seam of `kh-core` (`seam_play` / `seam_stop`: a NAME, where, volume,
//!   pitch) and the effect seam (`effect_spawned` / `effect_stopped`: sounds carried by particle
//!   systems, spec 5.5),
//! - feeds the bone positions of each simulation step (`auto_se`) with the surface under him,
//! - reports HP for the alarm (`alarm_tick`) and asks for UI sounds (`play_static_se`,
//!   `play_system_se`),
//! - calls `update` once per step and plays the returned voice commands (`mixer.rs` can).
//!
//! NOTHING HERE FEEDS BACK INTO THE SIMULATION. The engine has its own random generators,
//! seeded by the host.

use std::collections::HashMap;
use std::sync::Arc;

use glam::Vec3;

use crate::alarm::{AlarmAction, LowHpAlarm, ALARM_STATIC_SE};
use crate::autose::{AutoSe, AutoSeEvent, BoneFrame};
use crate::driver::{Driver, Listener, Note, PlayOutcome, PlayParams, SoundKey, VoiceCommand, VoiceParams};
use crate::pack::{Pack, ASSET_VOLUME_DEFAULT_ASSUMED};
use crate::rng::XorShift;
use crate::routes::{pick_voice, Context, Language, Resolved, Routes};
use crate::rules::RULES;

/// How a sound asset is started: its place, what it follows, and the trigger's mix.
#[derive(Clone, Debug, PartialEq)]
pub struct AssetPlay {
    pub position: Option<Vec3>,
    pub anchor: Option<Anchor>,
    pub volume: f32,
    pub pitch: f32,
    pub fade_in: f32,
}

impl AssetPlay {
    /// At the given place, at full volume and normal pitch, with no fade-in.
    pub fn at(position: Option<Vec3>, anchor: Option<Anchor>) -> AssetPlay {
        AssetPlay { position, anchor, volume: 1.0, pitch: 1.0, fade_in: 0.0 }
    }
}

/// One simulation step of the motion sounds (the detector of spec 4.2). `bone`: world position of a bone.
#[derive(Clone, Copy)]
pub struct AutoSeTick<'a> {
    pub dt: f32,
    pub frame: &'a BoneFrame,
    pub surface: u32,
    pub anim: Option<(&'a str, f32)>,
    pub listener: &'a Listener,
    pub bone: &'a dyn Fn(&str) -> Option<Vec3>,
}

/// A voice line asked of an outfit's table (ER77): the seam handle, why, the seam name, the KH3 line and its anim.
#[derive(Clone, Copy)]
struct OutfitVoiceAsk<'a> {
    handle: Option<u32>,
    cause: &'a str,
    name: &'a str,
    kh3_path: &'a str,
    anim: Option<&'a str>,
}

/// What a 3D sound follows. The host resolves it to a point each step.
#[derive(Clone, Debug, PartialEq)]
pub enum Anchor {
    /// The `Attach` of a seam sound: the host kept it under this seam handle.
    Seam(u32),
    /// A bone of Sora (an auto-SE part).
    Bone(&'static str),
    /// An effect of the effect seam, by its handle.
    Effect(u32),
    /// (party stream E) A bone of party member n (an auto-SE part of that member).
    PartyBone(u8, &'static str),
}

/// A seam request (`kh_core::SoundPlay`) as the engine needs it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeamPlay<'a> {
    pub name: &'a str,
    /// Where it is now (the host resolved the `Attach`), cm.
    pub position: Vec3,
    pub volume: f32,
    pub pitch: f32,
    /// The request came from a notify STATE (loops allowed; stopped at its End).
    pub looping: bool,
    pub spatial: bool,
    /// The animation whose notifies fired (logical name, time).
    pub anim: Option<(&'a str, f32)>,
}

/// One line of the audio log.
#[derive(Clone, Debug, PartialEq)]
pub struct LogLine {
    /// "play", "voice", "cancel", "drop", "steal", "macro-stop", "end", "silent", "unknown".
    pub kind: &'static str,
    /// What asked for it ("seam SWING3", "autose Run L_ashi1", "static 9" ...).
    pub cause: String,
    /// "SE/footstep/10000_sora/10010/se10010#2".
    pub sound: String,
    pub category: String,
    /// The material file of a "voice" line.
    pub file: String,
    pub params: Option<VoiceParams>,
    pub distance: Option<f32>,
    pub note: String,
}

/// A live sound of the engine, for a debug page.
#[derive(Clone, Debug, PartialEq)]
pub struct ActiveLine {
    pub key: SoundKey,
    pub cause: String,
    pub bank: String,
    pub index: u32,
    pub category: String,
    pub priority: i32,
    pub voices: usize,
    pub params: VoiceParams,
    pub distance: Option<f32>,
    /// (ER62) Where a 3D sound is (core cm).
    pub position: Option<Vec3>,
    pub age: f32,
    pub stopping: bool,
}

struct Tracked {
    cause: String,
    anchor: Option<Anchor>,
    /// The asset own stop fade (bOverrideStopFadeTime), else 0: Stop() is immediate.
    stop_fade: f32,
}

/// The engine.
pub struct Engine {
    pack: Arc<Pack>,
    pub routes: Routes,
    pub driver: Driver,
    pub auto: AutoSe,
    pub alarm: LowHpAlarm,
    pub language: Language,
    /// The pawn voice group number (pawn+0x4D8). The states set it after starting an animation
    /// (magic, items, some attacks, death: spec 5.2); the audio seam carries it
    /// (`kh_core::Audio::voice_group_no` -> `set_voice_group_no`). 0 = the default unit.
    pub voice_group_no: i32,
    /// The surface under him as of the latest `auto_se` call.
    pub surface: u32,
    voice_rng: XorShift,
    /// Per random-voice notify: the last pick.
    voice_memory: HashMap<usize, usize>,
    tracked: HashMap<SoundKey, Tracked>,
    seam: HashMap<u32, Vec<SoundKey>>,
    effects: HashMap<u32, Vec<(SoundKey, Option<f32>)>>,
    /// Live effects that have a sound to play at their end: (name, where it was spawned).
    effect_ends: HashMap<u32, (String, Vec3)>,
    statics: HashMap<u32, SoundKey>,
    log: Vec<LogLine>,
    /// Names nobody could resolve, each once.
    pub unknown: Vec<String>,
    /// (ER77) The worn outfit's own voice (an index of `Pack::outfit_voices`): every voice line goes through its table.
    voice_outfit: Option<usize>,
    picker: crate::outfit::Picker,
    /// (MIMIC-BOSS hotfix 3, MB3-AUDIO) A Sora copy's own outfit-voice picker, by driver scope (`picker` is the KH3 world's):
    /// his combo steps and his no-repeat memory are his, so his lines never move the player's combo voice on, nor his the copy's.
    copy_pickers: HashMap<u32, crate::outfit::Picker>,
    /// The engine clock (sum of the update steps), s.
    clock: f64,
    /// The outfit voices started since the last update: (the KH3 line, key, how specific the match was).
    /// (MIMIC-BOSS hotfix 3, MB3-AUDIO) Keyed by the driver scope too: a copy's line never stands in for the player's.
    outfit_step: Vec<(u32, String, SoundKey, u8)>,
    /// (party stream E) The party members' own audio state, by member index (0 Donald, 1 Goofy).
    party: Vec<Option<PartyAudio>>,
    /// (FACE) The voice lines started (a `/VOICE/` asset) for the faces' lip-sync: who (a party member index, None =
    /// Sora / the seam), the clock when the driver started its voice, its bank and material.
    lines: HashMap<SoundKey, VoiceLineRec>,
    /// (FACE) The voice line the latest `play_asset` started, for the caller that owns it.
    last_voice: Option<SoundKey>,
    /// (FACE) Each party member's latest voice line (by member index).
    party_line: Vec<Option<SoundKey>>,
    /// (THE USER 2026-11-02: "sometimes Audio from specifically our mod will become very loud") The live copies of each
    /// asset that has UE concurrency data (`Asset::concurrency`), oldest first: `play_asset` holds them to its count.
    /// (MIMIC-BOSS hotfix 3, MB3-AUDIO) Keyed (driver scope, bank, index): a copy's copies of an asset are counted apart from the player's.
    concurrent: HashMap<(u32, u32, u32), Vec<SoundKey>>,
}

/// (FACE) A voice line as the engine keeps it.
#[derive(Clone, Copy, Debug, PartialEq)]
struct VoiceLineRec {
    member: Option<u8>,
    started: Option<f64>,
    bank: u32,
    material: Option<u32>,
}

/// (FACE) A party member's voice line for its face's lip-sync (kh_anim::face_layer): since when it sounds, its
/// length, its loudness (the pack's `lip.json`: samples 0..255 of the line's peak at that rate; None = not in the
/// table, the face then uses the length only).
#[derive(Clone, Debug, PartialEq)]
pub struct VoiceLine {
    pub key: SoundKey,
    /// Seconds since the driver started its voice (0 in the update that started it).
    pub elapsed: f32,
    /// The material's length (s).
    pub length: f32,
    pub loudness: Option<(Arc<[u8]>, f32)>,
}

/// (party stream E) How a party voice line meets the member's current one (`PlayVoiceGroup` FUN_141275AA0 param_4 /
/// param_5; kh_core::host::party::VoiceMode).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartyVoiceMode {
    /// Stop the member's current line, then play.
    Replace,
    /// Nothing while the member's current line plays.
    IfIdle,
    /// Play over it.
    Overlap,
}

/// (party stream E) A party member's audio state: its routes (voice groups, weapon sets, footstep table), its voice
/// group number, the surface under it, its own motion-sound detector and its current voice line.
pub struct PartyAudio {
    pub owner: String,
    pub name: &'static str,
    pub routes: Routes,
    pub voice_group_no: i32,
    pub surface: u32,
    pub auto: AutoSe,
    /// The member's current `PlayVoiceGroup` line (the pawn keeps one voice component, pawn[0x192] [C]).
    pub current_voice: Option<SoundKey>,
    /// The member's playing animation (anim-set row name, time) as of the host's latest set_party_anim.
    pub anim: Option<(String, f32)>,
    voice_memory: HashMap<usize, usize>,
}

/// (party stream E) The pack owner of a party member index (kh_core::PartyMember::chara).
pub fn party_owner(member: u8) -> Option<(&'static str, &'static str)> {
    match member {
        0 => Some(("n_ex001", "Donald")),
        1 => Some(("n_ex002", "Goofy")),
        // (MIMIC-FX) a copy (crate::copies): the routes of the character it copies (a Sora copy: Sora's own, owner "")
        m => match crate::copies::copy_of(m)? {
            crate::copies::CopyOf::Donald => Some(("n_ex001", crate::copies::name(m))),
            crate::copies::CopyOf::Goofy => Some(("n_ex002", crate::copies::name(m))),
            crate::copies::CopyOf::Sora => Some(("", crate::copies::name(m))),
        },
    }
}

impl Engine {
    /// `seed`: the host seed of the sound generators.
    pub fn new(pack: Arc<Pack>, seed: u32, language: Language) -> Engine {
        Engine {
            routes: Routes::new(&pack),
            driver: Driver::new(pack.clone(), seed),
            pack,
            auto: AutoSe::new(),
            alarm: LowHpAlarm::default(),
            language,
            voice_group_no: 0,
            surface: 0,
            voice_rng: XorShift::new(seed ^ 0x5EED_0123),
            voice_memory: HashMap::new(),
            tracked: HashMap::new(),
            seam: HashMap::new(),
            effects: HashMap::new(),
            effect_ends: HashMap::new(),
            statics: HashMap::new(),
            log: Vec::new(),
            unknown: Vec::new(),
            voice_outfit: None,
            picker: crate::outfit::Picker::default(),
            copy_pickers: HashMap::new(), // (MIMIC-BOSS hotfix 3, MB3-AUDIO)
            clock: 0.0,
            outfit_step: Vec::new(),
            party: Vec::new(),
            lines: HashMap::new(),     // (FACE)
            last_voice: None,          // (FACE)
            party_line: Vec::new(),    // (FACE)
            concurrent: HashMap::new(),
        }
    }

    /// (FACE) Party member `member`'s voice line while it sounds (its last `/VOICE/` line through `party_seam_play`): for
    /// the face's lip-sync. None before the driver has started its voice and after it ended.
    pub fn party_voice(&self, member: u8) -> Option<VoiceLine> {
        let key = (*self.party_line.get(member as usize)?)?;
        let rec = self.lines.get(&key)?;
        let started = rec.started?;
        if !self.driver.is_playing(key) {
            return None;
        }
        let material = rec.material?;
        let length = self.pack.material_seconds(rec.bank, material).unwrap_or(0.0);
        Some(VoiceLine { key, elapsed: (self.clock - started).max(0.0) as f32, length, loudness: self.pack.lip_of(rec.bank, material) })
    }

    /// (party stream E) A party member's audio state, made on first use from the pack's rows of its owner. `None`: not a
    /// member this pack has rows for.
    pub fn party(&mut self, member: u8) -> Option<&mut PartyAudio> {
        let (owner, name) = party_owner(member)?;
        let i = member as usize;
        if self.party.len() <= i {
            self.party.resize_with(i + 1, || None);
        }
        if self.party[i].is_none() {
            // (MIMIC-FX) a Sora copy: Sora's own routes (his voice groups, his weapons' sets, his footstep table)
            let routes = if owner.is_empty() {
                Routes::new(&self.pack)
            } else {
                if !self.pack.triggers.iter().any(|t| t.owner.as_deref() == Some(owner)) {
                    return None;
                }
                Routes::for_owner(&self.pack, owner)
            };
            self.party[i] = Some(PartyAudio { owner: owner.to_string(), name, routes, voice_group_no: 0, surface: 0, auto: AutoSe::new(), current_voice: None, anim: None, voice_memory: HashMap::new() });
        }
        self.party[i].as_mut()
    }

    /// (party stream E) Runs `f` with the member's routes, voice number, surface, detector and random-voice memory in
    /// the engine's own slots (so every rule of the engine applies unchanged), then puts them back; the log lines `f`
    /// wrote are tagged with the member's name. The worn outfit's voice is Sora's: off for the call.
    fn as_member<R>(&mut self, member: u8, f: impl FnOnce(&mut Engine) -> R) -> Option<R> {
        let mut p = self.party(member).map(|p| std::mem::replace(p, PartyAudio { owner: String::new(), name: "", routes: Routes::default(), voice_group_no: 0, surface: 0, auto: AutoSe::new(), current_voice: None, anim: None, voice_memory: HashMap::new() }))?;
        std::mem::swap(&mut self.routes, &mut p.routes);
        std::mem::swap(&mut self.voice_group_no, &mut p.voice_group_no);
        std::mem::swap(&mut self.surface, &mut p.surface);
        std::mem::swap(&mut self.auto, &mut p.auto);
        std::mem::swap(&mut self.voice_memory, &mut p.voice_memory);
        // (MIMIC-BOSS hotfix 3, MB3-AUDIO) a copy's sounds are of its own driver scope (its macros, pools and concurrency are its own)
        let scope_before = self.driver.set_scope(crate::copies::scope(member));
        // (MIMIC-FX) a Sora copy wears the outfit he copied (the worn one): its voice is his too
        let outfit = self.voice_outfit;
        if !crate::copies::is_sora_copy(member) {
            self.voice_outfit = None;
        }
        let first_log = self.log.len();
        let result = f(self);
        for line in &mut self.log[first_log..] {
            line.cause = format!("{} {}", p.name, line.cause);
        }
        self.voice_outfit = outfit;
        self.driver.set_scope(scope_before); // (MIMIC-BOSS hotfix 3, MB3-AUDIO)
        std::mem::swap(&mut self.routes, &mut p.routes);
        std::mem::swap(&mut self.voice_group_no, &mut p.voice_group_no);
        std::mem::swap(&mut self.surface, &mut p.surface);
        std::mem::swap(&mut self.auto, &mut p.auto);
        std::mem::swap(&mut self.voice_memory, &mut p.voice_memory);
        self.party[member as usize] = Some(p);
        Some(result)
    }

    /// (party stream E) A seam sound attached to party member `member` (`Attach::Party`): resolved with the member's
    /// routes. `voice`: `Some(mode)` for a `PlayVoiceGroup` line (kh_core party ids PARTY_VOICE*): the member's
    /// current line is stopped first (Replace), or the new one is dropped while it lasts (IfIdle).
    pub fn party_seam_play(&mut self, member: u8, handle: u32, play: &SeamPlay, voice: Option<PartyVoiceMode>) -> Option<SoundKey> {
        let current = self.party(member)?.current_voice.filter(|k| self.driver.is_playing(*k));
        match (voice, current) {
            (Some(PartyVoiceMode::IfIdle), Some(_)) => {
                let name = self.party(member).map_or("", |p| p.name);
                self.silent("silent", &format!("{name} seam {}", play.name), "PlayVoiceGroup IfIdle: the member's voice line is still playing");
                return None;
            }
            (Some(PartyVoiceMode::Replace), Some(old)) => {
                // FUN_141275AA0: the playing voice component is stopped (Stop(), vt+0x698).
                self.driver.stop(old, Some(0.0));
            }
            _ => {}
        }
        // the member's own animation when the request does not name one (random voice lists, aux-table notifies)
        let anim = if play.anim.is_none() { self.party(member).and_then(|p| p.anim.clone()) } else { None };
        let request = SeamPlay { anim: play.anim.or(anim.as_ref().map(|(n, t)| (n.as_str(), *t))), ..*play };
        let key = self.as_member(member, |e| e.seam_play(handle, &request))??;
        if let (Some(cause), Some(t)) = (self.party.get(member as usize).and_then(|p| p.as_ref()).map(|p| p.name), self.tracked.get_mut(&key)) {
            t.cause = format!("{cause} {}", t.cause);
        }
        if voice.is_some() {
            if let Some(p) = self.party(member) {
                p.current_voice = Some(key);
            }
        }
        // (FACE) a voice line of this member (any `/VOICE/` asset: voice groups, notify voices, actor voices): its face's
        if self.last_voice == Some(key) {
            if let Some(rec) = self.lines.get_mut(&key) {
                rec.member = Some(member);
            }
            if self.party_line.len() <= member as usize {
                self.party_line.resize(member as usize + 1, None);
            }
            self.party_line[member as usize] = Some(key);
        }
        Some(key)
    }

    /// (party stream E) `Audio::party_voice_group_no`: the unit the member's voice-GROUP notifies play from now on.
    pub fn set_party_voice_group_no(&mut self, member: u8, number: i32) {
        if let Some(p) = self.party(member) {
            if p.voice_group_no != number {
                p.voice_group_no = number;
                let name = p.name;
                self.silent("voice-group", &format!("{name} seam"), &format!("voice group number {number}"));
            }
        }
    }

    /// (party stream E) The member's equipped weapon model ("w_do010" ..): its swing / hit sets from now on.
    pub fn set_party_weapon(&mut self, member: u8, model: &str) -> bool {
        self.party(member).is_some_and(|p| p.routes.select_weapon(model))
    }

    /// (MIMIC-FX) A Sora copy's swing / hit sets (kh-er kbdraw.rs `select_sounds`, the player's rule on HIS weapons): his
    /// attack model's by KH3's rule (`Routes::select_weapon_kh3`), the LH_WEAPON aliases from his left hand's ("" none).
    /// True: `model`'s own sets are in use.
    pub fn set_party_sora_weapons(&mut self, member: u8, model: &str, left: &str) -> bool {
        let Some(p) = self.party(member) else { return false };
        let own = if p.routes.weapon.as_deref() != Some(model) { p.routes.select_weapon_kh3(model) } else { true };
        p.routes.select_left_weapon(left);
        own
    }

    /// (party stream E) One simulation step of a party member's motion sounds (`SQEXSEADAutoSeComponent` with
    /// BoneSet_Sora1 and FsTable_Donald / _Goofy on both pawns [A]): the detector of spec 4.2 over the member's bones,
    /// the member's footstep table. `bone`: world position of a bone of THAT member.
    pub fn party_auto_se(&mut self, member: u8, tick: AutoSeTick<'_>) -> Vec<AutoSeEvent> {
        let AutoSeTick { dt, frame, surface, anim, listener, bone } = tick;
        let Some(name) = self.party(member).map(|p| p.name) else { return Vec::new() };
        self.as_member(member, |e| {
            e.surface = surface;
            let enabled = anim.is_none_or(|(name, time)| e.routes.auto_se_enabled(name, time));
            let distance = (frame.base - listener.position).length() * 0.01;
            let events = e.auto.step(dt, frame, distance, enabled);
            for event in &events {
                let cause = format!("autose {} {} surface {surface}", event.motion.name(), event.part.bone());
                match e.routes.motion_asset(event.motion, surface).map(str::to_string) {
                    Some(path) => {
                        let position = bone(event.part.bone()).unwrap_or(frame.base);
                        e.play_asset(&path, &format!("{name} {cause}"), AssetPlay { volume: event.volume.clamp(0.0, 1.0), ..AssetPlay::at(Some(position), Some(Anchor::PartyBone(member, event.part.bone()))) });
                    }
                    None => e.silent("silent", &cause, "no table entry for this surface"),
                }
            }
            events
        })
        .unwrap_or_default()
    }

    /// (party stream E) The member's playing animation (anim-set row name, time), once per step.
    pub fn set_party_anim(&mut self, member: u8, name: &str, time: f32) {
        if let Some(p) = self.party(member) {
            match &mut p.anim {
                Some((n, t)) if n == name => *t = time,
                slot => *slot = Some((name.to_string(), time)),
            }
        }
    }

    /// (party stream E) The members whose audio state exists.
    pub fn party_members(&self) -> Vec<u8> {
        self.party.iter().enumerate().filter(|(_, p)| p.is_some()).map(|(i, _)| i as u8).collect()
    }

    /// (ER77) THE WORN OUTFIT, for its own voice: `Some(id)` of an outfit with a voice table in the pack = every voice
    /// line from now on goes through that table (outfit.rs); `None` or an outfit without one = KH3's voice. Cheap: call
    /// it every step. Returns whether an outfit voice is now in use.
    pub fn set_voice_outfit(&mut self, outfit: Option<&str>) -> bool {
        let index = outfit.and_then(|o| self.pack.outfit_voices.iter().position(|v| v.outfit == o));
        if index != self.voice_outfit {
            self.voice_outfit = index;
            self.picker = crate::outfit::Picker::default();
            self.copy_pickers.clear(); // (MIMIC-BOSS hotfix 3, MB3-AUDIO)
            let note = match (outfit, index) {
                (Some(o), Some(_)) => format!("the {o} outfit's own voice replaces every voice line of Sora"),
                (Some(o), None) => format!("the {o} outfit has no voice table in the pack: KH3's voice"),
                (None, _) => "KH3's voice".to_string(),
            };
            self.silent("outfit-voice", "outfit", &note);
        }
        index.is_some()
    }

    /// The outfit whose voice is in use.
    pub fn voice_outfit(&self) -> Option<&str> {
        self.voice_outfit.map(|i| self.pack.outfit_voices[i].outfit.as_str())
    }

    /// (ER77) A VOICE LINE WHILE AN OUTFIT WITH ITS OWN VOICE IS WORN: the table's clip in place of `kh3_path` (the KH3
    /// line it would have been), with that line's asset volume and attenuation (3D, pan, distance as KH3's), or nothing
    /// (a silent event / no row). Never the KH3 line. `name`: the seam name (for the keys).
    fn outfit_voice_play(&mut self, outfit: usize, ask: OutfitVoiceAsk<'_>, play: AssetPlay) -> Option<SoundKey> {
        let OutfitVoiceAsk { handle, cause, name, kh3_path, anim } = ask;
        let AssetPlay { position, anchor, volume, pitch, fade_in } = play;
        let pack = self.pack.clone();
        let ov = &pack.outfit_voices[outfit];
        let routes = &self.routes;
        let keys = crate::outfit::voice_keys(name, &|g| routes.voice_groups.contains_key(g), Some(kh3_path), anim, self.voice_group_no);
        let line = crate::pack::asset_key(kh3_path).rsplit('/').next().unwrap_or(kh3_path).to_string();
        let Some((rule, key)) = ov.rule_for(&keys) else {
            self.silent("outfit-silent", cause, &format!("{} voice: no row for {line} (keys {}): silent, never KH3's voice", ov.outfit, keys.join(" ")));
            return None;
        };
        let specific = if key.contains('#') { 3 } else if key.starts_with("group:") { 2 } else if key.starts_with("anim:") { 1 } else { 0 };
        // The same KH3 line asked for twice in one step (a voice notify and the magic state's own play of it): KH3's
        // macro 106 cancels the copy; here the more specific match wins.
        let scope = self.driver.scope(); // (MIMIC-BOSS hotfix 3, MB3-AUDIO)
        if let Some(i) = self.outfit_step.iter().position(|p| p.0 == scope && p.1 == line) {
            if self.outfit_step[i].3 >= specific {
                self.silent("outfit-silent", cause, &format!("{} voice: {line} already started this step", ov.outfit));
                return None;
            }
            let (_, _, old, _) = self.outfit_step.remove(i);
            self.driver.stop(old, Some(0.0));
        }
        // (MIMIC-BOSS hotfix 3, MB3-AUDIO) the picker of this scope: a copy's combo steps are his own
        let picker = if scope == 0 { &mut self.picker } else { self.copy_pickers.entry(scope).or_default() };
        let Some(clip) = picker.pick(ov, rule, self.clock, &mut self.voice_rng) else {
            self.silent("outfit-silent", cause, &format!("{} voice: {} ({}) is silent for {line} ({key})", ov.outfit, rule.id, rule.kind));
            return None;
        };
        let asset = pack.asset(kh3_path).cloned();
        let attenuation = asset.as_ref().is_none_or(|a| a.attenuation);
        let asset_volume = asset.as_ref().and_then(|a| a.volume).unwrap_or(ASSET_VOLUME_DEFAULT_ASSUMED);
        let spatial = position.filter(|_| attenuation);
        let params = PlayParams { volume: volume * asset_volume, pitch, position: spatial, fade_in };
        let clip_name = ov.clips.get(clip as usize).cloned().unwrap_or_default();
        let full_cause = format!("{cause} -> {} {} {clip_name}", ov.outfit, rule.id);
        match self.driver.play(ov.bank, clip, &params) {
            PlayOutcome::Started(k) => {
                self.tracked.insert(k, Tracked { cause: full_cause.clone(), anchor: if spatial.is_some() { anchor } else { None }, stop_fade: asset.and_then(|a| a.stop_fade).unwrap_or(0.0) });
                if let Some(handle) = handle {
                    self.seam.entry(handle).or_default().push(k);
                }
                self.outfit_step.push((scope, line.clone(), k, specific));
                self.silent("outfit-voice", &full_cause, &format!("{} voice: {} ({}) clip {clip_name} for {line} ({key})", ov.outfit, rule.id, rule.kind));
                Some(k)
            }
            PlayOutcome::Missing => {
                self.silent("unknown", &full_cause, "the outfit's clip is not in the pack");
                None
            }
            _ => None,
        }
    }

    pub fn pack(&self) -> &Arc<Pack> {
        &self.pack
    }

    fn sound_label(&self, bank: u32, index: u32) -> String {
        format!("{}#{index}", self.pack.bank(bank).name)
    }

    fn silent(&mut self, kind: &'static str, cause: &str, note: &str) {
        self.log.push(LogLine { kind, cause: cause.to_string(), sound: String::new(), category: String::new(), file: String::new(), params: None, distance: None, note: note.to_string() });
    }

    /// Starts a sound asset: the UE side of spec 3 (volume = trigger multiplier x asset Volume;
    /// 3D iff the asset has attenuation settings and a place is given).
    pub fn play_asset(&mut self, path: &str, cause: &str, play: AssetPlay) -> Option<SoundKey> {
        let AssetPlay { position, anchor, volume, pitch, fade_in } = play;
        let Some(asset) = self.pack.asset(path).cloned() else {
            self.silent("unknown", cause, &format!("asset {path} is not in the pack"));
            return None;
        };
        let spatial = position.filter(|_| asset.attenuation);
        // (THE USER 2026-11-02: "sometimes Audio from specifically our mod will become very loud") THE ASSET'S UE
        // CONCURRENCY: at most `MaxCount` copies of it sound at once (KH3's own ConcurrencyOverrides); a further one is
        // refused (PreventNew) or the oldest copy stops for it (the other rules; "farthest" is not weighed).
        if let Some(c) = asset.concurrency {
            let driver = &self.driver;
            let live = self.concurrent.entry((driver.scope(), asset.bank, asset.index)).or_default();
            live.retain(|k| driver.is_playing(*k));
            if live.len() >= c.max_count as usize {
                if c.prevent_new {
                    let note = format!("concurrency: {} copies of this asset sound already (MaxCount {}, PreventNew): the new one is refused", live.len(), c.max_count);
                    self.silent("silent", cause, &note);
                    return None;
                }
                let oldest = live.remove(0);
                self.driver.stop(oldest, Some(0.0));
                // (ER-HITSFX-AUDIT) the only cut of the UE side: named in the log like the SEAD side's steals and macro stops
                let note = format!("concurrency: MaxCount {} reached: the oldest copy of this asset (sound key {oldest}) stops for the new one", c.max_count);
                self.silent("concurrency-stop", cause, &note);
            }
        }
        let params = PlayParams { volume: volume * asset.volume.unwrap_or(ASSET_VOLUME_DEFAULT_ASSUMED), pitch, position: spatial, fade_in };
        match self.driver.play(asset.bank, asset.index, &params) {
            PlayOutcome::Started(key) => {
                if asset.concurrency.is_some() {
                    self.concurrent.entry((self.driver.scope(), asset.bank, asset.index)).or_default().push(key);
                }
                self.tracked.insert(key, Tracked { cause: cause.to_string(), anchor: if spatial.is_some() { anchor } else { None }, stop_fade: asset.stop_fade.unwrap_or(0.0) });
                // (FACE) a voice line: kept for the faces' lip-sync until its voice ends
                if path.contains("/VOICE/") {
                    self.lines.insert(key, VoiceLineRec { member: None, started: None, bank: asset.bank, material: None });
                    self.last_voice = Some(key);
                }
                Some(key)
            }
            PlayOutcome::Missing => {
                self.silent("unknown", cause, &format!("sound {} of the asset is not in the pack", self.sound_label(asset.bank, asset.index)));
                None
            }
            // (AUDIT-A SOUND) Cancelled / dropped by KH3's own SEAD rule: the driver's note (kind `cancel` / `drop`, logged
            // at the next update without a cause) stays; this line names the request and the rule, so that every request of
            // the seam leaves a line of its own (an arrowgun's bullets hitting within 0.067 s of each other: se02002_039's
            // start macro 1)
            PlayOutcome::Cancelled { by_macro } => {
                let note = format!("SEAD start macro {by_macro} of sound {}: {}", self.sound_label(asset.bank, asset.index), self.macro_rule(by_macro));
                self.silent("silent", cause, &note);
                None
            }
            PlayOutcome::Dropped => {
                let note = format!("SEAD category pool full of sounds of higher priority: sound {} is dropped", self.sound_label(asset.bank, asset.index));
                self.silent("silent", cause, &note);
                None
            }
        }
    }

    /// (AUDIT-A SOUND) A start macro of the pack in words (spec 2.1), e.g. macro 1: "the same sound, started <= 0.067 s ago,
    /// count >= 1: the new one is cancelled".
    fn macro_rule(&self, number: u32) -> String {
        use crate::pack::{Cmp, MacroCmd, MacroField};
        let sym = |c: Cmp| match c {
            Cmp::Eq => "==",
            Cmp::Ne => "!=",
            Cmp::Gt => ">",
            Cmp::Ge => ">=",
            Cmp::Lt => "<",
            Cmp::Le => "<=",
        };
        let Some(commands) = self.pack.macros.get(&number) else { return "not in the pack's macros".to_string() };
        let mut parts = Vec::new();
        for command in commands {
            parts.push(match *command {
                MacroCmd::End => break,
                MacroCmd::ResetSet => "all sounds again".to_string(),
                MacroCmd::Filter { field: MacroField::SoundId, cmp: Cmp::Eq, relative: true, value: 0 } => "the same sound".to_string(),
                MacroCmd::Filter { field, cmp, relative, value } => format!("{} {field:?} {} {value}", if relative { "own - other" } else { "other" }, sym(cmp)),
                MacroCmd::FilterElapsed { cmp, value } => format!("started {} {value} s ago", sym(cmp)),
                MacroCmd::FilterUnread => "an unread filter".to_string(),
                MacroCmd::ActionUnread => "an unread action".to_string(),
                MacroCmd::StopOthers { count, fade } => format!("count {} {}: the others stop ({fade} s)", sym(count.0), count.1),
                MacroCmd::CancelSelf { count } => format!("count {} {}: the new one is cancelled", sym(count.0), count.1),
                MacroCmd::SelfVolume { count, volume, fade } => format!("count {} {}: own volume {volume} ({fade} s)", sym(count.0), count.1),
            });
        }
        parts.join(", ")
    }

    fn context<'a>(&self, anim: Option<(&'a str, f32)>) -> Context<'a> {
        Context { surface: self.surface, language: self.language, voice_group_no: self.voice_group_no, anim }
    }

    /// A sound of the audio seam. `handle` is the seam handle (for `seam_stop` and for following
    /// its attach point: `Anchor::Seam(handle)`).
    pub fn seam_play(&mut self, handle: u32, play: &SeamPlay) -> Option<SoundKey> {
        // (MIMIC-BOSS hotfix 3, MB3-AUDIO) a copy's sound that reached the seam outside its member's call (one on its victim, a hit at the
        // point): the copy's own routes (his weapon's sets), animation and driver scope, never the player's
        if self.driver.scope() == 0 {
            if let Some(member) = crate::copies::of_handle(handle) {
                if self.party(member).is_some() {
                    return self.party_seam_play(member, handle, &SeamPlay { anim: None, ..*play }, None);
                }
            }
        }
        let cause = format!("seam {}", play.name.rsplit('/').next().unwrap_or(play.name));
        let resolved = self.routes.resolve(&self.pack, play.name, &self.context(play.anim));
        // (ER77) an outfit with its own voice is worn: every voice line goes through its table, KH3's is never played
        if let Some(outfit) = self.voice_outfit {
            let kh3 = match &resolved {
                Resolved::Asset { path, voice: true } => Some(path.clone()),
                Resolved::RandomVoice { index } => self.routes.random_voices[*index].entries.first().map(|e| crate::routes::localized(&e.0, self.language)),
                _ => None,
            };
            if let Some(kh3) = kh3 {
                let ask = OutfitVoiceAsk { handle: Some(handle), cause: &cause, name: play.name, kh3_path: &kh3, anim: play.anim.map(|a| a.0) };
                return self.outfit_voice_play(outfit, ask, AssetPlay { volume: play.volume.clamp(0.0, 1.0), pitch: play.pitch, ..AssetPlay::at(play.spatial.then_some(play.position), Some(Anchor::Seam(handle))) });
            }
        }
        let (path, voice) = match resolved {
            Resolved::Asset { path, voice } => (path, voice),
            Resolved::RandomVoice { index } => {
                // Weighted, no immediate repeat; the memory lives on the notify (spec 5.1).
                let entries = self.routes.random_voices[index].entries.clone();
                let Some(pick) = pick_voice(&entries, self.voice_memory.get(&index).copied(), &mut self.voice_rng) else {
                    self.silent("silent", &cause, "random voice: nothing left to pick (a one-entry list plays once)");
                    return None;
                };
                self.voice_memory.insert(index, pick);
                // (AUDIT-A) which entry KH3's draw gave, for a host's log and the audits (`kh_host::audit_a`)
                let note = format!("random voice: entry {} of {} (list {})", pick + 1, entries.len(), self.routes.random_voices[index].id);
                self.silent("random-voice", &cause, &note);
                (crate::routes::localized(&entries[pick].0, self.language), true)
            }
            Resolved::Silent(why) => {
                self.silent("silent", &cause, why);
                return None;
            }
            Resolved::Unknown => {
                if !self.unknown.iter().any(|n| n == play.name) {
                    self.unknown.push(play.name.to_string());
                }
                self.silent("unknown", &cause, "no route for this name");
                return None;
            }
        };
        // `SQEXSEADAnimNotify_PlaySound` refuses loops (0x145815C90): an instant notify with a
        // looping asset plays nothing.
        if !play.looping && !voice && self.pack.asset(&path).is_some_and(|a| a.looping) {
            self.silent("silent", &cause, "a looping asset from an instant notify is refused");
            return None;
        }
        // Voice notifies: volume clamp(scale x multiplier, 0, 1), Sora's scale 1.0 [I].
        let volume = if voice { play.volume.clamp(0.0, 1.0) } else { play.volume };
        let key = self.play_asset(&path, &cause, AssetPlay { volume, pitch: play.pitch, ..AssetPlay::at(play.spatial.then_some(play.position), Some(Anchor::Seam(handle))) })?;
        self.seam.entry(handle).or_default().push(key);
        Some(key)
    }

    /// The audio seam's `voice_group_no`: the unit a voice-GROUP notify plays from now on.
    pub fn set_voice_group_no(&mut self, number: i32) {
        if self.voice_group_no != number {
            self.voice_group_no = number;
            self.silent("voice-group", "seam", &format!("voice group number {number}"));
        }
    }

    /// A spell / shot actor of class `class` hit a pawn (`map` false) or level geometry at
    /// `position`: its `m_PawnHitSEAsset` / `m_MapHitSEAsset` (spec 5.4, rows `code.hit_magic`;
    /// `PlaySoundAtLocation`), and the sounds carried by the class's own hit / map-hit effects
    /// (spec 5.5). `handle`: the seam handle the hit sound came under, when there is one.
    pub fn actor_hit(&mut self, handle: Option<u32>, class: &str, map: bool, position: Vec3) -> Option<SoundKey> {
        self.in_handle_scope(handle, |e| e.actor_hit_scoped(handle, class, map, position)) // (MIMIC-BOSS hotfix 3, MB3-AUDIO)
    }

    /// (MIMIC-BOSS hotfix 3, MB3-AUDIO) [`Engine::actor_hit`] under the driver scope already set.
    fn actor_hit_scoped(&mut self, handle: Option<u32>, class: &str, map: bool, position: Vec3) -> Option<SoundKey> {
        let what = if map { "map hit" } else { "hit" };
        let Some(actor) = self.pack.actor(class).cloned() else {
            if !self.unknown.iter().any(|n| n == class) {
                self.unknown.push(class.to_string());
            }
            self.silent("unknown", &format!("actor {class} {what}"), "no such actor class in the pack's tables");
            return None;
        };
        let cause = format!("actor {} {what}", actor.class);
        let mut first = None;
        match if map { &actor.map_hit_se } else { &actor.pawn_hit_se } {
            Some(asset) => {
                first = self.play_asset(asset, &cause, AssetPlay::at(Some(position), None));
                if let (Some(key), Some(handle)) = (first, handle) {
                    self.seam.entry(handle).or_default().push(key);
                }
            }
            None => self.silent("silent", &cause, "the class has no SE for this"),
        }
        let role = if map { "map_hit" } else { "hit" };
        for (_, effect) in actor.effects.iter().filter(|(r, _)| r == role) {
            for se in self.routes.effect_sounds(effect).to_vec().into_iter().filter(|se| !se.at_end) {
                let key = self.play_asset(&se.asset, &format!("{cause} effect {effect}"), AssetPlay::at(Some(position), None));
                first = first.or(key);
            }
        }
        first
    }

    /// (ER party S5) A party spell actor's hit: ONLY the class's own m_PawnHitSEAsset / m_MapHitSEAsset (kh-party spawns
    /// the actor's hit effect itself on the effect seam, whose SeAttach then plays through effect_spawned: playing the hit
    /// effects' sounds here too would double them). None: no such class, or it has no SE for this.
    pub fn actor_hit_se(&mut self, handle: Option<u32>, class: &str, map: bool, position: Vec3) -> Option<SoundKey> {
        self.in_handle_scope(handle, |e| e.actor_hit_se_scoped(handle, class, map, position)) // (MIMIC-BOSS hotfix 3, MB3-AUDIO)
    }

    /// (MIMIC-BOSS hotfix 3, MB3-AUDIO) Runs `f` under the driver scope of the copy whose seam / effect handle `handle` is (nothing changes for any
    /// other handle, or inside a member's call, whose scope is already set).
    fn in_handle_scope<R>(&mut self, handle: Option<u32>, f: impl FnOnce(&mut Engine) -> R) -> R {
        let scope = handle.and_then(crate::copies::of_handle).map_or(0, crate::copies::scope);
        if scope == 0 || self.driver.scope() != 0 {
            return f(self);
        }
        let before = self.driver.set_scope(scope);
        let result = f(self);
        self.driver.set_scope(before);
        result
    }

    /// (MIMIC-BOSS hotfix 3, MB3-AUDIO) [`Engine::actor_hit_se`] under the driver scope already set.
    fn actor_hit_se_scoped(&mut self, handle: Option<u32>, class: &str, map: bool, position: Vec3) -> Option<SoundKey> {
        let actor = self.pack.actor(class).cloned()?;
        let what = if map { "map hit" } else { "hit" };
        let asset = if map { actor.map_hit_se.as_ref() } else { actor.pawn_hit_se.as_ref() }?;
        let key = self.play_asset(asset, &format!("actor {} {what}", actor.class), AssetPlay::at(Some(position), None))?;
        if let Some(handle) = handle {
            self.seam.entry(handle).or_default().push(key);
        }
        Some(key)
    }

    /// A sound started under this seam handle is still playing.
    pub fn seam_live(&self, handle: u32) -> bool {
        self.seam.contains_key(&handle)
    }

    /// `Audio::stop` of the seam: `fade_out` 0 = `Stop()`, else `FadeOut(fade_out)`.
    pub fn seam_stop(&mut self, handle: u32, fade_out: f32) {
        for key in self.seam.remove(&handle).unwrap_or_default() {
            let asset_fade = self.tracked.get(&key).map_or(0.0, |t| t.stop_fade);
            self.driver.stop(key, Some(if fade_out > 0.0 { fade_out } else { asset_fade }));
        }
    }

    /// One simulation step of the automatic motion sounds (spec 4): runs the detector and plays
    /// what it found from `FsTable_Sora` for `surface`. `anim`: the animation shown (for the
    /// enable windows). `bone`: world position of a bone, cm. Returns the events (for logs and
    /// tests).
    pub fn auto_se(&mut self, tick: AutoSeTick<'_>) -> Vec<AutoSeEvent> {
        let AutoSeTick { dt, frame, surface, anim, listener, bone } = tick;
        self.surface = surface;
        let enabled = anim.is_none_or(|(name, time)| self.routes.auto_se_enabled(name, time));
        let distance = (frame.base - listener.position).length() * 0.01;
        let events = self.auto.step(dt, frame, distance, enabled);
        for event in &events {
            let cause = format!("autose {} {} surface {surface}", event.motion.name(), event.part.bone());
            match self.routes.motion_asset(event.motion, surface).map(str::to_string) {
                Some(path) => {
                    let position = bone(event.part.bone()).unwrap_or(frame.base);
                    self.play_asset(&path, &cause, AssetPlay { volume: event.volume.clamp(0.0, 1.0), ..AssetPlay::at(Some(position), Some(Anchor::Bone(event.part.bone()))) });
                }
                None => self.silent("silent", &cause, "no table entry for this surface"),
            }
        }
        events
    }

    /// A line of one of Sora's voice groups by number (`p_ex001_VoiceGroupSet`: "sora_magic",
    /// "sora_item", "sora_kaihuku" ...; spec 5.2), in the engine's language. For code that plays
    /// a group unit directly; a voice NOTIFY needs only `voice_group_no` set before it fires.
    pub fn play_voice_group(&mut self, group: &str, number: i32, position: Vec3, anchor: Option<Anchor>) -> Option<SoundKey> {
        let path = crate::routes::localized(self.routes.voice_group_asset(group, number)?, self.language);
        if let Some(outfit) = self.voice_outfit {
            let name = format!("{group}:{number}");
            return self.outfit_voice_play(outfit, OutfitVoiceAsk { handle: None, cause: &format!("voice group {group} no {number}"), name: &name, kh3_path: &path, anim: None }, AssetPlay::at(Some(position), anchor));
        }
        self.play_asset(&path, &format!("voice group {group} no {number}"), AssetPlay::at(Some(position), anchor))
    }

    /// A UI static SE by its `ETresStaticSE` id (spec 5.6): 2D, volume 1. A loop (9, 21, 34) is
    /// kept under its id for `stop_static_se`.
    pub fn play_static_se(&mut self, id: u32) -> Option<SoundKey> {
        let cause = format!("static {id}");
        let Some(path) = self.pack.static_se.get(id as usize).cloned().flatten() else {
            self.silent("silent", &cause, "this static SE id is not in the pack");
            return None;
        };
        let key = self.play_asset(&path, &cause, AssetPlay::at(None, None))?;
        if self.pack.asset(&path).is_some_and(|a| a.looping) {
            if let Some(old) = self.statics.insert(id, key) {
                self.driver.stop(old, Some(0.0));
            }
        }
        Some(key)
    }

    pub fn stop_static_se(&mut self, id: u32) {
        if let Some(key) = self.statics.remove(&id) {
            self.driver.stop(key, Some(0.0));
        }
    }

    /// `PlaySystemSE(kind)`: kinds 0..6 = Click, Decide, Cancel, Beep, Tab, GameHelpOpen,
    /// GameHelpMessage -> static SE 0, 1, 3, 4, 2, 2, 5 (0x14176F7D0).
    pub fn play_system_se(&mut self, kind: u32) -> Option<SoundKey> {
        let id = *RULES.ui_system_se_static_ids.get(kind as usize)? as u32;
        self.play_static_se(id)
    }

    /// The low-HP alarm, once per step: `low` = the HUD status tick's request for the player
    /// slot (`alarm::low_hp`), `paused` = the game is paused.
    pub fn alarm_tick(&mut self, dt: f32, low: bool, paused: bool) {
        self.alarm.request(1, low);
        self.alarm_step(dt, paused);
    }

    /// The alarm tick alone, for a host whose HUD sets the request bits itself (`alarm.request`).
    pub fn alarm_step(&mut self, dt: f32, paused: bool) {
        match self.alarm.tick(dt, paused) {
            AlarmAction::Start => {
                self.play_static_se(ALARM_STATIC_SE);
            }
            AlarmAction::Stop => self.stop_static_se(ALARM_STATIC_SE),
            AlarmAction::None => {}
        }
    }

    /// (DON-KIT) The WAV materials (bank, material) the sounds of these effects play (their SeAttach rows, every track
    /// of each sound): what a host reads ahead so an effect's first sound is never late (`Materials::preload`).
    pub fn effect_materials(&self, names: &[&str]) -> Vec<(u32, u32)> {
        let mut out: Vec<(u32, u32)> = Vec::new();
        for name in names {
            for se in self.routes.effect_sounds(name) {
                let Some(asset) = self.pack.asset(&se.asset) else { continue };
                let Some(def) = self.pack.sound(asset.bank, asset.index) else { continue };
                for track in def.tracks.values() {
                    if !out.contains(&(asset.bank, track.material)) {
                        out.push((asset.bank, track.material));
                    }
                }
            }
        }
        out
    }

    /// (DON-KIT) Reads these materials of this engine's pack into a host's cache (`Materials::preload`).
    pub fn preload_materials(&self, materials: &mut crate::Materials, list: &[(u32, u32)]) -> usize {
        materials.preload(&self.pack, list)
    }

    /// The effect seam spawned an effect: the sounds its particle system carries play attached
    /// to it (spec 5.5). `name`: the effect asset name or path.
    pub fn effect_spawned(&mut self, handle: u32, name: &str, position: Vec3) {
        self.in_handle_scope(Some(handle), |e| e.effect_spawned_scoped(handle, name, position)); // (MIMIC-BOSS hotfix 3, MB3-AUDIO)
    }

    /// (MIMIC-BOSS hotfix 3, MB3-AUDIO) [`Engine::effect_spawned`] under the driver scope already set.
    fn effect_spawned_scoped(&mut self, handle: u32, name: &str, position: Vec3) {
        // A spell / shot ACTOR (the effect seam names it by its class): its fire SE at the place
        // it is spawned (`MyFireEffects[].MySoundCue`, rows `code.projectile`), and the sounds
        // its fire and body particle systems carry, attached to it: they follow the actor and
        // end with it (`effect_stopped`: FadeOutTime, loops included).
        if let Some(actor) = self.pack.actor(name.rsplit(['/', '.']).next().unwrap_or(name)).cloned() {
            let cause = format!("actor {}", actor.class);
            if let Some(asset) = &actor.fire_se {
                self.play_asset(asset, &format!("{cause} fire"), AssetPlay::at(Some(position), None));
            }
            for (role, effect) in actor.effects.iter().filter(|(r, _)| r == "fire" || r == "body") {
                for se in self.routes.effect_sounds(effect).to_vec() {
                    if se.at_end {
                        self.effect_ends.entry(handle).or_insert_with(|| (effect.clone(), position));
                        continue;
                    }
                    if let Some(key) = self.play_asset(&se.asset, &format!("{cause} {role} effect {effect}"), AssetPlay::at(Some(position), Some(Anchor::Effect(handle)))) {
                        let fade = self.end_fade(&se);
                        self.effects.entry(handle).or_default().push((key, fade));
                    }
                }
            }
        }
        let sounds = self.routes.effect_sounds(name).to_vec();
        if sounds.iter().any(|se| se.at_end) {
            self.effect_ends.insert(handle, (name.to_string(), position));
        }
        for se in sounds.into_iter().filter(|se| !se.at_end) {
            let cause = format!("effect {}", name.rsplit(['/', '.']).next().unwrap_or(name));
            if let Some(key) = self.play_asset(&se.asset, &cause, AssetPlay::at(Some(position), Some(Anchor::Effect(handle)))) {
                let fade = self.end_fade(&se);
                self.effects.entry(handle).or_default().push((key, fade));
            }
        }
    }

    /// What happens to an effect's sound when the effect ends: `Some(fade)` = stopped over
    /// `FadeOutTime`; `None` = it plays out (`bNoSeFadeAtTheEffectEnd`). A LOOPING asset
    /// cannot play out: its audio component is attached to the particle component and goes
    /// with it, so it is stopped at once [inf: the destroy path was not read].
    fn end_fade(&self, se: &crate::routes::EffectSe) -> Option<f32> {
        se.fade_out.or_else(|| self.pack.asset(&se.asset).is_some_and(|a| a.looping).then_some(0.0))
    }

    /// The effect ended: its sounds fade out over their `FadeOutTime`, unless
    /// `bNoSeFadeAtTheEffectEnd` (they play out).
    pub fn effect_stopped(&mut self, handle: u32) {
        self.in_handle_scope(Some(handle), |e| e.effect_stopped_scoped(handle)); // (MIMIC-BOSS hotfix 3, MB3-AUDIO)
    }

    /// (MIMIC-BOSS hotfix 3, MB3-AUDIO) [`Engine::effect_stopped`] under the driver scope already set.
    fn effect_stopped_scoped(&mut self, handle: u32) {
        if let Some((name, position)) = self.effect_ends.remove(&handle) {
            for se in self.routes.effect_sounds(&name).to_vec().into_iter().filter(|se| se.at_end) {
                self.play_asset(&se.asset, &format!("effect end {}", name.rsplit(['/', '.']).next().unwrap_or(&name)), AssetPlay::at(Some(position), None));
            }
        }
        for (key, fade) in self.effects.remove(&handle).unwrap_or_default() {
            if let Some(fade) = fade {
                self.driver.stop(key, Some(fade));
            }
        }
    }

    /// A category event by name (spec 3.4).
    pub fn event(&mut self, name: &str) -> bool {
        self.driver.event(name)
    }

    /// One step: follows the anchors, runs the driver, returns the voice commands.
    pub fn update(&mut self, dt: f32, listener: &Listener, resolve: &dyn Fn(&Anchor) -> Option<Vec3>) -> Vec<VoiceCommand> {
        self.clock += dt as f64;
        self.outfit_step.clear();
        for (key, tracked) in &self.tracked {
            if let Some(position) = tracked.anchor.as_ref().and_then(resolve) {
                self.driver.set_position(*key, position);
            }
        }
        let commands = self.driver.update(dt, listener);
        for note in self.driver.take_notes() {
            let (kind, key, bank, index, material, params, distance, note_text) = match note {
                Note::Play { key, bank, index } => ("play", Some(key), bank, index, None, None, None, String::new()),
                Note::Cancel { bank, index, by_macro } => ("cancel", None, bank, index, None, None, None, format!("start macro {by_macro}")),
                Note::Drop { bank, index } => ("drop", None, bank, index, None, None, None, "category pool full of higher priority".to_string()),
                Note::Steal { victim, bank, index } => ("steal", Some(victim), bank, index, None, None, None, "category pool full: stopped at once".to_string()),
                Note::MacroStop { key, by_macro, fade } => {
                    let (bank, index) = self.driver.active().iter().find(|a| a.key == key).map_or((u32::MAX, 0), |a| (a.bank, a.index));
                    ("macro-stop", Some(key), bank, index, None, None, None, format!("start macro {by_macro}, fade {fade}"))
                }
                Note::Voice { key, bank, index, material, params, distance, .. } => {
                    // (FACE) a voice line's first voice: its start and its material for the lip-sync
                    if let Some(rec) = self.lines.get_mut(&key).filter(|r| r.started.is_none()) {
                        rec.started = Some(self.clock);
                        rec.bank = bank;
                        rec.material = Some(material);
                    }
                    ("voice", Some(key), bank, index, Some(material), Some(params), distance, String::new())
                }
                Note::End { key } => {
                    self.tracked.remove(&key);
                    self.lines.remove(&key); // (FACE)
                    continue;
                }
            };
            let def = self.pack.sound(bank, index);
            self.log.push(LogLine {
                kind,
                cause: key.and_then(|k| self.tracked.get(&k)).map_or(String::new(), |t| t.cause.clone()),
                sound: if bank == u32::MAX { String::new() } else { self.sound_label(bank, index) },
                category: def.map_or(String::new(), |d| self.pack.category_name(d.category).to_string()),
                file: material.and_then(|m| self.pack.banks.get(bank as usize)?.materials.get(&m)).map_or(String::new(), |m| m.file.clone()),
                params,
                distance,
                note: note_text,
            });
        }
        self.seam.retain(|_, keys| {
            keys.retain(|k| self.driver.is_playing(*k));
            !keys.is_empty()
        });
        self.effects.retain(|_, keys| {
            keys.retain(|k| self.driver.is_playing(k.0));
            !keys.is_empty()
        });
        self.statics.retain(|_, key| self.driver.is_playing(*key));
        self.lines.retain(|k, _| self.driver.is_playing(*k)); // (FACE) a cancelled / dropped / ended line goes
        commands
    }

    /// The log lines since the last call.
    pub fn take_log(&mut self) -> Vec<LogLine> {
        std::mem::take(&mut self.log)
    }

    /// The live sounds, oldest first.
    pub fn active(&self) -> Vec<ActiveLine> {
        self.driver
            .active()
            .into_iter()
            .map(|a| ActiveLine {
                key: a.key,
                cause: self.tracked.get(&a.key).map_or(String::new(), |t| t.cause.clone()),
                bank: self.pack.bank(a.bank).name.clone(),
                index: a.index,
                category: self.pack.category_name(a.category).to_string(),
                priority: a.priority,
                voices: a.voices,
                params: a.params,
                distance: a.distance,
                position: a.position,
                age: a.age,
                stopping: a.stopping,
            })
            .collect()
    }
}
