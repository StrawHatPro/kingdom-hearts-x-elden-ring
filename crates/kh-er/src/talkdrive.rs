//! (IMPL_ER36, the user 2026-10-13: "The Talk to Melina option in the save point menu should just talk to Melina and
//! thats it, not open grace menu") A STORY REST DRIVEN TO ITS END: the pure rules. savepoint.rs gives the machine what
//! it sees each frame and does what it says (the cover, the presses for the game); nothing here touches the game.
//! State note: build/sora_tmp/er/IMPL_ER33_STATE.md (addendum IMPL_ER36).
//!
//! HOW HER TALK IS STARTED: BY THE GAME'S OWN MENU ENTRY, selected for the player with the pad the game reads.
//! Not "directly": by the talk script (t000001000, PARSED) the entry's choice runs group x36 = summon Melina (x35:
//! command 128 with character 2180 and talk 3000, flag 4651), set flags 4652 and 4655, wait until her script clears
//! 4652. Setting those flags ourselves while the grace menu is up would start her talk UNDER the open menu (the menu's
//! state keeps waiting for a choice and keeps the pad), and the only way out of that menu is its Leave = the end of the
//! rest. The SDK can run single talk-script commands, but only on a talk object of our own, not on the grace's running
//! machine. So the cursor it is.
//!
//! WHERE THE ENTRY IS (PARSED, t000001000 group x31: the menu is built in this order, and the list shows entries in the
//! order they are added): Pass time 1, Level Up 2, Flasks 3, [50], Memorize spell 4, Physick 5, Sort chest 6, Great
//! Runes 7, Ashes of War 8, [9], [75], then group x52 -> x43 -> x49: "Talk to Melina" 11 (x57), a second talk 12 (x58,
//! only with flag 4699), [32 "Listen to the sounds of flame"], [41 "Begin Journey"], and Leave 99 last. Which of the
//! entries BEFORE hers exist depends on what the player carries (item tests): not derivable from flags. Which exist
//! AFTER hers is: 12 needs flag 4699; 32 and 41 need flags (108 / 11102790) under which x43 adds no entry of hers at
//! all. So hers is the LAST BUT ONE (last but two with 4699), and it is reached from the top with UP: once to wrap to
//! Leave, once more to her. ASSUMED, NOT MEASURED: the cursor starts on the first entry when the menu opens after
//! sitting, and UP on the first entry wraps to the last.
//!
//! WHAT IS READ TO VERIFY (MEASURED in the user's log of 2026-10-13, er/logs/kh-er.log 3834 - 3902 s; inputs.rs
//! `popup_state`): Elden Ring's popup menu has a top job and talk id 1000 = THE GRACE MENU IS UP (1.8 - 2.2 s after the
//! rest begins); after her entry was chosen the top job is gone while the rest goes on, and 4.7 s later the talk id is
//! 3000 (her talk); when her talk ends the grace menu is back (top job, talk id 1000); a choice box of hers (Accept /
//! Refuse) is a top job with talk id 3000. The cursor itself is NOT readable (nothing in the SDK names it), so each
//! cursor step is not verified by itself: the RESULT is (the menu closes without the rest ending = an entry that opens
//! no window = hers; flag 4652 / talk id 3000 confirm it). A wrong entry opens another window (the top job stays) or
//! ends the rest (Leave): both are seen and end the attempt cleanly.

/// The talk id Elden Ring's popup menu carries while the grace's own menu is up (the grace's talk script t000001000).
pub const TALK_GRACE: i32 = 1000;
/// ... and while Melina's talk has it (t000003000).
pub const TALK_MELINA: i32 = 3000;
/// Flags of the talk script: Melina is talking (set when her entry is chosen or she speaks by herself, cleared by her
/// script at the end: PARSED x36 / x50); the agreed warp to the Roundtable Hold (104, until 105).
pub const FLAG_TALKING: u32 = 4652;
pub const FLAG_WARP: u32 = 104;
pub const FLAG_ROUNDTABLE: u32 = 105;

/// CAN THE GAME'S GRACE MENU HOLD HER ENTRY HERE AT ALL (PARSED: x84, x52 state 0, x43 state 4)? `area`: the map area
/// of the player's block (60 = the open world). `Err`: why not.
pub fn menu_gate(flag: &dyn Fn(u32) -> Option<bool>, area: u32) -> Result<(), &'static str> {
    let on = |f: u32| flag(f) == Some(true);
    // x84: map ids 20000000-28999999, 40000000-43999999, 2000000000-2099999999 (the expansion's maps) skip x52 altogether
    if (20..=28).contains(&area) || (40..=43).contains(&area) || area == 61 {
        return Err("the talk script adds no entry of Melina's in this map (the expansion's areas)");
    }
    // x52 state 0: Ranni's entries take the place
    if on(400394) && ((!on(12019280) && (on(12012710) || on(12012711) || on(12012712))) || (on(1034509406) && on(12012713))) {
        return Err("the talk script shows Ranni's entry in the place of Melina's (flag 400394)");
    }
    // x43 state 4
    if !on(953) {
        return Err("flag 953 is off (the accord's scene has not played)");
    }
    if on(11102790) {
        return Err("flag 11102790 is on");
    }
    if on(11009260) && !on(400001) {
        return Err("flag 11009260 is on (she has left)");
    }
    if on(108) {
        return Err("flag 108 is on");
    }
    if on(110) {
        return Err("flag 110 is on (the Erdtree burns: she is gone)");
    }
    Ok(())
}

/// HOW MANY TIMES UP from the first entry to hers: Leave is last, a second talk entry (12, flag 4699) and nothing else
/// can stand between hers and Leave. `override_ups`: the key `story.menu.ups`.
pub fn entry_ups(flag: &dyn Fn(u32) -> Option<bool>, override_ups: Option<u8>) -> u8 {
    override_ups.unwrap_or(2 + u8::from(flag(4699) == Some(true))).clamp(1, 20)
}

/// What the driven rest is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Goal {
    /// A cutscene is expected right after the rest begins (the accord).
    pub scene: bool,
    /// She is expected to speak by herself.
    pub own: bool,
    /// Her entry of the game's menu is to be selected with this many UPs (None: there is none).
    pub ups: Option<u8>,
}

/// What savepoint.rs sees in a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Seen {
    /// The game's rest state (the body sits).
    pub resting: bool,
    pub cutscene: bool,
    /// The grace's own menu is up (top job, talk id 1000) / another window of the game's is (a top job otherwise).
    pub grace_menu: bool,
    pub other_window: bool,
    /// Her talk runs: flag 4652 is on, or the popup's talk id became 3000 during this rest.
    pub talking: bool,
    /// She takes him to the Roundtable Hold: flag 104 on, 105 off (the game's warp and load follow).
    pub warp: bool,
    /// savepoint.rs is still pressing for the rest.
    pub pressing: bool,
}

/// What savepoint.rs is to do in a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    None,
    /// Start asking the game for its rest (the body's action button, repeated by savepoint.rs's own flow).
    Rest,
    Up,
    Confirm,
    Back,
}

/// How it ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    /// Her talk (or the scene and her talk) was reached; the text says how it ended.
    Talked(&'static str),
    /// Clean, but nothing of hers came.
    Nothing(&'static str),
    /// A step failed: the step, for `!!! story:`.
    Failed(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    /// The black cover is up this frame.
    pub cover: bool,
    /// The player's own pad is kept from the game this frame (only our presses reach it).
    pub mask: bool,
    pub act: Act,
    /// A line for the log (a change of stage).
    pub log: Option<&'static str>,
    pub end: Option<End>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stage {
    /// The cover goes up; the press follows `COVER_LEAD` later.
    Up { since: f32 },
    /// The game is asked to rest.
    Pressing { since: f32 },
    /// The body rests, covered: the scene, her own talk or the grace menu is awaited.
    Resting { since: f32 },
    /// The event's cutscene plays (uncovered).
    Scene { since: f32 },
    /// The grace menu is up, covered: UPs, then the confirm, then the result is awaited.
    Select { since: f32, ups: u8, pressed: u8, next_at: f32, confirmed_at: Option<f32>, closed_for: f32 },
    /// Her talk (uncovered; the pad is the game's): until the grace menu is back or the rest ends.
    Talk { since: f32, after_scene: bool, seen: bool },
    /// Covered: B until the body stands.
    Leave { since: f32, presses: u8, next_at: f32, talked: bool, why: &'static str },
    Done,
}

/// THE MACHINE. Every path ends in `Done` with the cover down within the limits below, whatever the game does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drive {
    pub goal: Goal,
    pub stage: Stage,
}

impl Drive {
    /// Seconds the cover is up before the game's button is pressed.
    pub const COVER_LEAD: f32 = 0.2;
    /// Seconds the game has to begin its rest (MEASURED: its own "touch" of 4.7 s comes first at a save point unlocked
    /// by its flag alone).
    pub const PRESS_LIMIT: f32 = 14.0;
    /// Seconds a rest waits, covered, for the scene / her own talk / the grace menu (MEASURED: the menu 1.8 - 2.2 s, the
    /// accord's scene 1.5 s after the rest began; her own talk by the script: 3.5 s of her appearing first).
    pub const REST_LIMIT: f32 = 12.0;
    /// Seconds the grace menu is given to settle before the first cursor step, and between steps.
    pub const MENU_SETTLE: f32 = 0.6;
    pub const STEP_EVERY: f32 = 0.3;
    /// After the confirm: the menu must be closed for this long (no other window, still resting) to count as her entry
    /// taken; a window that is still up this long after the confirm is a wrong entry.
    pub const CLOSED_FOR: f32 = 0.7;
    pub const WRONG_AFTER: f32 = 3.0;
    /// Her talk without the script's flag or talk id ever seen: given up after this (MEASURED: the id came 4.7 s after
    /// the choice).
    pub const TALK_UNSEEN_LIMIT: f32 = 20.0;
    /// The longest talk / scene before the rest is left for the player.
    pub const TALK_LIMIT: f32 = 600.0;
    pub const SCENE_LIMIT: f32 = 600.0;
    /// Leaving: B every `BACK_EVERY` seconds, at most `BACKS` times, `LEAVE_LIMIT` seconds in all.
    pub const BACK_EVERY: f32 = 0.9;
    pub const BACKS: u8 = 7;
    pub const LEAVE_LIMIT: f32 = 10.0;

    pub fn new(goal: Goal) -> Drive {
        Drive { goal, stage: Stage::Up { since: 0.0 } }
    }

    /// The player's stick and buttons are not Sora's: the body rests and the game has the pad (her talk, her choices).
    pub fn holds_pad(&self) -> bool {
        !matches!(self.stage, Stage::Up { .. } | Stage::Pressing { .. } | Stage::Done)
    }

    fn leave(&mut self, talked: bool, why: &'static str) -> Step {
        self.stage = Stage::Leave { since: 0.0, presses: 0, next_at: 0.15, talked, why };
        Step { cover: true, mask: true, act: Act::None, log: Some(why), end: None }
    }

    fn done(&mut self, end: End) -> Step {
        self.stage = Stage::Done;
        Step { cover: false, mask: false, act: Act::None, log: None, end: Some(end) }
    }

    /// ONE FRAME.
    pub fn step(&mut self, dt: f32, seen: Seen) -> Step {
        let hold = |cover: bool, mask: bool| Step { cover, mask, act: Act::None, log: None, end: None };
        match self.stage {
            Stage::Done => hold(false, false),
            Stage::Up { since } => {
                let since = since + dt;
                if since >= Self::COVER_LEAD {
                    self.stage = Stage::Pressing { since: 0.0 };
                    return Step { act: Act::Rest, log: Some("the black cover is up: the game's real rest is asked for"), ..hold(true, false) };
                }
                self.stage = Stage::Up { since };
                hold(true, false)
            }
            Stage::Pressing { since } => {
                let since = since + dt;
                self.stage = Stage::Pressing { since };
                if seen.cutscene {
                    self.stage = Stage::Scene { since: 0.0 };
                    return Step { log: Some("a cutscene began: the cover is lifted"), ..hold(false, false) };
                }
                if seen.resting {
                    self.stage = Stage::Resting { since: 0.0 };
                    return hold(true, true);
                }
                if !seen.pressing || since > Self::PRESS_LIMIT {
                    // (he stands: nothing to leave)
                    return self.done(End::Failed("the rest: the game did not rest (its prompt went, or the presses did not reach it)"));
                }
                hold(true, false)
            }
            Stage::Resting { since } => {
                let since = since + dt;
                self.stage = Stage::Resting { since };
                if seen.cutscene {
                    self.stage = Stage::Scene { since: 0.0 };
                    return Step { log: Some("the scene's first frame: the cover is lifted"), ..hold(false, false) };
                }
                if seen.talking {
                    self.stage = Stage::Talk { since: 0.0, after_scene: false, seen: true };
                    return Step { log: Some("MELINA SPEAKS BY HERSELF: the cover is lifted"), ..hold(false, false) };
                }
                if !seen.resting {
                    return self.done(End::Nothing("the rest ended by itself before anything came"));
                }
                if seen.grace_menu {
                    return match self.goal.ups {
                        Some(ups) => {
                            self.stage = Stage::Select { since: 0.0, ups, pressed: 0, next_at: Self::MENU_SETTLE, confirmed_at: None, closed_for: 0.0 };
                            Step { log: Some("the game's grace menu is up (unseen): her entry is selected for the player"), ..hold(true, true) }
                        }
                        None if self.goal.scene => self.leave(false, "the game's grace menu came up and no scene began: the rest is left"),
                        None => self.leave(false, "the game's grace menu came up, she did not speak by herself and the menu holds no entry of hers: the rest is left"),
                    };
                }
                if since > Self::REST_LIMIT {
                    return self.leave(false, "nothing came of the rest in time (no scene, no talk, no menu): the rest is left");
                }
                hold(true, true)
            }
            Stage::Scene { since } => {
                let since = since + dt;
                self.stage = Stage::Scene { since };
                if !seen.cutscene {
                    if !seen.resting {
                        return self.done(End::Talked("the scene is over and the body stands"));
                    }
                    self.stage = Stage::Talk { since: 0.0, after_scene: true, seen: false };
                    return Step { log: Some("the scene is over: her talk is the game's (its choices take the pad)"), ..hold(false, false) };
                }
                if since > Self::SCENE_LIMIT {
                    return self.done(End::Failed("the scene: it did not end"));
                }
                hold(false, false)
            }
            Stage::Select { since, ups, pressed, next_at, confirmed_at, closed_for } => {
                let since = since + dt;
                if seen.talking {
                    self.stage = Stage::Talk { since: 0.0, after_scene: false, seen: true };
                    return Step { log: Some("HER TALK HAS BEGUN (flag 4652 / talk id 3000): the cover is lifted"), ..hold(false, false) };
                }
                if seen.cutscene {
                    self.stage = Stage::Scene { since: 0.0 };
                    return Step { log: Some("a cutscene began: the cover is lifted"), ..hold(false, false) };
                }
                if !seen.resting {
                    return self.done(End::Failed("the selection: the rest ended (the cursor was on Leave: the menu did not open on its first entry, or UP does not wrap)"));
                }
                match confirmed_at {
                    None => {
                        self.stage = Stage::Select { since, ups, pressed, next_at, confirmed_at, closed_for };
                        if since > Self::MENU_SETTLE + Self::STEP_EVERY * (ups as f32 + 2.0) + 4.0 {
                            return self.leave(false, "the selection: the grace menu went away before the cursor steps were made");
                        }
                        if since < next_at || !seen.grace_menu {
                            return hold(true, true);
                        }
                        if pressed < ups {
                            self.stage = Stage::Select { since, ups, pressed: pressed + 1, next_at: since + Self::STEP_EVERY, confirmed_at, closed_for };
                            return Step { act: Act::Up, ..hold(true, true) };
                        }
                        self.stage = Stage::Select { since, ups, pressed, next_at, confirmed_at: Some(since), closed_for: 0.0 };
                        Step { act: Act::Confirm, ..hold(true, true) }
                    }
                    Some(at) => {
                        let window = seen.grace_menu || seen.other_window;
                        let closed_for = if window { 0.0 } else { closed_for + dt };
                        self.stage = Stage::Select { since, ups, pressed, next_at, confirmed_at, closed_for };
                        if closed_for >= Self::CLOSED_FOR {
                            // (the menu closed and the body still rests: only her entry does that. She appears now.)
                            self.stage = Stage::Talk { since: 0.0, after_scene: false, seen: false };
                            return Step { log: Some("the grace menu closed and the body still rests = her entry was taken: the cover is lifted (she appears)"), ..hold(false, false) };
                        }
                        if since - at > Self::WRONG_AFTER && window {
                            return self.leave(false, "the selection: a window of the game's is still up after the confirm = ANOTHER ENTRY was under the cursor: backed out and left");
                        }
                        hold(true, true)
                    }
                }
            }
            Stage::Talk { since, after_scene, seen: was } => {
                let since = since + dt;
                let now = was || seen.talking;
                self.stage = Stage::Talk { since, after_scene, seen: now };
                if seen.warp {
                    return self.done(End::Talked("she takes him to the Roundtable Hold: the game's own warp follows (nothing is pressed)"));
                }
                if since > Self::TALK_LIMIT {
                    return self.leave(now, "her talk: it did not end in time: the rest is left");
                }
                if seen.cutscene {
                    return hold(false, false);
                }
                if !seen.resting {
                    return self.done(End::Talked("the rest ended by itself (her script ends it)"));
                }
                if seen.grace_menu {
                    return self.leave(now || !after_scene, "her talk is over and the game's grace menu is back: covered at once, the rest is left for the player");
                }
                if !now && !after_scene && since > Self::TALK_UNSEEN_LIMIT {
                    return self.leave(false, "her talk: neither flag 4652 nor talk id 3000 was seen after the entry was taken: the rest is left");
                }
                hold(false, false)
            }
            Stage::Leave { since, presses, next_at, talked, why } => {
                let since = since + dt;
                let end = |talked: bool| if talked { End::Talked("the rest was left for the player after her talk") } else if why.starts_with("the selection") || why.starts_with("her talk: neither") { End::Failed(why) } else { End::Nothing(why) };
                if !seen.resting {
                    return self.done(end(talked));
                }
                if seen.warp {
                    return self.done(End::Talked("she takes him to the Roundtable Hold: the game's own warp follows"));
                }
                if seen.cutscene {
                    self.stage = Stage::Scene { since: 0.0 };
                    return hold(false, false);
                }
                if presses >= Self::BACKS || since > Self::LEAVE_LIMIT {
                    // (the last resort: the cover never stays; what is then on screen is the game's)
                    return self.done(End::Failed("leaving the rest: the body still sits after every B (the cover is lifted all the same; B leaves the game's menu)"));
                }
                if since >= next_at {
                    self.stage = Stage::Leave { since, presses: presses + 1, next_at: since + Self::BACK_EVERY, talked, why };
                    return Step { act: Act::Back, ..hold(true, true) };
                }
                self.stage = Stage::Leave { since, presses, next_at, talked, why };
                hold(true, true)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;
    const REST: Seen = Seen { resting: true, cutscene: false, grace_menu: false, other_window: false, talking: false, warp: false, pressing: false };
    const MENU: Seen = Seen { grace_menu: true, ..REST };
    const STAND: Seen = Seen { resting: false, ..REST };

    fn flags(on: &[u32]) -> impl Fn(u32) -> Option<bool> + '_ {
        move |f| Some(on.contains(&f))
    }

    /// Runs the machine against a script of (what is seen, for how many seconds); returns every step that was not a
    /// plain hold, with its time, and whether the cover was up in each second.
    fn run(drive: &mut Drive, script: &[(Seen, f32)]) -> (Vec<(f32, Act, Option<End>)>, Vec<bool>) {
        let (mut out, mut covers, mut t) = (Vec::new(), Vec::new(), 0.0f32);
        for (seen, seconds) in script {
            for _ in 0..(*seconds / DT).round() as usize {
                let s = drive.step(DT, *seen);
                t += DT;
                covers.push(s.cover);
                if s.act != Act::None || s.end.is_some() {
                    out.push((t, s.act, s.end));
                }
            }
        }
        (out, covers)
    }

    #[test]
    fn her_entry_is_the_last_but_one_and_the_gates_are_the_scripts() {
        // two UPs: the first wraps to Leave, the second is hers; three with the second talk entry (flag 4699)
        assert_eq!(entry_ups(&flags(&[4680, 953]), None), 2);
        assert_eq!(entry_ups(&flags(&[4699]), None), 3);
        assert_eq!(entry_ups(&flags(&[]), Some(5)), 5, "story.menu.ups");
        assert_eq!(entry_ups(&flags(&[]), Some(0)), 1);
        // the gates (x84, x52, x43)
        let base = [953, 4680];
        assert_eq!(menu_gate(&flags(&base), 60), Ok(()));
        assert_eq!(menu_gate(&flags(&base), 10), Ok(()), "Stormveil");
        assert!(menu_gate(&flags(&base), 61).is_err() && menu_gate(&flags(&base), 21).is_err() && menu_gate(&flags(&base), 41).is_err(), "the expansion's maps");
        assert!(menu_gate(&flags(&[4680]), 60).is_err(), "before the accord's scene");
        assert!(menu_gate(&flags(&[953, 110]), 60).is_err() && menu_gate(&flags(&[953, 108]), 60).is_err() && menu_gate(&flags(&[953, 11102790]), 60).is_err());
        assert!(menu_gate(&flags(&[953, 11009260]), 60).is_err());
        assert!(menu_gate(&flags(&[953, 11009260, 400001]), 60).is_ok());
        assert!(menu_gate(&flags(&[953, 400394, 12012711]), 60).is_err(), "Ranni's entry");
        assert!(menu_gate(&flags(&[953, 400394, 12012711, 12019280]), 60).is_ok());
        // an unreadable flag counts as off for a veto and as off for the need: no entry
        assert!(menu_gate(&|_| None, 60).is_err());
    }

    /// THE USER'S SESSION, driven: the rest (the game's touch first), the grace menu 2.2 s in, two UPs and the confirm,
    /// the menu closes, she appears, talk id 3000 4.7 s later, 24 s of talk, the grace menu is back, B, he stands.
    #[test]
    fn talk_to_melina_is_driven_from_the_cover_to_standing_again() {
        let mut d = Drive::new(Goal { scene: false, own: false, ups: Some(2) });
        let pressing = Seen { resting: false, pressing: true, ..REST };
        let (steps, covers) = run(&mut d, &[(pressing, 5.5), (REST, 2.2), (MENU, 1.6)]);
        let acts: Vec<Act> = steps.iter().map(|s| s.1).collect();
        assert_eq!(acts, [Act::Rest, Act::Up, Act::Up, Act::Confirm], "{steps:?}");
        assert!((steps[0].0 - Drive::COVER_LEAD).abs() < 0.03, "the press comes after the cover");
        // (the cursor steps: not before the menu had settled, 0.3 s apart)
        let menu_at = 5.5 + 2.2;
        assert!(steps[1].0 - menu_at >= Drive::MENU_SETTLE - 0.02 && (steps[2].0 - steps[1].0 - Drive::STEP_EVERY).abs() < 0.03 && (steps[3].0 - steps[2].0 - Drive::STEP_EVERY).abs() < 0.03, "{steps:?}");
        assert!(covers.iter().all(|c| *c), "black from the first frame through the selection");
        assert!(matches!(d.stage, Stage::Select { confirmed_at: Some(_), .. }));
        // the menu closes, the rest goes on: hers. The cover is lifted 0.7 s later; she talks; the player's choices are his
        let (steps, covers) = run(&mut d, &[(REST, 1.0)]);
        assert!(steps.is_empty());
        assert!(matches!(d.stage, Stage::Talk { seen: false, .. }));
        assert!(!covers[covers.len() - 1] && covers[30], "{:?}", covers.iter().position(|c| !*c));
        let talking = Seen { talking: true, ..REST };
        let choice = Seen { talking: true, other_window: true, ..REST };
        let (steps, covers) = run(&mut d, &[(REST, 3.7), (talking, 15.0), (choice, 4.0), (talking, 5.0)]);
        assert!(steps.is_empty() && covers.iter().all(|c| !*c), "nothing is pressed and nothing covered while she talks");
        assert!(d.holds_pad());
        // her talk ends: the grace menu is back -> covered IN THAT FRAME, B, he stands, the cover is down
        let first = d.step(DT, MENU);
        assert!(first.cover && first.mask, "{first:?}");
        let (steps, covers) = run(&mut d, &[(MENU, 0.5), (STAND, 0.1)]);
        assert_eq!(steps.iter().map(|s| s.1).collect::<Vec<_>>(), [Act::Back, Act::None]);
        assert_eq!(steps[1].2, Some(End::Talked("the rest was left for the player after her talk")));
        assert!(covers[0] && !covers[covers.len() - 1]);
        assert_eq!(d.stage, Stage::Done);
        assert!(!d.holds_pad());
    }

    #[test]
    fn she_speaks_by_herself_or_the_rest_is_left_unseen() {
        // kind `self`: she speaks 4 s into the rest, before any menu: lifted, talk, the menu, left
        let mut d = Drive::new(Goal { scene: false, own: true, ups: None });
        let talking = Seen { talking: true, ..REST };
        let (steps, _) = run(&mut d, &[(Seen { resting: false, pressing: true, ..REST }, 0.5), (REST, 4.0), (talking, 20.0), (MENU, 0.4), (STAND, 0.1)]);
        assert_eq!(steps.iter().map(|s| s.1).collect::<Vec<_>>(), [Act::Rest, Act::Back, Act::None]);
        assert!(matches!(steps[2].2, Some(End::Talked(_))));
        // THE USER'S FIVE RESTS: the grace menu comes 2.2 s in and she has not spoken; no entry of hers: left at once,
        // never uncovered while the menu is up
        let mut d = Drive::new(Goal { scene: false, own: true, ups: None });
        let (steps, covers) = run(&mut d, &[(Seen { resting: false, pressing: true, ..REST }, 0.5), (REST, 2.2), (MENU, 0.6), (STAND, 0.1)]);
        assert_eq!(steps.iter().map(|s| s.1).collect::<Vec<_>>(), [Act::Rest, Act::Back, Act::None]);
        assert!(matches!(steps[2].2, Some(End::Nothing(_))), "{steps:?}");
        assert!(covers[..covers.len() - 6].iter().all(|c| *c) && !covers[covers.len() - 1]);
        // ... and with an entry of hers in the menu, that entry is taken instead
        let mut d = Drive::new(Goal { scene: false, own: true, ups: Some(2) });
        let (steps, _) = run(&mut d, &[(Seen { resting: false, pressing: true, ..REST }, 0.5), (REST, 2.2), (MENU, 1.6)]);
        assert_eq!(steps.iter().map(|s| s.1).collect::<Vec<_>>(), [Act::Rest, Act::Up, Act::Up, Act::Confirm]);
    }

    #[test]
    fn the_accord_is_scene_then_her_talk_then_the_rest_ends_by_itself() {
        // MEASURED (the user's log 1128 - 1243 s): rest, the scene 1.5 s in, her talk, her choice box, the rest ends itself
        let mut d = Drive::new(Goal { scene: true, own: false, ups: None });
        let scene = Seen { cutscene: true, ..REST };
        let (steps, covers) = run(&mut d, &[(Seen { resting: false, pressing: true, ..REST }, 6.3), (REST, 1.5), (scene, 14.0), (REST, 2.5), (Seen { talking: true, ..REST }, 43.0), (Seen { other_window: true, talking: true, ..REST }, 9.0), (REST, 38.0), (STAND, 0.1)]);
        assert_eq!(steps.iter().map(|s| s.1).collect::<Vec<_>>(), [Act::Rest, Act::None], "nothing but the rest is ever pressed: {steps:?}");
        assert_eq!(steps[1].2, Some(End::Talked("the rest ended by itself (her script ends it)")));
        let scene_at = ((6.3 + 1.5) / DT) as usize;
        assert!(covers[..scene_at - 2].iter().all(|c| *c) && covers[scene_at + 2..].iter().all(|c| !*c), "covered until the scene's first frame, never after");
        // the scene did not come (the menu did): left, covered
        let mut d = Drive::new(Goal { scene: true, own: false, ups: None });
        let (steps, _) = run(&mut d, &[(Seen { resting: false, pressing: true, ..REST }, 0.5), (REST, 2.0), (MENU, 0.5), (STAND, 0.1)]);
        assert!(matches!(steps.last().unwrap().2, Some(End::Nothing(_))));
    }

    #[test]
    fn she_takes_him_to_the_roundtable_hold_and_nothing_fights_the_warp() {
        let mut d = Drive::new(Goal { scene: false, own: true, ups: None });
        let talking = Seen { talking: true, ..REST };
        let (steps, covers) = run(&mut d, &[(Seen { resting: false, pressing: true, ..REST }, 0.5), (REST, 4.0), (talking, 30.0), (Seen { warp: true, ..talking }, 0.1)]);
        assert_eq!(steps.iter().map(|s| s.1).collect::<Vec<_>>(), [Act::Rest, Act::None]);
        assert!(matches!(steps[1].2, Some(End::Talked(t)) if t.contains("Roundtable")));
        assert!(!covers[covers.len() - 1]);
        assert_eq!(d.stage, Stage::Done);
        // also while leaving (the flag comes as the menu returns): no further B
        let mut d = Drive { goal: Goal { scene: false, own: true, ups: None }, stage: Stage::Leave { since: 0.0, presses: 0, next_at: 5.0, talked: true, why: "x" } };
        let s = d.step(DT, Seen { warp: true, grace_menu: true, ..REST });
        assert!(s.act == Act::None && !s.cover && matches!(s.end, Some(End::Talked(_))));
    }

    #[test]
    fn a_wrong_entry_and_every_stuck_step_end_standing_with_the_cover_down() {
        // the cursor was not where assumed: after the confirm another window is up (Pass time's list): backed out and left
        let mut d = Drive::new(Goal { scene: false, own: false, ups: Some(2) });
        let (steps, covers) = run(&mut d, &[(Seen { resting: false, pressing: true, ..REST }, 0.5), (REST, 2.0), (MENU, 1.6), (Seen { other_window: true, ..REST }, 3.4), (MENU, 1.0), (STAND, 0.1)]);
        let acts: Vec<Act> = steps.iter().map(|s| s.1).collect();
        assert_eq!(&acts[..4], [Act::Rest, Act::Up, Act::Up, Act::Confirm]);
        assert!(acts[4..acts.len() - 1].iter().all(|a| *a == Act::Back) && acts.len() >= 7, "B closes the wrong window, B leaves: {acts:?}");
        assert!(matches!(steps.last().unwrap().2, Some(End::Failed(t)) if t.contains("ANOTHER ENTRY")));
        assert!(covers[..covers.len() - 6].iter().all(|c| *c), "never uncovered while a menu of the game's is up");
        // UP did not wrap / the cursor was on Leave: the rest ends at the confirm: said, standing, uncovered
        let mut d = Drive::new(Goal { scene: false, own: false, ups: Some(2) });
        let (steps, _) = run(&mut d, &[(Seen { resting: false, pressing: true, ..REST }, 0.5), (REST, 2.0), (MENU, 1.6), (STAND, 0.1)]);
        assert!(matches!(steps.last().unwrap().2, Some(End::Failed(t)) if t.contains("Leave")));
        // EVERY STUCK WORLD: whatever is seen for ever, from any reachable stage, the machine is Done with the cover down
        // within the limits, and never presses more than the limits allow
        let worlds = [
            Seen { resting: false, pressing: true, ..REST },
            Seen { resting: false, pressing: false, ..REST },
            REST,
            MENU,
            Seen { other_window: true, ..REST },
            Seen { cutscene: true, ..REST },
            Seen { talking: true, ..REST },
            Seen { talking: true, grace_menu: true, ..REST },
        ];
        for goal in [Goal { scene: true, own: false, ups: None }, Goal { scene: false, own: true, ups: None }, Goal { scene: false, own: false, ups: Some(2) }, Goal { scene: false, own: true, ups: Some(3) }] {
            for (a, first) in worlds.iter().enumerate() {
                for (b, second) in worlds.iter().enumerate() {
                    for lead in [0.5f32, 3.0, 9.0] {
                        let mut d = Drive::new(goal);
                        let mut presses = 0;
                        let mut last = None;
                        let limit = Drive::TALK_LIMIT + Drive::SCENE_LIMIT + 60.0;
                        let mut t = 0.0;
                        while d.stage != Stage::Done && t < limit {
                            let seen = if t < lead { *first } else { *second };
                            // (a body that is said to stand cannot be sitting in a menu: the worlds are taken as given)
                            let s = d.step(DT, seen);
                            presses += usize::from(s.act != Act::None);
                            last = Some(s);
                            t += DT;
                        }
                        assert_eq!(d.stage, Stage::Done, "goal {goal:?} worlds {a} -> {b} lead {lead}: stuck in {:?}", d.stage);
                        let last = last.unwrap();
                        assert!(!last.cover && last.end.is_some(), "goal {goal:?} worlds {a} -> {b}: {last:?}");
                        assert!(presses <= 1 + 20 + 1 + Drive::BACKS as usize, "{presses} presses");
                        // after Done: nothing, for ever
                        let after = d.step(DT, *second);
                        assert!(!after.cover && !after.mask && after.act == Act::None && after.end.is_none());
                    }
                }
            }
        }
    }

    #[test]
    fn the_cover_is_never_up_longer_than_the_steps_limits() {
        // the longest covered stretch of any single path: press limit + rest limit + selection + leaving
        let mut d = Drive::new(Goal { scene: false, own: false, ups: Some(2) });
        let (_, covers) = run(&mut d, &[(Seen { resting: false, pressing: true, ..REST }, 13.9), (REST, 11.9), (MENU, 60.0)]);
        let longest = covers.split(|c| !*c).map(|run| run.len()).max().unwrap_or(0) as f32 * DT;
        assert!(longest < Drive::PRESS_LIMIT + Drive::REST_LIMIT + 8.0 + Drive::LEAVE_LIMIT + 1.0, "{longest}");
        assert_eq!(d.stage, Stage::Done);
        assert!(!covers[covers.len() - 1]);
    }
}
