//! `TresPlayerState_Die` (id 59): combat strand E 9.8 (`build/sora_tmp/combat/P2_E_section.md`).
//! Begin restores speeds and gravity; Tick: thrown away from the causer at 2000 cm/s (Z 125), the
//! blow clips, the horizontal speed x 0.95 per frame; then it waits: RESCUE / GAMEOVER are the game
//! instance's flow, which the host owns (the `PlayerDied` event).
//!
//! (HUD-REVIVE SORAKO, THE USER 2026-11-07: "Make sure Sora as a party member is using a proper knocked down animation and
//! isn't just floating in air using his usual defeat animation.") A PARTY MEMBER'S KO (OUR DESIGN on KH3's clips; KH3 has
//! no AI Sora). KH3's player Die ends holding the blow FLIGHT loop (`0212` / `0215`: pelvis 0.91 m, the ankles 0.31 / 0.65 m
//! over the floor, the body arched as in mid-air: FK of build/sora_pack/anims/0212.glb) because the game over cuts it off
//! at once; an AI Sora lay on in it for as long as he was down, so he "floated in the air". When the core is an AI's
//! (`combat.friend.sora_is_ai` at Begin: AI Sora while a friend leads, Mimic Sora, the Mimic boss's copy) the throw is the
//! same, and then, as the friends' `UTresNpcState_Die` (Tick 0x141249F80: in the air it waits for the ground, then the down
//! loop, then on HP > 0 the get-up and Idle; End 0x141244670):
//! - AIR: the flight loop while he is airborne (KH3's own air pose of the Die state);
//! - DOWN (on the ground): `M_0230` held at [`member::DOWN_POSE_TIME`]: the only lying pose of Sora's KH3 anim set (the
//!   Beat reaction's fall; FK: pelvis 0.11 m, chest 0.16 m, head 0.38 m: flat on his back), blended in over
//!   [`member::DOWN_BLEND`] while the throw's skid brakes as before;
//! - GET-UP (HP > 0, from any source: a save point's Recover, a heal, `PartyRuntime::revive_ai_sora`): the blow family's
//!   own down + get-up clip (`Blow(index, 2)` = `0213` / `0216`, in which KH3's slam / launch landings end too: damage.rs
//!   Toss / Launch sub 15, the NPCs' 0x141246960 phase 3) from its crouch at [`member::GET_UP_FROM`], invincible meanwhile,
//!   then the state finishes into Idle (End clears the invincibility, as the friends' End does).
//!
//! The leader's Die (the flag false) is untouched: the same sub-steps and clips, and it waits for the host's game over.
//!
//! (HUD-REVIVE, THE USER 2026-11-07, refining the above: "for Sora's downed animation as a party member, have him actually use
//! his game over animation, hovering in place") AI SORA (a friend leads: `combat.friend.ko_game_over_pose`, set by
//! kh-partyhost lead.rs with `sora_is_ai`) goes down in KH3's GAMEOVER pose: no throw; at the spot he went down the movement
//! stops, gravity is off and `M_0270` (sequence 0270, 5 s) loops in place, as KH3's game-over state plays it on the pawn
//! (kh-host gameover.rs: GAMEOVER state 0x140EAD210, "clip 0270 in place", looped). The hover is the clip's own: FK of
//! build/sora_pack/anims/0270.glb, Sora lying on his back in the air, pelvis 0.62 m over his root rising to 0.74 m and back
//! over the 5 s. When his HP is above 0 gravity comes back: on the ground the get-up above, in the air Idle (he falls and
//! lands). The Mimic copies (only `sora_is_ai`) keep the lying KO above.

use glam::{Vec2, Vec3};

use super::damage::{blow_clip, param, yaw_of};
use super::{PlayerState, StateId};
use crate::anim::{AnimId, PlayArgs};
use crate::pawn::Ctx;
use crate::sora::wrap_degrees;

/// (HUD-REVIVE SORAKO) The numbers of a party member's KO (OUR DESIGN, measured on the clips: see the module notes).
pub mod member {
    /// The down pose: Sora's p_ex001_AnimSet row `M_0230` (0.467 s), held at its flattest frame (FK of
    /// build/sora_pack/anims/0230.glb: pelvis 0.11 m between 0.23 and 0.31 s, 0.16 m at its ends).
    pub const DOWN_CLIP: &str = "M_0230";
    pub const DOWN_POSE_TIME: f32 = 0.27;
    /// Seconds the down pose blends in over the flight pose at the landing.
    pub const DOWN_BLEND: f32 = 0.15;
    /// The get-up: `Blow(index, 2)` (`0213` front / `0216` back, 0.833 s, EnableChange 0.6) from its deep crouch (FK of
    /// 0213.glb: pelvis 0.59 m at 0.14 - 0.28 s) to its end: 0.63 s.
    pub const GET_UP_FROM: f32 = 0.2;
    /// Seconds the get-up clip blends in over the down pose.
    pub const GET_UP_BLEND: f32 = 0.2;
    /// (HUD-REVIVE) AI Sora's KO: KH3's game-over clip (p_ex001_AnimSet `M_0270`, 5 s), looped in place.
    pub const GAME_OVER_CLIP: &str = "M_0270";
    /// Seconds the game-over pose blends in over the pose he went down in (OUR DESIGN: the game over cuts to it behind a plate).
    pub const GAME_OVER_BLEND: f32 = 0.25;
}

/// (HUD-REVIVE SORAKO) Where a party member's KO is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MemberPose {
    /// Thrown / airborne (the leader's Die stays here for good).
    Air,
    /// On the ground, the down pose held.
    Down,
    /// HP above 0: the get-up clip, then Idle.
    GetUp,
    /// (HUD-REVIVE) AI Sora: KH3's game-over pose, hovering where he went down.
    Hover,
}

/// `TresPlayerState_Die` (59; E 9.8).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Die {
    pub(super) sub: u8,
    knock: Vec3,
    /// +0x100: the causer is behind him.
    behind: bool,
    timer: f32,
    brake_on: bool,
    /// (HUD-REVIVE SORAKO) A party member's KO (`combat.friend.sora_is_ai` at Begin): the down pose and the get-up.
    member: bool,
    pose: MemberPose,
    /// (HUD-REVIVE) AI Sora's KO (`combat.friend.ko_game_over_pose` at Begin): the game-over pose, hovering.
    hover: bool,
}

impl Die {
    pub fn new(knock: Vec3, behind: bool) -> Die {
        Die { sub: 0, knock, behind, timer: 0.0, brake_on: true, member: false, pose: MemberPose::Air, hover: false }
    }

    /// (HUD-REVIVE) AI Sora's KO: at once the game-over pose in place (no throw, gravity off); HP above 0: gravity back, the
    /// get-up on the ground, Idle in the air.
    fn hover_tick(&mut self, cx: &mut Ctx) {
        let up = cx.pawn.status.hp > 0;
        match self.pose {
            MemberPose::Hover if up => {
                cx.pawn.cmc.gravity_off = false;
                if cx.pawn.cmc.on_ground() {
                    self.get_up(cx);
                } else {
                    cx.pawn.queue.do_finish();
                }
            }
            MemberPose::Hover => cx.pawn.cmc.stop_movement(),
            MemberPose::GetUp if up => {
                if cx.pawn.anim.ended || blow_clip(self.behind as u8, 2).is_none() {
                    cx.pawn.queue.do_finish();
                }
            }
            _ => {
                // the KO itself, or down again during the get-up (a kill plane's death goes past the invincibility)
                let s = &mut *cx.pawn;
                s.status.damage_invalid = false;
                s.cmc.stop_movement();
                s.cmc.gravity_off = true;
                match AnimId::from_name(member::GAME_OVER_CLIP) {
                    Some(clip) => s.anim.play_with(clip, PlayArgs::mode(1, member::GAME_OVER_BLEND)),
                    None => s.note("Die: the game-over clip M_0270 is not in the generated animation table"),
                }
                self.brake_on = false;
                self.sub = 3;
                self.pose = MemberPose::Hover;
            }
        }
    }

    /// (HUD-REVIVE SORAKO) A party member's KO, after the throw's own tick: the ground, the down pose, the get-up.
    fn member_tick(&mut self, cx: &mut Ctx) {
        let up = cx.pawn.status.hp > 0;
        match self.pose {
            MemberPose::Air if self.sub >= 1 => {
                if cx.pawn.cmc.on_ground() {
                    if up {
                        self.get_up(cx);
                    } else {
                        self.lie_down(cx);
                    }
                } else if up {
                    // stood up again in the air (a revive before the landing): no down, no get-up: Idle, which falls and lands
                    cx.pawn.cmc.stop_movement();
                    cx.pawn.queue.do_finish();
                }
            }
            // (HUD-REVIVE) the hover is AI Sora's own tick (hover_tick), never this one
            MemberPose::Air | MemberPose::Hover => {}
            MemberPose::Down => {
                if up {
                    self.get_up(cx);
                }
            }
            MemberPose::GetUp => {
                if !up {
                    // down again during the get-up (a kill plane's death goes past the invincibility): the down pose
                    cx.pawn.status.damage_invalid = false;
                    self.lie_down(cx);
                } else if cx.pawn.anim.ended || blow_clip(self.behind as u8, 2).is_none() {
                    cx.pawn.queue.do_finish();
                }
            }
        }
    }

    /// (HUD-REVIVE SORAKO) On the ground at HP 0: the down pose, held.
    fn lie_down(&mut self, cx: &mut Ctx) {
        let s = &mut *cx.pawn;
        match AnimId::from_name(member::DOWN_CLIP) {
            Some(clip) => {
                s.anim.play_with(clip, PlayArgs::mode(0, member::DOWN_BLEND));
                s.anim.set_time(member::DOWN_POSE_TIME);
                s.anim.hold = true;
            }
            None => s.note("Die: the down clip M_0230 is not in the generated animation table"),
        }
        self.pose = MemberPose::Down;
    }

    /// (HUD-REVIVE SORAKO) HP above 0 on the ground: the blow family's get-up from its crouch, invincible (the friends' Die
    /// keeps them so until End 0x141244670), the skid stopped.
    fn get_up(&mut self, cx: &mut Ctx) {
        let s = &mut *cx.pawn;
        self.brake_on = false;
        s.cmc.stop_movement();
        s.status.damage_invalid = true;
        match blow_clip(self.behind as u8, 2) {
            Some(clip) => {
                s.anim.play_with(clip, PlayArgs::mode(0, member::GET_UP_BLEND));
                s.anim.set_time(member::GET_UP_FROM);
            }
            None => s.note("Die: the blow family's get-up clip (0213 / 0216) is not in the generated animation table"),
        }
        self.pose = MemberPose::GetUp;
    }
}

impl PlayerState for Die {
    const ID: StateId = StateId::Die;

    fn sub_step(&self) -> u8 {
        // (HUD-REVIVE SORAKO) a party member's down pose 4, his get-up 5
        match self.pose {
            MemberPose::Air => self.sub,
            MemberPose::Down => 4,
            MemberPose::GetUp => 5,
            MemberPose::Hover => 6,
        }
    }

    fn begin(&mut self, cx: &mut Ctx) {
        let s = &mut *cx.pawn;
        let p = *s.params();
        s.cmc.max_walk_speed = p.speed_run;
        s.cmc.gravity_scale = p.gravity_scale;
        s.cmc.gravity_off = false;
        s.cmc.stop_movement();
        self.member = s.combat.friend.sora_is_ai; // (HUD-REVIVE SORAKO)
        self.hover = self.member && s.combat.friend.ko_game_over_pose; // (HUD-REVIVE)
    }

    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        // (audio implementer) Death: voice group "damage_dead" unit 1 (audio spec 5.2).
        if self.sub == 0 {
            cx.set_voice_group_no(crate::host::audio::DEATH_VOICE_GROUP_NO);
        }
        if self.hover {
            self.hover_tick(cx);
            return;
        }
        let s = &mut *cx.pawn;
        let index = self.behind as u8;
        // (HUD-REVIVE SORAKO) the flight loop only while he flies (always, for the leader)
        let flying = self.pose == MemberPose::Air;
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
                // (HUD-REVIVE SORAKO) a member landed during the start clip: no flight loop, the brake's timer all the same
                if s.anim.ended || !flying {
                    if flying {
                        if let Some(clip) = blow_clip(index, 1) {
                            s.anim.play_with(clip, PlayArgs::mode(2, -1.0));
                        }
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
        if self.member {
            self.member_tick(cx);
        }
    }

    fn end(&mut self, cx: &mut Ctx) {
        // (HUD-REVIVE SORAKO) the get-up's invincibility goes with the state (End 0x141244670 of the friends' Die)
        if self.pose == MemberPose::GetUp {
            cx.pawn.status.damage_invalid = false;
        }
        // (HUD-REVIVE) the hover's gravity comes back with the state, whatever ends it
        if self.hover {
            cx.pawn.cmc.gravity_off = false;
        }
    }
}

