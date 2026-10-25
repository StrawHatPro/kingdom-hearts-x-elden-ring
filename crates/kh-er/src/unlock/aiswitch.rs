//! (ER boss) THE REAL BOSS'S AI SWITCH, the game-free rule (the user, 2026-10-15: "Find the real boss' ai switch";
//! binding: "The boss should not be paused. Load the boss and the character the boss is fighting at the same time and
//! make them fight though both characters do 0 damage to each other until the player enters the arena").
//!
//! A staged boss's own event holds its AI off until the fight starts (m60_43_30_00 EVENT 1043302810, m10 EVENT
//! 10002860: SetCharacterAIState(boss, off) when the map loads; on, with SetNetworkUpdateRate and the boss bar, when the
//! player is through the fog). The instruction's whole effect is one bit (`crate::sdkx`, IMPL_BOSS_STATE.md), so the
//! REAL boss is woken by writing that bit, not by running the event: no boss bar, no fog, no music (those are other
//! instructions, tied to the fog flags). The update rate the fight start writes is written too, so the boss fights at
//! full rate while the player watches from afar.
//!
//! - `hold` every frame while staged: the first frame keeps what was there and writes the switch on (and the rate);
//!   every later frame re-asserts both, and each time the game had put them back (its event, a reload) is a FIGHT,
//!   counted and reported. A write that does not read back at once is REFUSED (the host's logged fallback decides).
//! - `hand_over` when the player is in: nothing more is written (the event's own fight start writes the same values);
//!   the fight goes on with the same boss, no swap.
//! - `release` when the staging ends without a fight (the player went away, the rule switched off): what was there is
//!   written back (AI off, its rate), so the boss is again as its event left it.
//! A new character behind the boss's entity (a reload made it again) starts afresh.

/// What the boss's two fields hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fields {
    pub ai_on: bool,
    pub rate: Rate,
}

/// The update rate (ChrIns + 0xB4 mode, + 0xB8 override, + 0x1C4 bit 0 fixed).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rate {
    pub mode: i32,
    pub override_: i32,
    pub fixed: bool,
}

/// What the fight start writes (EMEVD 2004[34], fixed, frequency 0).
pub const FIGHT_RATE: Rate = Rate { mode: 0, override_: -2, fixed: true };

/// The boss's fields as the host reads and writes them.
pub trait BossAi {
    /// None: not readable (no ChrCtrl, not its owner).
    fn read(&self) -> Option<Fields>;
    /// False: not written.
    fn write_ai(&mut self, on: bool) -> bool;
    fn write_rate(&mut self, rate: Rate);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wake {
    /// Not touched.
    Asleep,
    /// Ours: the boss character (its handle) and what was there before.
    Awake { handle: u64, before: Fields, ai_fights: u32, rate_fights: u32 },
    /// The player is in: the event has it (we write nothing).
    HandedOver { handle: u64, before: Fields },
    /// The write did not hold on this character.
    Refused { handle: u64, why: &'static str },
}

/// What happened, for the host's log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Note {
    /// The switch is on: before, after.
    On { before: Fields, after: Fields },
    /// The game had turned it off again (its event / a reset): written on again (the count so far).
    AiFought(u32),
    /// The game had changed the rate: written again (the count so far).
    RateFought(u32),
    Refused(&'static str),
    /// Another character behind the entity (a reload): the old one's state is dropped.
    NewBoss,
    /// The player is in: the event's fight start takes over.
    HandedOver,
    /// Written back as it was.
    Released(Fields),
}

#[derive(Clone, Debug)]
pub struct AiWake {
    pub state: Wake,
}

impl Default for AiWake {
    fn default() -> AiWake {
        AiWake { state: Wake::Asleep }
    }
}

impl AiWake {
    pub fn awake(&self) -> bool {
        matches!(self.state, Wake::Awake { .. })
    }

    pub fn refused(&self) -> bool {
        matches!(self.state, Wake::Refused { .. })
    }

    /// Every frame while staged.
    pub fn hold(&mut self, boss: &mut dyn BossAi, handle: u64) -> Vec<Note> {
        let mut notes = Vec::new();
        let same = match self.state {
            Wake::Asleep => true,
            Wake::Awake { handle: h, .. } | Wake::HandedOver { handle: h, .. } | Wake::Refused { handle: h, .. } => h == handle,
        };
        if !same {
            notes.push(Note::NewBoss);
            self.state = Wake::Asleep;
        }
        let Some(now) = boss.read() else {
            if !self.refused() {
                self.state = Wake::Refused { handle, why: "the boss's ChrCtrl is not readable" };
                notes.push(Note::Refused("the boss's ChrCtrl is not readable"));
            }
            return notes;
        };
        match self.state {
            Wake::Refused { .. } => {}
            Wake::Asleep | Wake::HandedOver { .. } => {
                // (after a hand-over that fell back to staging, `before` is the first one we saw)
                let before = match self.state {
                    Wake::HandedOver { before, .. } => before,
                    _ => now,
                };
                if !boss.write_ai(true) {
                    self.state = Wake::Refused { handle, why: "the AI byte could not be written" };
                    notes.push(Note::Refused("the AI byte could not be written"));
                    return notes;
                }
                boss.write_rate(FIGHT_RATE);
                match boss.read() {
                    Some(after) if after.ai_on => {
                        self.state = Wake::Awake { handle, before, ai_fights: 0, rate_fights: 0 };
                        notes.push(Note::On { before, after });
                    }
                    _ => {
                        self.state = Wake::Refused { handle, why: "the AI switch did not read back on after the write" };
                        notes.push(Note::Refused("the AI switch did not read back on after the write"));
                    }
                }
            }
            Wake::Awake { handle, before, mut ai_fights, mut rate_fights } => {
                if !now.ai_on {
                    ai_fights += 1;
                    boss.write_ai(true);
                    notes.push(Note::AiFought(ai_fights));
                }
                if now.rate != FIGHT_RATE {
                    rate_fights += 1;
                    boss.write_rate(FIGHT_RATE);
                    notes.push(Note::RateFought(rate_fights));
                }
                self.state = Wake::Awake { handle, before, ai_fights, rate_fights };
            }
        }
        notes
    }

    /// The player is in: stop writing (the event's fight start writes the same values).
    pub fn hand_over(&mut self) -> Vec<Note> {
        if let Wake::Awake { handle, before, .. } = self.state {
            self.state = Wake::HandedOver { handle, before };
            return vec![Note::HandedOver];
        }
        Vec::new()
    }

    /// The staging ends without a fight: what was there is written back (only on the same character).
    pub fn release(&mut self, boss: Option<&mut dyn BossAi>, handle: Option<u64>) -> Vec<Note> {
        let mut notes = Vec::new();
        if let Wake::Awake { handle: h, before, .. } = self.state {
            if let (Some(b), Some(now)) = (boss, handle) {
                if now == h {
                    b.write_ai(before.ai_on);
                    b.write_rate(before.rate);
                    notes.push(Note::Released(before));
                }
            }
        }
        self.state = Wake::Asleep;
        notes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A boss as the game holds it: its event writes AI off at load (`load`), and may write it off again.
    struct Fake {
        fields: Fields,
        readable: bool,
        sticky_off: bool,
    }

    const LOADED: Fields = Fields { ai_on: false, rate: Rate { mode: 5, override_: -1, fixed: false } };

    impl BossAi for Fake {
        fn read(&self) -> Option<Fields> {
            self.readable.then_some(self.fields)
        }
        fn write_ai(&mut self, on: bool) -> bool {
            if !self.sticky_off {
                self.fields.ai_on = on;
            }
            self.readable
        }
        fn write_rate(&mut self, rate: Rate) {
            self.fields.rate = rate;
        }
    }

    fn boss() -> Fake {
        Fake { fields: LOADED, readable: true, sticky_off: false }
    }

    #[test]
    fn staging_wakes_the_real_boss_and_keeps_it_awake() {
        let mut b = boss();
        let mut w = AiWake::default();
        let n = w.hold(&mut b, 7);
        assert_eq!(n, vec![Note::On { before: LOADED, after: Fields { ai_on: true, rate: FIGHT_RATE } }]);
        assert!(b.fields.ai_on && b.fields.rate == FIGHT_RATE && w.awake());
        assert_eq!(w.hold(&mut b, 7), vec![], "nothing to say while it holds");
        // its event turns the AI off again (SetCharacterAIState(off)): written on again, counted
        b.fields.ai_on = false;
        assert_eq!(w.hold(&mut b, 7), vec![Note::AiFought(1)]);
        assert!(b.fields.ai_on);
        b.fields.ai_on = false;
        b.fields.rate.mode = 5;
        assert_eq!(w.hold(&mut b, 7), vec![Note::AiFought(2), Note::RateFought(1)]);
        assert!(b.fields.ai_on && b.fields.rate == FIGHT_RATE);
    }

    #[test]
    fn the_player_in_hands_over_with_no_swap_and_no_more_writes() {
        let mut b = boss();
        let mut w = AiWake::default();
        w.hold(&mut b, 7);
        assert_eq!(w.hand_over(), vec![Note::HandedOver]);
        // the event's fight start writes the same values; whatever it does later is its own (no fight reported)
        b.fields.ai_on = false;
        assert_eq!(w.release(None, None), vec![], "a hand-over is not released (nothing written back)");
        assert!(!b.fields.ai_on, "the event's value stays");
    }

    #[test]
    fn walking_away_writes_back_what_the_event_left() {
        let mut b = boss();
        let mut w = AiWake::default();
        w.hold(&mut b, 7);
        assert_eq!(w.release(Some(&mut b), Some(7)), vec![Note::Released(LOADED)]);
        assert_eq!(b.fields, LOADED);
        assert_eq!(w.state, Wake::Asleep);
        // the next approach wakes it again
        assert!(matches!(w.hold(&mut b, 7)[..], [Note::On { .. }]));
    }

    #[test]
    fn a_reloaded_boss_is_woken_afresh_and_the_old_one_is_not_written() {
        let mut b = boss();
        let mut w = AiWake::default();
        w.hold(&mut b, 7);
        // the reload: a new character (handle 9) loaded with its AI off by its event
        let mut b2 = boss();
        let n = w.hold(&mut b2, 9);
        assert_eq!(n[0], Note::NewBoss);
        assert!(matches!(n[1], Note::On { .. }) && b2.fields.ai_on);
        assert_eq!(w.release(Some(&mut b), Some(7)), vec![], "the old character is never written back");
    }

    #[test]
    fn a_write_that_does_not_hold_is_refused_once() {
        let mut b = Fake { sticky_off: true, ..boss() };
        let mut w = AiWake::default();
        assert_eq!(w.hold(&mut b, 7), vec![Note::Refused("the AI switch did not read back on after the write")]);
        assert!(w.refused());
        assert_eq!(w.hold(&mut b, 7), vec![], "said once");
        let mut u = Fake { readable: false, ..boss() };
        let mut w2 = AiWake::default();
        assert_eq!(w2.hold(&mut u, 3), vec![Note::Refused("the boss's ChrCtrl is not readable")]);
        assert_eq!(w2.hold(&mut u, 3), vec![]);
    }

    /// Leonine's whole evening as the host runs it: S8's FightDirector decides, the switch follows its acts, and the
    /// boss's event (m60_43_30_00 EVENT 1043302810) writes its AI off at every load and on at the fight start.
    #[test]
    fn the_event_script_and_the_director_with_the_real_boss() {
        use super::super::rules::{FightAct, FightDirector, FightFacts, FightPhase};
        let near = FightFacts { defeated: Some(false), fog_passed: Some(false), boss_found: true, boss_alive: true, near: true, ..FightFacts::default() };
        let mut d = FightDirector::new(8.0);
        let mut w = AiWake::default();
        let mut b = boss(); // loaded: its event wrote AI off
        let mut handle = 1u64;
        let mut on_frames = 0;
        // the director's acts on the switch (as host.rs fight_step)
        let mut frame = |d: &mut FightDirector, w: &mut AiWake, b: &mut Fake, facts: FightFacts, handle: u64| -> Vec<Note> {
            let mut notes = Vec::new();
            for a in d.step(&facts, 1.0 / 60.0) {
                match a {
                    FightAct::Unstage(_) => notes.extend(w.release(Some(b), Some(handle))),
                    FightAct::Join => notes.extend(w.hand_over()),
                    FightAct::Unlock => *w = AiWake::default(),
                    _ => {}
                }
            }
            if d.phase == FightPhase::Staged {
                notes.extend(w.hold(b, handle));
            }
            notes
        };
        // the approach: staged, woken the first frame, held
        for k in 0..120 {
            let n = frame(&mut d, &mut w, &mut b, near, handle);
            if k == 0 {
                assert_eq!(d.phase, FightPhase::Staged);
                assert!(matches!(n[..], [Note::On { .. }]), "{n:?}");
            }
            on_frames += b.fields.ai_on as u32;
        }
        assert_eq!(on_frames, 120, "awake every frame of the staging");
        // a death and reload: a new character, its event writes AI off again; the director restages it
        frame(&mut d, &mut w, &mut b, FightFacts { leader_down: true, ..near }, handle);
        assert_eq!(d.phase, FightPhase::Away);
        assert_eq!(b.fields, LOADED, "walking off the stage wrote back what the event left");
        handle = 2;
        b = boss();
        let n = frame(&mut d, &mut w, &mut b, near, handle);
        // (the stage was released at the death, so the new character is simply woken: no stale state to drop)
        assert!(matches!(n[..], [Note::On { before: LOADED, .. }]) && b.fields.ai_on, "the reloaded boss is woken afresh: {n:?}");
        // its event's load runs late (after our first write): written off once more, we put it back the next frame
        b.fields.ai_on = false;
        assert_eq!(frame(&mut d, &mut w, &mut b, near, handle), vec![Note::AiFought(1)]);
        assert!(b.fields.ai_on);
        // THE PLAYER IS IN: handed over; the event's fight start writes the same values; nothing more of ours
        let n = frame(&mut d, &mut w, &mut b, FightFacts { fog_passed: Some(true), ..near }, handle);
        assert_eq!(n, vec![Note::HandedOver]);
        assert_eq!(d.phase, FightPhase::Fight);
        b.fields = Fields { ai_on: true, rate: FIGHT_RATE };
        for _ in 0..60 {
            assert_eq!(frame(&mut d, &mut w, &mut b, FightFacts { fog_passed: Some(true), boss_bar: true, ..near }, handle), vec![]);
        }
        // the boss falls: unlocked, nothing written back on the dead boss
        frame(&mut d, &mut w, &mut b, FightFacts { defeated: Some(true), boss_alive: false, fog_passed: Some(true), ..near }, handle);
        assert_eq!(d.phase, FightPhase::Won);
        assert_eq!(w.state, Wake::Asleep);
        assert!(b.fields.ai_on, "the dead boss's fields are the game's");
    }

    #[test]
    fn staging_again_after_a_lost_fight_keeps_the_first_before() {
        let mut b = boss();
        let mut w = AiWake::default();
        w.hold(&mut b, 7);
        w.hand_over();
        // the fight was lost without a reload (same character): staged again
        assert!(matches!(w.hold(&mut b, 7)[..], [Note::On { before: LOADED, .. }]));
        assert_eq!(w.release(Some(&mut b), Some(7)), vec![Note::Released(LOADED)]);
    }
}
