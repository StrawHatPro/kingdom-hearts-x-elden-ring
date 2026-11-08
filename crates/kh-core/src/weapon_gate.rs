//! (PARTY-WEAPONS) THE WEAPON-DRAW GATE: one rule for when every KH character's weapon is out.
//!
//! THE USER (2026-11-05): "Donald and Goofy's weapons should behave the same was as the keyblades, appearing when in
//! combat or when attacking with their effect". Party rules apply to every character through one shared gate.
//!
//! KH3's own rule (build/sora_tmp/weapons/PARTY-WEAPONS/STATE.md 1 has the evidence):
//! - Sora (the player pawn): kh-core's states (actions.rs `draw`, states/idle.rs, the attack states' Begin).
//! - Donald, Goofy and every friend (`ATresNpcPawnBase`, the same pawn `Draw` vt+0x1340 into the equipment's
//!   `SetVisible` 0x14178B320): drawn with the appear effect on the battle start (slot 1233 0x141274AA0: `Draw(1, 1, 1.0)`
//!   then `BattleStart`), in the battle idle (Idle Begin 0x141242140) and the battle locomotion (Land 0x141219F60), and at the
//!   Begin of every attack definition (`TresNpcAttackDefinitionBase.m_bWeaponVisible`, true from its constructor
//!   0x14121BBF0 and never overridden by the friends' 23 definitions: Begin 0x1412202E0 / 0x141220930); drawn WITHOUT the
//!   effect when hurt (Damage 0x1412444C0); put away WITH the effect out of battle by the idle (0x14124CD40: two idle
//!   loops, then `IdleBreakW`, then `Draw(0, 1, 1.0)`) or the run (0x141219F60: after `RunBattleEnd`, the loop past 80 %).
//!
//! This module is the part every character shares: the hold for the hero light (the light carrier only), the no-combat
//! zone (kh_core::nocombat: nothing is drawn there) and the appear / disappear sequence of a weapon (`SetVisible`'s
//! timeline with the weapon blueprint's own times). kh-party's `weapon_draw` runs the friends' rule on it; Sora's states
//! run his; the hosts ask [`hold`] for both.

use crate::nocombat::{self, Fighter};

/// A weapon's appear / disappear look (its blueprint's `TresWeaponBase` fields, read per weapon).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// `m_AppearEffect` (= `m_DisappearEffect` for every weapon of the port), spawned at the weapon mesh's origin
    /// (`m_AppearEffectAttach` None).
    pub effect: &'static str,
    /// `m_AppearVisibleDelayTime`: the mesh stays hidden this long after an appear with the effect.
    pub delay: f32,
    /// `m_AppearDitherAnimTime` / `m_DisappearDitherAnimTime`: the `OpacityFade` ramp.
    pub dither: f32,
    /// `m_DisappearVisibleDelayTime`: the ramp of a put-away starts this late.
    pub hide_delay: f32,
}

impl Look {
    /// The Kingdom Key (`w_so010_Wep`: delay 0.22, dither 0.06, `VW_SO010_APP0`; docs/sora-vfx-spec.md 2.4): the timing
    /// kh-er's and kh-app's Keyblade sequence already use.
    pub const KINGDOM_KEY: Look = Look { effect: "VW_SO010_APP0", delay: 0.22, dither: 0.06, hide_delay: 0.22 };
}

/// THE SEQUENCE (`SetVisible(visible, withEffect, rate 1)` 0x14178B320 and its Appear / Disappear states): what the
/// mesh's `OpacityFade` is `since` seconds after the latest change.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sequence {
    pub visible: bool,
    pub since: f32,
    /// The change was without the effect: the mesh changed at once (SetVisible's immediate path).
    pub instant: bool,
}

impl Default for Sequence {
    fn default() -> Self {
        Sequence { visible: false, since: 10.0, instant: true }
    }
}

impl Sequence {
    /// The weapon's `OpacityFade` now (0 = not drawn).
    pub fn opacity(&self, look: &Look) -> f32 {
        if self.instant {
            return if self.visible { 1.0 } else { 0.0 };
        }
        let (delay, ramp) = if self.visible { (look.delay, look.dither) } else { (look.hide_delay, look.dither) };
        let t = if ramp > 0.0 { ((self.since - delay) / ramp).clamp(0.0, 1.0) } else if self.since >= delay { 1.0 } else { 0.0 };
        // an appear with a delay keeps the mesh hidden through the delay (flags & 4), then ramps 0 -> 1
        if self.visible {
            t
        } else {
            1.0 - t
        }
    }

    /// A change (`effect`: with the appear / disappear effect and its dither; else at once). False: no change (the
    /// game's `if visible == visibleFlag: return`).
    pub fn set(&mut self, visible: bool, effect: bool) -> bool {
        if self.visible == visible {
            return false;
        }
        *self = Sequence { visible, since: 0.0, instant: !effect };
        true
    }

    pub fn tick(&mut self, dt: f32) {
        self.since = (self.since + dt).min(10.0);
    }
}

/// THE HOLD FOR THE HERO LIGHT (IMPL_ER44; the user 2026-11-05: "for the hero light, it should just be whoever is party
/// leader"): the character that carries the light holds its weapon out while the light wants it, whoever he is; nobody
/// else does. Nothing is held in a no-combat place (the Roundtable Hold's ban is the light's own as well).
pub fn hold(carrier: bool, light_wanted: bool) -> bool {
    carrier && light_wanted && !nocombat::zone()
}

/// May `who` have its weapon out now? Not where Elden Ring forbids attacks (kh_core::nocombat's zone): there every
/// character's weapon is put away and stays away.
pub fn may_draw(who: Fighter) -> bool {
    match who {
        Fighter::Sora | Fighter::Donald | Fighter::Goofy | Fighter::MimicSora | Fighter::SpiritAsh => !nocombat::zone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRIEND: Look = Look { effect: "vw_don_app00", delay: 0.0, dither: 0.1, hide_delay: 0.0 };

    #[test]
    fn the_friends_sequence_is_their_blueprints() {
        let mut s = Sequence::default();
        assert_eq!(s.opacity(&FRIEND), 0.0);
        assert!(s.set(true, true) && !s.set(true, true), "a second draw is the game's no-op");
        assert_eq!(s.opacity(&FRIEND), 0.0, "the ramp starts at once (no visible delay)");
        s.tick(0.05);
        assert!((s.opacity(&FRIEND) - 0.5).abs() < 1e-4);
        s.tick(0.05);
        assert_eq!(s.opacity(&FRIEND), 1.0);
        assert!(s.set(false, true));
        s.tick(0.025);
        assert!((s.opacity(&FRIEND) - 0.75).abs() < 1e-4, "{}", s.opacity(&FRIEND));
        s.tick(0.1);
        assert_eq!(s.opacity(&FRIEND), 0.0);
        assert!(s.set(true, false));
        assert_eq!(s.opacity(&FRIEND), 1.0, "without the effect: at once (the Damage state's draw)");
    }

    #[test]
    fn the_keyblade_keeps_its_delay() {
        let mut s = Sequence::default();
        s.set(true, true);
        s.tick(0.2);
        assert_eq!(s.opacity(&Look::KINGDOM_KEY), 0.0);
        s.tick(0.05);
        assert!((s.opacity(&Look::KINGDOM_KEY) - 0.5).abs() < 1e-3);
    }

    #[test]
    fn only_the_carrier_holds_and_never_in_the_zone() {
        let _out = nocombat::scope(false);
        assert!(hold(true, true) && !hold(false, true) && !hold(true, false));
        let _zone = nocombat::scope(true);
        assert!(!hold(true, true));
        for f in Fighter::ALL {
            assert!(!may_draw(f), "{}", f.name());
        }
    }
}
