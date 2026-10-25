//! (party stream E) THE PARTY SIDE OF THE AUDIO / EFFECT SEAM: how Donald and Goofy (and the
//! team-attack link pawns) are heard and seen through the same `Audio` / `Effects` traits as Sora.
//! Contract: docs/party-contracts.md, section "Stream E". Engine-free, no gameplay rule: WHEN a
//! voice plays is the business of the pawn (kh-party, stream B) and of the team attacks (stream
//! D); this module says HOW it is put on the seam so that every host resolves it the same way.
//!
//! - WHO: [`PartyMember`] (0 Donald, 1 Goofy: the `FriendRelationDataTable` index = HUD slot - 1).
//!   A sound / effect on a member is `Attach::Party { member, bone, offset }`; the host resolves
//!   bones / sockets on THAT member's skeleton, and resolves the seam NAME with that member's
//!   routes (voice groups `n_ex00x_VoiceGroupSet`, weapon sound sets, `FsTable_Donald/Goofy`).
//! - HANDLES: a member allocates its seam handles from its own range ([`SeamHandles`]), so that
//!   they never collide with Sora's (which count up from 1) inside a host's tables.
//! - NOTIFIES: [`PartyNotify::dispatch`] is the generic notify handler of a party clip (the same
//!   rules as kh-core notify.rs for Sora, with `Attach::Party`).
//! - VOICE GROUPS: the NPC's `PlayVoiceGroup` (FUN_141275aa0 [C]) is a play of the seam name
//!   `"<group>:<number>"` ([`voice_play`]) with the id [`PARTY_VOICE`] / [`PARTY_VOICE_IF_IDLE`] /
//!   [`PARTY_VOICE_OVERLAP`]; the pawn's own voice group number (anim interface +0x1F0, read by
//!   the voice-GROUP notifies of its clips) goes through `Audio::party_voice_group_no`.

use glam::Vec3;

use crate::anim::{NotifyData, NotifyPhase};
use crate::host::{Attach, Audio, EffectHandle, EffectId, EffectSpawn, Effects, SoundHandle, SoundId, SoundPlay};
use crate::notify::{effect_group_passes, notify_asset_id};

/// A party member as the seam names it: the index of `FriendRelationDataTable` (row 0 Donald,
/// row 1 Goofy [A]); HUD status slot = index + 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PartyMember(pub u8);

impl PartyMember {
    pub const DONALD: PartyMember = PartyMember(0);
    pub const GOOFY: PartyMember = PartyMember(1);
    /// The members this port has.
    pub const ALL: [PartyMember; 2] = [PartyMember::DONALD, PartyMember::GOOFY];

    /// `ETresChrUniqueID`: Donald 11, Goofy 12 [A].
    pub const fn uid(self) -> u8 {
        match self.0 {
            0 => 11,
            1 => 12,
            _ => 0,
        }
    }

    /// The member of a unique id.
    pub const fn from_uid(uid: u8) -> Option<PartyMember> {
        match uid {
            11 => Some(PartyMember::DONALD),
            12 => Some(PartyMember::GOOFY),
            _ => None,
        }
    }

    /// The model / pack folder (`Character/npc/n_ex001`): also the `owner` of the member's rows
    /// in the audio and effect packs.
    pub const fn chara(self) -> &'static str {
        match self.0 {
            0 => "n_ex001",
            1 => "n_ex002",
            _ => "",
        }
    }

    pub const fn name(self) -> &'static str {
        match self.0 {
            0 => "Donald",
            1 => "Goofy",
            _ => "?",
        }
    }
}

/// First seam handle of the party ranges. Sora's core counts from 1 and never gets near it.
pub const PARTY_HANDLE_BASE: u32 = 0x2000_0000;
/// Handles per member range.
pub const PARTY_HANDLE_SPAN: u32 = 0x0100_0000;
/// The range of the team-attack link pawns and their projectiles (stream D): the last range.
pub const TEAM_HANDLE_BASE: u32 = PARTY_HANDLE_BASE + 15 * PARTY_HANDLE_SPAN;

/// Whether a seam handle belongs to the party / team ranges (a host's "a new core counts from 1
/// again" check must only look at handles below [`PARTY_HANDLE_BASE`]).
pub const fn is_party_handle(handle: u32) -> bool {
    handle >= PARTY_HANDLE_BASE && handle < PARTY_HANDLE_BASE + 16 * PARTY_HANDLE_SPAN
}

/// The member a handle was allocated for (`None`: Sora's, or the team range).
pub const fn handle_member(handle: u32) -> Option<PartyMember> {
    if handle >= PARTY_HANDLE_BASE && handle < TEAM_HANDLE_BASE {
        Some(PartyMember(((handle - PARTY_HANDLE_BASE) / PARTY_HANDLE_SPAN) as u8))
    } else {
        None
    }
}

/// A seam-handle allocator for one member (or the team pawns). Sound and effect handles share
/// the counter (they are different namespaces, but one counter keeps a log readable).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeamHandles {
    next: u32,
    end: u32,
}

impl SeamHandles {
    pub const fn for_member(member: PartyMember) -> SeamHandles {
        let base = PARTY_HANDLE_BASE + member.0 as u32 * PARTY_HANDLE_SPAN;
        SeamHandles { next: base + 1, end: base + PARTY_HANDLE_SPAN }
    }

    pub const fn team() -> SeamHandles {
        SeamHandles { next: TEAM_HANDLE_BASE + 1, end: TEAM_HANDLE_BASE + PARTY_HANDLE_SPAN }
    }

    fn take(&mut self) -> u32 {
        let h = self.next;
        self.next = if self.next + 1 >= self.end { self.end - PARTY_HANDLE_SPAN + 1 } else { self.next + 1 };
        h
    }

    pub fn sound(&mut self) -> SoundHandle {
        SoundHandle(self.take())
    }

    pub fn effect(&mut self) -> EffectHandle {
        EffectHandle(self.take())
    }

    /// Plays `play` with a fresh handle.
    pub fn play(&mut self, audio: &mut dyn Audio, play: &SoundPlay) -> SoundHandle {
        let handle = self.sound();
        audio.play(handle, play);
        handle
    }

    /// Spawns `spawn` with a fresh handle.
    pub fn spawn(&mut self, effects: &mut dyn Effects, spawn: &EffectSpawn) -> EffectHandle {
        let handle = self.effect();
        effects.spawn(handle, spawn);
        handle
    }
}

impl Attach {
    /// On a party member: at a bone or socket of their skeleton (`None` = the actor origin).
    pub const fn party(member: PartyMember, bone: Option<&'static str>) -> Attach {
        Attach::Party { member, bone, offset: Vec3::ZERO }
    }
}

// ------------------------------------------------------------------------------------- voices

/// `PlayVoiceGroup` (FUN_141275aa0 [C]): stop the member's current voice line, then play.
/// (param_4 0, param_5 0: QuarterHP 0x1412795F0, RecoveryHP 0x141276C10, FriendLink 0x141275470.)
pub const PARTY_VOICE: SoundId = SoundId(0xAC70_0010);
/// As [`PARTY_VOICE`], but nothing plays while the member's current line is still playing
/// (param_4 1: BattleStart 0x141275210).
pub const PARTY_VOICE_IF_IDLE: SoundId = SoundId(0xAC70_0011);
/// As [`PARTY_VOICE`], but the current line is not stopped (param_5 1).
pub const PARTY_VOICE_OVERLAP: SoundId = SoundId(0xAC70_0012);

/// How a host queue that has no command of its own for it carries `Audio::party_voice_group_no`:
/// a `play` with handle 0 (never a real one), this id, `at` = `Attach::party(member, None)` and
/// the number in `volume` (kh-host `AudioQueue` does so; a host that does not know the id plays
/// nothing for it).
pub const PARTY_VOICE_GROUP_NO: SoundId = SoundId(0xAC70_0013);

/// The `party_voice_group_no` carried as a play (see [`PARTY_VOICE_GROUP_NO`]).
pub fn voice_group_no_play(member: PartyMember, number: i32) -> SoundPlay {
    SoundPlay { id: PARTY_VOICE_GROUP_NO, name: "", at: Attach::party(member, None), volume: number as f32, pitch: 1.0, looping: false, spatial: false }
}

/// The member and number of a play made by [`voice_group_no_play`].
pub fn as_voice_group_no(play: &SoundPlay) -> Option<(PartyMember, i32)> {
    match (play.id, play.at) {
        (PARTY_VOICE_GROUP_NO, Attach::Party { member, .. }) => Some((member, play.volume as i32)),
        _ => None,
    }
}

/// How a voice line meets the member's current one (FUN_141275aa0 param_4 / param_5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoiceMode {
    /// Stop the current line, play this one.
    Replace,
    /// Play only when no line of the member is playing.
    IfIdle,
    /// Play over the current line.
    Overlap,
}

impl VoiceMode {
    pub const fn id(self) -> SoundId {
        match self {
            VoiceMode::Replace => PARTY_VOICE,
            VoiceMode::IfIdle => PARTY_VOICE_IF_IDLE,
            VoiceMode::Overlap => PARTY_VOICE_OVERLAP,
        }
    }

    pub const fn of_id(id: SoundId) -> Option<VoiceMode> {
        match id.0 {
            0xAC70_0010 => Some(VoiceMode::Replace),
            0xAC70_0011 => Some(VoiceMode::IfIdle),
            0xAC70_0012 => Some(VoiceMode::Overlap),
            _ => None,
        }
    }
}

/// The voice groups of `n_ex001_VoiceGroupSet` / `n_ex002_VoiceGroupSet` [A].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VoiceGroup {
    QuarterHp,
    RecoveryHp,
    FriendLink,
    BattleStart,
    /// Donald only.
    MagicCast,
    GiveRecovery,
    /// `donald_damage_1` / `goofy_damage_1` (one unit, number 0).
    Damage,
}

impl VoiceGroup {
    /// The group's name in the member's voice group set.
    pub const fn name(self, member: PartyMember) -> &'static str {
        match self {
            VoiceGroup::QuarterHp => "QuarterHP",
            VoiceGroup::RecoveryHp => "RecoveryHP",
            VoiceGroup::FriendLink => "FriendLink",
            VoiceGroup::BattleStart => "BattleStart",
            VoiceGroup::MagicCast => "MagicCast",
            VoiceGroup::GiveRecovery => "GiveRecovery",
            VoiceGroup::Damage => match member.0 {
                0 => "donald_damage_1",
                _ => "goofy_damage_1",
            },
        }
    }
}

/// The seam name of one unit: `"<group>:<number>"` (the host resolves it in the member's set;
/// a unit the set does not have plays nothing: `m_bPlayOnGroupNotFound` is false on all [A]).
/// `None`: no such unit in the member's set (data of build/sora_tmp/party/inventory.json).
pub fn voice_name(member: PartyMember, group: VoiceGroup, number: i32) -> Option<&'static str> {
    use VoiceGroup::*;
    Some(match (member.0, group, number) {
        (_, QuarterHp, 1) => "QuarterHP:1",
        (_, QuarterHp, 2) => "QuarterHP:2",
        (_, RecoveryHp, 1) => "RecoveryHP:1",
        (_, RecoveryHp, 2) => "RecoveryHP:2",
        (_, FriendLink, 1) => "FriendLink:1",
        (_, FriendLink, 2) => "FriendLink:2",
        (_, FriendLink, 3) => "FriendLink:3",
        (_, FriendLink, 4) => "FriendLink:4",
        (_, BattleStart, 1) => "BattleStart:1",
        (_, BattleStart, 2) => "BattleStart:2",
        (_, GiveRecovery, 1) => "GiveRecovery:1",
        (_, GiveRecovery, 2) => "GiveRecovery:2",
        (0, Damage, 0) => "donald_damage_1:0",
        (1, Damage, 0) => "goofy_damage_1:0",
        (0, MagicCast, n) => match n {
            1 => "MagicCast:1",
            2 => "MagicCast:2",
            3 => "MagicCast:3",
            4 => "MagicCast:4",
            5 => "MagicCast:5",
            6 => "MagicCast:6",
            7 => "MagicCast:7",
            8 => "MagicCast:8",
            9 => "MagicCast:9",
            10 => "MagicCast:10",
            11 => "MagicCast:11",
            12 => "MagicCast:12",
            13 => "MagicCast:13",
            101 => "MagicCast:101",
            102 => "MagicCast:102",
            103 => "MagicCast:103",
            _ => return None,
        },
        _ => return None,
    })
}

/// A voice line of a member's group: attached at `atama` (the default socket of
/// FUN_141275aa0 [C]), volume 1, pitch 1 (its two float arguments 0x3F800000 at every call site
/// read [C]). Play it through the member's [`SeamHandles`].
pub fn voice_play(member: PartyMember, group: VoiceGroup, number: i32, mode: VoiceMode) -> Option<SoundPlay> {
    let name = voice_name(member, group, number)?;
    Some(SoundPlay { id: mode.id(), name, at: Attach::party(member, Some("atama")), volume: 1.0, pitch: 1.0, looping: false, spatial: true })
}

/// The number of variations the pawn's count virtuals answer for a group: slots 1318
/// (QuarterHP), 1319 (RecoveryHP), 1321 (FriendLink), 1323 (BattleStart) are `mov eax, 2; ret`
/// (0x14041F1B0) in both vtables (Donald 0x14676ACF0, Goofy 0x14676E560) [I].
pub const VOICE_VARIATIONS: i32 = 2;

/// The game's pick of a unit number from `count` variations: `n = (int)((r >> 8) * 2^-24 *
/// count)`, at most `count - 1`, plus 1 (QuarterHP 0x1412795F0, RecoveryHP 0x141276C10,
/// BattleStart 0x141275210 [C]). `r` is the next value of the xorshift128 the game draws from
/// (FUN_142CFF460); the port's pawn supplies its own deterministic stream (plan N6).
pub fn pick_number(count: i32, r: u32) -> i32 {
    if count < 1 {
        return 1;
    }
    let n = ((r >> 8) as f32 * 5.960_464_5e-8 * count as f32) as i32;
    n.min(count - 1) + 1
}

/// Donald's "magic voice number" (vtable slot 1325, 0x1411C8E00 [C]): 101 + min((r >> 8) *
/// 1.7881393e-07, 2) = 101 .. 103. NOT a line to play: the cast state (0x14122D300 [C]) gives it to
/// `Audio::party_voice_group_no` before the MagicCast clip, whose "MagicCast" GROUP notify then plays
/// unit 101..103 [A: clip 1341]. (The MagicFire clip 1321 carries its own random-voice notify of the
/// same three lines.) Goofy: the base class 0x1411AE7A0, a constant 101.
pub fn magic_voice_number(member: PartyMember, r: u32) -> i32 {
    match member.0 {
        0 => 101 + (((r >> 8) as f32 * 1.788_139_3e-7) as i32).min(2),
        _ => 101,
    }
}


/// The member's "voice number for a target" (vtable slot 1326): Donald 0x1411C8ED0 [C]: with `random` set,
/// 1 + min(u * 2, 1) = 1..2; else by the target's unique id: Sora (1) 1, Goofy (12) 2, 13 -> 7, 14 -> 5, 15 -> 6,
/// 16 -> 3, 17 -> 8, 18 -> 9 (10 once the game flag gameflow_RA / RA_END is set: not in the test level), 19 -> 11,
/// 20 -> 12, 22 -> 4, 23 -> 13, any other 0. Goofy: the base 0x1412641C0 [C]: 1..2 at random for any character
/// target, 0 without one. The caller 0x14122D300 [C] (a cast state) sets it as the anim interface's voice group number
/// (+0x1F0) for a Cure / Cura / Curaga (kinds 0x31..0x33) on ANOTHER character; for a cast of attack magic it uses
/// slot 1325 ([`magic_voice_number`]); on himself nothing (0). The cast clip's "MagicCast" group notify (Donald's
/// clip 1341) then plays that unit: units 1..13 are his cure lines per target, 101..103 his cast lines [A]. Which
/// argument fills `random` at that call site is [?] (not set by the decompiled call): `false` is the reading used.
pub fn target_voice_number(member: PartyMember, target_uid: u8, random: bool, r: u32) -> i32 {
    let coin = || 1 + (((r >> 8) as f32 * 1.192_092_9e-7) as i32).min(1);
    match member.0 {
        0 => {
            if random {
                return coin();
            }
            match target_uid {
                1 => 1,
                12 => 2,
                13 => 7,
                14 => 5,
                15 => 6,
                16 => 3,
                17 => 8,
                18 => 9,
                19 => 11,
                20 => 12,
                22 => 4,
                23 => 13,
                _ => 0,
            }
        }
        _ => {
            if target_uid == 0 {
                0
            } else {
                coin()
            }
        }
    }
}

// ------------------------------------------------------------------------------------- notifies

#[derive(Clone, Copy, Debug, PartialEq)]
enum Started {
    Sound(SoundHandle, f32),
    Effect(EffectHandle),
}

/// What a sound / voice / effect notify of a party clip carries, in the seam's terms. The party anim tables (stream A,
/// `party_anim_types.rs`) have the same fields as `kh_core::anim::NotifyData`; the pawn (stream B) maps a notify to
/// this with one match (an AttachEffect notify gives one `Effect` per entry), or uses [`SeamNotify::of_core`] for a
/// `kh_core::anim::NotifyData`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SeamNotify {
    /// `SQEXSEADAnimNotify_PlaySound` / `SQEXSEADAnimNotifyState_AttachSound`.
    Sound { sound: &'static str, attach_point: Option<&'static str>, volume: f32, pitch: f32, fade_out: f32, loop_end_sound: Option<&'static str>, loop_end_pitch: f32 },
    /// `TresAnimNotify(State)_PlayWeaponSwingSE` / `_PlayWeaponHitSE`: an alias of the equipped weapon's set.
    WeaponSe { alias: &'static str, socket: Option<&'static str>, follow: bool, volume: f32, pitch: f32, fade_out: f32 },
    /// `SQEXSEADAnimNotify(State)_PlayAutoSe`: the motion name (Walk, LandNormal ...) or the source type
    /// ("AuxSurfaceTable"); the member's footstep table and the surface decide.
    AutoSe { name: &'static str, fade_out: f32 },
    /// `TresAnimNotify(State)_PlayVoice`: the voice asset, the group name (the member's voice group number decides),
    /// or the FIRST entry of a weighted random list (the host draws, as for Sora).
    Voice { name: &'static str, fade_out: f32 },
    /// `TresAnimNotify_PlayActorVoice.m_ActorVoice` (a `TresActorVoice` asset path, from the notify's props): the
    /// field chat line of the talk clips.
    ActorVoice { asset: &'static str },
    /// One row of an AttachEffect notify: the particle system, its socket, its GroupID.
    Effect { effect: &'static str, socket: &'static str, group_id: i32 },
}

impl SeamNotify {
    /// The seam part of a `kh_core::anim::NotifyData` (Sora's table type, the same fields as the party tables').
    pub fn of_core(data: &NotifyData) -> Vec<SeamNotify> {
        match *data {
            NotifyData::Sound { sound, attach_point, volume, pitch, fade_out, loop_end_sound, loop_end_pitch, .. } => vec![SeamNotify::Sound { sound, attach_point, volume, pitch, fade_out, loop_end_sound, loop_end_pitch }],
            NotifyData::WeaponSe { alias, socket, follow, volume, pitch, fade_out, .. } => vec![SeamNotify::WeaponSe { alias, socket, follow, volume, pitch, fade_out }],
            NotifyData::AutoSe { motion, source, fade_out } => vec![SeamNotify::AutoSe { name: motion.or(source).unwrap_or(""), fade_out }],
            NotifyData::Voice { group, asset, random, fade_out } => vec![SeamNotify::Voice { name: asset.or(group).or(random.first().map(|r| r.0)).unwrap_or(""), fade_out }],
            NotifyData::Effect { entries } => entries.iter().filter_map(|e| Some(SeamNotify::Effect { effect: e.effect?, socket: e.socket, group_id: e.group_id })).collect(),
            _ => Vec::new(),
        }
    }
}

/// One notify event of a pose tick of a party member.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PartyNotifyEvent {
    /// Any key that names the sequence uniquely for this member (its index in the member's sequence table).
    pub sequence: u32,
    /// The notify's index in that sequence.
    pub index: u16,
    /// A notify STATE (Begin ... End) rather than an instant notify.
    pub state: bool,
    pub phase: NotifyPhase,
    pub data: SeamNotify,
}

/// The generic notify handler of a party member's clips (the rules of kh-core notify.rs, which serve Sora:
/// docs/sora-audio-spec.md 5.1, docs/sora-vfx-spec.md 4): sound, weapon sound, voice, automatic motion sound, actor
/// voice and effect notifies become seam requests on `Attach::Party`; a notify STATE starts a looping sound /
/// persistent effect and stops it at its End. Keep one per member.
#[derive(Clone, Debug, Default)]
pub struct PartyNotify {
    /// What a notify state started: (sequence key, notify index, what).
    started: Vec<(u32, u16, Started)>,
}

impl PartyNotify {
    pub fn new() -> PartyNotify {
        PartyNotify::default()
    }

    /// How many things the open notify states started.
    pub fn open(&self) -> usize {
        self.started.len()
    }

    /// Stops everything the open notify states started (a member removed or warped out).
    pub fn stop_all(&mut self, audio: &mut dyn Audio, effects: &mut dyn Effects) {
        for (_, _, s) in self.started.drain(..) {
            match s {
                Started::Sound(handle, fade) => audio.stop(handle, fade),
                Started::Effect(handle) => effects.stop(handle, false),
            }
        }
    }

    /// The events of one pose tick, in dispatch order. `location`: the pawn's actor location now (cm, core axes; a
    /// weapon sound that does not follow is left there). `effect_group`: the current effect group of the member's play
    /// (0 unless the state gave one: `notify::effect_group_passes`).
    #[allow(clippy::too_many_arguments)]
    pub fn dispatch(&mut self, member: PartyMember, ids: &mut SeamHandles, events: &[PartyNotifyEvent], location: Vec3, effect_group: i32, audio: &mut dyn Audio, effects: &mut dyn Effects) {
        let at = |socket: Option<&'static str>| Attach::Party { member, bone: socket.filter(|s| !s.is_empty() && *s != "None"), offset: Vec3::ZERO };
        for event in events {
            let key = (event.sequence, event.index);
            match event.phase {
                NotifyPhase::Tick => {}
                NotifyPhase::End => {
                    let mut k = 0;
                    while k < self.started.len() {
                        if (self.started[k].0, self.started[k].1) != key {
                            k += 1;
                            continue;
                        }
                        match self.started.remove(k).2 {
                            Started::Sound(handle, fade_out) => {
                                audio.stop(handle, fade_out);
                                // SQEXSEADAnimNotifyState_AttachSound End (0x1458162E0): LoopEndSound.
                                if let SeamNotify::Sound { loop_end_sound: Some(end), attach_point, volume, loop_end_pitch, .. } = event.data {
                                    ids.play(audio, &SoundPlay { id: SoundId(notify_asset_id(end)), name: end, at: at(attach_point), volume, pitch: loop_end_pitch, looping: false, spatial: true });
                                }
                            }
                            Started::Effect(handle) => effects.stop(handle, false),
                        }
                    }
                }
                NotifyPhase::Instant | NotifyPhase::Begin => {
                    let state = event.state;
                    let mut sound = |this: &mut PartyNotify, name: &'static str, attach: Attach, volume: f32, pitch: f32, fade_out: f32| {
                        if name.is_empty() {
                            return;
                        }
                        let handle = ids.play(audio, &SoundPlay { id: SoundId(notify_asset_id(name)), name, at: attach, volume, pitch, looping: state, spatial: true });
                        if state {
                            this.started.push((key.0, key.1, Started::Sound(handle, fade_out)));
                        }
                    };
                    match event.data {
                        SeamNotify::Sound { sound: name, attach_point, volume, pitch, fade_out, .. } => sound(self, name, at(attach_point), volume, pitch, fade_out),
                        // 0x1405615F0: attached at m_SocketName when m_bFollow, else at the mesh location of this
                        // moment. The alias resolves in the MEMBER'S equipped weapon's set (host side).
                        SeamNotify::WeaponSe { alias, socket, follow, volume, pitch, fade_out } => {
                            let attach = if follow { at(socket) } else { Attach::World(location) };
                            sound(self, alias, attach, volume, pitch, fade_out)
                        }
                        SeamNotify::AutoSe { name, fade_out } => sound(self, name, at(None), 1.0, 1.0, fade_out),
                        SeamNotify::Voice { name, fade_out } => sound(self, name, at(Some("atama")), 1.0, 1.0, fade_out),
                        // TresAnimNotify_PlayActorVoice 0x1405604A0: the actor voice's sound at the head [inf: socket].
                        SeamNotify::ActorVoice { asset } => sound(self, asset, at(Some("atama")), 1.0, 1.0, 0.0),
                        SeamNotify::Effect { effect, socket, group_id } => {
                            if !effect_group_passes(group_id, true, effect_group) {
                                continue;
                            }
                            let handle = ids.spawn(effects, &EffectSpawn { id: EffectId(notify_asset_id(effect)), name: effect, at: at(Some(socket)), rotation: Vec3::ZERO, scale: 1.0, persistent: state });
                            if state {
                                self.started.push((key.0, key.1, Started::Effect(handle)));
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn member_handles_never_meet_soras_or_each_other() {
        let mut d = SeamHandles::for_member(PartyMember::DONALD);
        let mut g = SeamHandles::for_member(PartyMember::GOOFY);
        let (a, b) = (d.sound().0, g.effect().0);
        assert!(a > PARTY_HANDLE_BASE && b > PARTY_HANDLE_BASE + PARTY_HANDLE_SPAN);
        assert_eq!(handle_member(a), Some(PartyMember::DONALD));
        assert_eq!(handle_member(b), Some(PartyMember::GOOFY));
        assert!(is_party_handle(SeamHandles::team().sound().0) && handle_member(SeamHandles::team().sound().0).is_none());
        assert!(!is_party_handle(1) && !is_party_handle(PARTY_HANDLE_BASE - 1));
    }

    #[test]
    fn the_unit_pick_is_the_games() {
        // r >> 8 in [0, 2^24): the low half picks 1, the high half 2.
        assert_eq!(pick_number(2, 0), 1);
        assert_eq!(pick_number(2, 0x7FFF_FF00), 1);
        assert_eq!(pick_number(2, 0x8000_0000), 2);
        assert_eq!(pick_number(2, u32::MAX), 2);
        assert_eq!(magic_voice_number(PartyMember::DONALD, 0), 101);
        assert_eq!(magic_voice_number(PartyMember::DONALD, u32::MAX), 103);
        assert_eq!(magic_voice_number(PartyMember::GOOFY, u32::MAX), 101);
    }

    #[derive(Default)]
    struct Rec(Vec<String>);
    impl Audio for Rec {
        fn play(&mut self, h: SoundHandle, p: &SoundPlay) {
            self.0.push(format!("play {} {} {:?} loop={}", h.0, p.name, p.at, p.looping));
        }
        fn stop(&mut self, h: SoundHandle, f: f32) {
            self.0.push(format!("stop {} {f}", h.0));
        }
    }
    impl Effects for Rec {
        fn spawn(&mut self, h: EffectHandle, s: &EffectSpawn) {
            self.0.push(format!("spawn {} {} persistent={}", h.0, s.name, s.persistent));
        }
        fn stop(&mut self, h: EffectHandle, _: bool) {
            self.0.push(format!("estop {}", h.0));
        }
    }

    #[test]
    fn a_party_clips_notifies_go_on_the_seam_on_that_member() {
        let (mut audio, mut effects, mut n) = (Rec::default(), Rec::default(), PartyNotify::new());
        let mut ids = SeamHandles::for_member(PartyMember::GOOFY);
        let ev = |index, state, phase, data| PartyNotifyEvent { sequence: 7, index, state, phase, data };
        let loop_sound = SeamNotify::Sound { sound: "/Game/Sound/SE/NPC/00302_n_ex002/se00302_010", attach_point: None, volume: 1.0, pitch: 1.0, fade_out: 0.3, loop_end_sound: Some("/Game/Sound/SE/NPC/00302_n_ex002/se00302_011"), loop_end_pitch: 1.0 };
        let begin = [
            ev(0, true, NotifyPhase::Begin, loop_sound),
            ev(1, false, NotifyPhase::Instant, SeamNotify::WeaponSe { alias: "SWING1", socket: Some("R_wep"), follow: false, volume: 1.0, pitch: 1.0, fade_out: 0.0 }),
            ev(2, true, NotifyPhase::Begin, SeamNotify::Effect { effect: "vn_ex002_trn_loc00", socket: "kh_root", group_id: -1 }),
            ev(3, true, NotifyPhase::Begin, SeamNotify::Effect { effect: "vn_ex002_trn_awa00", socket: "kh_root", group_id: 1088 }),
        ];
        n.dispatch(PartyMember::GOOFY, &mut ids, &begin, Vec3::new(5.0, 6.0, 0.0), 0, &mut audio, &mut effects);
        assert_eq!(n.open(), 2);
        assert!(audio.0[0].contains("se00302_010 Party { member: PartyMember(1), bone: None") && audio.0[0].ends_with("loop=true"), "{:?}", audio.0);
        // a weapon sound that does not follow stays where the mesh was
        assert!(audio.0[1].contains("SWING1 World(Vec3(5.0, 6.0, 0.0))"), "{:?}", audio.0);
        // the effect-group rule: the under-water row (group 1088) does not spawn in group 0
        assert_eq!(effects.0, vec![format!("spawn {} vn_ex002_trn_loc00 persistent=true", PARTY_HANDLE_BASE + PARTY_HANDLE_SPAN + 3)]);
        let end = [ev(0, true, NotifyPhase::End, loop_sound), ev(2, true, NotifyPhase::End, SeamNotify::Effect { effect: "vn_ex002_trn_loc00", socket: "kh_root", group_id: -1 })];
        n.dispatch(PartyMember::GOOFY, &mut ids, &end, Vec3::ZERO, 0, &mut audio, &mut effects);
        assert_eq!(n.open(), 0);
        assert!(audio.0[2].starts_with(&format!("stop {} 0.3", PARTY_HANDLE_BASE + PARTY_HANDLE_SPAN + 1)) && audio.0[3].contains("se00302_011"), "{:?}", audio.0);
        assert!(effects.0[1].starts_with("estop"));
    }

    #[test]
    fn voice_names_name_existing_units_only() {
        assert_eq!(voice_name(PartyMember::DONALD, VoiceGroup::MagicCast, 102), Some("MagicCast:102"));
        assert_eq!(voice_name(PartyMember::GOOFY, VoiceGroup::MagicCast, 1), None);
        assert_eq!(voice_name(PartyMember::GOOFY, VoiceGroup::Damage, 0), Some("goofy_damage_1:0"));
        let p = voice_play(PartyMember::GOOFY, VoiceGroup::QuarterHp, 2, VoiceMode::Replace).unwrap();
        assert_eq!((p.id, p.at), (PARTY_VOICE, Attach::Party { member: PartyMember::GOOFY, bone: Some("atama"), offset: Vec3::ZERO }));
        assert_eq!(VoiceMode::of_id(PARTY_VOICE_IF_IDLE), Some(VoiceMode::IfIdle));
    }
}
