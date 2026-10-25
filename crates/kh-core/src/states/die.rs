//! `TresPlayerState_Die` (id 59): combat strand E 9.8 (`build/sora_tmp/combat/P2_E_section.md`).
//! Begin restores speeds and gravity; Tick: thrown away from the causer at 2000 cm/s (Z 125), the
//! blow clips, the horizontal speed x 0.95 per frame; then it waits: RESCUE / GAMEOVER are the game
//! instance's flow, which the host owns (the `PlayerDied` event).

use glam::{Vec2, Vec3};

use super::damage::{blow_clip, param, yaw_of};
use super::{PlayerState, StateId};
use crate::anim::PlayArgs;
use crate::pawn::Ctx;
use crate::sora::wrap_degrees;

/// `TresPlayerState_Die` (59; E 9.8).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Die {
    pub(super) sub: u8,
    knock: Vec3,
    /// +0x100: the causer is behind him.
    behind: bool,
    timer: f32,
    brake_on: bool,
}

impl Die {
    pub fn new(knock: Vec3, behind: bool) -> Die {
        Die { sub: 0, knock, behind, timer: 0.0, brake_on: true }
    }
}

impl PlayerState for Die {
    const ID: StateId = StateId::Die;

    fn sub_step(&self) -> u8 {
        self.sub
    }

    fn begin(&mut self, cx: &mut Ctx) {
        let s = &mut *cx.pawn;
        let p = *s.params();
        s.cmc.max_walk_speed = p.speed_run;
        s.cmc.gravity_scale = p.gravity_scale;
        s.cmc.gravity_off = false;
        s.cmc.stop_movement();
    }

    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        // (audio implementer) Death: voice group "damage_dead" unit 1 (audio spec 5.2).
        if self.sub == 0 {
            cx.set_voice_group_no(crate::host::audio::DEATH_VOICE_GROUP_NO);
        }
        let s = &mut *cx.pawn;
        let index = self.behind as u8;
        match self.sub {
            0 => {
                s.cmc.set_falling();
                s.cmc.set_velocity_xy(self.knock.truncate() * param::DIE_POWER);
                s.cmc.set_velocity_z(param::DIE_VELOCITY_Z);
                s.yaw = wrap_degrees(if self.behind { yaw_of(self.knock) } else { yaw_of(-self.knock) });
                if let Some(clip) = blow_clip(index, 0) {
                    s.anim.play_with(clip, PlayArgs::mode(2, 0.0));
                }
                self.sub = 1;
                return;
            }
            1 => {
                if s.anim.ended {
                    if let Some(clip) = blow_clip(index, 1) {
                        s.anim.play_with(clip, PlayArgs::mode(2, -1.0));
                    }
                    self.timer = 0.5;
                    self.sub = 2;
                }
            }
            2 => {
                self.timer -= dt;
                if self.timer <= 0.0 {
                    self.brake_on = false;
                    s.cmc.stop_movement();
                    self.sub = 3;
                }
            }
            // Waits: the game-over flow is the host's (the `PlayerDied` event).
            _ => {}
        }
        if self.brake_on {
            let v: Vec2 = s.cmc.velocity.truncate() * param::DIE_BRAKE.powf(dt * 60.0);
            s.cmc.set_velocity_xy(v);
        }
    }

    fn end(&mut self, _cx: &mut Ctx) {}
}

