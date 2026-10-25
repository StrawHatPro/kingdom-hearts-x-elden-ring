//! (ER party S7) THE TEAM ATTACKS AND THEIR CAMERAS IN ELDEN RING. Plan: build/sora_tmp/er/party/ER_PARTY_PLAN.md
//! stream S7; state note build/sora_tmp/er/party/IMPL_S7_STATE.md. Cargo feature `party`, key `party.er` (0: nothing of
//! this runs, the camera is Sora's exactly as before the party).
//!
//! THE NINE (KH3's own code, kh-party `team` + kh-core's FR states, stepped by S1's glue in the contract's order):
//! - Unison Fire / Blizzard / Thunder, Fusion Spin / Rocket: the finisher of the LEADER's combo next to his partner
//!   (Sora's A combo next to Donald / Goofy; a friend leader's pad combo rolls `roll_leader_unison`, AI Sora plays it);
//! - Donald Meteor, Flare Force, Goofy Bombardier, Trinity Guard: the friends' offers in Sora's situation list, run by
//!   the PLAYER's Y (kh-partyhost `route`: the pad's Y reaches Sora's core whoever leads; AI Sora never presses Y and his
//!   own combos never roll a unison: `FriendState::sora_is_ai`). AN AI MEMBER NEVER STARTS ONE.
//!
//! THE HITS: every team hit (the link pawns' shots as Sora's, the friends' rows, the meteors / missiles) goes through
//! the party's targets = `ErTargets::apply_hit`: THE PLAYER'S BULLET with KH3's damage (`damage.out.scale`), its reaction
//! (puppet.rs: juggles on what may fly, a flinch on what may not, nothing through super armour / poise until it breaks,
//! bossguard.rs on bosses) and its poise damage. U14 (bullets owned by the friend's body) stays behind S3's
//! `party.bullet_owner`. A character the game does not draw or has not spawned (`target.visible_only`: inactive) is no
//! target at all: no ALIVE / HITTABLE / LOCKABLE flag, so no link pawn, meteor, missile or rush picks it or hits it.
//!
//! THE CAMERAS (cam.rs `PartyCamera`, implemented here over the party core): the field / lock-on camera follows the
//! LEADER (`rt.camera_target`, his RB lock: `feed.lock_change`); the team attacks' state-event cameras (CameraAnims,
//! the orbit cameras, shield_01 steered by the player's right stick) are laid over it with their collision pass
//! against THE CAMERA'S filter of Elden Ring's world (cam.rs's `ErWorld`, `FILTER_CAMERA`): a camera never goes into
//! the terrain; their FOV and shakes reach the drawn lens; their end blends back and puts the field camera behind the
//! leader. Their base actor is Sora's, as in KH3 (the FR states and link pawns are laid out round him). The boss
//! finish's cut stays on top (a team hit that kills a boss starts it as Sora's blow does: the game's confirmed death of
//! a body any party member hit).
//!
//! DRAWING (for S4): `props(rt)` = the meteors / missiles in flight (class, index, actor transform, scale);
//! `rt.team.mesh_pitch(member)` = Goofy's pitch in his Bombardier rush; `sora_pitch` = Sora's pitch on the GoofyShoot
//! link pawn's socket (applied to Sora's own draw in sora.rs). The friends in their FR states are ordinary members
//! (their views hold the transforms the link pawns give).

use glam::{Quat, Vec3};
use kh_anim::Trs;
use kh_camera::shake::ShakeOffset;
use kh_camera::{CameraInput, CameraView};
use kh_core::{EventKind, Output, StateId};
use kh_partyhost::PartyRuntime;

/// The nine team attacks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TeamAttack {
    UnisonFire,
    UnisonBlizzard,
    UnisonThunder,
    FusionSpin,
    FusionRocket,
    DonaldMeteor,
    FlareForce,
    GoofyBombardier,
    TrinityGuard,
}

/// How one is started by the player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    /// The finisher of the leader's combo (unison kind 22..26 = attack kinds 0x3F..0x43).
    Unison(u8),
    /// A friend's offer in the situation list (command kind), run by Y.
    Offer(i32),
}

impl TeamAttack {
    pub const ALL: [TeamAttack; 9] = [TeamAttack::UnisonFire, TeamAttack::UnisonBlizzard, TeamAttack::UnisonThunder, TeamAttack::FusionSpin, TeamAttack::FusionRocket, TeamAttack::DonaldMeteor, TeamAttack::FlareForce, TeamAttack::GoofyBombardier, TeamAttack::TrinityGuard];

    pub const fn name(self) -> &'static str {
        match self {
            TeamAttack::UnisonFire => "Unison Fire",
            TeamAttack::UnisonBlizzard => "Unison Blizzard",
            TeamAttack::UnisonThunder => "Unison Thunder",
            TeamAttack::FusionSpin => "Fusion Spin",
            TeamAttack::FusionRocket => "Fusion Rocket",
            TeamAttack::DonaldMeteor => "Donald Meteor",
            TeamAttack::FlareForce => "Flare Force",
            TeamAttack::GoofyBombardier => "Goofy Bombardier",
            TeamAttack::TrinityGuard => "Trinity Guard",
        }
    }

    pub const fn start(self) -> Start {
        match self {
            TeamAttack::UnisonFire => Start::Unison(22),
            TeamAttack::UnisonBlizzard => Start::Unison(23),
            TeamAttack::UnisonThunder => Start::Unison(24),
            TeamAttack::FusionSpin => Start::Unison(25),
            TeamAttack::FusionRocket => Start::Unison(26),
            TeamAttack::DonaldMeteor => Start::Offer(207),
            TeamAttack::FlareForce => Start::Offer(208),
            TeamAttack::GoofyBombardier => Start::Offer(195),
            TeamAttack::TrinityGuard => Start::Offer(193),
        }
    }

    /// The attack kind (Sora's combat history: 0x3F..0x43) or the command kind (`Team::running`, `TeamAttack` events).
    pub fn kind(self) -> i32 {
        match self.start() {
            Start::Unison(u) => kh_core::combat::friend::unison_attack_kind(u) as i32,
            Start::Offer(k) => k,
        }
    }

    /// The one a command kind / attack kind names.
    pub fn of_kind(kind: i32) -> Option<TeamAttack> {
        TeamAttack::ALL.into_iter().find(|a| a.kind() == kind)
    }
}

/// Sora is in one of his four FR states (a team attack holds him).
pub fn sora_in_team_attack(state: StateId) -> bool {
    matches!(state, StateId::FrShieldGuardian | StateId::FrGoofyshoot | StateId::FrCallmeteor | StateId::FrFlareforce)
}

/// The command kinds of the four offered team attacks.
pub fn is_team_command(kind: i32) -> bool {
    matches!(kind, 193 | 195 | 207 | 208)
}

/// Seconds the player's refused Y is held for Sora's executor while a FRIEND leads (`YHold`).
pub const Y_HOLD: f32 = 0.5;

/// (port design, a friend leading only) THE PLAYER'S Y ON A TEAM COMMAND IS NOT LOST TO AI SORA'S BUSINESS. With a
/// friend leading, the team attacks still run through Sora's core, and its executor refuses the command while something
/// of AI Sora's own is queued (an attack, a spell he chose: `try_team_attack`'s `nothing_queued`); KH3 has no buffer
/// because there the player IS Sora. Measured offline (team/tests.rs): Donald leading, the first Y on Flare Force was
/// refused while AI Sora queued Aeroga. So the press is held for `Y_HOLD` s (pressed again every step) until Sora's FR
/// state begins, the team command is no longer current, or the time is out. `party.team_y_hold = 0`: off.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct YHold {
    pub left: f32,
}

impl YHold {
    /// One fixed step BEFORE `route`: true = add a pressed Y to the pad of this step.
    pub fn step(&mut self, dt: f32, friend_leads: bool, pressed_y: bool, shoulders_held: bool, team_current: bool, sora_in_fr: bool) -> bool {
        if !friend_leads || sora_in_fr || !team_current || shoulders_held {
            self.left = 0.0;
            return false;
        }
        if pressed_y {
            self.left = Y_HOLD;
            return false;
        }
        if self.left > 0.0 {
            self.left -= dt;
            return true;
        }
        false
    }
}

/// The input of one step with the held Y added (`YHold::step` over Sora's situation list and state).
pub fn held_y_input(hold: &mut YHold, dt: f32, friend_leads: bool, sora: &kh_core::Sora, input: kh_core::Input) -> kh_core::Input {
    use kh_core::PadButton;
    let team_current = sora.combat().situation.current().is_some_and(|(_, k)| is_team_command(k));
    let shoulders = input.pad.held.has(PadButton::LeftShoulder) || input.pad.held.has(PadButton::RightShoulder);
    if hold.step(dt, friend_leads, input.pad.pressed.has(PadButton::SITUATION), shoulders, team_current, sora_in_team_attack(sora.output().state)) {
        let mut input = input;
        input.pad.pressed.set(PadButton::SITUATION, true);
        input.pad.held.set(PadButton::SITUATION, true);
        return input;
    }
    input
}

/// What the team cameras did to one step's view (the harness reads all of it; cam.rs the lens).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LayerOut {
    /// Put the field camera behind the leader (the blueprints' reset).
    pub reset_field: bool,
    /// The CameraAnim's horizontal FOV while a team camera has the view.
    pub fov: Option<f32>,
    pub shake: ShakeOffset,
    /// The running event cameras, oldest first.
    pub cameras: Vec<&'static str>,
}

/// (ZFIN round 7) Zettaflare's event white this step (0..1) and whether its cinematic has the view: kh-er finish.rs draws
/// the white on the plate, hud.rs hides KH3's HUD and the lock-on reticle meanwhile.
pub static ZETTA_WHITE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
pub static ZETTA_CINEMATIC: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn zetta_white() -> f32 {
    f32::from_bits(ZETTA_WHITE.load(std::sync::atomic::Ordering::Relaxed))
}

pub fn zetta_cinematic() -> bool {
    ZETTA_CINEMATIC.load(std::sync::atomic::Ordering::Relaxed)
}

/// THE CAMERA LAYERING of one fixed step (cam.rs through `TeamCam`, and the offline harness): kh-partyhost's
/// `team_camera` (the step's events and the link pawns' actor actions into the state-event cameras, then their view
/// with the collision pass against `world`) over `view`.
#[allow(clippy::too_many_arguments)]
pub fn layer(rt: &mut PartyRuntime, dt: f32, events: &[kh_core::Event], sora: &Output, view: &mut CameraView, world: &dyn kh_core::World, stick: CameraInput) -> LayerOut {
    let reset_field = rt.team_camera(dt, events, sora, view, world, stick);
    // (ZFIN round 7) Zettaflare's event white (the test level's white plate, finish.rs) and its cinematic (the HUD hidden)
    ZETTA_WHITE.store(rt.zetta_white.clamp(0.0, 1.0).to_bits(), std::sync::atomic::Ordering::Relaxed);
    ZETTA_CINEMATIC.store(rt.zetta_cinematic, std::sync::atomic::Ordering::Relaxed);
    LayerOut { reset_field, fov: rt.team_fov, shake: rt.team_shake, cameras: rt.cams.cameras() }
}

/// The once-per-attack log (the team attack began / ended, which cameras ran, how often the collision pass pulled a
/// camera out of the terrain).
#[derive(Clone, Debug, Default)]
pub struct Watch {
    pub running: Option<i32>,
    pub steps: u32,
    pub cameras: Vec<&'static str>,
    /// Steps a team camera had the view.
    pub camera_steps: u32,
}

impl Watch {
    /// One step: the lines to say.
    pub fn step(&mut self, events: &[kh_core::Event], out: &LayerOut, team_running: i32, now: f64) -> Vec<String> {
        let mut lines = Vec::new();
        for e in events {
            match e.kind {
                EventKind::TeamAttack { kind, phase: 0 } => {
                    self.running = Some(kind);
                    self.steps = 0;
                    self.cameras.clear();
                    self.camera_steps = 0;
                    lines.push(format!("team: t={now:.3}s {} STARTS (kind {kind}) by the player's command", TeamAttack::of_kind(kind).map_or("a team attack", |a| a.name())));
                }
                EventKind::TeamAttack { kind, phase: 255 } => {
                    lines.push(format!("team: t={now:.3}s {} ENDS after {} step(s); cameras {:?} had the view in {} step(s); the field camera goes back behind the leader", TeamAttack::of_kind(kind).map_or("a team attack", |a| a.name()), self.steps, self.cameras, self.camera_steps));
                    self.running = None;
                }
                _ => {}
            }
        }
        if self.running.is_some() || team_running != 0 {
            self.steps += 1;
            if out.fov.is_some() {
                self.camera_steps += 1;
            }
            for c in &out.cameras {
                if !self.cameras.contains(c) {
                    self.cameras.push(c);
                }
            }
        }
        lines
    }
}

/// A team prop in flight (Call Meteor's meteors, Flare Force's missiles) for the renderer (S4): the actor in KH3 space
/// (the model's own relative rotation from `PACK/party/team/props/props.json` goes on top, as kh-app party_draw.rs).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TeamProp {
    pub class: &'static str,
    /// The prop's model under `PACK/party/team` (kh-er-render `CharDraw::props` of the TEAM draw).
    pub model: &'static str,
    pub index: usize,
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: f32,
}

impl TeamProp {
    /// The actor as a transform in KH3 space (cm).
    pub fn trs(&self) -> Trs {
        Trs { translation: self.position, rotation: self.rotation, scale: Vec3::splat(self.scale) }
    }
}

/// The model of a prop class (tools/party_team_pack.js: Call Meteor's rock, Flare Force's missile).
pub fn prop_model(class: &str) -> &'static str {
    if class.contains("CallMeteor") {
        "o_ex210_0"
    } else {
        "o_ex208_0"
    }
}

/// The props to draw this step (the visible, living ones), with kh-app's actor rotation: the meteors' spawn spin while
/// they hover, else the facing.
pub fn props(rt: &PartyRuntime) -> Vec<TeamProp> {
    // (party X3D) + Donald's solo Meteor Drop meteors and Flare Force missiles (kh-party pad/donald.rs `party_props`)
    let kit = kh_party::pad::donald::party_props(&rt.party);
    rt.team
        .projs
        .iter()
        .chain(kit.iter())
        .filter(|p| p.visible && p.alive)
        .map(|p| {
            let f = p.forward.try_normalize().unwrap_or(Vec3::X);
            let rotation = match p.spin {
                Some((pitch, roll)) => kh_partyhost::poser::ue_rotator(pitch, 0.0, roll),
                None => kh_partyhost::poser::ue_rotator(f.z.clamp(-1.0, 1.0).asin().to_degrees(), f.y.atan2(f.x).to_degrees(), 0.0),
            };
            TeamProp { class: p.kind.class(), model: prop_model(p.kind.class()), index: p.index, position: p.position, rotation, scale: p.scale.max(0.001) }
        })
        .collect()
}

/// SORA'S DRAW PITCHED on Goofy Bombardier's link pawn (`Team::sora_mesh_pitch`, UE degrees, nose up): his mesh turns
/// about his feet in his actor frame (kh-app party_draw.rs `team_sora_pitch`). `to_world` is his mesh's transform
/// (actor yaw then the mesh's -90 yaw, sora.rs `component_to_world`).
pub fn pitch_sora(to_world: &mut Trs, pitch: Option<f32>) {
    let Some(p) = pitch.filter(|p| p.is_finite() && *p != 0.0) else { return };
    // mesh = R_actor * Rz(-90): pitched = R_actor * Ry(-p) * Rz(-90) = mesh * Rz(90) * Ry(-p) * Rz(-90)
    let mesh_yaw = Quat::from_rotation_z((-90.0f32).to_radians());
    to_world.rotation = (to_world.rotation * mesh_yaw.inverse() * Quat::from_rotation_y(-p.to_radians()) * mesh_yaw).normalize();
}

/// Goofy Bombardier's socket tables (`node tools/party_team_pack.js --gshoot` -> build/gen/rust/party_team_gshoot.rs)
/// were compiled into kh-party (without them the riders stay at the GShootStart pose).
pub fn gshoot_compiled() -> bool {
    !kh_party::team::link::GSHOOT_LINK_CLIPS.is_empty()
}

#[cfg(windows)]
pub use host::*;

#[cfg(windows)]
mod host {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;

    use glam::Vec3;
    use kh_camera::shake::ShakeOffset;
    use kh_camera::{CameraInput, CameraTarget, CameraView};
    use kh_core::{Output, TargetId};

    use super::{layer, Watch};
    use crate::party::glue::PartyCore;
    use crate::party::PartySlot;

    static WATCH: Mutex<Option<Watch>> = Mutex::new(None);
    static TOLD: AtomicBool = AtomicBool::new(false);
    static YHOLD: Mutex<super::YHold> = Mutex::new(super::YHold { left: 0.0 });

    /// sora.rs, every fixed step right BEFORE the party's `route`: the player's refused Y on a team command held for
    /// Sora's executor while a friend leads (`super::YHold`; `party.team_y_hold = 0`: off).
    pub fn hold_y(slot: &PartySlot, sora: &kh_core::Sora, input: kh_core::Input, dt: f32, control: &crate::control::Control) -> kh_core::Input {
        let friend_leads = control.flag("party.team_y_hold") != Some(false) && slot.core().is_some_and(|c| c.rt.friend_leads());
        let Ok(mut hold) = YHOLD.try_lock() else { return input };
        super::held_y_input(&mut hold, dt, friend_leads, sora, input)
    }

    /// The party's side of KH3's camera for one step (cam.rs `PartyCamera`).
    pub struct TeamCam<'a> {
        core: &'a mut PartyCore,
    }

    /// The party's camera side while the party runs (`None`: `party.er = 0` or no core: the camera is Sora's alone).
    pub fn camera(slot: &mut PartySlot) -> Option<TeamCam<'_>> {
        let core = slot.core_mut()?;
        if !TOLD.swap(true, Ordering::Relaxed) {
            let gshoot = kh_party::team::link::GSHOOT_LINK_CLIPS.len();
            core.say(if gshoot > 0 {
                format!("team: the team attacks run on Y for the leader only; Goofy Bombardier's socket tables are compiled in ({gshoot} clips)")
            } else {
                "team: the team attacks run on Y for the leader only; !!! Goofy Bombardier's socket tables are NOT compiled in (run `node tools/party_team_pack.js --gshoot` before the build): Sora and Goofy ride its link pawn at the start pose".to_string()
            });
        }
        Some(TeamCam { core })
    }

    impl crate::cam::PartyCamera for TeamCam<'_> {
        fn leader_target(&self, dt: f32) -> Option<CameraTarget> {
            self.core.rt.camera_target(dt)
        }

        fn lock_change(&mut self) -> Option<Option<(u32, Vec3)>> {
            self.core.feed.lock_change.take().map(|c| c.at)
        }

        fn leader_lock(&self) -> Option<TargetId> {
            self.core.rt.leader_lock()
        }

        fn team_layer(&mut self, dt: f32, events: &[kh_core::Event], sora: &Output, view: &mut CameraView, world: &dyn kh_core::World, stick: CameraInput) -> bool {
            let out = layer(&mut self.core.rt, dt, events, sora, view, world, stick);
            let running = self.core.rt.team.running();
            // (the log's clock: Sora's fixed steps)
            let now = sora.step as f64 / 60.0;
            let lines = match WATCH.try_lock() {
                Ok(mut w) => w.get_or_insert_with(Watch::default).step(events, &out, running, now),
                Err(_) => Vec::new(),
            };
            for l in lines {
                self.core.say(l);
            }
            out.reset_field
        }

        fn team_lens(&self) -> (Option<f32>, ShakeOffset) {
            (self.core.rt.team_fov, self.core.rt.team_shake)
        }
    }
}

#[cfg(test)]
mod tests;
