//! THE RULES EVERY HOST APPLIES between the core's seam and `kh_audio::Engine` (cut out of
//! kh-app/src/audio.rs `update`, which calls these now; the Elden Ring host calls them through
//! `HostAudio`):
//!
//! - a seam `Play`: the low-HP alarm has ONE owner; a spell / shot actor's hit sound is named
//!   by its class; everything else is a NAME the engine's routes resolve; the attach is kept so
//!   that the sound follows it;
//! - the effect seam: sounds carried by particle systems play with their effect and end with it;
//! - the sound states the core announces (the shotlock's slow);
//! - ONE ENGINE STEP PER SIMULATION STEP: the automatic motion sounds from the step's bones and
//!   the surface under him, the alarm, the anchors, the driver -> voice commands;
//! - spec 5.4: no hit sound when the attacker result of the hit is REFLECT.

use std::collections::HashMap;

use glam::Vec3;
use kh_audio::{Anchor, AutoSeEvent, BoneFrame, Engine, Listener, LogLine, SeamPlay, VoiceCommand};
use kh_core::{Attach, SoundPlay};

use super::pose::{BoneSample, PoseTap};
use crate::seam::EffectCommand;

/// The attach of every live seam sound and effect, for following.
#[derive(Default)]
pub struct Attaches {
    pub seam: HashMap<u32, Attach>,
    pub effect: HashMap<u32, Attach>,
}

impl Attaches {
    /// Forgets the sounds that have ended (once per frame, after the engine's steps).
    pub fn retire(&mut self, engine: &Engine) {
        self.seam.retain(|handle, _| engine.seam_live(*handle));
    }

    pub fn clear(&mut self) {
        self.seam.clear();
        self.effect.clear();
    }
}

/// (FACE) The party members' voice lines for their faces' lip-sync (kh_anim::face_layer), by member index (0 Donald,
/// 1 Goofy): kh_audio `Engine::party_voice`, once per step after the engine's step; a host hands them to its poser
/// (kh-partyhost `PartyPoser::set_voices`) before the next pose.
pub fn party_voice_inputs(engine: &Engine) -> Vec<(u8, Option<kh_anim::face_layer::VoiceInput>)> {
    (0u8..2)
        .map(|m| {
            let v = engine.party_voice(m).map(|l| kh_anim::face_layer::VoiceInput { id: l.key as u64, elapsed: l.elapsed, length: l.length, loudness: l.loudness });
            (m, v)
        })
        .collect()
}

/// Who asks for the low-HP alarm this step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Alarm {
    /// The host's HUD sets the request bits itself (`engine.alarm.request`).
    Hud,
    /// No such HUD: the request follows his HP (`kh_audio::alarm::low_hp`).
    Hp { low: bool },
}

/// Spec 5.4 with combat strand C (build/sora_tmp/combat/P2_C_section.md): NO HIT SOUND WHEN THE
/// ATTACKER RESULT OF THE HIT IS REFLECT. The core emits a hit's sound just before it delivers
/// the hit; the seam handles of the sounds whose hit came back reflected are returned. (Kingdom
/// Key hits on level geometry are silent too: the core emits no sound for them at all.)
/// `events` must hold EVERY step since the last call, not only the last step's: a frame can take
/// several steps. (The core's hit code now withholds the Keyblade's hit sound itself when it
/// decides REFLECT; this stays for the sounds it does not decide for.)
pub fn reflected_hit_sounds(events: &[kh_core::Event]) -> Vec<u32> {
    use kh_core::host::{EventKind, ReactionResult};
    let mut reflected = Vec::new();
    let mut pending = None;
    for event in events {
        match &event.kind {
            EventKind::SoundPlay { handle, play } => pending = matches!(play.at, Attach::World(_)).then_some(handle.0),
            EventKind::Hit { record, .. } => {
                if let Some(handle) = pending.take() {
                    if record.info.attacker_result == ReactionResult::Reflect {
                        reflected.push(handle);
                    }
                }
            }
            _ => {}
        }
    }
    reflected
}

/// A `Play` of the audio seam that is not one of the host's own debug sounds. `position`: where
/// its attach is now. `hud_owns_alarm`: see below. False: nothing could be played for it.
pub fn seam_play(engine: &mut Engine, attaches: &mut Attaches, hud_owns_alarm: &mut bool, handle: u32, play: &SoundPlay, position: Vec3, anim: (&str, f32)) -> bool {
    seam_play_key(engine, attaches, hud_owns_alarm, handle, play, position, anim).is_some()
}

/// (ui gain) [`seam_play`] answering the engine's key of the sound that started (`None`: nothing was played).
pub fn seam_play_key(engine: &mut Engine, attaches: &mut Attaches, hud_owns_alarm: &mut bool, handle: u32, play: &SoundPlay, position: Vec3, anim: (&str, f32)) -> Option<kh_audio::SoundKey> {
    // THE LOW-HP ALARM HAS ONE OWNER. A game HUD (kh-app's gfx_hud: kh_ui's status driver) plays
    // static SE 9 through the seam queue by itself; the first time it does, the audio side
    // stops requesting the alarm on its own (it only does so for a host without that HUD).
    let base = |path: &str| path.rsplit('/').next().map(|n| n.split('.').next().unwrap_or(n).to_string());
    if !*hud_owns_alarm && engine.pack().static_se.get(kh_audio::alarm::ALARM_STATIC_SE as usize).and_then(|p| p.as_deref()).and_then(base) == base(play.name) {
        *hud_owns_alarm = true;
        engine.alarm.request(1, false);
        engine.stop_static_se(kh_audio::alarm::ALARM_STATIC_SE);
    }
    // A hit of a spell / shot actor: the seam names the actor's class, the id says which of its
    // hit sounds (kh_core::host::audio::ACTOR_PAWN_HIT / ACTOR_MAP_HIT).
    if play.id == kh_core::host::audio::ACTOR_PAWN_HIT || play.id == kh_core::host::audio::ACTOR_MAP_HIT {
        // (ER party S5) a party spell actor's (its own handle range): only the class's hit SE (its hit effect is on the effect seam)
        if kh_core::host::party::is_party_handle(handle) {
            return engine.actor_hit_se(Some(handle), play.name, play.id == kh_core::host::audio::ACTOR_MAP_HIT, position);
        }
        return engine.actor_hit(Some(handle), play.name, play.id == kh_core::host::audio::ACTOR_MAP_HIT, position);
    }
    // (party stream E) a party member's: its voice group number (carried as a play, party::PARTY_VOICE_GROUP_NO), or a
    // sound resolved with the member's routes (PlayVoiceGroup lines by their ids PARTY_VOICE*).
    if let Attach::Party { member, .. } = play.at {
        if let Some((m, number)) = kh_core::host::party::as_voice_group_no(play) {
            engine.set_party_voice_group_no(m.0, number);
            return None;
        }
        let mode = kh_core::host::party::VoiceMode::of_id(play.id).map(|m| match m {
            kh_core::host::party::VoiceMode::Replace => kh_audio::engine::PartyVoiceMode::Replace,
            kh_core::host::party::VoiceMode::IfIdle => kh_audio::engine::PartyVoiceMode::IfIdle,
            kh_core::host::party::VoiceMode::Overlap => kh_audio::engine::PartyVoiceMode::Overlap,
        });
        // the member's animation (logical name, time): the engine's (Engine::set_party_anim, given by the host per step)
        let request = SeamPlay { name: play.name, position, volume: play.volume, pitch: play.pitch, looping: play.looping, spatial: play.spatial, anim: None };
        let key = engine.party_seam_play(member.0, handle, &request, mode)?;
        attaches.seam.insert(handle, play.at);
        return Some(key);
    }
    // (AUD) A party member's sound left at a WORLD point (a weapon swing notify without m_bFollow, a melee hit at its
    // impact: kh_core host/party.rs `dispatch`, kh-party hitfx.rs): the member is known by the handle's range, so the
    // alias ("SWING1", "HIT1" ..) resolves in HIS weapon's sets. Before, these went to Sora's routes: Donald's and
    // Goofy's swings played the Kingdom Key's se02001 (the audit's audio log, build/sora_tmp/party/aud/run1/audio.csv).
    if let (Attach::World(_), Some(m)) = (play.at, kh_core::host::party::handle_member(handle)) {
        if engine.party(m.0).is_some() {
            let request = SeamPlay { name: play.name, position, volume: play.volume, pitch: play.pitch, looping: play.looping, spatial: play.spatial, anim: None };
            let key = engine.party_seam_play(m.0, handle, &request, None)?;
            attaches.seam.insert(handle, play.at);
            return Some(key);
        }
    }
    let request = SeamPlay { name: play.name, position, volume: play.volume, pitch: play.pitch, looping: play.looping, spatial: play.spatial, anim: Some(anim) };
    let key = engine.seam_play(handle, &request)?;
    attaches.seam.insert(handle, play.at);
    Some(key)
}

/// The effect seam: the sounds an effect's particle system carries start with it (attached to
/// it) and end with it.
pub fn effect_command(engine: &mut Engine, attaches: &mut Attaches, command: &EffectCommand, place: &dyn Fn(&Attach) -> Vec3) {
    match command {
        EffectCommand::Spawn(handle, spawn) => {
            engine.effect_spawned(handle.0, spawn.name, place(&spawn.at));
            attaches.effect.insert(handle.0, spawn.at);
        }
        EffectCommand::Stop(handle, _) => {
            engine.effect_stopped(handle.0);
            attaches.effect.remove(&handle.0);
        }
        EffectCommand::Tint(..) => {} // (DON-MAGIC tint) a colour: nothing to hear
        // (ZETTA) a moved world effect's sounds follow it
        EffectCommand::Transform(handle, at, _, _) => {
            if let Some(a @ kh_core::Attach::World(_)) = attaches.effect.get_mut(&handle.0) {
                *a = kh_core::Attach::World(*at);
            }
        }
        // (DON-MAGIC orbit) a moved effect's sounds follow it (a world one; an attached one follows its owner already)
        EffectCommand::Place(handle, at) => {
            if let Some(a @ kh_core::Attach::World(_)) = attaches.effect.get_mut(&handle.0) {
                *a = kh_core::Attach::World(*at);
            }
        }
    }
}

/// The sound states the core announces (the shotlock's slow: STATE_SLOW_START / _END). Returns
/// the names that are not events of the pack.
pub fn sound_states(engine: &mut Engine, events: &[kh_core::Event]) -> Vec<&'static str> {
    let mut unknown = Vec::new();
    for event in events {
        if let kh_core::EventKind::SoundState { name } = event.kind {
            if !engine.event(name) {
                unknown.push(name);
            }
        }
    }
    unknown
}

/// What one engine step needs from its host.
pub struct Tick<'a> {
    pub dt: f32,
    /// The simulation stands (a menu holds the pause): the engine runs on the frame clock.
    pub standing: bool,
    /// The bones of the simulation step this engine step belongs to (none while standing).
    pub sample: Option<&'a BoneSample>,
    /// The surface (`SurfaceType`) under a point of the world: the host's own floor probe.
    pub surface: &'a dyn Fn(Vec3) -> u32,
    pub alarm: Alarm,
    pub listener: &'a Listener,
    pub tap: &'a PoseTap,
    /// Where an attach is now.
    pub place: &'a dyn Fn(&Attach) -> Vec3,
}

/// One engine step: the automatic motion sounds of the step, the alarm, the anchors, the
/// driver. Returns the voice commands for the mixer and the motion sounds detected (with the
/// surface they were looked up on).
pub fn tick(engine: &mut Engine, attaches: &Attaches, t: &Tick) -> (Vec<VoiceCommand>, Vec<(AutoSeEvent, u32)>) {
    let mut detected = Vec::new();
    if let Some(sample) = t.sample.filter(|_| !t.standing) {
        let frame = BoneFrame::from_positions(&sample.positions, sample.grounded);
        let surface = (t.surface)(sample.positions[0]);
        let tap = t.tap;
        for event in engine.auto_se(t.dt, &frame, surface, Some((sample.anim, sample.anim_time)), t.listener, &|bone| tap.bone(bone)) {
            detected.push((event, surface));
        }
    }
    // The low-HP alarm (pausing stops and re-arms it: spec 5.6).
    match t.alarm {
        Alarm::Hud => engine.alarm_step(t.dt, t.standing),
        Alarm::Hp { low } => engine.alarm_tick(t.dt, low, t.standing),
    }
    let (tap, place) = (t.tap, t.place);
    let commands = engine.update(t.dt, t.listener, &|anchor| match anchor {
        Anchor::Seam(handle) => attaches.seam.get(handle).map(place),
        Anchor::Bone(bone) => tap.bone(bone),
        Anchor::Effect(handle) => attaches.effect.get(handle).map(place),
        // (party stream E) an auto-SE part of a party member
        Anchor::PartyBone(member, bone) => Some(place(&Attach::Party { member: kh_core::PartyMember(*member), bone: Some(bone), offset: Vec3::ZERO })),
    });
    (commands, detected)
}

fn csv(text: &str) -> String {
    if text.contains(',') || text.contains('"') {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_string()
    }
}

/// The header and one line of a host's audio log (`--audio-log` of the test app).
pub const AUDIO_LOG_HEADER: &str = "step,time,kind,cause,sound,category,file,volume,pitch,pan,distance,note";

pub fn log_line(step: u64, time: f64, line: &LogLine) -> String {
    let (volume, pitch, pan) = line.params.map_or((String::new(), String::new(), String::new()), |p| (format!("{:.4}", p.volume), format!("{:.4}", p.pitch), format!("{:.4}", p.pan)));
    format!("{step},{time:.4},{},{},{},{},{},{volume},{pitch},{pan},{},{}", line.kind, csv(&line.cause), csv(&line.sound), line.category, csv(&line.file), line.distance.map_or(String::new(), |d| format!("{d:.1}")), csv(&line.note))
}
