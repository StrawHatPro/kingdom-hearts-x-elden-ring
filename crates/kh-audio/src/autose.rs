//! AUTO SE: footsteps, jump and landing sounds and cloth rustle from the motion of the bones
//! (docs/sora-audio-spec.md section 4; component `SQEXSEADAutoSeComponent` with `BoneSet_Sora1`,
//! `DetectioinSetting_Sora`; analyzer 0x14594E6D0, detectors 0x14597D600 / 0x14597D930 /
//! 0x14597E180). No animation of Sora carries a footstep notify: this detector is where his
//! steps come from.
//!
//! The detector works in METRES (bone positions x 0.01), once per frame after the pose. THE
//! ORIGINAL IS FRAME-RATE DEPENDENT (finite differences per rendered frame). THE PORT RUNS IT AT
//! THE SIMULATION STEP, the fixed 60 Hz reference rate: that is the game at 60 fps.
//!
//! THE FOOT-CONTACT FLAG (spec open question 4): for Tres pawns a step is normally the rising
//! edge of the pawn's foot-contact flag (leg-IK array of the mesh, entry `+0x394 & 5`), whose
//! exact meaning was not established. The port implements the LITERAL reading of spec 4.2: a
//! step = the rising edge of the contact flag when the host supplies one (`foot_contact`), OR the
//! rising edge of the geometric foot-grounded test with the previous foot speed above 0.125 m/s.
//! `kh-app` supplies no flag (its leg IK has no equivalent whose meaning is known), so there the
//! geometric rule alone makes the steps.

use glam::Vec3;

use crate::rules::RULES;

/// `ESQEXSEADAutoSeMotionSoundType` (the values the table is indexed with, spec 4.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Motion {
    Walk = 1,
    Run = 2,
    Jump = 3,
    LandNormal = 4,
    LandHard = 5,
    FootShuffle = 6,
    RustleShort = 7,
    RustleLong = 8,
    RustleFootShort = 9,
    RustleFootLong = 10,
}

impl Motion {
    pub const ALL: [Motion; 10] = [Motion::Walk, Motion::Run, Motion::Jump, Motion::LandNormal, Motion::LandHard, Motion::FootShuffle, Motion::RustleShort, Motion::RustleLong, Motion::RustleFootShort, Motion::RustleFootLong];

    pub fn name(self) -> &'static str {
        match self {
            Motion::Walk => "Walk",
            Motion::Run => "Run",
            Motion::Jump => "Jump",
            Motion::LandNormal => "LandNormal",
            Motion::LandHard => "LandHard",
            Motion::FootShuffle => "FootShuffle",
            Motion::RustleShort => "RustleShort",
            Motion::RustleLong => "RustleLong",
            Motion::RustleFootShort => "RustleFootShort",
            Motion::RustleFootLong => "RustleFootLong",
        }
    }

    pub fn from_name(name: &str) -> Option<Motion> {
        Motion::ALL.into_iter().find(|m| m.name() == name)
    }

    /// Types 1..6 are looked up per surface, the rest are not (spec 4.1).
    pub fn per_surface(self) -> bool {
        (self as u32) <= 6
    }
}

/// The part an event is played at (attached to its bone).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Part {
    /// 0 left, 1 right: bone `L_ashi1` / `R_ashi1`.
    Foot(usize),
    /// 0 left, 1 right: the shoulder `L_ude1` / `R_ude1`.
    Arm(usize),
}

impl Part {
    pub fn bone(self) -> &'static str {
        match self {
            Part::Foot(0) => "L_ashi1",
            Part::Foot(_) => "R_ashi1",
            Part::Arm(0) => "L_ude1",
            Part::Arm(_) => "R_ude1",
        }
    }
}

/// The bones `BoneSet_Sora1` names, in the order `BoneFrame::from_positions` takes them.
pub const BONES: [&str; 14] = ["Trans", "koshi", "L_momo", "L_sune", "L_ashi1", "L_ashi2", "R_momo", "R_sune", "R_ashi1", "R_ashi2", "L_ude1", "L_ude2", "R_ude1", "R_ude2"];

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FootBones {
    pub femur: Vec3,
    pub tibia: Vec3,
    pub foot: Vec3,
    pub toe: Vec3,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ArmBones {
    pub shoulder: Vec3,
    pub elbow: Vec3,
}

/// One frame of input: WORLD positions in cm, core axes (Z up).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BoneFrame {
    /// `Trans`.
    pub base: Vec3,
    /// `koshi`.
    pub waist: Vec3,
    pub feet: [FootBones; 2],
    pub arms: [ArmBones; 2],
    /// Movement mode walking (I: `IsMovingOnGround`).
    pub grounded: bool,
    /// The pawn's foot-contact flag per foot, when the host has one (see the module text).
    pub foot_contact: [Option<bool>; 2],
    /// The owner is hidden: nothing is analysed, and the detector warms up again afterwards.
    pub hidden: bool,
}

impl BoneFrame {
    /// From the positions of `BONES`, in that order.
    pub fn from_positions(p: &[Vec3; 14], grounded: bool) -> BoneFrame {
        BoneFrame {
            base: p[0],
            waist: p[1],
            feet: [FootBones { femur: p[2], tibia: p[3], foot: p[4], toe: p[5] }, FootBones { femur: p[6], tibia: p[7], foot: p[8], toe: p[9] }],
            arms: [ArmBones { shoulder: p[10], elbow: p[11] }, ArmBones { shoulder: p[12], elbow: p[13] }],
            grounded,
            foot_contact: [None; 2],
            hidden: false,
        }
    }
}

/// A detected motion sound.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AutoSeEvent {
    pub motion: Motion,
    pub part: Part,
    /// `clamp(event volume x VolumeScale)`, VolumeScale 1.0.
    pub volume: f32,
}

/// One analysed frame (metres).
#[derive(Clone, Copy, Debug, Default)]
struct Analysis {
    grounded: bool,
    waist: Vec3,
    waist_speed: f32,
    running: bool,
    foot: [Vec3; 2],
    heel: [Vec3; 2],
    knee: [Vec3; 2],
    elbow: [Vec3; 2],
    foot_speed: [f32; 2],
    heel_speed: [f32; 2],
    knee_speed: [f32; 2],
    foot_grounded: [bool; 2],
    contact: [Option<bool>; 2],
    /// Elbow speed minus waist speed.
    arm_rel: [f32; 2],
    /// dot(normalize(Tibia - Femur), normalize(Base - Waist)).
    leg_dot: [f32; 2],
    air_time: f32,
}

/// The detector's state.
#[derive(Clone, Debug)]
pub struct AutoSe {
    previous: Option<Analysis>,
    /// Frames still to skip (after a reset: 3).
    warmup: i32,
    cooldown_foot: [f32; 2],
    cooldown_arm: [f32; 2],
    was_hidden: bool,
}

impl Default for AutoSe {
    fn default() -> AutoSe {
        AutoSe::new()
    }
}

fn clamp01(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

fn map(v: f32, range: [f32; 2]) -> f32 {
    (v - range[0]) / (range[1] - range[0])
}

impl AutoSe {
    pub fn new() -> AutoSe {
        AutoSe { previous: None, warmup: RULES.autose_warmup_frames, cooldown_foot: [0.0; 2], cooldown_arm: [0.0; 2], was_hidden: false }
    }

    /// Forgets the history (a teleport, an attach change): no event for the next 3 frames.
    pub fn reset(&mut self) {
        self.previous = None;
        self.warmup = RULES.autose_warmup_frames;
    }

    /// One frame. `listener_distance`: metres from the listener to `Trans`. `enabled`: the
    /// component's enable flag (the `ChangeAutoSeEnable` notify windows, spec 4.3). At most 2
    /// events, in the detector order Landing, FootStep, Rustle.
    pub fn step(&mut self, dt: f32, frame: &BoneFrame, listener_distance: f32, enabled: bool) -> Vec<AutoSeEvent> {
        let mut events = Vec::new();
        for c in self.cooldown_foot.iter_mut().chain(self.cooldown_arm.iter_mut()) {
            *c = (*c - dt).max(0.0);
        }
        // Hidden, a visibility change, or beyond the clip distance (20 m): no analysis.
        if frame.hidden != self.was_hidden {
            self.was_hidden = frame.hidden;
            self.reset();
        }
        if frame.hidden || dt <= 0.0 {
            return events;
        }
        if listener_distance > RULES.autose_clip_distance {
            self.reset();
            return events;
        }

        // The analyzer.
        const M: f32 = 0.01;
        let prev = self.previous;
        let mut now = Analysis { grounded: frame.grounded, waist: frame.waist * M, contact: frame.foot_contact, ..Analysis::default() };
        let base = frame.base * M;
        let speed = |now: Vec3, before: Option<Vec3>| before.map_or(0.0, |b| (now - b).length() / dt);
        now.waist_speed = speed(now.waist, prev.map(|p| p.waist));
        now.running = now.waist_speed >= RULES.autose_run_speed;
        let spine = (base - now.waist).normalize_or_zero();
        for k in 0..2 {
            let f = &frame.feet[k];
            let (femur, tibia, foot) = (f.femur * M, f.tibia * M, f.foot * M);
            now.foot[k] = foot;
            // The heel is computed: Tibia + (Foot - Tibia) x 1.2.
            now.heel[k] = tibia + (foot - tibia) * RULES.autose_heel_ratio;
            now.knee[k] = tibia;
            now.foot_speed[k] = speed(foot, prev.map(|p| p.foot[k]));
            now.heel_speed[k] = speed(now.heel[k], prev.map(|p| p.heel[k]));
            now.knee_speed[k] = speed(tibia, prev.map(|p| p.knee[k]));
            // Foot grounded = body grounded and (heel.up - Base.up) < 0.2 x |Tibia - Foot|.
            now.foot_grounded[k] = frame.grounded && (now.heel[k].z - base.z) < RULES.autose_foot_grounded_ratio * (tibia - foot).length();
            now.leg_dot[k] = (tibia - femur).normalize_or_zero().dot(spine);
            now.elbow[k] = frame.arms[k].elbow * M;
            now.arm_rel[k] = speed(now.elbow[k], prev.map(|p| p.elbow[k])) - now.waist_speed;
        }
        // Air time runs while no foot is grounded.
        now.air_time = if now.foot_grounded.iter().any(|g| *g) { 0.0 } else { prev.map_or(0.0, |p| p.air_time) + dt };

        self.previous = Some(now);
        let Some(prev) = prev else { return events };
        if self.warmup > 0 {
            self.warmup -= 1;
            return events;
        }
        if !enabled {
            return events;
        }

        let limit = RULES.autose_events_per_frame as usize;
        let take_off = prev.grounded && !now.grounded;
        let landing = !prev.grounded && now.grounded;

        // 1. Landing detector (0x14597D930): Jump on take-off (exempt from the cooldown),
        // LandNormal / LandHard on touch-down by air time. Played at the left foot.
        if take_off {
            events.push(AutoSeEvent { motion: Motion::Jump, part: Part::Foot(0), volume: 1.0 });
        } else if landing && prev.air_time >= RULES.autose_land_normal_air_time && self.cooldown_foot[0] <= 0.0 {
            let motion = if prev.air_time >= RULES.autose_land_hard_air_time { Motion::LandHard } else { Motion::LandNormal };
            events.push(AutoSeEvent { motion, part: Part::Foot(0), volume: 1.0 });
            self.cooldown_foot[0] = RULES.autose_part_cooldown;
        }

        // 2. FootStep detector (0x14597D600): the first foot that matches.
        for k in 0..2 {
            if events.len() >= limit {
                break;
            }
            let add = clamp01(map((0.5 * now.foot_speed[k] * now.foot_speed[k] - 0.5 * prev.foot_speed[k] * prev.foot_speed[k]).abs(), RULES.autose_volume_add_energy_range));
            let shuffle = prev.foot_grounded[k] && now.foot_grounded[k] && now.waist_speed < RULES.autose_shuffle_body_velocity && ((now.heel_speed[k] - prev.heel_speed[k]) / dt).abs() > RULES.autose_shuffle_foot_accel;
            let contact_edge = matches!((prev.contact[k], now.contact[k]), (Some(false), Some(true)));
            let grounded_edge = !prev.foot_grounded[k] && now.foot_grounded[k] && prev.foot_speed[k] > RULES.autose_step_min_foot_speed;
            let event = if shuffle {
                Some((Motion::FootShuffle, clamp01(map(now.waist_speed, RULES.autose_shuffle_volume_speed_range) + add)))
            } else if contact_edge || grounded_edge {
                let (motion, range) = if now.running { (Motion::Run, RULES.autose_run_volume_speed_range) } else { (Motion::Walk, RULES.autose_walk_volume_speed_range) };
                Some((motion, clamp01(map(now.waist_speed, range) + add)))
            } else {
                None
            };
            if let Some((motion, volume)) = event {
                // Events of a part in cooldown are dropped.
                if self.cooldown_foot[k] <= 0.0 {
                    events.push(AutoSeEvent { motion, part: Part::Foot(k), volume });
                    self.cooldown_foot[k] = RULES.autose_part_cooldown;
                }
                break;
            }
        }

        // 3. Rustle detector (0x14597E180), only within 10 m of the listener.
        if listener_distance <= RULES.autose_rustle_distance {
            if (take_off || landing) && events.len() < limit && self.cooldown_arm[0] <= 0.0 {
                events.push(AutoSeEvent { motion: Motion::RustleLong, part: Part::Arm(0), volume: 1.0 });
                self.cooldown_arm[0] = RULES.autose_part_cooldown;
            }
            let (threshold, motion) = if now.running { (RULES.autose_rustle_arm_threshold_run, Motion::RustleShort) } else { (RULES.autose_rustle_arm_threshold_walk, Motion::RustleLong) };
            for k in 0..2 {
                if events.len() >= limit {
                    break;
                }
                // Elbow speed minus waist speed crossing upward through the threshold.
                if prev.arm_rel[k] < threshold && now.arm_rel[k] >= threshold && self.cooldown_arm[k] <= 0.0 {
                    let volume = clamp01((now.arm_rel[k].abs() - threshold) / (RULES.autose_rustle_arm_speed_max - threshold));
                    events.push(AutoSeEvent { motion, part: Part::Arm(k), volume });
                    // 0.1333 s on both arms.
                    self.cooldown_arm = [RULES.autose_rustle_arm_cooldown; 2];
                }
            }
            let leg_motion = if now.running { Motion::RustleFootShort } else { Motion::RustleFootLong };
            for k in 0..2 {
                if events.len() >= limit {
                    break;
                }
                // Exempt from the cooldown.
                let t = RULES.autose_rustle_leg_dot_threshold;
                if prev.leg_dot[k] < t && now.leg_dot[k] >= t {
                    events.push(AutoSeEvent { motion: leg_motion, part: Part::Foot(k), volume: clamp01(map(now.knee_speed[k], RULES.autose_rustle_leg_knee_speed_range)) });
                }
            }
        }
        events
    }
}
