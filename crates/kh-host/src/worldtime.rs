//! (AUDIT-A WORLDTIME) KH3'S WORLD TIME FOR A HOST: what the formchanges and their finishers slow, and by how much, as one
//! engine-free rule every host shares (kh-er `worldtime.rs`, the recorder `audit_a::Session::world`, the effects harness).
//!
//! KH3 has two routes, both traced in the decompiled C / the cooked blueprints:
//!
//! 1. **WorldSlow** `0x14140E500(ctrl, on)` (with ctrl+0x878 bit 3 so it is applied once): on ->
//!    `FUN_1405DA460(pawn, 1, 0x419C, 0.1, 0)` = GameState vt+0x8A8 `SetTypeRates(channel 1 CH_PLAYER, mask, rate, blend 0)`;
//!    off -> `FUN_1405DA3E0(pawn, 1, 0)` = the same call with mask 0x7FFFFFFF and rate 1.0 (channel 1 back to normal).
//!    The mask is a set of `ESQEX_ObjectType` bits: **0x419C = 2 ENEMY, 3 GIMMICK, 4 AVATAR, 7 MAP, 8 BG, 14 PHYSX**.
//!    Not in it: 1 PLAYER (Sora), 10 FRIEND (Donald, Goofy, the party), 11 VEHICLE (the player controller), 9 CAMERA,
//!    5 PROJECTILE (a projectile takes its OWNER's type unless `m_bDisableOwnerObjTypeCalcTimeDilation`: an enemy's shot
//!    is slowed, Sora's is not), 6 PRIZE, 12 EVENT, 13 UI. Each actor's tick is `dt x CustomTimeDilation x
//!    GameState.SQEX_ObjTypeTimeDilation[m_ObjType]` (FActorTickFunction::ExecuteTick 0x143E968D0, actor slot 204
//!    0x143EAE9C0; docs/sora-magic-spec.md A6). The core keeps that table on Sora (`kh_core::host::time`,
//!    `Sora::time_dilation(type)`); this module reads it.
//! 2. **SetGlobalGameSpeed** (`UTresGameBlueprintLibrary` 0x14222C9D0 -> AGameStateBase vt+0x880 0x144119CE0, tick
//!    0x14411A230): a target rate (>= 0.01) and a linear ramp time per channel, `min(channels) x max(channels)` into
//!    `WorldSettings.SqEX_CustomTimeDilation` = THE WHOLE WORLD, Sora, his party, the cameras and the blueprints' Delay
//!    nodes included (docs/sora-bossfinish-spec.md 3.1; `kh_core::host::time::GameSpeed`). Of every cooked asset of KH3's
//!    Content (`rg -a -l SetGlobalGameSpeed`, 2026-11-05: 5 hits) the only form one is
//!    `BP_TresPlayerStateEventCamera_tate_finish` (Counter Shield): B418 ID 2 `SetGlobalGameSpeed(1, 0.1, 0.08)`, back at
//!    the end of ID 3 `SetGlobalGameSpeed(1, 1, 0.2)`; B426 ID 4 `SetGlobalGameSpeed(1, 0.05, 0.2)`, back at its end
//!    `ResetGlobalGameSpeed(1, 0.01)`; kh-camera `forms_shield.rs` publishes them as `ShieldCameras::game_speed`. (The other
//!    four are Ralph's link and p_ex002 / p_ex003's state events, not Sora's forms.) No asset writes `CustomTimeDilation`.
//!
//! So a host applies, per fixed step: everything it steps at [`WorldRates::global`] (Sora's step, the party, the cameras,
//! Sora's effects); its enemies at [`WorldRates::enemy`] (`global x table[ENEMY]`); Sora's own `table[PLAYER]` is applied
//! by the core itself (`Sora::step_hosted`), the friends' `table[FRIEND]` by the party host (1.0 for every form slow).

use kh_core::host::time::{GameSpeed, CH_PLAYER};

/// `ESQEX_ObjectType` indices of the table (docs/sora-magic-spec.md A6).
pub const OBJ_PLAYER: u32 = 1;
pub const OBJ_ENEMY: u32 = 2;
pub const OBJ_GIMMICK: u32 = 3;
/// (WORLDTIME-2) `MAP` (the level's own clock and effects; bit 7 of WorldSlow's 0x419C).
pub const OBJ_MAP: u32 = 7;
/// (WORLDTIME-2) `VEHICLE` (the player controller; Torrent in Elden Ring) and `PHYSX` (physics bodies: ragdolls, objects).
pub const OBJ_VEHICLE: u32 = 11;
pub const OBJ_PHYSX: u32 = 14;
pub const OBJ_CAMERA: u32 = 9;
pub const OBJ_FRIEND: u32 = 10;

/// What the world runs at this step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldRates {
    /// KH3's global game speed: every actor, Sora included.
    pub global: f32,
    /// `SQEX_ObjTypeTimeDilation[ENEMY]` (0.1 while a WorldSlow is on), on top of `global`.
    pub enemy_type: f32,
    /// `[GIMMICK]` (the level's objects; same as ENEMY for WorldSlow).
    pub gimmick_type: f32,
    /// (WORLDTIME-2) `[MAP]` (the map: its clock; same as ENEMY for WorldSlow and the shotlock aim).
    pub map_type: f32,
    /// (WORLDTIME-2) `[VEHICLE]` (1.0 for WorldSlow, 0.4 in the shotlock aim) and `[PHYSX]` (0.1 for WorldSlow).
    pub vehicle_type: f32,
    pub physx_type: f32,
    /// `[PLAYER]` (Sora: the core applies it; 1.0 for WorldSlow, 0.4 in the shotlock aim).
    pub player_type: f32,
    /// `[FRIEND]` (the party: 1.0 for WorldSlow).
    pub friend_type: f32,
    /// `[CAMERA]` (1.0 but for the menu's slow option).
    pub camera_type: f32,
}

impl WorldRates {
    pub const NORMAL: WorldRates = WorldRates { global: 1.0, enemy_type: 1.0, gimmick_type: 1.0, map_type: 1.0, vehicle_type: 1.0, physx_type: 1.0, player_type: 1.0, friend_type: 1.0, camera_type: 1.0 };

    /// An enemy actor's whole rate: `global x [ENEMY]`.
    pub fn enemy(&self) -> f32 {
        self.global * self.enemy_type
    }

    /// A friend's whole rate: `global x [FRIEND]`.
    pub fn friend(&self) -> f32 {
        self.global * self.friend_type
    }

    /// The camera's whole rate: `global x [CAMERA]`.
    pub fn camera(&self) -> f32 {
        self.global * self.camera_type
    }

    /// Nothing is slowed.
    pub fn normal(&self) -> bool {
        *self == WorldRates::NORMAL
    }
}

impl Default for WorldRates {
    fn default() -> WorldRates {
        WorldRates::NORMAL
    }
}

/// Two owners' global rates as the game state combines its channels (0x14411A230): the lowest request below 1 times the
/// highest above 1.
pub fn combine(a: f32, b: f32) -> f32 {
    let low = a.min(b).min(1.0);
    let high = a.max(b).max(1.0);
    low * high
}

/// The host's world clock: the forms' `SetGlobalGameSpeed` channel (channel 1 CH_PLAYER, as the blueprint calls it) and
/// the core's per-type table, read once per fixed step.
#[derive(Clone, Debug, Default)]
pub struct WorldClock {
    speed: GameSpeed,
    rates: WorldRates,
    /// What changed, for the hosts' logs (taken with [`WorldClock::take_log`]).
    log: Vec<String>,
}

impl WorldClock {
    /// A form blueprint's request of this step (`ShieldCameras::game_speed`: `SetGlobalGameSpeed(1, rate, time)` /
    /// `ResetGlobalGameSpeed(1, time)` = rate 1). The game state applies it from its next tick on, as here.
    pub fn request(&mut self, request: Option<(f32, f32)>) {
        if let Some((rate, time)) = request {
            self.speed.set(CH_PLAYER, rate, time);
            self.log.push(format!("SetGlobalGameSpeed(1, {rate}, {time} s)"));
        }
    }

    /// One fixed step of `real_dt` real seconds (the game state's tick counts the application's frame delta): `other` is
    /// another owner's global rate (kh-er: the boss finish's, kh_core::combat::finish), `table` the core's per-type
    /// dilation (`|t| sora.time_dilation(t)`).
    pub fn step(&mut self, real_dt: f32, other: f32, table: impl Fn(u32) -> f32) -> WorldRates {
        let form = self.speed.step(real_dt);
        let rates = WorldRates {
            global: combine(form, other),
            enemy_type: table(OBJ_ENEMY),
            gimmick_type: table(OBJ_GIMMICK),
            map_type: table(OBJ_MAP), // (WORLDTIME-2)
            vehicle_type: table(OBJ_VEHICLE), // (WORLDTIME-2)
            physx_type: table(OBJ_PHYSX), // (WORLDTIME-2)
            player_type: table(OBJ_PLAYER),
            friend_type: table(OBJ_FRIEND),
            camera_type: table(OBJ_CAMERA),
        };
        if (rates.global - self.rates.global).abs() > 1.0e-6 && (rates.global == 1.0 || self.rates.global == 1.0 || (rates.global - self.rates.global).abs() > 0.05) {
            self.log.push(format!("global game speed x{:.3}", rates.global));
        }
        if rates.enemy_type != self.rates.enemy_type {
            self.log.push(format!("enemies (ObjType ENEMY) x{:.3}", rates.enemy_type));
        }
        self.rates = rates;
        rates
    }

    /// The forms' own channel alone (without `other`), as of the last step.
    pub fn form_rate(&self) -> f32 {
        self.speed.rate()
    }

    /// The rates of the last step.
    pub fn rates(&self) -> WorldRates {
        self.rates
    }

    /// Every channel back to 1 at once (a load, the host's stop; the game state's vt+0x870).
    pub fn clear(&mut self) {
        self.speed.clear();
        self.rates = WorldRates::NORMAL;
    }

    pub fn take_log(&mut self) -> Vec<String> {
        std::mem::take(&mut self.log)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kh_core::host::time::MASK_WORLD;

    const DT: f32 = 1.0 / 60.0;

    /// The table as WorldSlow leaves it: 0x419C at 0.1.
    fn world_slow(t: u32) -> f32 {
        if MASK_WORLD & (1 << t) != 0 {
            0.1
        } else {
            1.0
        }
    }

    #[test]
    fn world_slow_slows_the_enemies_not_sora_his_party_or_the_camera() {
        assert_eq!(MASK_WORLD, 0x419C);
        let mut c = WorldClock::default();
        let r = c.step(DT, 1.0, world_slow);
        assert_eq!((r.global, r.enemy_type, r.gimmick_type), (1.0, 0.1, 0.1));
        assert_eq!(r.map_type, 0.1, "(WORLDTIME-2) MAP is bit 7 of 0x419C: the map's clock slows with the enemies");
        assert_eq!((r.physx_type, r.vehicle_type), (0.1, 1.0), "(WORLDTIME-2) PHYSX is bit 14 of 0x419C; VEHICLE (11) is not");
        assert_eq!((r.player_type, r.friend_type, r.camera_type), (1.0, 1.0, 1.0));
        assert!((r.enemy() - 0.1).abs() < 1e-6 && r.friend() == 1.0 && r.camera() == 1.0);
        let r = c.step(DT, 1.0, |_| 1.0);
        assert!(r.normal());
        assert_eq!(c.take_log(), ["enemies (ObjType ENEMY) x0.100", "enemies (ObjType ENEMY) x1.000"]);
    }

    /// (PARTY-AI) The shotlock aim's table (`SetTypeRates(CH_PLAYER, 0x4D9E, 0.4)`): unlike WorldSlow its mask holds FRIEND
    /// (bit 10), so the party's rate reaches the hosts as `friend_type` (kh-er feeds it to kh-party `Party::set_friend_rate`).
    #[test]
    fn the_shotlock_aim_slows_the_friend_type_unlike_world_slow() {
        assert_ne!(0x4D9E & (1 << OBJ_FRIEND), 0, "the aim's mask holds FRIEND");
        assert_eq!(MASK_WORLD & (1 << OBJ_FRIEND), 0, "WorldSlow's does not");
        let aim = |t: u32| if 0x4D9E & (1 << t) != 0 { 0.4 } else { 1.0 };
        let r = WorldClock::default().step(DT, 1.0, aim);
        assert_eq!((r.friend_type, r.enemy_type), (0.4, 0.4));
        assert!((r.friend() - 0.4).abs() < 1e-6);
    }

    #[test]
    fn the_counter_shield_hit_stops_ramp_linearly_and_slow_everything() {
        let mut c = WorldClock::default();
        // B418 ID 2: x0.1 in 0.08 s (4.8 steps): 1 - 11.25 t
        c.request(Some((0.1, 0.08)));
        let first = c.step(DT, 1.0, |_| 1.0).global;
        assert!((first - (1.0 - 0.9 / 0.08 * DT)).abs() < 1e-4, "{first}");
        for _ in 0..4 {
            c.step(DT, 1.0, |_| 1.0);
        }
        assert!((c.rates().global - 0.1).abs() < 1e-5, "{}", c.rates().global);
        // with a WorldSlow on as well (ComboSH's grown attack): the enemies at 0.1 x 0.1
        let r = c.step(DT, 1.0, world_slow);
        assert!((r.enemy() - 0.01).abs() < 1e-6 && (r.global - 0.1).abs() < 1e-6);
        // the end of ID 3: back x1 in 0.2 s (12 steps)
        c.request(Some((1.0, 0.2)));
        let mut n = 0;
        while c.step(DT, 1.0, |_| 1.0).global < 1.0 {
            n += 1;
            assert!(n < 20);
        }
        assert_eq!(n, 11, "0.2 s back to 1: the 12th step is at 1");
        // B426 ID 4: x0.05 in 0.2 s, ResetGlobalGameSpeed(1, 0.01): one step back
        c.request(Some((0.05, 0.2)));
        for _ in 0..12 {
            c.step(DT, 1.0, |_| 1.0);
        }
        assert!((c.rates().global - 0.05).abs() < 1e-5);
        c.request(Some((1.0, 0.01)));
        assert_eq!(c.step(DT, 1.0, |_| 1.0).global, 1.0);
    }

    #[test]
    fn another_owner_combines_as_the_game_states_channels() {
        assert_eq!(combine(1.0, 1.0), 1.0);
        assert!((combine(0.1, 0.05) - 0.05).abs() < 1e-7, "the lowest wins");
        assert!((combine(0.5, 2.0) - 1.0).abs() < 1e-7, "times the highest above 1");
        let mut c = WorldClock::default();
        c.request(Some((0.1, 0.0)));
        assert!((c.step(DT, 0.05, |_| 1.0).global - 0.05).abs() < 1e-7, "the boss finish's 0.05 under a hit-stop");
        assert!((c.form_rate() - 0.1).abs() < 1e-7);
        c.clear();
        assert!(c.rates().normal() && c.step(DT, 1.0, |_| 1.0).normal());
    }
}
