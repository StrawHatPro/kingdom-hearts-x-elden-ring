//! The audio pack as the driver reads it: `build/sora_pack/audio/manifest.json`
//! (`tools/sora_audio_pack.js`: banks with the playback data of every used sound, sound assets,
//! triggers, the SEAD configuration's categories and macros) and `tables.json`
//! (`tools/sora_audio_tables.js`: category events, the static SE table, the UI system SE kinds).
//!
//! Everything here is data as stored; the rules that read it are in `driver.rs` (what plays when
//! a sound index is started) and `routes.rs` (which sound a trigger means).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use kh_anim::json::Json;

/// Open question 1 of the spec: `Volume` of a sound asset that does not serialize it (643 of the
/// 947 packed assets). ASSUMED: the stock `USoundWave` default.
pub const ASSET_VOLUME_DEFAULT_ASSUMED: f32 = 1.0;

/// How a sound chooses its sequences (spec 2.2). A group = consecutive sequences with the same
/// group byte; `members` are indices into `SoundDef::sequences`.
#[derive(Clone, Debug, PartialEq)]
pub enum Selection {
    /// Sound: every sequence starts.
    All,
    /// Random: `always` plus one weighted pick.
    Random { always: Vec<Group>, choose: Vec<Group>, no_repeat: bool },
    /// Order: `always` plus the next of `rotate` (round robin).
    Order { always: Vec<Group>, rotate: Vec<Group> },
    /// Cycle: `always`, one weighted pick, and a further pick whenever the interval runs out
    /// while fewer than `max_active` groups are active.
    Cycle { always: Vec<Group>, choose: Vec<Group>, interval_ms: (u32, u32), max_active: u32 },
    /// Switch: every sequence whose `[min, max)` holds the switch value (none of Sora's).
    Switch,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub weight: u32,
    pub members: Vec<usize>,
}

/// A sequence command (spec 2.3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SeqCmd {
    KeyOn { track: u32, looped: bool },
    Wait { ms: u32 },
    WaitRandom { min_ms: u32, max_ms: u32 },
    KeyOff,
    /// `loopStart(max, min)`: 0 / 0 = endless. None of Sora's sounds has one.
    LoopStart { min: u32, max: u32 },
    LoopEnd,
    End,
}

/// Which track parameter a command drives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Param {
    Volume,
    Pitch,
    Pan,
    PanDepth,
}

/// A track command (spec 2.3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TrackCmd {
    Wait { ms: u32 },
    /// Fade the parameter to `target` over `ms` with fade curve `curve`.
    Set { param: Param, target: f32, ms: u32, curve: u8 },
    /// Drawn once: `min + (rand() % 1000) * 0.001 * (max - min)`; mode 1 added, 2 multiplied.
    Random { param: Param, min: f32, max: f32, mode: u8 },
    Lfo { param: Param, period: f32, depth: f32, wave: u8 },
    /// `lfeSend`, `centerMix` (stereo output: not used) and anything unknown.
    Other,
    End,
}

/// A zero-one curve: piecewise, each segment shaped by the fade curve of its first point.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Curve {
    /// (x, y, curve number)
    pub points: Vec<(f32, f32, u8)>,
}

impl Curve {
    pub fn eval(&self, x: f32) -> f32 {
        let Some(first) = self.points.first() else { return 1.0 };
        if x <= first.0 {
            return first.1;
        }
        for pair in self.points.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if x <= b.0 {
                let t = if b.0 > a.0 { (x - a.0) / (b.0 - a.0) } else { 1.0 };
                return a.1 + (b.1 - a.1) * crate::driver::curve(a.2, t);
            }
        }
        self.points.last().map_or(1.0, |p| p.1)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Sequence {
    pub commands: Vec<SeqCmd>,
    /// `zeroone_distance_rate` -> low-pass factor (1.0 = open).
    pub lowpass: Option<Curve>,
    /// `zeroone_distance_rate` -> aux send in dB.
    pub aux_send_db: Option<Curve>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub commands: Vec<TrackCmd>,
    /// The material (WAV) it plays, a key of `Bank::materials`.
    pub material: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    /// Relative to the audio pack folder.
    pub file: String,
    pub channels: u32,
    pub sample_rate: u32,
    pub samples: u32,
    /// `[start, end)` in samples.
    pub loop_points: Option<(u32, u32)>,
}

/// One sound of a bank: everything the driver needs to play it.
#[derive(Clone, Debug, PartialEq)]
pub struct SoundDef {
    pub index: u32,
    pub number: u32,
    pub category: u32,
    pub priority: i32,
    pub volume: f32,
    pub macro_start: u32,
    pub macro_stop: u32,
    pub flags: u32,
    pub flags2: u32,
    pub release_seconds: f32,
    /// The first aux send's volume (bus "Revibe"), 0 when there is none.
    pub aux_send_volume: f32,
    pub main_bus: String,
    pub selection: Selection,
    pub sequences: Vec<Sequence>,
    pub tracks: HashMap<u32, Track>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Bank {
    /// "SE/footstep/10000_sora/10010/se10010".
    pub name: String,
    pub number: i32,
    pub sounds: HashMap<u32, SoundDef>,
    pub materials: HashMap<u32, Material>,
}

/// A sound asset (the UE side, spec 3).
#[derive(Clone, Debug, PartialEq)]
pub struct Asset {
    pub bank: u32,
    pub index: u32,
    /// `None` = not serialized (see `ASSET_VOLUME_DEFAULT_ASSUMED`).
    pub volume: Option<f32>,
    /// `Duration == 10000`: a loop for the UE side.
    pub looping: bool,
    /// Has `AttenuationSettings`: 3D when played at a location or attached.
    pub attenuation: bool,
    /// `bOverrideStopFadeTime` + `StopFadeTime`.
    pub stop_fade: Option<f32>,
}

/// A category of the SEAD configuration.
#[derive(Clone, Debug, PartialEq)]
pub struct Category {
    pub number: u32,
    pub name: String,
    /// The category whose instance pool it uses (itself when it has its own).
    pub pool: u32,
    /// Size of that pool.
    pub pool_size: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmp {
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
}

impl Cmp {
    fn parse(text: &str) -> Cmp {
        match text {
            "==" => Cmp::Eq,
            "!=" => Cmp::Ne,
            ">" => Cmp::Gt,
            ">=" => Cmp::Ge,
            "<" => Cmp::Lt,
            _ => Cmp::Le,
        }
    }

    pub fn test(self, a: f64, b: f64) -> bool {
        match self {
            Cmp::Eq => a == b,
            Cmp::Ne => a != b,
            Cmp::Gt => a > b,
            Cmp::Ge => a >= b,
            Cmp::Lt => a < b,
            Cmp::Le => a <= b,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MacroField {
    Category,
    SoundId,
    Priority,
    Macro,
    BankNumber,
}

/// A command of a start macro (spec 2.1; interpreter 0x145976BA0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MacroCmd {
    ResetSet,
    /// Keep the sounds with `self.field - other.field CMP value` (relative) or `other.field CMP value`.
    Filter { field: MacroField, cmp: Cmp, relative: bool, value: i32 },
    /// Keep the sounds whose seconds since start `CMP value`.
    FilterElapsed { cmp: Cmp, value: f32 },
    /// A filter the port does not evaluate (`filterPan`, `filterVolume` ...): keeps the set.
    FilterUnread,
    StopOthers { count: (Cmp, u32), fade: f32 },
    CancelSelf { count: (Cmp, u32) },
    /// `selfVolume`: the new sound's macro fade slot goes to `volume` over `fade`.
    SelfVolume { count: (Cmp, u32), volume: f32, fade: f32 },
    /// An action the port does not perform.
    ActionUnread,
    End,
}

/// A command of a category event (spec 3.4).
#[derive(Clone, Debug, PartialEq)]
pub enum EventCmd {
    SetVolume { category: u32, layer: usize, volume: f32, fade: f32 },
    SetPitch { category: u32, pitch: f32, fade: f32 },
    Stop { category: u32, fade: f32 },
    Pause { category: u32, fade: f32 },
    Resume { category: u32, fade: f32 },
}

/// One sound of a trigger.
#[derive(Clone, Debug, PartialEq)]
pub struct TriggerSound {
    /// The asset path as referenced (voices: under `/Localization/ja/`).
    pub asset: String,
    pub weight: Option<u32>,
    pub alias_index: Option<u32>,
    pub group: Option<String>,
    pub group_no: Option<i32>,
}

/// A row of the manifest's `triggers` (spec 5).
#[derive(Clone, Debug, PartialEq)]
pub struct Trigger {
    pub id: String,
    pub route: String,
    /// What starts it, in words.
    pub trigger: String,
    pub clip: Option<String>,
    /// The logical animations (rows of the anim set) that play the clip.
    pub logical: Vec<String>,
    pub time: Option<f32>,
    pub duration: Option<f32>,
    pub sounds: Vec<TriggerSound>,
    /// "3D" / "2D".
    pub spatial_3d: bool,
    pub volume: Option<f32>,
    pub pitch: Option<f32>,
    pub condition: Option<String>,
    /// How the code names the sound ("sora_magic no 20", "ETresStaticSE 33 (se00000_067)" ...).
    pub sound_id: Option<String>,
    pub surface: Option<u32>,
    pub slot: Option<String>,
    /// `bEnableSetting` of an auto-SE enable window.
    pub enable_setting: Option<bool>,
    /// `FadeOutTime` of an effect-attached sound; `no_fade_at_end` = `bNoSeFadeAtTheEffectEnd`.
    pub fade_out: Option<f32>,
    pub no_fade_at_end: bool,
    /// `bPlayAtEnd`: the sound plays when the effect ends, not when it starts.
    pub play_at_end: bool,
    /// (party stream E) The character the row belongs to: `None` = Sora; "n_ex001" Donald, "n_ex002" Goofy
    /// (tools/party_audio_pack.js). Routes are built per owner (`Routes::for_owner`).
    pub owner: Option<String>,
    /// (party stream E) The weapon model of a weapon-set row ("w_do010"): a party member has 14, one equipped.
    pub weapon: Option<String>,
}

impl Trigger {
    /// "sora_magic no 20" -> ("sora_magic", 20): the voice group and the number a state sets as
    /// the pawn voice group number before its animation's voice notify fires (spec 5.2).
    pub fn voice_group(&self) -> Option<(&str, i32)> {
        let text = self.sound_id.as_deref()?.strip_prefix("voice group ").map_or(self.sound_id.as_deref()?, |rest| rest);
        let (group, no) = text.split_once(" no ")?;
        Some((group.trim_matches('"'), no.trim().parse().ok()?))
    }

    /// The trigger text after its subject ("cast voice number", "collected", "bounce on the ground").
    pub fn what(&self) -> &str {
        self.trigger.split_once(": ").map_or(self.trigger.as_str(), |(_, what)| what)
    }
}

/// The pack.
#[derive(Clone, Debug, Default)]
pub struct Pack {
    pub dir: PathBuf,
    pub banks: Vec<Bank>,
    pub bank_by_name: HashMap<String, u32>,
    pub assets: HashMap<String, Asset>,
    pub categories: HashMap<u32, Category>,
    pub category_by_name: HashMap<String, u32>,
    pub macros: HashMap<u32, Vec<MacroCmd>>,
    pub triggers: Vec<Trigger>,
    pub events: HashMap<String, Vec<EventCmd>>,
    /// `ETresStaticSE` id -> asset path (43 rows; `None` = not in the pack).
    pub static_se: Vec<Option<String>>,
    /// Spell / shot actor classes and their sounds, by lower-case class name
    /// ("tresproj_magfirenml_c"; tables.json `projectiles`).
    pub actors: HashMap<String, ActorSounds>,
    /// Sora's skeleton sockets by name (tables.json `sockets`): a sound attached to a socket
    /// (`R_wep_skt`, `vfx_root` ...) is at the parent bone's transform applied to `location`.
    pub sockets: HashMap<String, Socket>,
    /// What the loader could not use, for the host's log.
    pub warnings: Vec<String>,
    /// (ER77) The outfits' own voices (`load_outfit_voices`, outfit.rs): each outfit's clips are a bank of `banks`.
    pub outfit_voices: Vec<crate::outfit::OutfitVoice>,
    /// (party stream E) `TresActorVoice` asset name ("av_ex_chat_donald_0070") -> the sound assets of its sound
    /// sections (party_tables.json `actor_voices`; the field chat lines of the talk clips M_0811 / M_0812).
    pub actor_voices: HashMap<String, Vec<String>>,
    /// (FACE) THE VOICE LINES' LOUDNESS for the faces' lip-sync (tools/face_lip_pack.js: `lip.json` beside a manifest):
    /// a material by its file from `wav/` on ([`lip_key`]) -> its loudness at `lip_rate` Hz, 0..255 of the line's own
    /// peak. Read by `Pack::load` and `merge_party_root`; empty without the file (the faces then use the line's length).
    pub lip: HashMap<String, std::sync::Arc<[u8]>>,
    /// (FACE) The samples per second of `lip`; 0 = no table.
    pub lip_rate: f32,
}

/// (FACE) The key of a material file in `Pack::lip`: from its `wav/` folder on, '/' separators, lower case.
pub fn lip_key(file: &str) -> String {
    let f = file.replace('\\', "/").to_ascii_lowercase();
    if f.starts_with("wav/") {
        return f;
    }
    match f.rfind("/wav/") {
        Some(i) => f[i + 1..].to_string(),
        None => f,
    }
}

/// The sounds of a spell / shot actor class (`ATresProjectileBase` blueprints; spec 5.4 - 5.6).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ActorSounds {
    /// The blueprint class, as the effect seam names the actor ("TresProj_MagFireNml_C").
    pub class: String,
    /// The spell command that spawns it ("TRES_CMD_KIND_FIRE"), for the first stage of a spell.
    pub command: Option<String>,
    /// (role, effect name): the particle systems of the class by role (fire, body, hit, map_hit,
    /// PutEffect); the sounds they carry are the effect-attached ones of spec 5.5.
    pub effects: Vec<(String, String)>,
    /// `MyFireEffects[].MySoundCue` (rows `code.projectile`): when the actor is spawned.
    pub fire_se: Option<String>,
    /// `m_HitEffects[0].m_PawnHitSEAsset` / `m_MapHitSEAsset` (rows `code.hit_magic`), of the
    /// class or the nearest class of its chain.
    pub pawn_hit_se: Option<String>,
    pub map_hit_se: Option<String>,
}

/// A `SkeletalMeshSocket` of Sora's skeleton: UE axes, cm, degrees (pitch, yaw, roll).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Socket {
    pub bone: String,
    pub location: [f32; 3],
    pub rotation: [f32; 3],
}

fn opt_f32(j: &Json) -> Option<f32> {
    j.as_f32()
}

fn opt_string(j: &Json) -> Option<String> {
    j.as_str().map(str::to_string)
}

fn groups(j: &Json, index_of: &HashMap<usize, usize>) -> Vec<Group> {
    j.items()
        .iter()
        .map(|g| Group { weight: g.get("weight").as_usize().unwrap_or(0) as u32, members: g.get("members").items().iter().filter_map(|m| m.as_usize().and_then(|m| index_of.get(&m).copied())).collect() })
        .collect()
}

fn curve_of(z: &Json) -> Curve {
    Curve { points: z.get("points").items().iter().map(|p| (p.get("x").as_f32().unwrap_or(0.0), p.get("y").as_f32().unwrap_or(1.0), p.get("curve").as_usize().unwrap_or(0) as u8)).collect() }
}

fn count_of(text: Option<&str>) -> (Cmp, u32) {
    let text = text.unwrap_or(">= 1");
    let mut parts = text.split_whitespace();
    let cmp = Cmp::parse(parts.next().unwrap_or(">="));
    (cmp, parts.next().and_then(|n| n.parse().ok()).unwrap_or(1))
}

impl Pack {
    /// Reads `dir/manifest.json` and, when it is there, `dir/tables.json`.
    pub fn load(dir: &Path) -> Result<Pack, String> {
        let text = std::fs::read_to_string(dir.join("manifest.json")).map_err(|e| format!("{}: {e}", dir.join("manifest.json").display()))?;
        let manifest = Json::parse(&text)?;
        let tables = match std::fs::read_to_string(dir.join("tables.json")) {
            Ok(text) => Some(Json::parse(&text)?),
            Err(_) => None,
        };
        let mut pack = Pack::from_json(&manifest, tables.as_ref())?;
        pack.dir = dir.to_path_buf();
        // (party stream E) the party's spell actor classes and actor voices (tools/party_audio_pack.js)
        if let Ok(text) = std::fs::read_to_string(dir.join("party_tables.json")) {
            pack.add_party_tables(&Json::parse(&text)?);
        }
        pack.load_lip(dir); // (FACE) the voice lines' loudness for the lip-sync
        if tables.is_none() {
            pack.warnings.push("tables.json not found (run node tools/sora_audio_tables.js): no category events, no static SE table".to_string());
        }
        Ok(pack)
    }

    pub fn from_json(manifest: &Json, tables: Option<&Json>) -> Result<Pack, String> {
        let format = manifest.get("format").as_str().unwrap_or("");
        if !format.starts_with("kh3-sora-audio-pack/") {
            return Err(format!("not an audio pack manifest (format {format:?})"));
        }
        let mut pack = Pack::default();

        // Categories and their pools.
        let config = manifest.get("config");
        let listed: Vec<(u32, String, u32, Option<u32>)> = config
            .get("categories")
            .items()
            .iter()
            .map(|c| (c.get("number").as_usize().unwrap_or(0) as u32, c.get("name").as_str().unwrap_or("").to_string(), c.get("max_sounds").as_usize().unwrap_or(0) as u32, c.get("shares_pool_of").as_usize().map(|n| n as u32)))
            .collect();
        for (number, name, max, shares) in &listed {
            let pool = shares.unwrap_or(*number);
            let pool_size = listed.iter().find(|c| c.0 == pool).map_or(*max, |c| c.2);
            pack.category_by_name.insert(name.clone(), *number);
            pack.categories.insert(*number, Category { number: *number, name: name.clone(), pool, pool_size });
        }

        // Macros.
        for m in config.get("macros").items() {
            let number = m.get("number").as_usize().unwrap_or(0) as u32;
            let commands = m
                .get("commands")
                .items()
                .iter()
                .map(|c| {
                    let name = c.get("name").as_str().unwrap_or("");
                    let cmp = Cmp::parse(c.get("cmp").as_str().unwrap_or("=="));
                    let relative = c.get("relativeToSelf").as_bool().unwrap_or(false);
                    let value = c.get("value").as_f64().unwrap_or(0.0);
                    let field = |field| MacroCmd::Filter { field, cmp, relative, value: value as i32 };
                    let count = count_of(c.get("ifCount").as_str());
                    match name {
                        "end" => MacroCmd::End,
                        "resetSet" => MacroCmd::ResetSet,
                        "filterCategory" => field(MacroField::Category),
                        "filterSoundId" => field(MacroField::SoundId),
                        "filterPriority" => field(MacroField::Priority),
                        "filterMacro" => field(MacroField::Macro),
                        "filterBankNumber" => field(MacroField::BankNumber),
                        "filterElapsed" => MacroCmd::FilterElapsed { cmp, value: value as f32 },
                        "stopOthers" => MacroCmd::StopOthers { count, fade: c.get("fadeSeconds").as_f32().unwrap_or(0.0) },
                        "cancelSelf" => MacroCmd::CancelSelf { count },
                        "selfVolume" => MacroCmd::SelfVolume { count, volume: c.get("f1").as_f32().unwrap_or(1.0), fade: c.get("f2").as_f32().unwrap_or(0.0) },
                        n if n.starts_with("filter") => MacroCmd::FilterUnread,
                        _ => MacroCmd::ActionUnread,
                    }
                })
                .collect();
            pack.macros.insert(number, commands);
        }

        // Banks.
        for (name, b) in manifest.get("banks").members() {
            let mut bank = Bank { name: name.clone(), number: b.get("bank_number").as_i64().unwrap_or(0) as i32, sounds: HashMap::new(), materials: HashMap::new() };
            for (key, m) in b.get("materials").members() {
                let Ok(key) = key.parse::<u32>() else { continue };
                let lp = m.get("loop");
                bank.materials.insert(
                    key,
                    Material {
                        file: m.get("file").as_str().unwrap_or("").to_string(),
                        channels: m.get("channels").as_usize().unwrap_or(1) as u32,
                        sample_rate: m.get("sample_rate").as_usize().unwrap_or(48_000) as u32,
                        samples: m.get("samples").as_usize().unwrap_or(0) as u32,
                        loop_points: if lp.is_null() { None } else { Some((lp.get("start").as_usize().unwrap_or(0) as u32, lp.get("end").as_usize().unwrap_or(0) as u32)) },
                    },
                );
            }
            for (_, s) in b.get("sounds").members() {
                let index = s.get("index").as_usize().unwrap_or(0) as u32;
                // Sequences are named by their bank-wide number in `selection`; the driver indexes
                // the sound's own list.
                let mut index_of = HashMap::new();
                let mut sequences = Vec::new();
                for (k, q) in s.get("sequences").items().iter().enumerate() {
                    index_of.insert(q.get("sequence").as_usize().unwrap_or(k), k);
                    let commands = q
                        .get("commands")
                        .items()
                        .iter()
                        .map(|c| match c.get("name").as_str().unwrap_or("") {
                            "keyOn" => SeqCmd::KeyOn { track: c.get("track").as_usize().unwrap_or(0) as u32, looped: c.get("loopFlag").as_usize().unwrap_or(0) == 1 },
                            "wait" => SeqCmd::Wait { ms: c.get("ms").as_usize().unwrap_or(0) as u32 },
                            "waitRandom" => SeqCmd::WaitRandom { min_ms: c.get("minMs").as_usize().unwrap_or(0) as u32, max_ms: c.get("maxMs").as_usize().unwrap_or(0) as u32 },
                            "keyOff" => SeqCmd::KeyOff,
                            "loopStart" => SeqCmd::LoopStart { min: c.get("min").as_usize().unwrap_or(0) as u32, max: c.get("max").as_usize().unwrap_or(0) as u32 },
                            "loopEnd" => SeqCmd::LoopEnd,
                            _ => SeqCmd::End,
                        })
                        .collect();
                    let zero_one = |target: &str| q.get("zero_ones").items().iter().find(|z| z.get("name").as_str() == Some("zeroone_distance_rate") && z.get("target").as_str() == Some(target)).map(curve_of);
                    sequences.push(Sequence { commands, lowpass: zero_one("lowpass"), aux_send_db: zero_one("auxSendDb") });
                }
                let mut tracks = HashMap::new();
                for (key, t) in s.get("tracks").members() {
                    let Ok(key) = key.parse::<u32>() else { continue };
                    let commands = t
                        .get("commands")
                        .items()
                        .iter()
                        .map(|c| {
                            let name = c.get("name").as_str().unwrap_or("");
                            let (kind, param) = if let Some(rest) = name.strip_prefix("random") {
                                ("random", rest)
                            } else if let Some(rest) = name.strip_prefix("lfo") {
                                ("lfo", rest)
                            } else {
                                ("set", name)
                            };
                            let param = match param.to_ascii_lowercase().as_str() {
                                "volume" => Some(Param::Volume),
                                "pitch" => Some(Param::Pitch),
                                "pan" => Some(Param::Pan),
                                "pandepth" => Some(Param::PanDepth),
                                _ => None,
                            };
                            let f = |k: &str| c.get(k).as_f32().unwrap_or(0.0);
                            match (name, kind, param) {
                                ("end", ..) => TrackCmd::End,
                                ("wait", ..) => TrackCmd::Wait { ms: c.get("ms").as_usize().unwrap_or(0) as u32 },
                                (_, "set", Some(param)) => TrackCmd::Set { param, target: f("target"), ms: c.get("ms").as_usize().unwrap_or(0) as u32, curve: c.get("curve").as_usize().unwrap_or(0) as u8 },
                                (_, "random", Some(param)) => TrackCmd::Random { param, min: f("min"), max: f("max"), mode: c.get("mode").as_usize().unwrap_or(0) as u8 },
                                (_, "lfo", Some(param)) => TrackCmd::Lfo { param, period: f("period"), depth: f("depth"), wave: c.get("wave").as_usize().unwrap_or(0) as u8 },
                                _ => TrackCmd::Other,
                            }
                        })
                        .collect();
                    tracks.insert(key, Track { commands, material: t.get("material").as_usize().unwrap_or(0) as u32 });
                }
                let sel = s.get("selection");
                let selection = match sel.get("rule").as_str().unwrap_or("all") {
                    "random" => Selection::Random { always: groups(sel.get("always"), &index_of), choose: groups(sel.get("choose"), &index_of), no_repeat: sel.get("noRepeat").as_bool().unwrap_or(true) },
                    "order" => Selection::Order { always: groups(sel.get("always"), &index_of), rotate: groups(sel.get("rotate"), &index_of) },
                    "cycle" => Selection::Cycle {
                        always: groups(sel.get("always"), &index_of),
                        choose: groups(sel.get("choose"), &index_of),
                        interval_ms: (sel.get("intervalMs").at(0).as_usize().unwrap_or(0) as u32, sel.get("intervalMs").at(1).as_usize().unwrap_or(0) as u32),
                        max_active: sel.get("maxActive").as_usize().unwrap_or(1) as u32,
                    },
                    "switch" => Selection::Switch,
                    _ => Selection::All,
                };
                let aux = s.get("aux_sends").at(0);
                bank.sounds.insert(
                    index,
                    SoundDef {
                        index,
                        number: s.get("number").as_usize().unwrap_or(index as usize) as u32,
                        category: s.get("category").get("number").as_usize().unwrap_or(0) as u32,
                        priority: s.get("priority").as_i64().unwrap_or(0) as i32,
                        volume: s.get("volume").as_f32().unwrap_or(1.0),
                        macro_start: s.get("macro_start").as_usize().unwrap_or(0) as u32,
                        macro_stop: s.get("macro_stop").as_usize().unwrap_or(0) as u32,
                        flags: s.get("flags").as_usize().unwrap_or(0) as u32,
                        flags2: s.get("flags2").as_usize().unwrap_or(0) as u32,
                        release_seconds: s.get("release_seconds").as_f32().unwrap_or(0.0),
                        aux_send_volume: aux.get("volume").as_f32().unwrap_or(0.0),
                        main_bus: s.get("main_bus").get("name").as_str().unwrap_or("").to_string(),
                        selection,
                        sequences,
                        tracks,
                    },
                );
            }
            pack.bank_by_name.insert(name.clone(), pack.banks.len() as u32);
            pack.banks.push(bank);
        }

        // Sound assets.
        for (path, a) in manifest.get("sound_assets").members() {
            let Some(&bank) = a.get("bank").as_str().and_then(|b| pack.bank_by_name.get(b)) else { continue };
            let other = a.get("other_props");
            let stop_fade = if other.get("bOverrideStopFadeTime").as_bool() == Some(true) { other.get("StopFadeTime").as_f32().or(Some(crate::rules::RULES.asset_stop_fade_time_default)) } else { None };
            pack.assets.insert(
                path.clone(),
                Asset { bank, index: a.get("sound_index").as_usize().unwrap_or(0) as u32, volume: opt_f32(a.get("volume")), looping: a.get("looping").as_bool().unwrap_or(false), attenuation: !a.get("attenuation").is_null(), stop_fade },
            );
        }

        // Triggers.
        for t in manifest.get("triggers").items() {
            let sounds = t
                .get("sounds")
                .items()
                .iter()
                .filter_map(|s| {
                    Some(TriggerSound {
                        asset: s.get("asset").as_str()?.to_string(),
                        weight: s.get("weight").as_usize().map(|w| w as u32),
                        alias_index: s.get("alias_index").as_usize().map(|w| w as u32),
                        group: opt_string(s.get("group")),
                        group_no: s.get("group_no").as_i64().map(|n| n as i32),
                    })
                })
                .collect();
            // "FadeOutTime 1; bNoSeFadeAtTheEffectEnd" of the effect-attached rows.
            let loop_stop = t.get("loop_stop").as_str().unwrap_or("");
            let fade_out = loop_stop.split("FadeOutTime ").nth(1).and_then(|rest| rest.split(|c: char| c == ';' || c == ' ').next()).and_then(|n| n.parse::<f32>().ok());
            pack.triggers.push(Trigger {
                id: t.get("id").as_str().unwrap_or("").to_string(),
                route: t.get("route").as_str().unwrap_or("").to_string(),
                trigger: t.get("trigger").as_str().unwrap_or("").to_string(),
                clip: opt_string(t.get("clip")),
                logical: t.get("logical").items().iter().filter_map(|l| l.as_str().map(str::to_string)).collect(),
                time: opt_f32(t.get("time")),
                duration: opt_f32(t.get("duration")),
                sounds,
                spatial_3d: t.get("spatial").as_str() != Some("2D"),
                volume: opt_f32(t.get("volume")),
                pitch: opt_f32(t.get("pitch")),
                condition: opt_string(t.get("condition")),
                sound_id: opt_string(t.get("sound_id")),
                surface: t.get("surface").as_usize().map(|s| s as u32),
                // (An aux-table PlayAutoSe notify names its record in its play settings.)
                slot: opt_string(t.get("slot")).or_else(|| opt_string(t.get("params").get("PlaySettings").get("SurfaceAssetTableRecordName")).filter(|s| !s.is_empty() && s != "None")),
                enable_setting: t.get("params").get("bEnableSetting").as_bool(),
                fade_out,
                no_fade_at_end: loop_stop.contains("bNoSeFadeAtTheEffectEnd"),
                play_at_end: loop_stop.contains("bPlayAtEnd"),
                owner: opt_string(t.get("owner")),
                weapon: opt_string(t.get("weapon")),
            });
        }

        // Category events, static SE.
        if let Some(tables) = tables {
            for e in tables.get("events").items() {
                let mut commands = Vec::new();
                for c in e.get("commands").items() {
                    let Some(&category) = c.get("category").as_str().and_then(|n| pack.category_by_name.get(n)) else { continue };
                    let fade = c.get("fade").as_f32().unwrap_or(0.0);
                    commands.push(match c.get("op").as_str().unwrap_or("") {
                        "CategorySetVolume" => EventCmd::SetVolume { category, layer: c.get("layer").as_usize().unwrap_or(1), volume: c.get("volume").as_f32().unwrap_or(1.0), fade },
                        "CategorySetPitch" => EventCmd::SetPitch { category, pitch: c.get("pitch").as_f32().unwrap_or(1.0), fade },
                        "CategoryStop" => EventCmd::Stop { category, fade },
                        "CategoryPause" => EventCmd::Pause { category, fade },
                        "CategoryResume" => EventCmd::Resume { category, fade },
                        _ => continue,
                    });
                }
                pack.events.insert(e.get("name").as_str().unwrap_or("").to_string(), commands);
            }
            for row in tables.get("static_se").items() {
                let id = row.get("id").as_usize().unwrap_or(0);
                if pack.static_se.len() <= id {
                    pack.static_se.resize(id + 1, None);
                }
                pack.static_se[id] = row.get("asset").as_str().filter(|a| pack.assets.contains_key(*a)).map(str::to_string);
            }
            for row in tables.get("projectiles").items() {
                let class = row.get("class").as_str().unwrap_or("").to_string();
                let effects = row.get("effects").members().iter().flat_map(|(role, names)| names.items().iter().filter_map(|n| Some((role.clone(), n.as_str()?.to_string()))).collect::<Vec<_>>()).collect();
                let actor = ActorSounds { class: class.clone(), command: opt_string(row.get("command")), effects, fire_se: opt_string(row.get("fire_se")), pawn_hit_se: opt_string(row.get("pawn_hit_se")), map_hit_se: opt_string(row.get("map_hit_se")) };
                pack.actors.insert(class.to_ascii_lowercase(), actor);
            }
            for (name, s) in tables.get("sockets").members() {
                let three = |j: &Json| {
                    let v = j.floats();
                    [v.first().copied().unwrap_or(0.0), v.get(1).copied().unwrap_or(0.0), v.get(2).copied().unwrap_or(0.0)]
                };
                pack.sockets.insert(name.clone(), Socket { bone: s.get("bone").as_str().unwrap_or("").to_string(), location: three(s.get("location")), rotation: three(s.get("rotation")) });
            }
        }
        Ok(pack)
    }

    /// (party stream E) `party_tables.json`: spell actor classes (the `projectiles` format of tables.json) are added to
    /// `actors`; `actor_voices` to `actor_voices`.
    pub fn add_party_tables(&mut self, tables: &Json) {
        for row in tables.get("projectiles").items() {
            let class = row.get("class").as_str().unwrap_or("").to_string();
            let effects = row.get("effects").members().iter().flat_map(|(role, names)| names.items().iter().filter_map(|n| Some((role.clone(), n.as_str()?.to_string()))).collect::<Vec<_>>()).collect();
            let actor = ActorSounds { class: class.clone(), command: opt_string(row.get("command")), effects, fire_se: opt_string(row.get("fire_se")), pawn_hit_se: opt_string(row.get("pawn_hit_se")), map_hit_se: opt_string(row.get("map_hit_se")) };
            self.actors.insert(class.to_ascii_lowercase(), actor);
        }
        for (name, list) in tables.get("actor_voices").members() {
            self.actor_voices.insert(name.clone(), list.items().iter().filter_map(|a| a.as_str().map(str::to_string)).collect());
        }
    }

    /// (party D5) THE PARTY'S OWN AUDIO ROOT: `dir/manifest.json` of `node tools/sora_audio_pack.js --party-extra`
    /// (build/sora_pack/party/audio: effect-attached SE of effects the port plays for the party that the main pack
    /// does not carry, so the main pack the Elden Ring build reads is never rewritten). Its banks, sound assets and
    /// triggers are added to this pack; a bank or asset the main pack already has is kept as it is; material files
    /// become absolute paths under `dir`. Returns whether a root was found.
    pub fn merge_party_root(&mut self, dir: &Path) -> Result<bool, String> {
        let Ok(text) = std::fs::read_to_string(dir.join("manifest.json")) else { return Ok(false) };
        let extra = Pack::from_json(&Json::parse(&text)?, None)?;
        let mut index = HashMap::new();
        for (old, mut bank) in extra.banks.into_iter().enumerate() {
            if let Some(&have) = self.bank_by_name.get(&bank.name) {
                index.insert(old as u32, have);
                // (KITFX) the same KH3 bank packed in both roots with different subsets of its sounds (SE/gumi/00801: the
                // main pack has two of its sounds, YFX's party root the Meteor hit, Ultima blast and Gravity bomb): the
                // party root's sounds and materials the main bank lacks are added to it (indices are the KH3 bank's own,
                // so they agree); what the main bank has is kept as it is. Before, those sounds were "not in the pack".
                let main = &mut self.banks[have as usize];
                for (k, m) in bank.materials.iter_mut() {
                    if !main.materials.contains_key(k) {
                        m.file = dir.join(&m.file).to_string_lossy().into_owned();
                        main.materials.insert(*k, m.clone());
                    }
                }
                for (k, s) in bank.sounds.drain() {
                    main.sounds.entry(k).or_insert(s);
                }
                continue;
            }
            for m in bank.materials.values_mut() {
                m.file = dir.join(&m.file).to_string_lossy().into_owned();
            }
            let new = self.banks.len() as u32;
            index.insert(old as u32, new);
            self.bank_by_name.insert(bank.name.clone(), new);
            self.banks.push(bank);
        }
        for (path, mut asset) in extra.assets {
            if self.assets.contains_key(&path) {
                continue;
            }
            let Some(&bank) = index.get(&asset.bank) else { continue };
            asset.bank = bank;
            self.assets.insert(path, asset);
        }
        self.triggers.extend(extra.triggers);
        self.load_lip(dir); // (FACE) the party root's own voice lines' loudness
        Ok(true)
    }

    /// (FACE) `dir/lip.json` (tools/face_lip_pack.js): `{ "rate": 60, "lines": { "<lip_key>": "<hex bytes>" } }`, added to
    /// `lip` (a key already there is kept). A missing file changes nothing.
    pub fn load_lip(&mut self, dir: &Path) {
        let Ok(text) = std::fs::read_to_string(dir.join("lip.json")) else { return };
        let Ok(json) = Json::parse(text.trim_start_matches('\u{feff}')) else {
            self.warnings.push(format!("{}: not JSON: no lip-sync loudness from it", dir.join("lip.json").display()));
            return;
        };
        let rate = json.get("rate").as_f32().unwrap_or(0.0);
        if rate <= 0.0 || (self.lip_rate > 0.0 && (self.lip_rate - rate).abs() > 1e-3) {
            self.warnings.push(format!("{}: rate {rate} (the table's is {}): skipped", dir.join("lip.json").display(), self.lip_rate));
            return;
        }
        self.lip_rate = rate;
        for (key, hex) in json.get("lines").members() {
            let Some(hex) = hex.as_str() else { continue };
            let bytes: Vec<u8> = (0..hex.len() / 2).filter_map(|i| u8::from_str_radix(hex.get(i * 2..i * 2 + 2)?, 16).ok()).collect();
            self.lip.entry(lip_key(key)).or_insert_with(|| bytes.into());
        }
    }

    /// (FACE) The loudness of a bank's material (`lip`) and the table's rate.
    pub fn lip_of(&self, bank: u32, material: u32) -> Option<(std::sync::Arc<[u8]>, f32)> {
        let m = self.banks.get(bank as usize)?.materials.get(&material)?;
        self.lip.get(&lip_key(&m.file)).map(|e| (e.clone(), self.lip_rate))
    }

    /// (FACE) A bank material's length (s), from its sample count.
    pub fn material_seconds(&self, bank: u32, material: u32) -> Option<f32> {
        let m = self.banks.get(bank as usize)?.materials.get(&material)?;
        (m.sample_rate > 0).then(|| m.samples as f32 / m.sample_rate as f32)
    }

    pub fn bank(&self, bank: u32) -> &Bank {
        &self.banks[bank as usize]
    }

    pub fn sound(&self, bank: u32, index: u32) -> Option<&SoundDef> {
        self.banks.get(bank as usize)?.sounds.get(&index)
    }

    /// The sounds of an actor class, by the name the effect seam gives it (any case, with or
    /// without the `_C` suffix).
    pub fn actor(&self, class: &str) -> Option<&ActorSounds> {
        let key = class.to_ascii_lowercase();
        self.actors.get(&key).or_else(|| self.actors.get(&format!("{key}_c")))
    }

    /// The asset at `path` (with or without the `.Name` object suffix).
    pub fn asset(&self, path: &str) -> Option<&Asset> {
        self.assets.get(asset_key(path))
    }

    pub fn category_name(&self, number: u32) -> &str {
        self.categories.get(&number).map_or("?", |c| c.name.as_str())
    }

    /// Every trigger of a route ("code.prize", "code.hit_magic" ...): the lookup for the routes
    /// later implementers wire (magic, items, prizes).
    pub fn triggers_of<'a>(&'a self, route: &'a str) -> impl Iterator<Item = &'a Trigger> + 'a {
        self.triggers.iter().filter(move |t| t.route == route)
    }

    /// THE LOOKUP FOR THE CODE ROUTES of later implementers (spec 5.2, 5.4, 5.6): the rows of `route`
    /// whose trigger text starts with `subject`. Examples:
    /// `lookup("code.voice_magic", "TRES_CMD_KIND_FIRE:")` (cast and finisher voice numbers of a
    /// spell: `Trigger::voice_group`), `lookup("code.voice_item", "Item used on target char id 3 ")`,
    /// `lookup("code.prize", "PrizeBoxHeal:")` (collected / bounce), `lookup("code.projectile",
    /// "TresProj_MagFireNml:")`, `lookup("code.hit_magic", "TresProj_MagFireBase:")`. Each row names
    /// its sound asset(s) in `sounds` (`Engine::play_asset`).
    pub fn lookup<'a>(&'a self, route: &'a str, subject: &'a str) -> impl Iterator<Item = &'a Trigger> + 'a {
        self.triggers_of(route).filter(move |t| t.trigger.starts_with(subject))
    }

    /// The file of a material, absolute.
    pub fn material_path(&self, bank: u32, material: u32) -> Option<PathBuf> {
        Some(self.dir.join(&self.banks.get(bank as usize)?.materials.get(&material)?.file))
    }
}

/// "/Game/A/B/name.name" -> "/Game/A/B/name".
pub fn asset_key(path: &str) -> &str {
    match (path.rfind('/'), path.rfind('.')) {
        (Some(slash), Some(dot)) if dot > slash => &path[..dot],
        _ => path,
    }
}
