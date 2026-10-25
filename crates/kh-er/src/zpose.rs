//! (ZETTA 2026-10-23) ZETTAFLARE'S TARGET IN ITS HEAVY HURT, FROZEN AT ITS MIDDLE. The user: "please make sure the enemy hurt
//! animations are their heavy hurt animations, and it should be paused around the middle of the animation so they are visibly
//! hurt"; "could the fast forward happen while Donald is charging so it is never actually visible?"; "sounds wont play from
//! the fast forward will they?".
//!
//! Generic for every enemy (no per-enemy table): the game is asked for its HEAVIEST standing damage reaction through the
//! existing route (a 1-point bullet carrying the `dmgLevel`, `ErTargets::ask_animation`), [`HEAVY_LEVELS`] in order:
//! Large (3: the soldier's 8050, 2.3 s, the heavier stagger, no travel), then Middle (2), Small (1), Minimum (8); a level
//! that starts no damage animation within [`ASK_WAIT_S`] gives way to the next. The clip it starts is run to
//! [`FREEZE_SHARE`] of its length and held there (animation speed 0) for the whole hold.
//!
//! HOW IT GETS TO ITS MIDDLE: the pinned SDK has no seek: the TimeAct queue's `play_time` is the game's record of the
//! clip, not the Havok player's clock (writing it moves nothing), so it is FAST-FORWARDED (the character's animation speed
//! raised so the middle is reached in two or three frames, [`forward_speed`]), then frozen. A real seek (the Havok
//! animation control's local time) is future work (reverse engineering). It is done in the PRE-POSE WINDOW
//! (kh-party `zettaflare::prepose_window`: Donald's last shot before the hit with the target out of frame, else the last
//! frames before the hit's cut), so the jump is never seen; when there is no cinematic camera it is done at the hit.
//!
//! PAST ITS ARMOUR (the user, 2026-10-23: "Margit also didn't look like he was playing any kind of hurt animation"; the log: Margit, super armour 80 / 80, played nothing for any level): every ask breaks the
//! body's armour first: its super armour (`CSChrSuperArmorModule::sa_durability`) and its toughness (stance) set to 0, and
//! the asking bullet carries more poise damage than any body has ([`FORCE_POISE`]), so the game plays the reaction of the
//! level asked whatever its poise (an armoured boss as a soldier); the armour as it was is put back when the pose is let
//! go (a survivor fights on with it). IF NOTHING STARTS (no level plays a damage animation): the body's own animation is
//! frozen where it is at once ([`Do::FreezeAsIs`]): it never idles or attacks on through the sequence.
//!
//! SILENT: while it is asked, fast-forwarded and frozen, the character's sounds are switched off every frame (the game's
//! own `ChrInsFlags1ca::sounds_active`, the switch its distance rule drives), so neither the skipped half's sound events nor
//! a looped sound of the clip play; a frozen clip fires no further events (its time stands still). The TimeAct events of the
//! skip are not counted one by one (the SDK has no event list): the log gives the clip span skipped and the frames muted.

use crate::chranim::AnimNow;

/// The `dmgLevel`s asked, heaviest standing reaction first (IMPL_ER6's survey on the Godrick soldier c4311: 3 Large 8050
/// 2.30 s "heavier stagger, no travel", 2 Middle 8040 1.87 s, 1 Small, 8 Minimum 8190 0.5 s twitch). Blow levels (4, 7, 10)
/// throw the body off its feet: never asked here.
pub const HEAVY_LEVELS: [u8; 4] = [3, 2, 1, 8];

/// Where it is frozen: this share of the clip's length (its most visibly hurt pose).
pub const FREEZE_SHARE: f32 = 0.5;

/// The fastest the fast-forward runs the clip.
pub const FORWARD_SPEED_MAX: f32 = 32.0;

/// How long a level may take to start its damage animation before the next is asked (s).
pub const ASK_WAIT_S: f32 = 0.35;

/// The asking bullet's poise damage: more than any body's super armour, so its armour never stops the reaction.
pub const FORCE_POISE: f32 = 100_000.0;

/// (the user, 2026-10-23: "Margit also didn't look like he was playing any kind of hurt animation") What one ask is: the level, and its armour broken first (always: an armoured boss reacts as a soldier).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ask {
    pub level: u8,
    pub break_armour: bool,
}

/// The `k`th ask of the order (None: none left: the pose is frozen as it is).
pub fn ask_of(k: usize) -> Option<Ask> {
    HEAVY_LEVELS.get(k).map(|l| Ask { level: *l, break_armour: true })
}

/// Close enough to the middle to freeze (s of the clip).
pub const FREEZE_TOLERANCE_S: f32 = 1.0 / 60.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stage {
    /// `HEAVY_LEVELS[level]` asked `waited` s ago; `before`: the damage animation the body had then.
    Asked { level: usize, waited: f32, before: Option<AnimNow> },
    /// Its clip runs to the middle.
    Forward { level: usize, anim: AnimNow, frames: u32, from: f32 },
    /// Held at `at` s of `anim`.
    Frozen { level: usize, anim: AnimNow, at: f32 },
    /// No level started a damage animation: left as it is.
    GaveUp,
}

/// What the host does this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Do {
    Nothing,
    /// Ask the game for this `dmgLevel` (a 1-point bullet).
    Ask(u8),
    /// Run the clip at this animation speed.
    Speed(f32),
    /// Hold the pose (animation speed 0). `first`: it froze this frame.
    Freeze { first: bool },
    /// No level started a damage animation: its own animation frozen where it is (never idling or attacking on).
    FreezeAsIs,
}

/// The speed that brings the clip from `time` to its middle (`length` x [`FREEZE_SHARE`]) in about one frame of `dt`, at
/// most [`FORWARD_SPEED_MAX`], at least 1. None: there (freeze).
pub fn forward_speed(time: f32, length: f32, dt: f32) -> Option<f32> {
    let left = length * FREEZE_SHARE - time;
    if left <= FREEZE_TOLERANCE_S {
        return None;
    }
    Some((left / dt.max(1e-3)).clamp(1.0, FORWARD_SPEED_MAX))
}

/// Is `now` a damage animation started since `before`?
fn started(now: Option<AnimNow>, before: Option<AnimNow>) -> Option<AnimNow> {
    let a = now?;
    match before {
        Some(b) if b.id == a.id && a.time + 0.05 >= b.time => None,
        _ => Some(a),
    }
}

/// One frame of the pose: `now` the body's newest damage animation (puppet.rs `damage_anim`), `dt` the frame's seconds.
pub fn step(stage: &mut Stage, now: Option<AnimNow>, dt: f32) -> Do {
    match *stage {
        Stage::Asked { level, waited, before } => {
            if let Some(a) = started(now, before) {
                *stage = Stage::Forward { level, anim: a, frames: 0, from: a.time };
                return step(stage, now, dt);
            }
            let waited = waited + dt;
            if waited < ASK_WAIT_S {
                *stage = Stage::Asked { level, waited, before };
                return Do::Nothing;
            }
            if level + 1 < HEAVY_LEVELS.len() {
                *stage = Stage::Asked { level: level + 1, waited: 0.0, before: now.or(before) };
                Do::Ask(HEAVY_LEVELS[level + 1])
            } else {
                *stage = Stage::GaveUp;
                Do::FreezeAsIs
            }
        }
        Stage::Forward { level, anim, frames, from } => {
            // the clip as it plays now (another one taking over: held where it is)
            let a = match now {
                Some(a) if a.id == anim.id => a,
                _ => {
                    *stage = Stage::Frozen { level, anim, at: anim.time };
                    return Do::Freeze { first: true };
                }
            };
            match forward_speed(a.time, a.length, dt) {
                Some(v) => {
                    *stage = Stage::Forward { level, anim: a, frames: frames + 1, from };
                    Do::Speed(v)
                }
                None => {
                    *stage = Stage::Frozen { level, anim: a, at: a.time };
                    Do::Freeze { first: true }
                }
            }
        }
        Stage::Frozen { .. } => Do::Freeze { first: false },
        Stage::GaveUp => Do::Freeze { first: false },
    }
}

/// The first level asked.
pub fn first_level() -> u8 {
    HEAVY_LEVELS[0]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anim(id: i32, time: f32, length: f32) -> Option<AnimNow> {
        Some(AnimNow { id, time, length })
    }

    /// (2026-10-23, the user: "please make sure the enemy hurt animations are their heavy hurt animations") the heaviest
    /// standing level first; a level that plays nothing gives way to the next; none at all: left as it is
    #[test]
    fn zpose_the_heaviest_available_level_is_chosen() {
        assert_eq!(first_level(), 3, "Large first (the heavier stagger, no travel)");
        assert!(!HEAVY_LEVELS.iter().any(|l| [4, 7, 10].contains(l)), "no level that throws the body");
        let dt = 1.0 / 60.0;
        // level 3 plays nothing on this body: after ASK_WAIT_S, level 2 is asked; it starts 8040
        let mut s = Stage::Asked { level: 0, waited: 0.0, before: anim(8030, 0.9, 1.17) };
        let mut asked = Vec::new();
        for _ in 0..50 {
            if let Do::Ask(l) = step(&mut s, anim(8030, 0.9, 1.17), dt) {
                asked.push(l);
            }
        }
        assert_eq!(asked, vec![2, 1], "the next levels down, in order");
        let mut s = Stage::Asked { level: 1, waited: 0.0, before: None };
        assert!(matches!(step(&mut s, anim(8040, 0.0, 1.87), dt), Do::Speed(_)));
        assert!(matches!(s, Stage::Forward { level: 1, .. }), "level 2 (Middle) chosen: {s:?}");
        // nothing anywhere
        let mut s = Stage::Asked { level: 3, waited: ASK_WAIT_S, before: None };
        assert_eq!(step(&mut s, None, dt), Do::FreezeAsIs, "nothing started: frozen as it is");
        assert_eq!(step(&mut s, None, dt), Do::Freeze { first: false }, "held so");
    }

    /// (the user, 2026-10-23: "Margit also didn't look like he was playing any kind of hurt animation") AN ARMOURED BOSS (Margit: super armour 80 / 80, no reaction to a 1-point bullet): every ask breaks its armour
    /// first (the asking bullet's poise damage past any body's), heaviest level first, then the next ones down; when none plays,
    /// its own animation is frozen as it is (never idling or attacking on)
    #[test]
    fn zpose_an_armoured_boss_is_forced_then_frozen_as_it_is() {
        let order: Vec<Ask> = (0..8).map_while(ask_of).collect();
        assert_eq!(order.iter().map(|a| a.level).collect::<Vec<_>>(), vec![3, 2, 1, 8], "heaviest first, then down");
        assert!(order.iter().all(|a| a.break_armour), "its armour broken for every ask");
        assert!(FORCE_POISE > 80.0 * 100.0, "past any poise (Margit 80)");
        // the boss plays nothing for any level: frozen as it is, then held
        let dt = 1.0 / 60.0;
        let mut s = Stage::Asked { level: 0, waited: 0.0, before: anim(2000020, 0.4, 2.0) };
        let mut seen = Vec::new();
        for _ in 0..120 {
            let d = step(&mut s, anim(2000020, 0.4, 2.0), dt);
            if !matches!(d, Do::Nothing) && !seen.contains(&d) {
                seen.push(d);
            }
        }
        assert_eq!(seen, vec![Do::Ask(2), Do::Ask(1), Do::Ask(8), Do::FreezeAsIs, Do::Freeze { first: false }]);
        // the same boss with its armour broken plays its stagger: forward to the middle, frozen
        let mut s = Stage::Asked { level: 0, waited: 0.0, before: anim(2000020, 0.4, 2.0) };
        assert!(matches!(step(&mut s, anim(2008050, 0.0, 2.3), dt), Do::Speed(_)));
    }

    /// (2026-10-23, the user: "it should be paused around the middle of the animation"; "could the fast forward happen while
    /// Donald is charging") the clip is run to its middle in a few frames (never past it), then held there
    #[test]
    fn zpose_fast_forward_to_the_middle_then_freeze() {
        let dt = 1.0 / 60.0;
        let length = 2.30;
        let mut s = Stage::Asked { level: 0, waited: 0.0, before: None };
        let (mut time, mut frames) = (0.0f32, 0);
        loop {
            match step(&mut s, anim(8050, time, length), dt) {
                Do::Speed(v) => {
                    assert!(v <= FORWARD_SPEED_MAX && v >= 1.0);
                    time += v * dt;
                    frames += 1;
                    assert!(time <= length * FREEZE_SHARE + FREEZE_TOLERANCE_S + 1e-4, "never past the middle ({time})");
                }
                Do::Freeze { first } => {
                    assert!(first);
                    break;
                }
                other => panic!("{other:?}"),
            }
            assert!(frames < 10);
        }
        assert!(frames <= 3, "in {frames} frames");
        assert!((time - length * 0.5).abs() <= FREEZE_TOLERANCE_S + 1e-4, "frozen at {time} of {length}");
        assert_eq!(step(&mut s, anim(8050, time, length), dt), Do::Freeze { first: false }, "held for the whole hold");
        assert!(forward_speed(1.15, 2.3, dt).is_none() && forward_speed(0.0, 2.3, dt) == Some(FORWARD_SPEED_MAX));
    }
}
