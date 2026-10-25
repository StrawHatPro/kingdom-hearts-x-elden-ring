//! (GOOFY-HR) The hit reactions of every playable character: per character (Donald, Goofy), as the leader (pad) and as
//! an AI ally, plus a mocked future character through the same functions, Sora's own reaction as the reference, and the
//! step-length independence of the throw. THE BUG these pin: build/sora_tmp/er/fix3/ref/kh-er_goofy_session.log
//! (t = 102.407 s: M_0211, M_0212 held ~0.8 s, a 0.6 s cut of M_0213, IdleB).

use super::*;
use crate::anim::{ClipNotify, ClipNotifyData};
use crate::host::PartyHost;
use crate::ids::{Chara, MemberId};
use crate::pad::{PadFrame, RUN as PAD_RUN};
use crate::{Party, PartyConfig, SoraView};
use glam::{Vec2, Vec3};
use kh_core::enums::ETresDamageKind as Kind;
use kh_core::host::{EffectHandle, EffectSpawn, HitRecord, SoundHandle, SoundPlay, TargetId};
use kh_core::input::{PadButton, PadButtons, PadState as Buttons};
use kh_core::test_world::TestWorld;
use kh_react::{EnemyReaction, FlatWorld, Pose, ReactionRequest};

const DT: f32 = 1.0 / 60.0;

#[derive(Default)]
struct Fx(Vec<&'static str>);
impl kh_core::host::Effects for Fx {
    fn spawn(&mut self, _h: EffectHandle, s: &EffectSpawn) {
        self.0.push(s.name);
    }
    fn stop(&mut self, _h: EffectHandle, _immediate: bool) {}
}

#[derive(Default)]
struct Sfx(Vec<&'static str>);
impl kh_core::host::Audio for Sfx {
    fn play(&mut self, _h: SoundHandle, p: &SoundPlay) {
        self.0.push(p.name);
    }
    fn stop(&mut self, _h: SoundHandle, _f: f32) {}
}

/// One traced step of a member: what a renderer would draw.
#[derive(Clone, Copy, Debug)]
struct Row {
    state: NpcStateId,
    clip: &'static str,
    time: f32,
    finished: bool,
    looped: bool,
    pos: Vec3,
    speed: f32,
}

struct Rig {
    world: TestWorld,
    targets: kh_core::host::NullTargets,
    audio: Sfx,
    effects: Fx,
    party: Party,
    sora: SoraView,
    id: MemberId,
    held: PadButtons,
    trace: Vec<Row>,
}

impl Rig {
    fn new(chara: Chara, leader: bool, world: TestWorld) -> Rig {
        let mut party = Party::new(PartyConfig::test_level99());
        let id = party.spawn(chara, Vec3::ZERO, 0.0);
        if leader {
            party.set_pad(id, Some(kh_core::Tuning::USER));
        }
        let mut sora = SoraView::default();
        sora.position = Vec3::new(-3000.0, 0.0, 0.0);
        let mut rig = Rig { world, targets: Default::default(), audio: Default::default(), effects: Default::default(), party, sora, id, held: PadButtons::NONE, trace: Vec::new() };
        rig.step(Vec2::ZERO, &[], 20);
        rig.trace.clear();
        rig
    }

    fn m(&self) -> &NpcPawn {
        self.party.member(self.id).unwrap()
    }

    fn mm(&mut self) -> &mut NpcPawn {
        self.party.member_mut(self.id).unwrap()
    }

    fn step(&mut self, stick: Vec2, held: &[PadButton], n: usize) {
        for _ in 0..n {
            let mut now = PadButtons::NONE;
            for b in held {
                now.set(*b, true);
            }
            let pad = Buttons { held: now, pressed: PadButtons(now.0 & !self.held.0), released: PadButtons(self.held.0 & !now.0) };
            self.held = now;
            if self.m().pad.is_some() {
                self.party.set_pad_frame(self.id, PadFrame { stick, camera_yaw: 0.0, pad });
            }
            let mut host = PartyHost { world: &self.world, targets: &mut self.targets, audio: &mut self.audio, effects: &mut self.effects };
            self.party.step(DT, &self.sora, &mut host);
            let m = self.m();
            self.trace.push(Row { state: m.state, clip: m.anim.clip.name, time: m.anim.time, finished: m.anim.finished, looped: m.anim.force_loop || m.anim.clip.looping, pos: m.position(), speed: Vec2::new(m.cmc.velocity.x, m.cmc.velocity.y).length() });
        }
    }

    /// A hit of `kind` from an attacker in front of him (+X; he faces +X): the knock pushes him toward -X.
    fn hit(&mut self, kind: Kind, damage: f32) -> kh_core::host::HitResult {
        let at = self.m().position();
        let mut h = HitRecord::empty(TargetId(7));
        h.damage = damage;
        h.damage_kind = kind;
        h.position = at + Vec3::Z * 90.0;
        h.direction = Vec3::new(-1.0, 0.0, 0.0);
        h.attack_name = "test.enemy";
        self.party.receive_hit(self.id, &h)
    }

    /// Steps until he is out of the damage state (at most `max` steps).
    fn until_free(&mut self, max: usize) -> usize {
        for k in 1..=max {
            self.step(Vec2::ZERO, &[], 1);
            if self.m().state != NpcStateId::Damage {
                return k;
            }
        }
        panic!("still in Damage after {max} steps: {:?}", self.trace.last());
    }

    /// The clips in the order they were drawn during the damage state.
    fn clips(&self) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = Vec::new();
        for r in self.trace.iter().filter(|r| r.state == NpcStateId::Damage) {
            if out.last() != Some(&r.clip) {
                out.push(r.clip);
            }
        }
        out
    }

    /// THE FROZEN-POSE CHECK: the most consecutive damage-state steps a one-shot clip stood on its last frame.
    fn longest_hold(&self) -> usize {
        let (mut best, mut run) = (0, 0);
        for r in &self.trace {
            if r.state == NpcStateId::Damage && r.finished && !r.looped {
                run += 1;
                best = best.max(run);
            } else {
                run = 0;
            }
        }
        best
    }

    fn played(&self, clip: &str) -> f32 {
        self.trace.iter().filter(|r| r.state == NpcStateId::Damage && r.clip == clip).map(|r| r.time).fold(0.0, f32::max)
    }
}

fn len(chara: Chara, name: &str) -> f32 {
    crate::tables::clip(chara, name).length
}

// ------------------------------------------------------------------------------------------- the user's bug, per character

/// THE USER'S REPORT: a heavy hit (BLOW) is a throw with ground friction, a landing, the whole down + get-up clip, and a
/// blend to idle - never a held pose. For every playable character, as the AI ally and as the leader.
#[test]
fn a_heavy_hit_is_a_throw_a_landing_and_the_whole_get_up_for_every_character_ai_and_leader() {
    for chara in Chara::ALL {
        for leader in [false, true] {
            let mut rig = Rig::new(chara, leader, TestWorld::floor());
            let start = rig.m().position();
            let r = rig.hit(Kind::Blow, 5.0);
            assert!(r.accepted && !r.killed);
            assert_eq!(rig.m().state, NpcStateId::Damage);
            let steps = rig.until_free(600);
            let clips = rig.clips();
            let who = format!("{} {}", chara.name(), if leader { "leader" } else { "AI" });
            // KH3's Blow(0, phase): the start, the flight LOOP, the landing + get-up
            assert_eq!(&clips[..3], &["M_0211", "M_0212", "M_0213"], "{who}: {clips:?}");
            let loop_rows: Vec<&Row> = rig.trace.iter().filter(|r| r.clip == "M_0212").collect();
            assert!(loop_rows.iter().all(|r| r.looped), "{who}: the flight loop plays looped");
            // the whole get-up clip plays (the bug: 0.6 s of 2.0)
            let down = len(chara, "M_0213");
            assert!(rig.played("M_0213") >= down - 2.0 * DT, "{who}: M_0213 played {} of {down} s", rig.played("M_0213"));
            // never frozen: a one-shot stands on its last frame at most one step
            assert!(rig.longest_hold() <= 1, "{who}: held a finished one-shot {} steps: {:?}", rig.longest_hold(), rig.trace.iter().filter(|r| r.finished).take(3).collect::<Vec<_>>());
            // the knock-back travels and the friction stops it (KH3's blow numbers: FriendDamageCommonParam)
            let travel = start.x - rig.m().position().x;
            assert!(travel > 150.0, "{who}: thrown {travel} cm");
            assert!(rig.trace.last().unwrap().speed < 5.0, "{who}: still sliding");
            let peak = rig.trace.iter().map(|r| r.speed).fold(0.0, f32::max);
            let after = rig.trace.iter().skip(20).map(|r| r.speed).fold(0.0, f32::max);
            assert!(peak > 500.0 && after < peak, "{who}: speed {peak} then {after}");
            // out into idle with the idle clip's own blend (no snap)
            let m = rig.m();
            assert_eq!(m.state, NpcStateId::Idle, "{who}");
            assert!(m.anim.clip.name.starts_with("Idle") && m.anim.prev.is_some_and(|(c, _)| c.name == "M_0213"), "{who}: {:?}", m.anim.clip.name);
            let total = steps as f32 * DT;
            assert!(total > down && total < down + 1.5, "{who}: the reaction took {total} s");
        }
    }
}

/// The ground flinch ends with ITS clip (Goofy's M_0201 is 1.33 s; the enemy median cut it at 1.0 s): AI ally.
#[test]
fn a_light_hit_flinches_for_the_characters_own_clip() {
    for chara in Chara::ALL {
        let mut rig = Rig::new(chara, false, TestWorld::floor());
        rig.hit(Kind::Small, 3.0);
        let steps = rig.until_free(300);
        let clips = rig.clips();
        assert_eq!(clips.first(), Some(&"M_0201"), "{}: {clips:?}", chara.name());
        let l = len(chara, "M_0201");
        assert!(((steps as f32) * DT - l).abs() <= 3.0 * DT, "{}: flinch {} s, clip {l} s", chara.name(), steps as f32 * DT);
        assert!(rig.longest_hold() <= 1);
        // a second front hit alternates the clip (Small(index, variant): M_0205)
        rig.trace.clear();
        rig.hit(Kind::Small, 3.0);
        rig.step(Vec2::ZERO, &[], 2);
        rig.hit(Kind::Small, 3.0);
        rig.step(Vec2::ZERO, &[], 2);
        assert!(rig.clips().contains(&"M_0205"), "{}: {:?}", chara.name(), rig.clips());
    }
}

/// Hit in the air: the air flinch, the fall loop (looped), the landing, idle.
#[test]
fn an_air_hit_flinches_falls_and_lands() {
    for chara in Chara::ALL {
        let mut rig = Rig::new(chara, false, TestWorld::floor());
        rig.mm().cmc.location.z += 300.0;
        rig.mm().cmc.set_falling();
        rig.hit(Kind::Small, 3.0);
        rig.until_free(600);
        let clips = rig.clips();
        assert_eq!(clips.first(), Some(&"M_0207"), "{}: {clips:?}", chara.name());
        assert!(clips.contains(&"Land"), "{}: {clips:?}", chara.name());
        assert!(rig.longest_hold() <= 1, "{}: {clips:?}", chara.name());
        assert!(rig.m().on_ground());
    }
}

/// LAUNCH / TOSS: the friends' own Toss handler (0x141251570): M_0227, the rise loop M_0228, the fall loop M_0237, the
/// down M_0238 (Goofy) or M_0213 (Donald, who has no M_0238).
#[test]
fn a_launch_rises_hovers_falls_and_gets_up() {
    for chara in Chara::ALL {
        for kind in [Kind::Toss, Kind::Launch] {
            let mut rig = Rig::new(chara, false, TestWorld::floor());
            rig.hit(kind, 3.0);
            rig.until_free(900);
            let clips = rig.clips();
            let down = if crate::tables::anim_set(chara).get("M_0238").is_some() { "M_0238" } else { "M_0213" };
            assert_eq!(clips, vec!["M_0227", "M_0228", "M_0237", down], "{} {kind:?}", chara.name());
            let peak = rig.trace.iter().map(|r| r.pos.z).fold(0.0, f32::max);
            assert!(peak > 150.0, "{} {kind:?}: rose {peak} cm", chara.name());
            assert!(rig.longest_hold() <= 1, "{} {kind:?}", chara.name());
            assert!(rig.played(down) >= len(chara, down) - 2.0 * DT);
        }
    }
}

/// WALL BOUNCE: a blow into a wall plays KH3's wall clip (M_0223 / M_0224), the fall, the landing.
#[test]
fn a_blow_into_a_wall_hits_the_wall_and_drops() {
    for chara in Chara::ALL {
        // a wall 120 cm behind him (he is knocked toward -X)
        let world = TestWorld::floor().with_box(Vec3::new(-400.0, -1000.0, 0.0), Vec3::new(-120.0, 1000.0, 600.0));
        let mut rig = Rig::new(chara, false, world);
        rig.hit(Kind::Blow, 3.0);
        rig.until_free(900);
        let clips = rig.clips();
        assert!(clips.iter().any(|c| *c == "M_0223" || *c == "M_0224"), "{}: {clips:?}", chara.name());
        assert!(rig.longest_hold() <= 1, "{}: {clips:?}", chara.name());
        assert!(rig.m().position().x > -125.0, "{}: through the wall {:?}", chara.name(), rig.m().position());
    }
}

/// BEAT / PARABOLA / SIDESPIN: never a held pose either; the spin is a blow (no spin clip in a friend's set).
#[test]
fn every_other_heavy_kind_ends_with_a_clip_that_plays_out() {
    for chara in Chara::ALL {
        for kind in [Kind::Beat, Kind::Parabola, Kind::Sidespin, Kind::Verticalroll] {
            let mut rig = Rig::new(chara, false, TestWorld::floor());
            if kind == Kind::Beat {
                rig.mm().cmc.location.z += 200.0;
                rig.mm().cmc.set_falling();
            }
            rig.hit(kind, 3.0);
            rig.until_free(900);
            assert!(rig.longest_hold() <= 1, "{} {kind:?}: {:?}", chara.name(), rig.clips());
            assert_eq!(rig.m().state, NpcStateId::Idle);
        }
    }
}

// ------------------------------------------------------------------------------------------- the leader's inputs

/// THE QUICK GET-UP (KH3's exits rule): the leader's stick ends the down clip at its CheckBadStat, not before; an AI ally
/// plays it out.
#[test]
fn the_leader_gets_up_early_with_the_stick_at_the_clips_mark() {
    for chara in Chara::ALL {
        let mut rig = Rig::new(chara, true, TestWorld::floor());
        rig.hit(Kind::Blow, 3.0);
        // hold the stick all the way
        let mut left_at = None;
        for k in 0..600 {
            rig.step(Vec2::new(0.0, 1.0), &[], 1);
            if rig.m().state != NpcStateId::Damage {
                left_at = Some(k);
                break;
            }
        }
        assert!(left_at.is_some());
        let last_damage = rig.trace.iter().rev().find(|r| r.state == NpcStateId::Damage).copied().unwrap();
        let mark = stand_time(crate::tables::clip(chara, "M_0213")).unwrap()
            + if chara == Chara::Donald { HP.hurt_n_ex001_exit_late * super::down_rate(chara) } else { 0.0 }; // (DON-KIT) his later get-up (5.3)
        assert_eq!(last_damage.clip, "M_0213", "{}", chara.name());
        assert!(last_damage.time + DT >= mark && last_damage.time < mark + 2.0 * DT, "{}: left at {} (mark {mark})", chara.name(), last_damage.time);
        assert_eq!(rig.m().state, NpcStateId::Pad(PAD_RUN), "{}: the stick runs at once", chara.name());
        assert!(rig.m().hurt.log.iter().any(|l| l.0 == "leader_exit"));
        // a light hit: out at M_0201's CheckBadStat
        let mut rig = Rig::new(chara, true, TestWorld::floor());
        rig.hit(Kind::Small, 3.0);
        let mut out = 0;
        for k in 0..300 {
            rig.step(Vec2::new(0.0, 1.0), &[], 1);
            if rig.m().state != NpcStateId::Damage {
                out = k;
                break;
            }
        }
        let mark = stand_time(crate::tables::clip(chara, "M_0201")).unwrap();
        assert!(((out + 1) as f32 * DT - mark).abs() <= 2.0 * DT, "{}: flinch left at {} s (mark {mark})", chara.name(), (out + 1) as f32 * DT);
    }
}

/// AERIAL RECOVERY: B from the hit + 0.2 s while thrown takes the leader out (the kit's move for Donald / Goofy); not
/// before 0.2 s; the shared gate (`recoverable`) is the one the kits read.
#[test]
fn the_leader_recovers_in_the_air_with_b_after_the_kh3_delay() {
    for chara in Chara::ALL {
        let mut rig = Rig::new(chara, true, TestWorld::floor());
        rig.hit(Kind::Launch, 3.0);
        rig.step(Vec2::ZERO, &[], 3);
        // too early: B does nothing for Goofy (3 steps < 0.2 s; Donald's Flutter is his kit's own move: any time in the air)
        if chara == Chara::Goofy {
            rig.step(Vec2::ZERO, &[PadButton::JUMP], 1);
            assert_eq!(rig.m().state, NpcStateId::Damage, "Goofy: before the delay");
        }
        rig.step(Vec2::ZERO, &[], 12);
        assert!(recoverable(rig.m()), "{}: thrown {:?} t {}", chara.name(), rig.m().react.clip(), rig.m().state_time);
        rig.step(Vec2::ZERO, &[PadButton::JUMP], 1);
        assert_ne!(rig.m().state, NpcStateId::Damage, "{}: recovered", chara.name());
        assert!(!rig.m().react.is_active());
    }
}

/// The shared aerial recovery (a character without a kit move): the gate, Sora's AIR_RECOVERY, the release.
#[test]
fn the_shared_aerial_recovery_needs_the_window_and_sora_s_ability() {
    let mut rig = Rig::new(Chara::Goofy, false, TestWorld::floor());
    rig.hit(Kind::Launch, 3.0);
    rig.step(Vec2::ZERO, &[], 2);
    assert!(!pad_air_recovery(rig.mm(), true), "before the 0.2 s");
    rig.step(Vec2::ZERO, &[], 12);
    rig.mm().hurt.air_recovery = false;
    assert!(!pad_air_recovery(rig.mm(), true), "without Sora's AIR_RECOVERY");
    rig.mm().hurt.air_recovery = true;
    assert!(!pad_air_recovery(rig.mm(), false), "without B");
    assert!(recoverable(rig.m()), "{:?}", rig.m().react.clip());
    assert!(pad_air_recovery(rig.mm(), true));
    assert!(!rig.m().react.is_active() && rig.m().cmc.velocity == Vec3::ZERO);
}

// ------------------------------------------------------------------------------------------- KO, sound, effects

/// A LETHAL heavy hit plays its blow; the KO (Die) starts at the down clip's CheckBadStat; nothing hits him meanwhile.
/// A lethal flinch is the KO at once (KH3's Die Begin).
#[test]
fn a_lethal_heavy_hit_plays_the_blow_then_the_ko() {
    for chara in Chara::ALL {
        for leader in [false, true] {
            let mut rig = Rig::new(chara, leader, TestWorld::floor());
            rig.mm().status.hp = 5;
            let r = rig.hit(Kind::Blow, 50.0);
            assert!(r.killed);
            assert_eq!(rig.m().state, NpcStateId::Damage, "{}: the blow first", chara.name());
            assert!(!rig.m().view().ko, "not KO'd yet: the death flow waits for the blow");
            rig.step(Vec2::ZERO, &[], 10);
            assert!(!rig.hit(Kind::Blow, 50.0).accepted, "invincible while the blow plays");
            let mut ko_at = None;
            for k in 0..600 {
                rig.step(Vec2::ZERO, &[], 1);
                if rig.m().state == NpcStateId::Die {
                    ko_at = Some(k);
                    break;
                }
            }
            assert!(ko_at.is_some(), "{}: KO", chara.name());
            let last = rig.trace.iter().rev().find(|r| r.state == NpcStateId::Damage).copied().unwrap();
            let mark = stand_time(crate::tables::clip(chara, "M_0213")).unwrap();
            assert_eq!(last.clip, "M_0213");
            assert!((last.time - mark).abs() <= 2.0 * DT, "{}: KO at {} (mark {mark})", chara.name(), last.time);
            assert!(rig.m().view().ko && rig.longest_hold() <= 1);
        }
        let mut rig = Rig::new(chara, false, TestWorld::floor());
        rig.mm().status.hp = 5;
        rig.hit(Kind::Small, 50.0);
        assert_eq!(rig.m().state, NpcStateId::Die, "a lethal flinch: KO at once");
    }
}

/// SOUND AND EFFECT: the hurt voice is the clips' own PlayVoice notify (KH3's), and KH3's hit effect VS_non_HIT0 spawns
/// at the contact of every hit that takes HP.
#[test]
fn the_hurt_voice_and_the_hit_effect_play() {
    for chara in Chara::ALL {
        for kind in [Kind::Small, Kind::Blow] {
            let mut rig = Rig::new(chara, false, TestWorld::floor());
            rig.audio.0.clear();
            rig.effects.0.clear();
            rig.hit(kind, 3.0);
            rig.step(Vec2::ZERO, &[], 3);
            assert!(rig.effects.0.contains(&IMPACT_EFFECT), "{} {kind:?}: {:?}", chara.name(), rig.effects.0);
            #[cfg(kh_party_anim_full)]
            assert!(!rig.audio.0.is_empty(), "{} {kind:?}: no hurt voice", chara.name());
            let _ = &rig.audio;
        }
    }
}

// ------------------------------------------------------------------------------------------- the data rule, any character

/// The reaction's clip times are the character's own clip lengths (per hit index), the clips by KH3's rows.
#[test]
fn clip_times_and_clips_are_the_characters_own() {
    for chara in Chara::ALL {
        let set = crate::tables::anim_set(chara);
        for index in 0..4u8 {
            let t = clip_times(&set, index, false);
            assert_eq!(t.blow_down, len(chara, m(213 + 3 * index as u32)), "{} {index}", chara.name());
            assert_eq!(t.blow_start, len(chara, m(211 + 3 * index as u32)));
            assert_eq!(t.small_ground, len(chara, m(201 + index as u32)));
            assert_eq!(t.small_air, len(chara, m(207 + index as u32)));
            assert_eq!(pick(&set, ReactClip::BlowLoop, Phase::BlownFlying, index, false).map(|p| (p.clip.name, p.looped)), Some((m(212 + 3 * index as u32), true)));
            assert_eq!(pick(&set, ReactClip::BlowDown, Phase::Down, index, false).map(|p| p.clip.name), Some(m(213 + 3 * index as u32)));
        }
        assert_eq!(clip_times(&set, 0, true).small_ground, len(chara, "M_0205"));
        assert_eq!(pick(&set, ReactClip::WallDown, Phase::Down, 0, false).map(|p| p.clip.name), Some("Land"));
        assert_eq!(pick(&set, ReactClip::WallFall, Phase::BlownFalling, 0, false).map(|p| (p.clip.name, p.looped)), Some(("JumpNormalDownLoop", true)));
        assert!(clip_times(&set, 0, false).spin.is_none(), "no spin clip: a spin is a blow");
    }
}

static FUTURE_DOWN_NOTIFIES: [ClipNotify; 1] = [ClipNotify { class: "TresAnimNotify_CheckBadStat", start: 1.1, end: 1.1, data: ClipNotifyData::Raw("CheckBadStat") }];
/// A future character's anim set: KH3's damage rows with lengths no friend has (and no M_0238 / M_0230 / wall rows).
static FUTURE: [ClipDef; 10] = [
    ClipDef::bare("M_0201", 0.9, false),
    ClipDef::bare("M_0205", 0.95, false),
    ClipDef::bare("M_0207", 0.7, false),
    ClipDef::bare("M_0211", 0.15, false),
    ClipDef::bare("M_0212", 0.4, false),
    ClipDef { notifies: &FUTURE_DOWN_NOTIFIES, ..ClipDef::bare("M_0213", 1.6, false) },
    ClipDef::bare("M_0227", 0.2, false),
    ClipDef::bare("M_0228", 0.5, false),
    ClipDef::bare("Land", 0.4, false),
    ClipDef::bare("JumpNormalDownLoop", 1.0, true),
];

/// The pawn's driver as the pawn runs it (hurt::started / hurt::tick), over a mock set and kh-react on a flat floor,
/// with an anim clock as NpcAnim's (one-shots stand at their end, loops wrap). Returns the trace.
fn drive(set: &dyn ClipSet, kind: Kind, z: f32, dt: f32) -> Vec<(&'static str, f32, bool, bool, Vec3)> {
    let mut react = EnemyReaction::new(kh_react::Body::of(kh_react::BodyClass::Medium));
    react.body.params = crate::actions::friend_damage_params(1.0);
    let req = ReactionRequest::new(kind, Vec3::new(-1.0, 0.0, 0.0));
    let started = react.hit(&req, Pose { position: Vec3::new(0.0, 0.0, z), yaw: 0.0, grounded: z <= 0.0 });
    assert!(matches!(started, kh_react::HitOutcome::Started { .. }));
    react.body.clips = clip_times(set, react.index(), kh3_variant(&react));
    let first = pick(set, react.clip(), react.phase(), react.index(), kh3_variant(&react)).unwrap();
    let (mut clip, mut looped, mut time) = (first.clip, first.looped, 0.0f32);
    let world = FlatWorld::default();
    let mut out = Vec::new();
    for _ in 0..((10.0 / dt) as usize) {
        let o = react.step_dt(&world, dt);
        // the clock
        time += dt;
        let finished = !looped && time >= clip.length;
        if finished {
            time = clip.length;
        } else if looped && time >= clip.length {
            time -= clip.length;
        }
        if o.finished {
            break;
        }
        if let Some(k) = pick(set, react.clip(), o.phase, react.index(), kh3_variant(&react)) {
            if k.clip.name != clip.name {
                (clip, looped, time) = (k.clip, k.looped, 0.0);
            }
        }
        out.push((clip.name, time, finished && !looped, looped, o.position));
    }
    out
}

fn hold(trace: &[(&'static str, f32, bool, bool, Vec3)]) -> usize {
    let (mut best, mut run) = (0, 0);
    for r in trace {
        if r.2 {
            run += 1;
            best = best.max(run);
        } else {
            run = 0;
        }
    }
    best
}

/// A MOCKED FUTURE CHARACTER through the same code path: his own lengths decide every phase, his missing rows fall back
/// to KH3's index-0 / fall-loop rows, no held pose.
#[test]
fn a_future_character_reacts_by_his_own_clips() {
    let set = Clips(&FUTURE);
    let t = clip_times(&set, 2, false);
    assert_eq!((t.small_ground, t.blow_start, t.blow_down, t.toss_down), (0.9, 0.15, 1.6, 1.6), "index 2 falls back to the index-0 rows");
    assert_eq!(t.getup_tail, 0.6, "no IK state: kh-react's default tail");
    assert_eq!(stand_time(set.clip("M_0213").unwrap()), Some(1.1));
    for (kind, z, want) in [(Kind::Blow, 0.0, vec!["M_0211", "M_0212", "M_0213"]), (Kind::Small, 0.0, vec!["M_0201"]), (Kind::Small, 300.0, vec!["M_0207", "JumpNormalDownLoop", "Land"]), (Kind::Launch, 0.0, vec!["M_0227", "M_0228", "M_0212", "M_0213"])] {
        let trace = drive(&set, kind, z, DT);
        let mut clips: Vec<&str> = Vec::new();
        for r in &trace {
            if clips.last() != Some(&r.0) {
                clips.push(r.0);
            }
        }
        assert_eq!(clips, want, "{kind:?} at {z}");
        assert!(hold(&trace) <= 1, "{kind:?}: held {}", hold(&trace));
        // the reaction ends with its last clip
        let last = trace.last().unwrap();
        let l = set.clip(last.0).unwrap().length;
        assert!(last.1 >= l - 2.0 * DT, "{kind:?}: ended {} into {} ({l} s)", last.1, last.0);
    }
}

/// FRAME-RATE INDEPENDENCE: the same throw at 60 Hz and at 120 Hz (kh-react's brakes are `factor^(dt x 60)`).
#[test]
fn the_throw_is_the_same_at_60_and_120_hz() {
    let set = crate::tables::anim_set(Chara::Goofy);
    for kind in [Kind::Blow, Kind::Launch, Kind::Small] {
        let a = drive(&set, kind, 0.0, 1.0 / 60.0);
        let b = drive(&set, kind, 0.0, 1.0 / 120.0);
        let (ea, eb) = (a.last().unwrap().4, b.last().unwrap().4);
        assert!((ea - eb).length() < 0.05 * ea.length().max(100.0), "{kind:?}: 60 Hz ends at {ea:?}, 120 Hz at {eb:?}");
        let (ta, tb) = (a.len() as f32 / 60.0, b.len() as f32 / 120.0);
        assert!((ta - tb).abs() < 2.0 / 60.0, "{kind:?}: {ta} s vs {tb} s");
        let pa = a.iter().map(|r| r.4.z).fold(0.0, f32::max);
        let pb = b.iter().map(|r| r.4.z).fold(0.0, f32::max);
        assert!((pa - pb).abs() < 4.0, "{kind:?}: peak {pa} vs {pb}");
    }
}

/// SORA, THE REFERENCE (kh-core's TresPlayerState_Damage): his blow is 0211 -> 0212 (looped) -> 0213 and no frame of it
/// holds a finished one-shot - the friends now follow the same rule.
#[test]
fn sora_s_own_blow_is_the_reference() {
    use kh_core::{AnimId, Input, Pose as SPose, Sora, StateId, SORA_PARAMS};
    let world = TestWorld::floor();
    let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
    let mut status = kh_core::Status::default();
    (status.hp, status.hp_max) = (100, 100);
    let mut sora = Sora::new(SORA_PARAMS, SPose { position: origin, yaw: 0.0 }).with_tuning(kh_core::Tuning::GAME_EXACT).with_status(status);
    for _ in 0..5 {
        sora.step(DT, &Input::default(), &world);
    }
    sora.take_hit(kh_core::states::HitOnPlayer { direction: Vec3::new(-1.0, 0.0, 0.0), ..kh_core::states::HitOnPlayer::new(Kind::Blow, 1.0, TargetId(7), Vec3::new(100.0, 0.0, 0.0)) });
    let mut names: Vec<AnimId> = Vec::new();
    let mut held = 0;
    for _ in 0..400 {
        let out = sora.step(DT, &Input::default(), &world);
        if out.state != StateId::Damage {
            break;
        }
        if names.last() != Some(&out.anim.name) {
            names.push(out.anim.name);
        }
        if !out.anim.active && !out.anim.looping {
            held += 1;
        }
        if out.anim.name == AnimId::A0212 {
            assert!(out.anim.looping, "Sora's flight loop loops");
        }
    }
    assert_eq!(names[..3], [AnimId::A0211, AnimId::A0212, AnimId::A0213]);
    assert!(held <= 1, "Sora held {held}");
}

