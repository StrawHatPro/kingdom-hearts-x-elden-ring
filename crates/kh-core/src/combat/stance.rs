//! (MIMIC-STANCE, THE USER 2026-11-05: "make sure Goofy's mimic tear boss is able to parry you properly, can stagger you by
//! breaking your stance and can use his Breaker on you") A KH CHARACTER'S STANCE BREAK: ONE ROUTE FOR EVERY PLAYABLE
//! CHARACTER (Sora's core here, through `Sora::stance_break`; kh-party's pawns, `NpcPawn::stance_break`, the same struct).
//!
//! WHAT FILLS IT: nothing of the character's own. A Goofy that fights the party (the Mimic Tear bosses' copy, kh-partyhost
//! mimicboss.rs) keeps the party member's gauge in his own Stagger book (kh-party goofy/stagger.rs), by the very rows the
//! enemy side uses (`stagger.gauge` 100, a deflect 25 ...); when it is full the book asks the host (`StaggerAsk::Break {
//! window }`, `stagger.window` 2.5 s for a body of a KH character's size) and the host breaks the character for that window.
//! `Hold` (a Breaker winds up) keeps it at least that long; `Release` (the window ran out, or the Breaker's slam is about to
//! land) ends it: as the enemy side's BROKEN, hits in between do not end it.
//!
//! WHAT IT LOOKS LIKE (traced, the choice of KH3's damage reactions): KH3 has no "posture break" of its own characters; of its
//! reactions that hold a character in place with no action, KH3's STUN (`TresPlayerState_BadStatStun` id 122 for Sora,
//! `TresNpcState_BadStatStun` for the friends: `BadStatStunStart` -> `BadStatStunLoop` -> `BadStatStunEnd`, the friends'
//! M_0241 - M_0243 where their set lacks the names) is the one: he staggers, sways helpless and recovers. Its others do not
//! fit: FREEZE pauses the pose (an ice block), the damage kinds' reactions (Small, Blow, Beat ...) end by themselves within
//! their clip and are a hit's knock, and the down / get-up of a Blow is a fall. So the break runs the STUN state for the
//! window, without the STUN status: no status effect, no stick flick shortens it (a status's rule), no Stun Guard or form
//! refuses it (they refuse a status), and a damaging hit does not cure it (the enemy side's rule above).

/// One character's stance break (Sora: `combat.bad.stance`; a pawn: `NpcPawn::stance`).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct StanceBreak {
    /// Seconds left of the break (0: not broken).
    pub left: f32,
    /// The break's length as it stands (for a HUD).
    pub total: f32,
}

impl StanceBreak {
    /// Broken now.
    pub fn broken(&self) -> bool {
        self.left > 0.0
    }

    /// BROKEN for `seconds` (a running break lasts at least that long more). False: no time asked.
    pub fn open(&mut self, seconds: f32) -> bool {
        if seconds.is_nan() || seconds <= 0.0 {
            return false;
        }
        self.left = self.left.max(seconds);
        self.total = self.total.max(self.left);
        true
    }

    /// A Breaker winds up: a running break lasts at least `seconds` more (nothing when not broken).
    pub fn hold(&mut self, seconds: f32) {
        if self.broken() && seconds > 0.0 {
            self.left = self.left.max(seconds);
            self.total = self.total.max(self.left);
        }
    }

    /// The break ends now. True: it was broken.
    pub fn release(&mut self) -> bool {
        let was = self.broken();
        *self = StanceBreak::default();
        was
    }

    /// One step of its clock. True: it ran out in this step.
    pub fn tick(&mut self, dt: f32) -> bool {
        if !self.broken() {
            return false;
        }
        self.left -= dt.max(0.0);
        if self.left > 0.0 {
            return false;
        }
        *self = StanceBreak::default();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_break_lasts_its_window_holds_and_releases() {
        let mut s = StanceBreak::default();
        assert!(!s.broken() && !s.open(0.0) && !s.open(f32::NAN));
        assert!(s.open(2.5) && s.broken());
        assert_eq!((s.left, s.total), (2.5, 2.5));
        assert!(!s.tick(1.0));
        s.hold(3.0);
        assert_eq!((s.left, s.total), (3.0, 3.0), "a Breaker's hold");
        s.hold(1.0);
        assert_eq!(s.left, 3.0, "a hold never shortens it");
        assert!(!s.tick(2.9) && s.tick(0.2), "ran out");
        assert!(!s.broken() && !s.tick(1.0));
        s.hold(5.0);
        assert!(!s.broken(), "a hold opens nothing");
        assert!(s.open(2.5) && s.release() && !s.broken() && !s.release());
    }
}
