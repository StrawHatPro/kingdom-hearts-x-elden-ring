//! (MIMIC-FX, THE USER 2026-11-06: "sounds made by the copy MUST be played from it's position and not the player's" and "all of
//! the mimic tears (this goes for bosses and summons) need to have their VFX visible and working") THE COPIES' OWN EFFECTS
//! AND SOUNDS, ONE SHARED ROUTE for every copy of a character: the Mimic Tear boss's Sora copy, its summoned Mimic, the
//! player's Mimic Sora, a boss Donald copy, a boss Goofy copy (declared in fx.rs by `#[path]`, behind the feature `party`).
//!
//! Every copy is a member of its OWN id (`kh_audio::copies`: 8 Donald's copy, 9 Goofy's, 10 the boss's Sora copy, 11 its
//! Mimic, 12 Mimic Sora), whatever character it copies, also when it copies the leader himself:
//! - ITS HANDLES lie in its own party range ([`own_handle`]): the effect side and the audio side know it by them;
//! - ITS ATTACHES are its own: a Sora copy's `Attach::Sora` (on HIM) is his to the effect side (kh-host vfx/instances.rs:
//!   `EffectHost::step_instance` with HIS scene: his output, his core, his bones, his Keyblade, so every Sora rule of the
//!   effect side applies to him as to the player) and `Attach::Party { member: id }` to the audio side; a friend copy's
//!   `Attach::Party { member: its own }` becomes `Attach::Party { member: id }` (its runtime's ids are the real party's);
//! - ITS TAPS (its bones and sockets, grounded, anim, weapon) join the party's in fx.rs `set_party`
//!   ([`taps`]): its sounds are placed and followed on ITS bones, its footsteps come from ITS feet (kh-audio's member
//!   detector), its effects of the party's route sit on ITS bones (`EffectHost::set_party`), its spell actors' bodies follow
//!   them ([`projectiles`]);
//! - its victims (a boss copy's: the leader) are the real ones: the player Sora (`Attach::Sora` of the player's route) or a
//!   friend (his real member id), never the copy.
//!
//! What the player-side rules do for an ENEMY copy (the coordinator's list, decided here):
//! the hero light is the leader's (HEROLIGHT-KB), never a copy's; the world slow is the player's finishers' (a copy is slowed
//! by it as every enemy, `wdt * enemy_type`, and never slows the world); the dark screen (SQEX_ChangeSceneColor) and the camera
//! shakes (SQEX_CameraShake, by the eye's distance) are modules of the effect itself with no owner test in KH3: a copy's spell
//! darkens / shakes as the player's does; Zettaflare stays non-cinematic for a copy (MIMIC-BOSS-KIT).

use std::collections::HashMap;
use std::sync::Mutex;

use glam::{Quat, Vec3};
use kh_core::host::party::{PartyMember, PARTY_HANDLE_BASE, PARTY_HANDLE_SPAN, TEAM_HANDLE_BASE};
use kh_core::{Attach, EffectHandle, EffectSpawn, Output, Sora, SoundHandle, SoundPlay, TargetId};
use kh_host::audio::PoseTap;
use kh_host::seam::{EffectCommand, EffectQueue};
use kh_host::vfx::Scene;

use crate::erlog;

/// The team range's handles of a copy's runtime sit above this in its own range (its member's below).
const TEAM_BIT: u32 = 0x0080_0000;
const LOW: u32 = TEAM_BIT - 1;

/// A copy's own handle for one of its core's / runtime's: in member `member`'s party range. A party handle keeps its low bits
/// (a friend copy's runtime counts in its member's range, its team pawns in the team range: above [`TEAM_BIT`]); a Sora
/// core's (counted from 1) keeps its own number.
pub fn own_handle(member: u8, raw: u32) -> u32 {
    let low = if raw >= TEAM_HANDLE_BASE && kh_core::host::party::is_party_handle(raw) { TEAM_BIT | (raw & LOW) } else { raw & LOW };
    PARTY_HANDLE_BASE + u32::from(member) * PARTY_HANDLE_SPAN + low
}

fn own_effect(member: u8, h: EffectHandle) -> EffectHandle {
    EffectHandle(own_handle(member, h.0))
}

/// A sound's attach of a Sora copy for the audio side: on HIM is on his own taps (`Attach::Party { member }`); anything else
/// after the host's `retarget` (a boss copy's victim id -> the real character it is).
fn sora_audio_at(member: u8, retarget: Option<&dyn Fn(Attach) -> Attach>, at: Attach) -> Attach {
    match at {
        Attach::Sora { bone, offset } => Attach::Party { member: PartyMember(member), bone, offset },
        other => retarget.map_or(other, |f| f(other)),
    }
}

/// A friend copy's attach: ITS member (its runtime's id `own`) and its runtime's stand-in Sora (kept at its feet) are the
/// copy; a victim after `retarget`.
fn friend_at(member: u8, own: Option<u8>, retarget: Option<&dyn Fn(Attach) -> Attach>, at: Attach) -> Attach {
    match at {
        Attach::Party { member: m, bone, offset } if Some(m.0) == own => Attach::Party { member: PartyMember(member), bone, offset },
        Attach::Sora { offset, .. } => Attach::Party { member: PartyMember(member), bone: None, offset },
        other => retarget.map_or(other, |f| f(other)),
    }
}

// ------------------------------------------------------------------------------------------------ the Sora copies' seams

/// A SORA COPY'S EFFECTS: his own handles, into his own queue (`queue`: his step's commands for
/// `EffectHost::step_instance`, `Attach::Sora` = on HIM); a victim after `retarget` (its instance form: `Attach::Target`
/// stays a target, the instance scene answers it).
pub struct SoraCopyFx<'a> {
    pub queue: &'a mut EffectQueue,
    pub member: u8,
}

impl kh_core::Effects for SoraCopyFx<'_> {
    fn spawn(&mut self, handle: EffectHandle, spawn: &EffectSpawn) {
        self.queue.spawn(own_effect(self.member, handle), spawn);
    }
    fn timing(&mut self, handle: EffectHandle, timing: kh_core::host::EffectTiming) {
        self.queue.timing(own_effect(self.member, handle), timing);
    }
    fn tint(&mut self, handle: EffectHandle, tint: kh_core::host::Tint) {
        self.queue.tint(own_effect(self.member, handle), tint);
    }
    fn color(&mut self, handle: EffectHandle, rgba: [f32; 4]) {
        self.queue.color(own_effect(self.member, handle), rgba);
    }
    fn place(&mut self, handle: EffectHandle, at: Vec3) {
        self.queue.place(own_effect(self.member, handle), at);
    }
    fn transform(&mut self, handle: EffectHandle, at: Vec3, rotation: Vec3, scale: Vec3) {
        self.queue.transform(own_effect(self.member, handle), at, rotation, scale);
    }
    fn stop(&mut self, handle: EffectHandle, immediate: bool) {
        self.queue.stop(own_effect(self.member, handle), immediate);
    }
}

/// A SORA COPY'S SOUNDS: his own handles, on his own taps, his voice group number his own (a party voice group number of his
/// id: kh-audio keeps it in his own state, never the player's).
pub struct SoraCopyAudio<'a> {
    pub inner: &'a mut dyn kh_core::Audio,
    pub member: u8,
    pub retarget: Option<&'a dyn Fn(Attach) -> Attach>,
}

impl kh_core::Audio for SoraCopyAudio<'_> {
    fn play(&mut self, handle: SoundHandle, play: &SoundPlay) {
        let p = SoundPlay { at: sora_audio_at(self.member, self.retarget, play.at), ..*play };
        self.inner.play(started_sound(self.member, handle), &p); // (ER-HITSFX) its generation
    }
    fn stop(&mut self, handle: SoundHandle, fade_out: f32) {
        self.inner.stop(stopped_sound(self.member, handle), fade_out);
    }
    fn voice_group_no(&mut self, number: i32) {
        self.inner.party_voice_group_no(PartyMember(self.member), number);
    }
}

// ------------------------------------------------------------------------------------------------ the friend copies' seams

/// A FRIEND COPY'S EFFECTS (its runtime of ONE member, ids as the real party's): its own handles, its own member, its victims
/// the real ones. They go the party's route (the shared queue, `EffectHost::set_party` with its tap).
pub struct FriendCopyFx<'a> {
    pub inner: &'a mut dyn kh_core::Effects,
    pub member: u8,
    /// Its runtime's id of it.
    pub own: Option<u8>,
    pub retarget: Option<&'a dyn Fn(Attach) -> Attach>,
}

impl kh_core::Effects for FriendCopyFx<'_> {
    fn spawn(&mut self, handle: EffectHandle, spawn: &EffectSpawn) {
        let s = EffectSpawn { at: friend_at(self.member, self.own, self.retarget, spawn.at), ..*spawn };
        self.inner.spawn(own_effect(self.member, handle), &s);
    }
    fn timing(&mut self, handle: EffectHandle, timing: kh_core::host::EffectTiming) {
        self.inner.timing(own_effect(self.member, handle), timing);
    }
    fn tint(&mut self, handle: EffectHandle, tint: kh_core::host::Tint) {
        self.inner.tint(own_effect(self.member, handle), tint);
    }
    fn color(&mut self, handle: EffectHandle, rgba: [f32; 4]) {
        self.inner.color(own_effect(self.member, handle), rgba);
    }
    fn place(&mut self, handle: EffectHandle, at: Vec3) {
        self.inner.place(own_effect(self.member, handle), at);
    }
    fn transform(&mut self, handle: EffectHandle, at: Vec3, rotation: Vec3, scale: Vec3) {
        self.inner.transform(own_effect(self.member, handle), at, rotation, scale);
    }
    fn stop(&mut self, handle: EffectHandle, immediate: bool) {
        self.inner.stop(own_effect(self.member, handle), immediate);
    }
}

/// A FRIEND COPY'S SOUNDS: its own handles, its own member (its voice group number, its voice line, its routes), its victims
/// the real ones. Its runtime's stand-in Sora has no voice of its own: a Sora-level voice group number is dropped.
pub struct FriendCopyAudio<'a> {
    pub inner: &'a mut dyn kh_core::Audio,
    pub member: u8,
    pub own: Option<u8>,
    pub retarget: Option<&'a dyn Fn(Attach) -> Attach>,
}

impl kh_core::Audio for FriendCopyAudio<'_> {
    fn play(&mut self, handle: SoundHandle, play: &SoundPlay) {
        let p = SoundPlay { at: friend_at(self.member, self.own, self.retarget, play.at), ..*play };
        self.inner.play(started_sound(self.member, handle), &p); // (ER-HITSFX) its generation
    }
    fn stop(&mut self, handle: SoundHandle, fade_out: f32) {
        self.inner.stop(stopped_sound(self.member, handle), fade_out);
    }
    fn party_voice_group_no(&mut self, member: PartyMember, number: i32) {
        let m = if Some(member.0) == self.own { PartyMember(self.member) } else { member };
        self.inner.party_voice_group_no(m, number);
    }
}

// ------------------------------------------------------------------------------------------------ the copies' state

/// One copy's state of this route.
#[derive(Default)]
struct CopyState {
    /// His bones of his latest step (a Sora copy's: the effect side's sockets hang on them).
    tap: PoseTap,
    /// His steps so far on this route (never reset: the member detector runs once per new step).
    steps: u64,
    /// The Keyblade the effect side was told he holds.
    kb_key: String,
    ground: Option<f32>,
    told: bool,
}

/// One copy's tap of a step (fx_party `PartyTap`'s fields): member, step, position, yaw, bones and sockets, grounded,
/// anim, weapon.
pub type CopyTap = (u8, u64, Vec3, f32, HashMap<String, (Vec3, Quat)>, bool, (String, f32), String);

struct Shared {
    copies: HashMap<u8, CopyState>,
    /// The copies' latest taps, with their age in party feeds (a copy not stepped for a while is dropped).
    taps: HashMap<u8, (CopyTap, u32)>,
    projectiles: HashMap<u8, Vec<(u32, Vec3, Vec3)>>,
    /// Copies gone since the last party feed (their looping sounds are stopped there).
    gone: Vec<u8>,
    /// Copies whose effects are to end (member, at once), carried out as soon as the effect side is free.
    ending: Vec<(u8, bool)>,
}

static SHARED: Mutex<Option<Shared>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut Shared) -> R) -> R {
    let mut g = SHARED.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(|| Shared { copies: HashMap::new(), taps: HashMap::new(), projectiles: HashMap::new(), gone: Vec::new(), ending: Vec::new() }))
}

/// The effects asked to end (`copy_gone`, `end_effects`) end now, those the effect side could not take this instant (busy for
/// a moment, never waited for) at a later call. Returns how many effects ended.
fn end_pending() -> usize {
    let asked = with(|s| std::mem::take(&mut s.ending));
    let mut left = Vec::new();
    let mut n = 0;
    for (member, immediate) in asked {
        if !crate::fx::with_effects(|e| n += e.stop_member(member, immediate)) {
            left.push((member, immediate));
        }
    }
    with(|s| s.ending.extend(left));
    n
}

/// A tap older than this many party feeds is dropped (the copy is no longer stepped).
const TAP_AGE: u32 = 120;

/// (fx.rs `set_party`) The copies' taps for the coming step (each kept until its copy goes or stops being stepped).
pub fn taps() -> Vec<CopyTap> {
    end_pending();
    with(|s| {
        s.taps.retain(|_, (_, age)| {
            *age += 1;
            *age <= TAP_AGE
        });
        let mut out: Vec<CopyTap> = s.taps.values().map(|(t, _)| t.clone()).collect();
        out.sort_by_key(|t| t.0);
        out
    })
}

/// (fx.rs `set_party`) The friend copies' spell actors / props (body-effect handle, position, flight direction).
pub fn projectiles() -> Vec<(u32, Vec3, Vec3)> {
    with(|s| s.projectiles.values().flatten().copied().collect())
}

/// (fx.rs `set_party`) The copies gone since the last call (their looping sounds stop).
pub fn take_gone() -> Vec<u8> {
    with(|s| std::mem::take(&mut s.gone))
}

/// EVERYTHING OF COPY `member` ENDS (its effects fade out now; `immediate`: gone at once) and its tap is forgotten (it left,
/// died away, the fight ended). Its looping sounds stop at the next party feed.
pub fn copy_gone(member: u8, immediate: bool) {
    // (nothing known of it: nothing to end; called every step while a copy is away)
    let had = with(|s| {
        let had = s.taps.remove(&member).is_some() | s.projectiles.remove(&member).is_some() | s.copies.remove(&member).is_some();
        if had {
            s.gone.push(member);
            s.ending.push((member, immediate));
        }
        had
    });
    if had {
        let n = end_pending();
        erlog!("copyfx: {} is gone: {n} effect(s) of it end{}, its looping sounds stop", kh_audio::copies::name(member), if immediate { " at once" } else { "" });
    }
}

/// The effects copy `member` has started end (fading); it stays (Mimic Sora's dismissal: his dissolve still plays).
pub fn end_effects(member: u8) {
    with(|s| s.ending.push((member, false)));
    end_pending();
}

/// (a friend copy) Its step's taps (its runtime's id `own` becomes `member`) and spell actors (their handles its own).
pub fn friend_feed(member: u8, own: Option<u8>, taps: Vec<kh_partyhost::MemberTap>, projectiles: &[kh_partyhost::ProjectileTap]) {
    let tap = taps.into_iter().find(|t| Some(t.member) == own);
    with(|s| {
        let st = s.copies.entry(member).or_default();
        if let Some(t) = tap {
            st.steps += 1;
            let entry = (member, st.steps, t.position, t.yaw, t.bones, t.grounded, t.anim, t.weapon);
            s.taps.insert(member, (entry, 0));
        }
        s.projectiles.insert(member, projectiles.iter().map(|p| (own_handle(member, p.effect), p.position, p.forward)).collect());
    });
}

/// What a Sora copy hands this route after his core's step and his pose.
pub struct SoraCopyStep<'a> {
    pub member: u8,
    pub out: &'a Output,
    pub core: &'a Sora,
    pub skeleton: &'a kh_anim::Skeleton,
    pub pose: &'a kh_anim::Pose,
    /// His step's effect commands (his [`SoraCopyFx`] queue).
    pub commands: Vec<EffectCommand>,
    /// The centre of a target of his (ER's characters; a boss copy's victims).
    pub target: &'a dyn Fn(TargetId) -> Option<Vec3>,
    /// His sounds' / effects' victims for the audio side (as [`SoraCopyAudio`]'s).
    pub retarget: Option<&'a dyn Fn(Attach) -> Attach>,
}

/// ONE STEP OF A SORA COPY ON THIS ROUTE (after his core's step and pose): his effects on his own scene (the effect side's
/// Sora instance `member`: his Keyblade, his bones, his core), their audio side into the shared queue (`Attach::Party` on
/// him), his tap (bones, the effect side's sockets on him, grounded, anim, weapon) for the party feed.
pub fn sora_copy_step(fx: &mut crate::fx::Fx, c: SoraCopyStep<'_>) {
    let SoraCopyStep { member, out, core, skeleton, pose, commands, target, retarget } = c;
    let mut st = with(|s| s.copies.remove(&member)).unwrap_or_default();
    st.steps += 1;
    // his bones (in the outfit's proportions he is drawn in: boc.rs, as the player's)
    let outfit_pose = crate::boc::effect_pose(skeleton, pose);
    st.tap.record(st.steps, out, skeleton, outfit_pose.as_ref().unwrap_or(pose), &kh_host::rig::component_to_world(out));
    st.tap.samples.clear();
    if out.on_floor {
        st.ground = Some(out.position.z);
    }
    let status = core.status();
    // the Keyblade in HIS hand (kbdraw.rs `equip`'s rule on his status), told to the effect side when it changes
    let (kb_key, kb_base) = crate::sora::kbdraw::effect_key(status);
    let kb = (st.kb_key != kb_key).then(|| crate::sora::kbdraw::effect_facts(status));
    let mut sockets = Vec::new();
    if fx.vfx_active() {
        let scene = Scene { out, sora: core, tap: &st.tap, target, ground: st.ground.or(Some(out.position.z)), eye: out.position };
        let told = crate::fx::with_effects(|e| {
            if let Some(kb) = kb.as_ref() {
                e.set_instance_keyblade(member, kb.trail.as_deref(), kb.sockets.as_ref(), &kb.allowed);
            }
            e.step_instance(member, &commands, &scene);
            sockets = e.instance_sockets(member, &scene, &[]);
        });
        if let (true, Some(kb)) = (told, kb.as_ref()) {
            erlog!("copyfx: {}'s effects: his own Keyblade {} (trail {}; {} effect prefixes pass), his own bones and sockets (the effect side's Sora instance {member})", kh_audio::copies::name(member), kb_base, kb.trail.as_deref().unwrap_or("the Kingdom Key's"), kb.allowed.len());
            st.kb_key = kb_key;
        }
    }
    // the audio side of his effects (their sounds start, follow and end with them): on him, the victims real
    for c in &commands {
        fx.effects.0.push(match *c {
            EffectCommand::Spawn(h, s) => EffectCommand::Spawn(h, EffectSpawn { at: sora_audio_at(member, retarget, s.at), ..s }),
            other => other,
        });
    }
    // his tap
    let mut bones: HashMap<String, (Vec3, Quat)> = HashMap::with_capacity(skeleton.bones.len() + sockets.len());
    for b in &skeleton.bones {
        if let Some(t) = st.tap.bone_world(&b.name) {
            bones.insert(b.name.clone(), (t.translation, t.rotation));
        }
    }
    for (name, p, r) in sockets {
        bones.entry(name).or_insert((p, r));
    }
    let weapon = format!("{}|{}", if status.attack_model.is_empty() { kb_base.as_str() } else { status.attack_model }, status.left_attack_model);
    let tap: CopyTap = (member, st.steps, out.position, out.yaw, bones, out.on_floor, (out.anim_ticked.name.name().to_string(), out.anim_ticked.time), weapon);
    if !st.told {
        st.told = true;
        erlog!("copyfx: {} plays his sounds and effects from his own place (member {member}: his handles, his taps, his own voice line and footsteps)", kh_audio::copies::name(member));
    }
    with(|s| {
        s.taps.insert(member, (tap, 0));
        s.copies.insert(member, st);
    });
}

// ------------------------------------------------------------------------- (ER-HITSFX) the copies' handle generations
//
// THE USER (2026-11-07): "Make sure audio is never choppy or cut off before it's supposed to." A copy whose core is REBUILT
// (a new Sora core / friend runtime for the same member id: the boss's copy re-made, Mimic Sora summoned again) counts its
// handles from the start of its range again. [`own_handle`] alone gave the new core's handle 1 the SAME own handle as the old
// core's handle 1, so a stop of the new core's sound / effect cut a sound of the old one still ringing out (a one-shot of
// its last swing, a voice line). Now every copy's SOUND handles go through a GENERATION per member (its EFFECT handles keep
// the effect side's own new-core rule, kh-host vfx/instances.rs `step_instance`, which ends the old core's effects):
// - a sound handle STARTED (played) gets `own_handle` moved by its generation's offset, and is
//   remembered (raw -> own) while it may still be addressed;
// - a new core is seen when it starts a handle at or below the highest its generation has started (kh-core's counter never
//   gives one twice): the next generation begins ABOVE every handle of the old one (offset += that highest), so the old core's ringing sounds keep
//   handles nobody will ever stop again (they end by themselves, or by the copy's "gone" loop stop of fx.rs);
// - a stop goes to the own handle its raw handle was started with: a spurious new
//   generation (a handle taken early, started late) never loses a stop of a live handle. The range and the member stay the copy's (kh-audio `copies::of_handle`,
//   its scope), only the low bits move (23 of them: wrapping after 8 million handles of one copy).

/// Remembered live handles of one generation table at most (the oldest half is forgotten past it: natural ends send nothing).
const LIVE_CAP: usize = 4096;

/// One member's and kind's handle generations.
#[derive(Default, Debug)]
pub struct Generation {
    /// Added to the low bits of every handle started in this generation.
    offset: u32,
    /// The highest low bits started in this generation, per class (0: its own range, 1: its team range).
    high: [u32; 2],
    /// raw -> (own, order of start).
    live: HashMap<u32, (u32, u64)>,
    started: u64,
    /// Generations begun after the first (for the log and the tests).
    pub rebuilds: u32,
}

fn class_and_low(raw: u32) -> (usize, u32) {
    if raw >= TEAM_HANDLE_BASE && kh_core::host::party::is_party_handle(raw) {
        (1, raw & LOW)
    } else {
        (0, raw & LOW)
    }
}

fn own_with(member: u8, raw: u32, offset: u32) -> u32 {
    let first = own_handle(member, raw); // generation 0: the copy's range, the team bit, the raw low bits
    let mut moved = (first & LOW).wrapping_add(offset) & LOW;
    if moved == 0 {
        moved = 1; // (the range's base itself is never a handle: kh-core counts from base + 1)
    }
    (first & !LOW) | moved
}

impl Generation {
    /// A handle started (a sound played): its own handle (a new generation first when a new core shows).
    pub fn start(&mut self, member: u8, raw: u32) -> u32 {
        let (class, low) = class_and_low(raw);
        if low <= self.high[class] {
            // a handle at or below this generation's highest (a core never starts one handle twice): a NEW CORE counting
            // from its start again. The old raw -> own entries stay: each is replaced when the new core starts that raw.
            self.offset = self.offset.wrapping_add(self.high[0].max(self.high[1])) & LOW;
            self.high = [0, 0];
            self.rebuilds += 1;
        }
        self.high[class] = self.high[class].max(low);
        let own = own_with(member, raw, self.offset);
        self.started += 1;
        self.live.insert(raw, (own, self.started));
        if self.live.len() > LIVE_CAP {
            let mut order: Vec<u64> = self.live.values().map(|v| v.1).collect();
            order.sort_unstable();
            let keep_from = order[order.len() / 2];
            self.live.retain(|_, v| v.1 >= keep_from);
        }
        own
    }

    /// A stop: its own handle, and the raw handle is no longer live.
    pub fn stop(&mut self, member: u8, raw: u32) -> u32 {
        self.live.remove(&raw).map_or_else(|| own_with(member, raw, self.offset), |v| v.0)
    }
}

/// Every copy's sound-handle generations.
static GENERATIONS: Mutex<Option<HashMap<u8, Generation>>> = Mutex::new(None);

fn with_gen<R>(member: u8, f: impl FnOnce(&mut Generation) -> R) -> R {
    let mut g = GENERATIONS.lock().unwrap_or_else(|e| e.into_inner());
    let gen = g.get_or_insert_with(HashMap::new).entry(member).or_default();
    let before = gen.rebuilds;
    let r = f(gen);
    if gen.rebuilds != before {
        erlog!("copyfx: (ER-HITSFX) {} counts its sound handles again (a new core): generation {} starts above the old one's, whose ringing sounds are never stopped by the new one", kh_audio::copies::name(member), gen.rebuilds);
    }
    r
}

fn started_sound(member: u8, h: SoundHandle) -> SoundHandle {
    SoundHandle(with_gen(member, |g| g.start(member, h.0)))
}

fn stopped_sound(member: u8, h: SoundHandle) -> SoundHandle {
    SoundHandle(with_gen(member, |g| g.stop(member, h.0)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kh_audio::copies::{BOSS_SORA, DONALD, MIMIC_SORA};

    /// A copy's handles lie in its own range, whatever range its runtime counted them in; two copies (and the player, and
    /// the real party) never share one.
    #[test]
    fn mimic_fx_a_copys_handles_are_in_its_own_range() {
        use kh_core::host::party::{handle_member, SeamHandles};
        // a Sora core's (from 1)
        assert_eq!(handle_member(own_handle(MIMIC_SORA, 7)).map(|m| m.0), Some(MIMIC_SORA));
        assert_ne!(own_handle(MIMIC_SORA, 7), own_handle(BOSS_SORA, 7));
        // a friend copy's runtime counts in Donald's range (0) like the real Donald: its copy's range is DONALD's (8)
        let mut real = SeamHandles::for_member(PartyMember::DONALD);
        let h = real.effect();
        assert_eq!(handle_member(h.0).map(|m| m.0), Some(0));
        assert_eq!(handle_member(own_handle(DONALD, h.0)).map(|m| m.0), Some(DONALD));
        assert_eq!(own_handle(DONALD, h.0) & LOW, h.0 & LOW);
        // its team pawns' never meet its own
        let mut team = SeamHandles::team();
        let t = team.effect();
        assert_eq!(t.0 & LOW, h.0 & LOW);
        assert_ne!(own_handle(DONALD, t.0), own_handle(DONALD, h.0));
        assert_eq!(handle_member(own_handle(DONALD, t.0)).map(|m| m.0), Some(DONALD));
    }

    /// (ER-HITSFX) A REBUILT copy never stops the old copy's ringing sounds: its sound handles start a generation above
    /// the old one's; the old core's handles that rang on are never addressed again; every handle stays in the copy's own
    /// range and audio scope; a stop of a live handle is never lost, even across a spurious generation.
    #[test]
    fn hitsfx_a_rebuilt_copys_handles_never_meet_the_old_ones() {
        use kh_audio::copies::{of_handle, BOSS_MIMIC};
        use kh_core::host::party::SeamHandles;
        let m = BOSS_MIMIC;
        let mut g = Generation::default();
        // the old core (a Sora core counts from 1): five sounds, the 2nd stopped, the others ring out
        let old: Vec<u32> = (1..=5).map(|raw| g.start(m, raw)).collect();
        assert_eq!(g.stop(m, 2), old[1]);
        assert_eq!(old[0], own_handle(m, 1), "generation 0 is the plain rule");
        // the copy is rebuilt: its new core counts from 1 again, plays and stops its own
        let new: Vec<u32> = (1..=3).map(|raw| g.start(m, raw)).collect();
        assert_eq!(g.rebuilds, 1);
        for (raw, h) in (1..=3).zip(&new) {
            assert!(!old.contains(h), "new handle {h:#x} (raw {raw}) is one of the old core's");
            assert_eq!(g.stop(m, raw), *h, "the new core's stop goes to ITS sound");
            assert_eq!(of_handle(*h), Some(m), "still the copy's own scope");
        }
        // a second rebuild starts above both
        let third = g.start(m, 1);
        assert!(!old.contains(&third) && !new.contains(&third));
        assert_eq!(g.rebuilds, 2);
        // the same core, a handle taken early and started late (a spurious generation): its other live handle's stop holds
        let mut g = Generation::default();
        let ten = g.start(m, 10);
        let eleven = g.start(m, 11);
        let nine = g.start(m, 9);
        assert_eq!(g.rebuilds, 1);
        assert_eq!(g.stop(m, 10), ten);
        assert_eq!(g.stop(m, 11), eleven);
        assert_eq!(g.stop(m, 9), nine);
        assert_ne!(nine, ten);
        // a friend copy's runtime: its own range and its team range move apart, never into each other
        let mut g = Generation::default();
        let (mut own, mut team) = (SeamHandles::for_member(PartyMember::DONALD), SeamHandles::team());
        let (a, b) = (own.effect().0, team.effect().0);
        let (ha, hb) = (g.start(DONALD, a), g.start(DONALD, b));
        assert_ne!(ha, hb);
        let (mut own2, mut team2) = (SeamHandles::for_member(PartyMember::DONALD), SeamHandles::team());
        let (ha2, hb2) = (g.start(DONALD, own2.effect().0), g.start(DONALD, team2.effect().0));
        assert!(![ha, hb].contains(&ha2) && ![ha, hb].contains(&hb2), "the rebuilt runtime's first handles are new");
        for h in [ha, hb, ha2, hb2] {
            assert_eq!(of_handle(h), Some(DONALD));
        }
    }

    /// (ER-HITSFX) THE USER'S RULE THROUGH THE COPY'S AUDIO SEAM: the boss's Mimic plays a long one-shot, is rebuilt in the
    /// middle of it, and the new core plays and stops its own sounds (a combo, a stop of every one): no stop ever reaches the
    /// old one-shot's handle, so it plays to its end; the new core's stops all reach its own sounds.
    #[test]
    fn hitsfx_rebuilding_a_copy_mid_ring_out_lets_the_old_sound_play_to_its_end() {
        use kh_audio::copies::BOSS_MIMIC;
        use kh_core::{SoundId, SoundPlay};
        use kh_host::seam::{AudioCommand, AudioQueue};
        let mut q = AudioQueue::default();
        let play = SoundPlay::at_sora(SoundId(1), "long one-shot");
        {
            let mut seam = SoraCopyAudio { inner: &mut q, member: BOSS_MIMIC, retarget: None };
            // the old core: its first sound (a Sora core counts from 1), still ringing when the copy is rebuilt
            kh_core::Audio::play(&mut seam, SoundHandle(1), &play);
            kh_core::Audio::play(&mut seam, SoundHandle(2), &play);
            kh_core::Audio::stop(&mut seam, SoundHandle(2), 0.1);
            // the rebuilt copy: a new core counting from 1 again, a dense combo with stops of every one of its sounds
            for raw in 1..=40 {
                kh_core::Audio::play(&mut seam, SoundHandle(raw), &play);
            }
            for raw in 1..=40 {
                kh_core::Audio::stop(&mut seam, SoundHandle(raw), 0.0);
            }
        }
        let AudioCommand::Play(ringing, _) = q.0[0] else { panic!("the first command is the old one-shot") };
        let plays: Vec<SoundHandle> = q.0.iter().filter_map(|c| if let AudioCommand::Play(h, _) = c { Some(*h) } else { None }).collect();
        let stops: Vec<SoundHandle> = q.0.iter().filter_map(|c| if let AudioCommand::Stop(h, _) = c { Some(*h) } else { None }).collect();
        assert!(!stops.contains(&ringing), "the old core's ringing one-shot {ringing:?} was stopped by the rebuilt copy");
        assert_eq!(stops[0], plays[1], "the old core's own stop reached its own sound");
        for (k, h) in plays[2..].iter().enumerate() {
            assert_eq!(stops[k + 1], *h, "the new core's stop {k} reached its own sound");
            assert_eq!(kh_audio::copies::of_handle(h.0), Some(BOSS_MIMIC));
        }
        assert_eq!(kh_audio::copies::of_handle(ringing.0), Some(BOSS_MIMIC));
    }

    /// A Sora copy's sound on him is on HIS taps; a friend copy's on its own member or its stand-in is on ITS taps; a victim
    /// after the host's retarget; anything else as it is.
    #[test]
    fn mimic_fx_a_copys_attaches_are_its_own() {
        let on_him = Attach::Sora { bone: Some("vfx_call"), offset: Vec3::X };
        assert_eq!(sora_audio_at(BOSS_SORA, None, on_him), Attach::Party { member: PartyMember(BOSS_SORA), bone: Some("vfx_call"), offset: Vec3::X });
        let victim = |a: Attach| match a {
            Attach::Target { offset, .. } => Attach::Sora { bone: None, offset },
            other => other,
        };
        let hit = Attach::Target { target: TargetId(77), offset: Vec3::Z };
        assert_eq!(sora_audio_at(BOSS_SORA, Some(&victim), hit), Attach::Sora { bone: None, offset: Vec3::Z });
        assert_eq!(sora_audio_at(BOSS_SORA, None, Attach::World(Vec3::ONE)), Attach::World(Vec3::ONE));
        let his = Attach::Party { member: PartyMember(0), bone: Some("wep:vfx_call"), offset: Vec3::ZERO };
        assert_eq!(friend_at(DONALD, Some(0), None, his), Attach::Party { member: PartyMember(DONALD), bone: Some("wep:vfx_call"), offset: Vec3::ZERO });
        assert_eq!(friend_at(DONALD, Some(0), None, Attach::Sora { bone: Some("x"), offset: Vec3::Y }), Attach::Party { member: PartyMember(DONALD), bone: None, offset: Vec3::Y });
        // the real Goofy (a victim of a Donald copy while Goofy leads) stays the real one
        let goofy = Attach::Party { member: PartyMember(1), bone: None, offset: Vec3::ZERO };
        assert_eq!(friend_at(DONALD, Some(0), None, goofy), goofy);
    }

    /// (MIMIC-BOSS hotfix 3, MB3-AUDIO) Every handle a copy's sound or effect carries ([`own_handle`]) puts it in ITS OWN
    /// audio scope (kh-audio `copies::of_handle` / `scope`), also when it reaches the audio side outside its member's call
    /// (a hit on its victim, an effect's sound, a spell actor's hit): kh-audio's copy of the handle layout is kh-core's; the
    /// player's handles (a Sora core counts from 1), the real party's and the team's are of the KH3 world (scope 0).
    #[test]
    fn mb3_a_copys_handles_put_its_sounds_in_its_own_audio_scope() {
        use kh_audio::copies::{of_handle, scope, BOSS_MIMIC, GOOFY};
        use kh_core::host::party::SeamHandles;
        assert_eq!((kh_audio::copies::PARTY_HANDLE_BASE, kh_audio::copies::PARTY_HANDLE_SPAN), (PARTY_HANDLE_BASE, PARTY_HANDLE_SPAN));
        let mut donald = SeamHandles::for_member(PartyMember::DONALD);
        let mut team = SeamHandles::team();
        let raws = [1, 7, 0x7F_FFFF, donald.effect().0, donald.effect().0, team.effect().0];
        for member in [DONALD, GOOFY, BOSS_SORA, BOSS_MIMIC, MIMIC_SORA] {
            for raw in raws {
                let h = own_handle(member, raw);
                assert_eq!(of_handle(h), Some(member), "{raw:#x} of member {member}");
                assert_eq!(scope(member), u32::from(member));
            }
        }
        for raw in [1, 7, 0x1F_FFFF, donald.effect().0, team.effect().0] {
            assert_eq!(of_handle(raw), None, "{raw:#x}: the player's, the real party's or the team's");
        }
        assert_eq!(scope(0), 0);
        assert_eq!(scope(1), 0);
    }
}

// (MIMIC-FX) the friend copies through this route against the real trees and packs
#[cfg(test)]
#[path = "copyfx_tests.rs"]
mod friend_tests;
