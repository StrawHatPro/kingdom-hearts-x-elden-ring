//! (IMPL_ER58) CHESTS: the user: "Sora's chest animation plays from KH3 for opening chests".
//!
//! WHAT THE GAME DOES (the user's log er\logs\kh-er.prev.log, 1568.9 - 1573.4, Gatefront, block m60_42_37): Y under
//! Elden Ring's prompt (`y: press #4 -> ELDEN RING`), the hidden body plays ANIMATION 60110 (4.03 s: the chest-open of
//! c0000); when it ends the item reaches our gate (`loot: gate: ... "Lordsworn's Greatsword"`, found in a carriage chest
//! there). 60110 was no row of `own.*`, so Sora went on driving the body meanwhile and showed his own pose.
//!
//! THE RULE NOW (default ON, `chest.kh = 0` = as before): while the body plays one of the chest animations (`chest.anims`,
//! default 60110; 15 s at most) Sora FOLLOWS the body (`Follow::Action`: the game places and turns its body at the chest,
//! Sora is seated on it, facing the chest; the pad is the game's; nothing of ours writes the body, so the chest opens and
//! gives its item exactly as before) and, `chest.delay` s (default 0) after it began, his own simulation is asked for
//! KH3's `TresPlayerState_OpenTreasureBox` (kh-core states/open_treasure_box.rs): `OpenTreasureL` (2.97 s: the Keyblade
//! drawn, the voice, `se00001_023`, the Keyblade's hit at 1.75 s, the box "told" at 2.0 s) or with `chest.size = s`
//! `OpenTreasureS` (0.97 s, the hit at 0.47 s; `auto`: L when the body's animation lasts 2.5 s or more). Its notifies
//! play their sounds and effects through the usual path. The retargeted Elden Ring pose is NOT laid over him meanwhile.
//! When the body's animation ends the body is handed back as after a door (the wait for the ground).

/// The chest animations of the body (c0000) by default: what the user's log showed.
pub const DEFAULT_ANIMS: (i32, i32) = (60110, 60110);

/// The size of KH3's box to open: `chest.size` = l (default) | s | auto (by the length of the body's animation).
pub fn large(size: Option<&str>, body_length: f32) -> bool {
    match size.map(|s| s.trim().to_ascii_lowercase()) {
        Some(s) if s == "s" || s == "small" => false,
        Some(s) if s == "auto" => body_length >= 2.5,
        _ => true,
    }
}

/// One chest opening as Sora sees it (pure; sora.rs feeds it once a frame).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Chest {
    /// The chest animation being followed, and for how long.
    anim: Option<i32>,
    for_s: f32,
    /// KH3's state was asked for in this opening.
    asked: bool,
    /// An animation of the list that outlasted the limit: not followed again until another animation plays.
    spent: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Edge {
    None,
    /// The body began a chest animation: Sora follows it.
    Begin { anim: i32 },
    /// Now KH3's treasure-box state is to be asked for.
    Ask,
    /// The opening is over (the animation ended, or the limit / a reason to stop).
    End { anim: i32, seconds: f32, asked: bool },
}

impl Chest {
    /// The longest a chest animation is followed (the game's is 4.03 s).
    pub const MAX_S: f32 = 15.0;

    /// One frame. `anim`: the body's animation; `listed`: it is one of the chest animations; `allowed`: the rule may
    /// apply (on by its key, no cutscene, Sora alive, no load / warp this frame); `delay`: seconds before KH3's state.
    pub fn frame(&mut self, anim: i32, listed: bool, allowed: bool, delay: f32, dt: f32) -> (bool, Edge) {
        if !listed || self.anim.is_some_and(|a| a != anim) {
            self.spent = false;
        }
        let want = listed && allowed && !self.spent;
        match (self.anim, want) {
            (None, true) => {
                self.anim = Some(anim);
                self.for_s = 0.0;
                self.asked = false;
                // (KH3's state is asked for from the next frame: after the follow's start has seated him)
                (true, Edge::Begin { anim })
            }
            (Some(a), true) if a == anim => {
                self.for_s += dt;
                if self.for_s > Self::MAX_S {
                    self.spent = true;
                    return (false, self.end());
                }
                if !self.asked && self.for_s + 1.0e-4 >= delay {
                    self.asked = true;
                    return (true, Edge::Ask);
                }
                (true, Edge::None)
            }
            (Some(_), _) => (false, self.end()),
            (None, false) => (false, Edge::None),
        }
    }

    fn end(&mut self) -> Edge {
        let edge = Edge::End { anim: self.anim.unwrap_or(0), seconds: self.for_s, asked: self.asked };
        self.anim = None;
        self.for_s = 0.0;
        self.asked = false;
        edge
    }

    pub fn on(&self) -> bool {
        self.anim.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    /// The user's chest: 60110 for 4.03 s: followed from its first frame, KH3's state asked for on the next frame
    /// (no delay), the end when the animation changes back to 0.
    #[test]
    fn the_chest_animation_is_followed_and_kh3_is_asked_once() {
        let mut c = Chest::default();
        assert_eq!(c.frame(0, false, true, 0.0, DT), (false, Edge::None));
        assert_eq!(c.frame(60110, true, true, 0.0, DT), (true, Edge::Begin { anim: 60110 }));
        assert_eq!(c.frame(60110, true, true, 0.0, DT), (true, Edge::Ask));
        let frames = (0..240).filter(|_| c.frame(60110, true, true, 0.0, DT) == (true, Edge::None)).count();
        assert_eq!(frames, 240, "followed for the whole animation, asked once");
        match c.frame(0, false, true, 0.0, DT) {
            (false, Edge::End { anim: 60110, seconds, asked: true }) => assert!((seconds - 241.0 * DT).abs() < 0.02, "{seconds}"),
            other => panic!("{other:?}"),
        }
        assert!(!c.on());
    }

    /// With a delay KH3's state starts that much later; a chest animation that never ends is let go after 15 s and not
    /// followed again while it lasts; the key off never follows.
    #[test]
    fn delay_limit_and_key() {
        let mut c = Chest::default();
        c.frame(60110, true, true, 1.0, DT);
        let asked_after = (1..200).find(|_| c.frame(60110, true, true, 1.0, DT).1 == Edge::Ask).unwrap();
        assert!((asked_after as f32 * DT - 1.0).abs() < 0.03, "{asked_after}");
        let mut stuck = Chest::default();
        let followed = (0..2000).take_while(|_| stuck.frame(60110, true, true, 0.0, DT).0).count();
        assert!((followed as f32 * DT - Chest::MAX_S).abs() < 0.05, "{followed}");
        assert!((0..100).all(|_| !stuck.frame(60110, true, true, 0.0, DT).0));
        let mut off = Chest::default();
        assert!((0..100).all(|_| !off.frame(60110, true, false, 0.0, DT).0));
    }

    #[test]
    fn the_size_of_kh3s_box() {
        assert!(large(None, 4.03));
        assert!(!large(Some("s"), 4.03));
        assert!(large(Some("auto"), 4.03));
        assert!(!large(Some("auto"), 1.5));
    }
}
