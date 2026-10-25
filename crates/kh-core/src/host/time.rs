//! The per-object-type time dilation: ONE SEAM for everything that slows part of the world
//! (the formchange / finish "world slow" of combat strand F 10.4: `WorldSlow(ctrl, on)`
//! 0x14140E500 = object types mask 0x419C at rate 0.1; the shotlock aim of the magic spec: the
//! player at 0.4; the slow sub-menu 0.3). The game keeps a table `GameState.ObjTypeTimeDilation`
//! indexed by `ETresObjType`; the core keeps the same table on the pawn
//! (`Sora::time_dilation(type)`), changes it with `Ctx::set_time_dilation(mask, rate)` and tells
//! the host with `EventKind::TimeDilation { mask, rate }`.
//!
//! The HOST applies it to what it owns (its enemies' and gimmicks' ticks, its physics); the core
//! applies the player's own entry where the game does (the situation-command countdown:
//! `dt x ObjTypeTimeDilation[PLAYER]`, magic spec J 5.6). The formchange world slow never
//! names the PLAYER type. (Magic implementer: since the shotlock aim mode, `Sora::step_hosted` scales SORA'S OWN STEP by
//! the entry PLAYER and the controller's timers by the entry VEHICLE (11): the aim mode slows Sora himself, magic spec F
//! A6. Both entries are 1.0 otherwise.)
//!
//! Agreed between the combat and the magic implementers through their progress notes
//! (build/sora_tmp/IMPL_COMBAT_STATE.md): use this, do not add a second one.

/// `ETresObjType` values used as indices / bits (object type n = bit n of a mask).
pub const OBJ_PLAYER: u32 = 1;
/// The number of object types the table holds.
pub const OBJ_TYPES: usize = 16;
/// `WorldSlow`'s mask (0x14140E500): enemies, gimmicks, avatars, map, background, physics.
pub const MASK_WORLD: u32 = 0x419C;

/// The table (`GameState.ObjTypeTimeDilation`), every entry 1.0 at rest.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimeTable(pub [f32; OBJ_TYPES]);

impl Default for TimeTable {
    fn default() -> TimeTable {
        TimeTable([1.0; OBJ_TYPES])
    }
}

impl crate::pawn::Pawn {
    /// The time dilation of object type `obj` (1.0 = normal).
    pub fn time_dilation(&self, obj: u32) -> f32 {
        self.combat.time.0.get(obj as usize).copied().unwrap_or(1.0)
    }
}

impl crate::pawn::Ctx<'_, '_> {
    /// Sets the dilation of every object type in `mask` to `rate` and tells the host (an event:
    /// the host owns what the types' time means for its own things).
    pub fn set_time_dilation(&mut self, mask: u32, rate: f32) {
        let mut changed = false;
        for (i, slot) in self.pawn.combat.time.0.iter_mut().enumerate() {
            if mask & (1 << i) != 0 && *slot != rate {
                *slot = rate;
                changed = true;
            }
        }
        if changed {
            self.pawn.event(crate::host::EventKind::TimeDilation { mask, rate });
        }
    }
}

impl TimeTable {
    /// Every object type at `rate` (the global game speed below slows them all alike).
    pub const fn uniform(rate: f32) -> TimeTable {
        TimeTable([rate; OBJ_TYPES])
    }
}

/// `ETresObjTypeProcChannel`: who asks for a speed (the lowest request below 1 wins, times the
/// highest above 1). `CH_EVENT` is the level scripts' channel (the boss finish).
pub const SPEED_CHANNELS: usize = 16;
pub const CH_COMMON: usize = 0;
pub const CH_PLAYER: usize = 1;
pub const CH_EVENT: usize = 2;

/// THE GLOBAL GAME SPEED (boss finish, docs/sora-bossfinish-spec.md 3.1):
/// `UTresGameBlueprintLibrary::SetGlobalGameSpeed(channel, rate, interp)` 0x14222C9D0 ->
/// `AGameStateBase` vt+0x880 0x144119CE0 stores a target rate (not below 0.01) and an
/// interpolation time per channel; the game state's tick 0x14411A230 (vt+0x818, called with the
/// application's frame delta by 0x144119710) moves each channel linearly to its target and writes
/// `min(channels) x max(channels)` into `WorldSettings.SqEX_CustomTimeDilation`: THE WHOLE WORLD
/// (the player, enemies, cameras, the level scripts' delays) runs at that rate. [decompiled C]
///
/// Unlike the per-type table above this is owned by whoever runs the world's clock: a HOST keeps
/// one (or lets `combat::finish::FinishSequence` keep it) and scales the `dt` of every step it
/// makes, Sora's included, by [`GameSpeed::rate`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GameSpeed {
    current: [f32; SPEED_CHANNELS],
    target: [f32; SPEED_CHANNELS],
    left: [f32; SPEED_CHANNELS],
    rate: f32,
}

impl Default for GameSpeed {
    fn default() -> GameSpeed {
        GameSpeed { current: [1.0; SPEED_CHANNELS], target: [1.0; SPEED_CHANNELS], left: [0.0; SPEED_CHANNELS], rate: 1.0 }
    }
}

impl GameSpeed {
    /// The floor of a requested rate (0x144119CE0).
    pub const MIN_RATE: f32 = 0.01;

    /// `SetGlobalGameSpeed(channel, rate, interp)`.
    pub fn set(&mut self, channel: usize, rate: f32, interp: f32) {
        let Some(k) = (channel < SPEED_CHANNELS).then_some(channel) else { return };
        self.target[k] = rate.max(Self::MIN_RATE);
        self.left[k] = interp.max(0.0);
    }

    /// `ResetGlobalGameSpeed(channel, interp)` 0x140E62B00: the same call with rate 1.
    pub fn reset(&mut self, channel: usize, interp: f32) {
        self.set(channel, 1.0, interp);
    }

    /// Every channel back to 1 at once (vt+0x870 0x144119820).
    pub fn clear(&mut self) {
        *self = GameSpeed::default();
    }

    /// The game state's tick 0x14411A230 with the REAL frame delta: returns the world's rate.
    pub fn step(&mut self, real_dt: f32) -> f32 {
        let (mut low, mut high) = (1.0f32, 1.0f32);
        for k in 0..SPEED_CHANNELS {
            let (current, target) = (self.current[k], self.target[k]);
            if current != target {
                if self.left[k] > 0.0 && real_dt > 0.0 {
                    let speed = (target - current) / self.left[k];
                    let next = current + speed * real_dt;
                    self.current[k] = if speed <= 0.0 { next.max(target) } else { next.min(target) };
                    self.left[k] = (self.left[k] - real_dt).max(0.0);
                } else if self.left[k] <= 0.0 {
                    self.current[k] = target;
                }
            }
            low = low.min(self.current[k]);
            high = high.max(self.current[k]);
        }
        let rate = low * high;
        self.rate = if (rate - 1.0).abs() < 0.0001 { 1.0 } else { rate };
        self.rate
    }

    /// The world's rate as of the last `step` (1.0 = normal).
    pub fn rate(&self) -> f32 {
        self.rate
    }
}

#[cfg(test)]
mod game_speed_tests {
    use super::*;

    #[test]
    fn a_channel_ramps_linearly_in_real_time_and_the_lowest_wins() {
        let mut s = GameSpeed::default();
        s.set(CH_EVENT, 0.05, 0.1);
        let mut rates = Vec::new();
        for _ in 0..8 {
            rates.push(s.step(1.0 / 60.0));
        }
        // 1 - 9.5 t: after one frame 0.8417, after six 0.05 exactly.
        assert!((rates[0] - (1.0 - 9.5 / 60.0)).abs() < 1.0e-4, "{rates:?}");
        assert!((rates[2] - (1.0 - 9.5 * 3.0 / 60.0)).abs() < 1.0e-3);
        assert!((rates[5] - 0.05).abs() < 1.0e-5 && (rates[7] - 0.05).abs() < 1.0e-6, "{rates:?}");
        // a second, slower request does not speed the world up; a faster one multiplies
        s.set(CH_PLAYER, 0.5, 0.0);
        assert!((s.step(1.0 / 60.0) - 0.05).abs() < 1.0e-6);
        s.set(CH_COMMON, 2.0, 0.0);
        assert!((s.step(1.0 / 60.0) - 0.1).abs() < 1.0e-6);
        // the reset with time 0 is at once; below 0.01 is 0.01
        s.clear();
        s.set(CH_EVENT, 0.0, 0.0);
        assert_eq!(s.step(1.0 / 60.0), GameSpeed::MIN_RATE);
        s.reset(CH_EVENT, 0.0);
        assert_eq!(s.step(1.0 / 60.0), 1.0);
    }
}
