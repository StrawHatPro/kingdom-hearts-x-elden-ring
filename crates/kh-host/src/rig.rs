//! SORA WITHOUT A GAME, for offline proofs of a host's sound and effects: the core on a flat
//! floor (`kh_core::test_world`), one training dummy he can hit, his loadout from the sheet, the
//! pose of every step from `kh-anim`, and a small script of pad input (the test app's words:
//! `stop N; walk N; run N; turn DEG; jump; tap BTN; press BTN; release BTN; hold BTN N; cast NAME`).
//! Nothing here is drawn or heard: the caller hands the seam objects in and uses the outputs.

use std::path::Path;

use glam::{Quat, Vec2, Vec3};
use kh_anim::{Animator, BodySlot, Frame as PoseFrame, Settings, Trs};
use kh_core::test_world::TestWorld;
use kh_core::{Audio, Effects, HitRecord, HitResult, Host, HurtShape, Input, NullHost, Output, PadButton, PadButtons, PadState, Pose, Sora, Status, TargetFlags, TargetId, TargetInfo, Targets, SORA_PARAMS};

/// `kh-app`'s loadout reader (sheet rows -> `kh_core::Loadout`), shared as source.
#[allow(dead_code)]
#[path = "loadout_shared.rs"]
pub mod loadout; // (save) public: save.rs builds the new-game loadout with it, a host parses its sheet with it

pub const STEP: f32 = 1.0 / 60.0;

/// The mesh component in the KH3 world: at the actor origin, turned by the yaw minus 90 degrees.
pub fn component_to_world(out: &Output) -> Trs {
    Trs { translation: out.position, rotation: Quat::from_rotation_z((out.yaw - 90.0).to_radians()), scale: Vec3::ONE }
}

/// The body slot of one step's pose (as kh-app's `step_pose` builds it) and its evaluation.
pub fn step_pose(animator: &mut Animator, out: &Output, dt: f32, world: &dyn kh_anim::IkWorld) -> kh_anim::Pose {
    let (shown, ticked) = (out.anim, out.anim_ticked);
    let sequences = ticked.name.sequences();
    let (samples, time, blend_with) = match shown.locomotion {
        Some(l) => (l.run.sample_weights(shown.axis), l.run_time, Some((l.walk.sequences()[0], l.walk_time, 1.0 - l.run_weight))),
        None => {
            let [(first, first_weight, first_time), (second, second_weight, second_time)] = shown.body_samples();
            if second_weight > 0.0 && second_time != first_time {
                ([(first, 1.0), (first, 0.0)], first_time, Some((second, second_time, second_weight)))
            } else {
                ([(first, first_weight), (second, second_weight)], first_time, None)
            }
        }
    };
    let body = BodySlot { samples, blend_with, notify_sequences: &sequences, time, weight: shown.weight, active: shown.active, notify_window: ticked.notify_window };
    animator.step(&PoseFrame { dt, body, component_to_world: component_to_world(out) }, world).clone()
}

/// A training dummy: an upright capsule that takes every hit.
#[derive(Clone, Copy, Debug)]
pub struct Dummy {
    pub info: TargetInfo,
    pub hits: u32,
}

impl Dummy {
    pub fn at(position: Vec3) -> Dummy {
        let flags = TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY);
        Dummy { info: TargetInfo { id: TargetId(1), position, center: position + Vec3::Z * 90.0, velocity: Vec3::ZERO, yaw: 180.0, radius: 45.0, half_height: 90.0, flags, hp: 100_000.0, hp_max: 100_000.0 }, hits: 0 }
    }
}

impl Targets for Dummy {
    fn enumerate(&self, out: &mut Vec<TargetInfo>) {
        out.push(self.info);
    }

    fn info(&self, id: TargetId) -> Option<TargetInfo> {
        (id == self.info.id).then_some(self.info)
    }

    fn hurt_shapes(&self, _id: TargetId, out: &mut Vec<HurtShape>) {
        let up = Vec3::Z * (self.info.half_height - self.info.radius).max(0.0);
        out.push(HurtShape { a: self.info.center - up, b: self.info.center + up, radius: self.info.radius, part: 0 });
    }

    fn apply_hit(&mut self, hit: &HitRecord) -> HitResult {
        self.hits += 1;
        self.info.hp = (self.info.hp - hit.damage).max(1.0);
        HitResult { accepted: true, damage_dealt: hit.damage, hp_after: self.info.hp, ..HitResult::default() }
    }

    fn engaged_count(&self) -> u32 {
        1
    }
}

/// One word of the script, expanded to steps.
#[derive(Clone, Debug, Default)]
struct Frame {
    stick: Vec2,
    held: PadButtons,
    cast: Option<String>,
}

/// The script as one entry per fixed step.
#[derive(Clone, Debug, Default)]
pub struct Script {
    frames: Vec<Frame>,
}

impl Script {
    pub fn parse(text: &str) -> Result<Script, String> {
        let mut frames: Vec<Frame> = Vec::new();
        let mut held = PadButtons::NONE;
        let mut taps: Vec<PadButton> = Vec::new();
        let mut casts: Vec<String> = Vec::new();
        let mut turn = 0.0f32;
        for word in text.split(';').map(str::trim).filter(|w| !w.is_empty()) {
            let parts: Vec<&str> = word.split_whitespace().collect();
            let number = |k: usize| parts.get(k).and_then(|v| v.parse::<f32>().ok()).ok_or(format!("\"{word}\": a number is missing"));
            let button = |k: usize| parts.get(k).and_then(|v| PadButton::from_name(v)).ok_or(format!("\"{word}\": not a pad button"));
            let mut timed = |seconds: f32, stick: Vec2, extra: Option<PadButton>, held: PadButtons, taps: &mut Vec<PadButton>, casts: &mut Vec<String>| {
                let (sin, cos) = turn.to_radians().sin_cos();
                let stick = Vec2::new(stick.x * cos + stick.y * sin, stick.y * cos - stick.x * sin);
                for k in 0..(seconds * 60.0).round().max(1.0) as usize {
                    let mut now = held;
                    if let Some(b) = extra {
                        now.set(b, true);
                    }
                    if k == 0 {
                        for b in taps.drain(..) {
                            now.set(b, true);
                        }
                    }
                    frames.push(Frame { stick, held: now, cast: if k == 0 { casts.pop() } else { None } });
                }
            };
            match parts[0] {
                "stop" => timed(number(1)?, Vec2::ZERO, None, held, &mut taps, &mut casts),
                "walk" => timed(number(1)?, Vec2::new(0.0, 0.5), None, held, &mut taps, &mut casts),
                "run" => timed(number(1)?, Vec2::new(0.0, 1.0), None, held, &mut taps, &mut casts),
                "hold" => timed(number(2)?, Vec2::ZERO, Some(button(1)?), held, &mut taps, &mut casts),
                "turn" => turn += number(1)?,
                "jump" => taps.push(PadButton::JUMP),
                "tap" => taps.push(button(1)?),
                "press" => held.set(button(1)?, true),
                "release" => held.set(button(1)?, false),
                "cast" => casts.push(parts.get(1).ok_or("cast NAME")?.to_string()),
                other => return Err(format!("unknown script word \"{other}\"")),
            }
        }
        Ok(Script { frames })
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
}

pub struct Rig {
    pub sora: Sora,
    pub world: TestWorld,
    pub dummy: Option<Dummy>,
    pub animator: Animator,
    pub pose: kh_anim::Pose,
    pub out: Output,
    held_before: PadButtons,
    null: NullHost,
    /// What was cast at which step (for a proof's report).
    pub casts: Vec<(u64, String)>,
    /// The view of the last drawn frame (the shotlock's reticle test needs one; `None`: nothing can be locked).
    pub view: Option<kh_core::magic::View>,
    /// (hud) The latest snapshot the core published (what a HUD is driven from).
    pub ui: UiKeep,
}

/// (hud) A UI sink that keeps the latest snapshot.
#[derive(Default)]
pub struct UiKeep {
    pub snapshot: kh_core::UiSnapshot,
}

impl kh_core::UiSink for UiKeep {
    fn publish(&mut self, snapshot: &kh_core::UiSnapshot) {
        self.snapshot = snapshot.clone();
    }
}

impl Rig {
    /// Sora at the origin facing +X on a flat floor, level-99 loadout from `loadout` (the sheet
    /// file), the pack's skeleton and clips from `pack`. `dummy_at`: a training dummy there.
    pub fn new(pack: &Path, loadout: &Path, dummy_at: Option<Vec3>) -> Result<Rig, String> {
        let start = Pose { position: Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS)), yaw: 0.0 };
        let text = std::fs::read_to_string(loadout).map_err(|e| format!("{}: {e}", loadout.display()))?;
        let (sheet, _) = loadout::parse(&loadout.to_string_lossy(), &text)?;
        let sora = Sora::new(SORA_PARAMS, start).with_status(Status::from_loadout(&sheet));
        let settings = Settings { leg_ik: false, kine_driver: true, face: true, bonamik: true, first_update_holds_position: kh_core::anim::FIRST_UPDATE_HOLDS_POSITION };
        let animator = kh_anim::Pack::load(pack).and_then(|pack| Animator::new(pack, settings))?;
        let out = sora.output();
        Ok(Rig { sora, world: TestWorld::floor(), dummy: dummy_at.map(Dummy::at), animator, pose: kh_anim::Pose::default(), out, held_before: PadButtons::NONE, null: NullHost::default(), casts: Vec::new(), view: None, ui: UiKeep::default() })
    }

    /// One fixed step with the script's input of step `k` (past its end: no input), the seam
    /// objects of the caller, and the pose of the step.
    pub fn step(&mut self, script: &Script, k: usize, camera_yaw: f32, audio: &mut dyn Audio, effects: &mut dyn Effects) -> &Output {
        let input = self.input(script, k, camera_yaw);
        self.step_with(&input, audio, effects)
    }

    /// (hud) The first half of [`Rig::step`]: the input of step `k` (a host whose menus can hold the pause
    /// reads the input every step and skips `step_with` while paused).
    pub fn input(&mut self, script: &Script, k: usize, camera_yaw: f32) -> Input {
        let frame = script.frames.get(k).cloned().unwrap_or_default();
        if let Some(name) = &frame.cast {
            match kh_core::magic::spells::command_by_name(name) {
                Some(kind) => {
                    self.sora.command_request(kh_core::command::CommandRequest::Execute(kind));
                    self.casts.push((self.out.step + 1, name.clone()));
                }
                None => eprintln!("rig: no command named \"{name}\""),
            }
        }
        let pad = PadState::from_held(frame.held, self.held_before);
        self.held_before = frame.held;
        Input { move_stick: frame.stick, camera_yaw, view: self.view, ..Input::default() }.with_pad(pad)
    }

    /// (hud) The second half of [`Rig::step`]: one fixed step with `input`.
    pub fn step_with(&mut self, input: &Input, audio: &mut dyn Audio, effects: &mut dyn Effects) -> &Output {
        let out = {
            let targets: &mut dyn Targets = match self.dummy.as_mut() {
                Some(dummy) => dummy,
                None => &mut self.null.targets,
            };
            let mut host = Host { world: &self.world, targets, audio, effects, ui: &mut self.ui };
            self.sora.step_hosted(STEP, input, &mut host)
        };
        self.pose = step_pose(&mut self.animator, &out, STEP, &kh_anim::NoIkWorld);
        self.out = out;
        &self.out
    }

    /// Where an attach that is not a bone of the pose is now.
    pub fn place(&self, at: &kh_core::Attach) -> Vec3 {
        match at {
            kh_core::Attach::World(point) => *point,
            kh_core::Attach::Sora { bone, offset } => crate::seam::on_actor(self.out.position, self.out.yaw, bone.is_some(), *offset),
            kh_core::Attach::Target { offset, .. } => self.dummy.map_or(*offset, |d| d.info.center + *offset),
            // (party stream E) No party members in this rig.
            kh_core::Attach::Party { offset, .. } => self.out.position + *offset,
        }
    }
}
