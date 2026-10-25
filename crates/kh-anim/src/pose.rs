//! Pose evaluation: what the game's anim graph, leg IK and KineDriver do to turn the clips into
//! the displayed pose (docs/sora-animation-spec.md Part II; docs/sora-animation-spec-changes.md).
//!
//! ONE CALL OF [`Animator::step`] PER SIMULATION STEP. The saved pose, the montage weights, the
//! leg-IK alpha and the leg-IK node are per-update recurrences: their result depends on the
//! update rate and cannot be sampled at another instant. A host that draws between two steps
//! interpolates the two latest poses ([`Pose::lerp`]); it never re-evaluates (change A14).
//!
//! Order of one step (change list C):
//!
//! ```text
//! face montages advance (weight, position)
//! notifies of the body's update window: TresIK states -> the leg-IK alpha interpolator's
//!     targets; TresFaceAnim* -> face plays (new montages: weight 0 this frame)
//! P = saved pose of the previous step
//! body = slot(FullBody montage pose, P); eyes = slot(Face, P); mouth = slot(Mouth, P)
//! saved pose = eyes on the G_eye subtree, mouth on the G_mouth subtree, body elsewhere
//! leg IK on a copy (alpha = the interpolator's value before this step's pawn tick)
//! KineDriver on the component-space pose
//! Bonamik (30 Hz evaluation + interpolation) on the component-space pose
//! the alpha interpolator advances (the pawn's tick comes after the movement tick)
//! ```
//!
//! Not here: look-at (an exact pass-through for the player without a target, spec 11).
//! Bonamik (hair, hood, sleeves, coat: spec 14) is [`crate::bonamik`]; it is stepped here with the
//! simulation dt and its bones are interpolated for drawing like every other bone ([`Pose::lerp`]).

use std::collections::BTreeMap;

use crate::blend::{key_rotation, key_scale, slot, weighted};
use crate::bonamik::{Bonamik, BonamikHost, BonamikOptions};
use crate::kine::KineDriver;
use crate::legik::{IkWorld, LegIk, LegIkReport};
use crate::pack::{Clip, ClipInfo, Notify, Pack, Skeleton};
use crate::Trs;

/// What to run. Everything on is the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub leg_ik: bool,
    pub kine_driver: bool,
    pub face: bool,
    /// Bonamik secondary motion (hair, hood, sleeves, coat) after KineDriver.
    pub bonamik: bool,
    /// The animation spec's reading of a montage's first update (the position stays 0); it must
    /// agree with the simulation's clock (`kh_core::anim::FIRST_UPDATE_HOLDS_POSITION`). Used
    /// here for the face montages, whose clocks this crate owns.
    pub first_update_holds_position: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings { leg_ik: true, kine_driver: true, face: true, bonamik: true, first_update_holds_position: false }
    }
}

/// The `FullBody` slot after this step's pose tick, as the simulation reports it
/// (`kh_core::AnimRequest`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodySlot<'a> {
    /// Up to two sequences and their weights (the second weight may be 0): a blend space's
    /// samples around its axis value, or one sequence.
    pub samples: [(&'a str, f32); 2],
    /// A second cycle blended over the samples above: (sequence, its own time in seconds, its
    /// weight; the samples keep the rest). For the port's speed-matched locomotion loop (the walk
    /// cycle against the run blend space, phase-synchronised by the simulation); `None` is the
    /// game's playback.
    pub blend_with: Option<(&'a str, f32, f32)>,
    /// Every sample sequence of the playing asset: a blend space in mode `SQEX_AllAnimation`
    /// fires the notifies of all of them, whatever their weight (spec 9).
    pub notify_sequences: &'a [&'a str],
    /// Position, seconds.
    pub time: f32,
    /// Montage weight of this update.
    pub weight: f32,
    /// False once a one-shot has ended: the slot shows the saved pose.
    pub active: bool,
    /// The notify window of this update, `(from, to]`, already shifted; `from < 0` on the first
    /// update; `to < from` when a loop wrapped; `(0, 0)` when nothing was updated.
    pub notify_window: (f32, f32),
}

/// The inputs of one step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame<'a> {
    pub dt: f32,
    pub body: BodySlot<'a>,
    /// The mesh component in the world (UE: cm, Z up): at the actor origin, turned by the actor's
    /// yaw minus 90 degrees (in mesh space the character faces +Y).
    pub component_to_world: Trs,
}

/// A pose: one transform per bone of the skeleton, in the skeleton's order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pose {
    /// Relative to the parent bone.
    pub local: Vec<Trs>,
    /// Relative to the mesh component (what the game skins with).
    pub component: Vec<Trs>,
}

impl Pose {
    /// For DRAWING between two simulation steps only: `self` at 0, `next` at 1.
    pub fn lerp(&self, next: &Pose, alpha: f32) -> Pose {
        let mix = |a: &Trs, b: &Trs| Trs { translation: a.translation.lerp(b.translation, alpha), rotation: key_rotation(a.rotation, b.rotation, alpha), scale: a.scale.lerp(b.scale, alpha) };
        Pose { local: self.local.iter().zip(&next.local).map(|(a, b)| mix(a, b)).collect(), component: self.component.iter().zip(&next.component).map(|(a, b)| mix(a, b)).collect() }
    }
}

/// A clip with its tracks resolved to bone indices.
struct Bound {
    /// The keys and each track's bone: read on first use for the clips beyond stage 1.
    data: std::sync::OnceLock<(Clip, Vec<Option<usize>>)>,
    /// The file of a clip that is read on first use.
    path: Option<std::path::PathBuf>,
    name: String,
    info: ClipInfo,
}

impl Bound {
    fn data(&self, skeleton: &Skeleton) -> &(Clip, Vec<Option<usize>>) {
        self.data.get_or_init(|| {
            let clip = match &self.path {
                Some(path) => Clip::load(path, &self.name).unwrap_or_else(|e| {
                    eprintln!("kh-anim: {e}: the clip shows the reference pose");
                    Clip { name: self.name.clone(), tracks: Vec::new() }
                }),
                None => Clip { name: self.name.clone(), tracks: Vec::new() },
            };
            let bones = clip.tracks.iter().map(|t| skeleton.index_of(&t.bone)).collect();
            (clip, bones)
        })
    }
}

/// A face montage on its slot.
#[derive(Clone, Debug, PartialEq)]
struct Montage {
    row: String,
    clip: String,
    time: f32,
    blend_time: f32,
    blend_in: f32,
    length: f32,
    first: bool,
}

impl Montage {
    fn weight(&self) -> f32 {
        if self.blend_in <= 0.0 {
            1.0
        } else {
            (self.blend_time / self.blend_in).min(1.0)
        }
    }
}

/// (ZETTA) The face slots' plays kept by `Animator::face_save` (opaque).
#[derive(Clone, Debug, PartialEq)]
pub struct FaceSave {
    face: [Option<Montage>; 2],
    last_row: [Option<String>; 2],
}

/// The pawn's leg-IK interpolator (`FTresInterpolator`-like, update 0x143F0AA50).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LegAlpha {
    pub value: f32,
    pub target: f32,
    pub remaining: f32,
}

impl LegAlpha {
    /// `NotifyStartIK` (0x1405B4260).
    fn begin(&mut self, time: f32, target: f32) {
        self.target = target.clamp(0.0, 1.0);
        self.remaining = time;
    }

    /// `NotifyEndIK` (0x1405B3010): back to `MySkelCtrl_IkInitValue` = 0.
    fn end(&mut self, time: f32) {
        self.target = 0.0;
        self.remaining = time;
    }

    fn update(&mut self, dt: f32) {
        if self.target == self.value {
            return;
        }
        if self.remaining == 0.0 {
            self.value = self.target;
            return;
        }
        let rate = (self.target - self.value) / self.remaining;
        let next = self.value + rate * dt;
        self.value = if rate > 0.0 {
            if next < 0.0 {
                0.0
            } else {
                next.min(self.target)
            }
        } else if next >= self.target {
            next.min(1.0)
        } else {
            self.target
        };
        self.remaining -= dt;
    }
}

/// The only clip whose Bonamik notify states have an effect with Sora's asset (the fall loop).
const BONAMIK_NOTIFY_CLIP: &str = "0033";
const BONAMIK_GROUP_PARAMS: &str = "TresAnimNotifyState_Bonamik_ChangeGroupParams";
const BONAMIK_CHARA_WIND: &str = "TresAnimNotifyState_Bonamik_CharaWind";
/// (party A) handled for the friends' packs only (format kh3-party-pack/).
const BONAMIK_RESET_AND_PRE_ROLL: &str = "TresAnimNotify_Bonamik_ResetAndPreRoll";

const EYE: usize = 0;
const MOUTH: usize = 1;

pub struct Animator {
    skeleton: Skeleton,
    clips: BTreeMap<String, Bound>,
    reference: Vec<Trs>,
    settings: Settings,
    /// `MySavedPose`: local space, per bone.
    saved: Vec<Trs>,
    /// The two face slots: Face (G_eye subtree), Mouth (G_mouth subtree).
    face: [Option<Montage>; 2],
    last_row: [Option<String>; 2],
    /// (FACE) The face slots a host face layer holds (Face, Mouth): a body notify's play on a held slot is kept for
    /// later, not played (KH3's play priority: a higher play blocks the lower ones while it runs, eye play 0x14055D270).
    face_hold: [bool; 2],
    /// (FACE) The latest notify play a held slot did not take: (name, type, same check, blend-in).
    face_deferred: [Option<(Option<String>, String, bool, f32)>; 2],
    /// (FACE) The slots a body notify asked to play in the latest step (held or not).
    face_notified: [bool; 2],
    subtree: [Vec<usize>; 2],
    /// (row name, sequence) of the face anim set; type -> row base name.
    rows: Vec<(String, String)>,
    types: Vec<(String, String)>,
    /// The `TresIK` notify states that are active: (sequence, index of the notify).
    ik_states: Vec<(String, usize)>,
    leg_alpha: LegAlpha,
    leg_ik: Option<LegIk>,
    kine: Option<KineDriver>,
    bonamik: Option<Bonamik>,
    /// The bones Bonamik may write (ascending); empty without it.
    bonamik_bones: Vec<usize>,
    /// The Bonamik notify states of clip 0033 that are active: (sequence, index of the notify).
    bonamik_states: Vec<(String, usize)>,
    pose: Pose,
    /// What could not be set up (shown by the host).
    pub warnings: Vec<String>,
    /// (IMPL_ER59) An outfit's OWN clips (tools/sora_outfit_anims.js: on this skeleton, time-warped onto the clip of the
    /// same name, its root tracks): sampled instead of the pack's while it is worn. Read on first use.
    own: BTreeMap<String, Bound>,
    /// (ER83) How much of the shown body pose comes from the outfit's OWN clips (0..1, through the saved-pose blend): the
    /// renderer's hand contact IK leaves those alone (kh-er-render handik.rs).
    own_share: f32,
    /// (party A) The pack is a friend's (`format` kh3-party-pack/: tools/party_pack.js): the notify handling the friends'
    /// clips need beyond Sora's (Bonamik reset notifies).
    party: bool,
    /// (party A2) The body montage's samples for the NEXT step only, replacing `BodySlot::samples` when not empty: a 2D
    /// blend space's whole grid result (stock `UBlendSpace::GetSamplesFromBlendInput` gives up to 4 x 3 entries; the
    /// slot's two samples cannot hold them). Set by [`Animator::set_body_samples`].
    body_override: Vec<(String, f32)>,
    /// (party A2) `TresAnimNode_CollTwoBoneIK` nodes of a friend's graph (Donald), in graph order; empty for Sora (his
    /// two CollTwoBoneIK nodes sit in the branch that does not run, native_D_legik 2).
    coll_ik: Vec<crate::legik_coll::CollTwoBoneIk>,
    /// (party A2) The friends' active Bonamik notify states (sequence, notify index) and, per SpeedRate state, the
    /// rate it saved at its begin (0x140562780 stores `FUN_143b5e170` = the current rate at +0x60).
    party_cloth: Vec<(String, usize)>,
    party_speed_saved: Vec<((String, usize), f32)>,
    /// (party A2) The ground the Bonamik's Step clamps against (owner vf10, 0x143B57E80: the CMC floor or a trace
    /// under the root): world (point, normal), set by the host before each step; `None` = no ground.
    bonamik_ground: Option<(glam::Vec3, glam::Vec3)>,
    /// (party A2) Notify states seen that the port does not apply (class names), for the host's text.
    pub unported_cloth: Vec<String>,
    /// (party I2) The friend's TresAnimNode_ExLookAt with the pawn / anim-instance look-at feed (crate::lookat);
    /// `None` for Sora (his node is a pass-through without a look-at target, spec 11) and packs without lookat.json.
    look_at: Option<crate::lookat::ExLookAt>,
    /// (party I2) The active TresAnimNotifyState_LookAt states: (sequence, notify index).
    look_states: Vec<(String, usize)>,
}

impl Animator {
    /// (party I2) The AI controller's focal point this tick (world, cm), `None` = no focus: SetLookAt(valid, 0.5) and
    /// SetLookAtLocation (0x140527460). Call before `step`.
    pub fn set_look_focus(&mut self, point: Option<glam::Vec3>) {
        if let Some(node) = &mut self.look_at {
            node.set_focus(point);
        }
    }

    /// (FACE) The cast look's hold: the head's look-at on over the clips' LookAt notify states (lookat::ExLookAt::set_hold).
    pub fn set_look_hold(&mut self, on: bool) {
        if let Some(node) = &mut self.look_at {
            node.set_hold(on);
        }
    }

    /// (party I2) The look-at node and its feed (rate, location, current / target / eye rotation), when the pack has one.
    pub fn look_at(&self) -> Option<&crate::lookat::ExLookAt> {
        self.look_at.as_ref()
    }

    /// (FACE) The look-at node to tune (the head's look-up cap: `ExLookAt::set_head_up_max`).
    pub fn look_at_mut(&mut self) -> Option<&mut crate::lookat::ExLookAt> {
        self.look_at.as_mut()
    }

    /// (party A) True for a friend's pack (Donald / Goofy).
    pub fn is_party(&self) -> bool {
        self.party
    }

    /// From a pack read by [`Pack::load`].
    pub fn new(pack: Pack, settings: Settings) -> Result<Animator, String> {
        let Pack { manifest, skeleton, clips, lazy, info, leg_ik, kine_driver, bonamik, look_at } = pack;
        let reference = skeleton.reference_pose();
        let mut bound = BTreeMap::new();
        for (name, clip) in clips {
            let bones = clip.tracks.iter().map(|t| skeleton.index_of(&t.bone)).collect();
            let info = info.get(&name).cloned().ok_or(format!("the manifest does not describe the clip {name}"))?;
            bound.insert(name.clone(), Bound { data: std::sync::OnceLock::from((clip, bones)), path: None, name, info });
        }
        for (name, path) in lazy {
            let info = info.get(&name).cloned().ok_or(format!("the manifest does not describe the clip {name}"))?;
            bound.insert(name.clone(), Bound { data: std::sync::OnceLock::new(), path: Some(path), name, info });
        }
        let mut warnings = Vec::new();
        let party = manifest.get("format").as_str().is_some_and(|f| f.starts_with("kh3-party-pack/"));
        // (party A2) a friend whose graph runs CollTwoBoneIK nodes (Donald) has no Multiped node: its legik.json lists
        // `coll_two_bone_ik` only, and those nodes run instead.
        // (party I2) the head look-at node of a friend's graph
        let look_at = match look_at.filter(|_| party).map(|data| crate::lookat::ExLookAt::from_json(&data, &skeleton)) {
            Some(Ok(node)) => Some(node),
            Some(Err(e)) => {
                warnings.push(e);
                None
            }
            None => None,
        };
        let mut coll_ik = Vec::new();
        let leg_ik = match leg_ik {
            Some(data) if party && data.get("two_bone_ik_multiped").items().is_empty() => {
                match crate::legik_coll::CollTwoBoneIk::from_json_all(&data, &skeleton) {
                    Ok(nodes) => coll_ik = nodes,
                    Err(e) => warnings.push(e),
                }
                None
            }
            other => other,
        };
        let leg_ik = match leg_ik.map(|data| LegIk::from_json(&data, &skeleton)) {
            Some(Ok(node)) => Some(node),
            Some(Err(e)) => {
                warnings.push(e);
                None
            }
            None => {
                if coll_ik.is_empty() {
                    warnings.push("the pack has no data/legik.json: no leg IK".to_string());
                }
                None
            }
        };
        let kine = match kine_driver.map(|data| KineDriver::from_json(&data)) {
            Some(Ok(driver)) => Some(driver),
            Some(Err(e)) => {
                warnings.push(e);
                None
            }
            None => {
                warnings.push("the pack has no data/kinedriver.json: no KineDriver".to_string());
                None
            }
        };
        let bonamik = match bonamik.map(|data| Bonamik::new(&data, &skeleton, BonamikOptions::default())) {
            Some(Ok(solver)) => Some(solver),
            Some(Err(e)) => {
                warnings.push(format!("Bonamik setup failed: {e}: no Bonamik"));
                None
            }
            None => {
                warnings.push("the pack has no data/bonamik.json: no Bonamik".to_string());
                None
            }
        };
        let bonamik_bones = bonamik.as_ref().map(|b| b.driven_bones()).unwrap_or_default();
        let runtime = manifest.get("runtime");
        let subtrees = runtime.get("saved_pose").get("face_subtrees");
        let indices = |key: &str| -> Vec<usize> { subtrees.get(key).items().iter().filter_map(|i| i.as_usize()).filter(|i| *i < skeleton.len()).collect() };
        let subtree = [indices("G_eye"), indices("G_mouth")];
        let face = runtime.get("face");
        let stem = |file: &str| file.rsplit('/').next().unwrap_or(file).trim_end_matches(".glb").to_string();
        let rows = face.get("rows").items().iter().filter_map(|r| Some((r.get("name").as_str()?.to_string(), stem(r.get("file").as_str()?)))).collect();
        let types = face.get("type_to_row").members().iter().filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string()))).collect();
        // The saved pose starts as `m_CacheStartPoseAnimName` = IdleN at t = 0 (spec 6).
        let initial = manifest
            .get("logical_animations")
            .items()
            .iter()
            .find(|a| a.get("name").as_str() == Some("IdleN"))
            .and_then(|a| a.get("file").as_str())
            .map(stem)
            .unwrap_or_else(|| "0001".to_string());
        let mut animator = Animator {
            party,
            body_override: Vec::new(),
            coll_ik,
            party_cloth: Vec::new(),
            party_speed_saved: Vec::new(),
            bonamik_ground: None,
            unported_cloth: Vec::new(),
            look_at,
            look_states: Vec::new(),
            saved: reference.clone(),
            pose: Pose::default(),
            skeleton,
            clips: bound,
            reference,
            settings,
            face: [None, None],
            last_row: [None, None],
            face_hold: [false, false],           // (FACE)
            face_deferred: [None, None],         // (FACE)
            face_notified: [false, false],       // (FACE)
            subtree,
            rows,
            types,
            ik_states: Vec::new(),
            leg_alpha: LegAlpha::default(),
            leg_ik,
            kine,
            bonamik,
            bonamik_bones,
            bonamik_states: Vec::new(),
            warnings,
            own: BTreeMap::new(),
            own_share: 0.0,
        };
        animator.saved = animator.sample(&initial, 0.0);
        animator.pose = Pose { component: component_space(&animator.skeleton, &animator.saved), local: animator.saved.clone() };
        Ok(animator)
    }

    pub fn skeleton(&self) -> &Skeleton {
        &self.skeleton
    }

    pub fn settings(&self) -> Settings {
        self.settings
    }

    /// Changes what runs from the next step on (for comparisons).
    pub fn set_settings(&mut self, settings: Settings) {
        self.settings = settings;
    }

    /// The pose of the latest step.
    pub fn pose(&self) -> &Pose {
        &self.pose
    }

    /// The saved pose (`MySavedPose`): local space, before leg IK.
    pub fn saved_pose(&self) -> &[Trs] {
        &self.saved
    }

    /// (er-host, IMPL_ER23A) The leg IK switched on / off after construction (a host that turns it off for a cutscene's
    /// retargeted pose, or by a key). Its per-leg state stays as it is while off.
    pub fn set_leg_ik(&mut self, on: bool) {
        self.settings.leg_ik = on;
    }

    /// The pawn's leg-IK alpha interpolator after the latest step.
    pub fn leg_alpha(&self) -> LegAlpha {
        self.leg_alpha
    }

    /// What the leg-IK node did in the latest step.
    pub fn leg_ik_report(&self) -> LegIkReport {
        self.leg_ik.as_ref().map(|n| n.report).unwrap_or_default()
    }

    /// The bones the leg IK writes (hips, then per leg thigh, shin, foot); empty without it.
    pub fn leg_ik_bones(&self) -> Vec<usize> {
        let mut bones = self.leg_ik.as_ref().map(|n| n.bones()).unwrap_or_default();
        for node in &self.coll_ik {
            bones.extend(node.bones());
        }
        bones
    }

    /// (party A2) What each CollTwoBoneIK node (Donald's legs) did in the latest step; empty without them.
    pub fn coll_ik_reports(&self) -> Vec<crate::legik_coll::CollIkReport> {
        self.coll_ik.iter().map(|n| n.report).collect()
    }

    /// (party A2) Which leg-IK node type runs: "multiped (Maya)", "multiped (stock solver)", "coll x2", or "none".
    pub fn leg_ik_kind(&self) -> String {
        match (&self.leg_ik, self.coll_ik.len()) {
            (Some(node), _) => format!("multiped ({})", if node.maya() { "Maya" } else { "stock solver" }),
            (None, 0) => "none".to_string(),
            (None, n) => format!("coll x{n}"),
        }
    }

    /// The bones KineDriver writes; empty without it.
    pub fn kine_driver_bones(&self) -> &[usize] {
        self.kine.as_ref().map_or(&[], |k| k.written())
    }

    /// The bones Bonamik may write (the bones with a Bonamik weight); empty without it.
    pub fn bonamik_bones(&self) -> &[usize] {
        &self.bonamik_bones
    }

    /// The Bonamik solver, when the pack has its data.
    pub fn bonamik(&self) -> Option<&Bonamik> {
        self.bonamik.as_ref()
    }

    /// (game over) The Bonamik solver for a host that drives a notify state of a sequence itself (the
    /// game-over sequence 0270: speed rate, character wind, group parameters).
    pub fn bonamik_mut(&mut self) -> Option<&mut Bonamik> {
        self.bonamik.as_mut()
    }

    /// What plays on the Face and Mouth slots: (sequence, weight).
    pub fn face_playing(&self) -> [Option<(&str, f32)>; 2] {
        [EYE, MOUTH].map(|s| self.face[s].as_ref().map(|m| (m.clip.as_str(), m.weight())))
    }

    /// The bones of the Face (G_eye) and Mouth (G_mouth) slots.
    pub fn face_subtrees(&self) -> [&[usize]; 2] {
        [&self.subtree[EYE], &self.subtree[MOUTH]]
    }

    /// (IMPL_ER59) THE WORN OUTFIT'S OWN CLIPS: `dir` = its folder (`PACK/outfits/<id>`), whose `anims/<name>.glb` and
    /// `face/<name>.glb` replace the pack's clips of those names; `None` = the pack's clips only. Returns the names.
    pub fn set_own_clips(&mut self, dir: Option<&std::path::Path>) -> Vec<String> {
        self.set_own_clips_except(dir, &std::collections::BTreeSet::new())
    }

    /// (ER79) `set_own_clips` without the clips named in `kh3` (`outfit_clips::kh3_clips`): those stay KH3 Sora's, which the
    /// renderer retargets onto the outfit's skeleton like every clip the outfit lacks.
    pub fn set_own_clips_except(&mut self, dir: Option<&std::path::Path>, kh3: &std::collections::BTreeSet<String>) -> Vec<String> {
        self.own.clear();
        let Some(dir) = dir else { return Vec::new() };
        for (name, bound) in &self.clips {
            if kh3.contains(name) {
                continue;
            }
            for sub in ["anims", "face"] {
                let path = dir.join(sub).join(format!("{name}.glb"));
                if path.is_file() {
                    self.own.insert(name.clone(), Bound { data: std::sync::OnceLock::new(), path: Some(path), name: name.clone(), info: bound.info.clone() });
                    break;
                }
            }
        }
        self.own.keys().cloned().collect()
    }

    /// (ER83) How much of the shown body pose comes from the worn outfit's own clips (0 = KH3 Sora's clips only).
    pub fn own_share(&self) -> f32 {
        self.own_share
    }

    /// (ER83) The own clips' share of one body slot request (its samples, then its second cycle).
    fn own_share_of(&self, body: &BodySlot) -> f32 {
        if self.own.is_empty() {
            return 0.0;
        }
        let own = |s: &str| if self.own.contains_key(s) { 1.0 } else { 0.0 };
        let total: f32 = body.samples.iter().map(|(_, w)| w.max(0.0)).sum();
        let samples = if total > 0.0 { body.samples.iter().map(|(s, w)| own(s) * w.max(0.0)).sum::<f32>() / total } else { own(body.samples[0].0) };
        match body.blend_with {
            Some((s, _, w)) if w > 0.0 => samples * (1.0 - w.min(1.0)) + own(s) * w.min(1.0),
            _ => samples,
        }
    }

    pub fn clip_info(&self, sequence: &str) -> Option<&ClipInfo> {
        self.clips.get(sequence).map(|b| &b.info)
    }

    /// Animation tracks that name a bone the skeleton does not have.
    pub fn unbound_tracks(&self) -> usize {
        self.clips.values().filter_map(|b| b.data.get()).map(|d| d.1.iter().filter(|i| i.is_none()).count()).sum()
    }

    /// One sequence at `time` as a full local pose (spec 7): translation linear between keys,
    /// rotation a normalised linear blend along the shorter arc, scale geometric; bones without a
    /// track at the mesh reference pose; never from the last key to the first. The root bone is
    /// locked to the mesh reference root when the sequence has `bEnableRootMotion`.
    pub fn sample(&self, sequence: &str, time: f32) -> Vec<Trs> {
        let mut pose = self.reference.clone();
        let Some(bound) = self.clips.get(sequence) else { return pose };
        // (IMPL_ER59) the worn outfit's own clip of this name, when it has one (the clip info stays the pack's)
        let (clip, bones) = self.own.get(sequence).unwrap_or(bound).data(&self.skeleton);
        for (track, bone) in clip.tracks.iter().zip(bones) {
            let Some(i) = *bone else { continue };
            if i == 0 && bound.info.root_motion_enabled {
                continue;
            }
            if let Some((a, b, f)) = track.translation.around(time) {
                pose[i].translation = a.lerp(b, f);
            }
            if let Some((a, b, f)) = track.rotation.around(time) {
                pose[i].rotation = if f == 0.0 { a } else { key_rotation(a, b, f) };
            }
            if let Some((a, b, f)) = track.scale.around(time) {
                pose[i].scale = if f == 0.0 { a } else { key_scale(a, b, f) };
            }
        }
        pose
    }

    /// The montage pose of the body slot: one sequence, or a blend space's samples blended with
    /// the slot's formula, heaviest first (spec 8).
    fn body_pose(&self, body: &BodySlot) -> Vec<Trs> {
        let base = self.samples_pose(body);
        match body.blend_with {
            Some((sequence, time, weight)) if weight > 0.0 => {
                let other = self.sample(sequence, time);
                if weight >= 1.0 {
                    return other;
                }
                // The game's pose blend rule, heaviest first.
                let ((heavy, heavy_weight), (light, light_weight)) = if weight > 0.5 { ((&other, weight), (&base, 1.0 - weight)) } else { ((&base, 1.0 - weight), (&other, weight)) };
                heavy.iter().zip(light).map(|(h, l)| weighted(&[(*h, heavy_weight), (*l, light_weight)])).collect()
            }
            _ => base,
        }
    }

    /// (party A2) The body montage's samples for the next step only (a 2D blend space's grid result: every sample with
    /// a weight > 0); empty = the `BodySlot`'s two samples. The notify sequences stay the `BodySlot`'s.
    pub fn set_body_samples(&mut self, samples: Vec<(String, f32)>) {
        self.body_override = samples;
    }

    fn samples_pose(&self, body: &BodySlot) -> Vec<Trs> {
        if !self.body_override.is_empty() {
            // The game's pose blend of the blend space's sample list (spec 8): heaviest first, accumulated.
            let mut list: Vec<(&str, f32)> = self.body_override.iter().filter(|(_, w)| *w > 0.0).map(|(s, w)| (s.as_str(), *w)).collect();
            list.sort_by(|a, b| b.1.total_cmp(&a.1));
            if list.len() == 1 || list.is_empty() {
                return self.sample(list.first().map_or(body.samples[0].0, |s| s.0), body.time);
            }
            let poses: Vec<Vec<Trs>> = list.iter().map(|(s, _)| self.sample(s, body.time)).collect();
            return (0..self.reference.len()).map(|bone| weighted(&poses.iter().zip(&list).map(|(p, (_, w))| (p[bone], *w)).collect::<Vec<_>>())).collect();
        }
        let [(first, first_weight), (second, second_weight)] = body.samples;
        if !(second_weight > 0.0) {
            return self.sample(first, body.time);
        }
        if !(first_weight > 0.0) {
            return self.sample(second, body.time);
        }
        let (a, b) = (self.sample(first, body.time), self.sample(second, body.time));
        let ((heavy, heavy_weight), (light, light_weight)) = if second_weight > first_weight { ((&b, second_weight), (&a, first_weight)) } else { ((&a, first_weight), (&b, second_weight)) };
        heavy.iter().zip(light).map(|(h, l)| weighted(&[(*h, heavy_weight), (*l, light_weight)])).collect()
    }

    fn find_row(&self, base: &str, slot: usize) -> Option<(String, String)> {
        let last = if slot == EYE { "_EYE" } else { "_LIP" };
        for candidate in [base.to_string(), format!("{base}_EL"), format!("{base}{last}")] {
            if let Some(row) = self.rows.iter().find(|(name, _)| name.eq_ignore_ascii_case(&candidate)) {
                return Some(row.clone());
            }
        }
        None
    }

    /// A `TresAnimNotify_FaceAnim*` notify: a play on the Face and / or the Mouth slot (spec 12).
    fn face_notify(&mut self, notify: &Notify) {
        let p = &notify.properties;
        let text = |keys: [&str; 2]| keys.iter().find_map(|k| p.get(k).as_str()).map(str::to_string);
        let name = text(["FaceAnimName", "m_AnimName"]);
        let kind = text(["FaceAnimType", "m_AnimType"]).unwrap_or_else(|| "NORMAL_AUTO".to_string());
        let same_check = p.get("SameCheck").as_bool().or(p.get("m_bSameCheck").as_bool()).unwrap_or(true);
        let blend_in = p.get("m_BlendInTime").as_f32().or(p.get("BlendInTime").as_f32()).unwrap_or(-1.0);
        let slots: &[usize] = match notify.class.as_str() {
            "TresAnimNotify_FaceAnimEye" => &[EYE],
            "TresAnimNotify_FaceAnimLip" => &[MOUTH],
            _ => &[EYE, MOUTH],
        };
        // (FACE) a slot a host face layer holds keeps the play for when it lets go (the latest one wins)
        let mut free: Vec<usize> = Vec::new();
        for &s in slots {
            self.face_notified[s] = true;
            if self.face_hold[s] {
                self.face_deferred[s] = Some((name.clone(), kind.clone(), same_check, blend_in));
            } else {
                free.push(s);
            }
        }
        self.face_play(name, &kind, same_check, blend_in, &free);
    }

    /// (FACE) The eyeballs' turn of the default eye-look grid (NO_0_Eye, else the first) at the look-at node's eye input,
    /// relative to the grid's centre, both at clip time 0 (no baked blink): (bone, local rotation delta) of the eye
    /// subtree's eyeball bones (names with "eye", not lids / brows). Empty without a look-at with eye grids or a look.
    fn eye_overlay(&self) -> Vec<(usize, glam::Quat)> {
        let Some(l) = &self.look_at else { return Vec::new() };
        if l.eye_rate() <= 0.0 {
            return Vec::new();
        }
        let Some(space) = l.eye_spaces.iter().find(|e| e.row.eq_ignore_ascii_case("NO_0_Eye")).or(l.eye_spaces.first()) else { return Vec::new() };
        let Some(at) = l.eye_weights(&space.row) else { return Vec::new() };
        let centre = space.weights(0.0, 0.0);
        let grid = |list: &[(String, f32)]| -> Vec<Trs> {
            let poses: Vec<Vec<Trs>> = list.iter().map(|(c, _)| self.sample(c, 0.0)).collect();
            (0..self.reference.len()).map(|bone| weighted(&poses.iter().zip(list).map(|(p, (_, w))| (p[bone], *w)).collect::<Vec<_>>())).collect()
        };
        let (a, c) = (grid(&at), grid(&centre));
        let s = &self.skeleton;
        self.subtree[EYE]
            .iter()
            .copied()
            .filter(|&b| {
                let n = s.bones[b].name.to_ascii_lowercase();
                n.contains("eye") && !n.contains("lid") && !n.contains("brow") && n != "g_eye"
            })
            .map(|b| (b, (a[b].rotation * c[b].rotation.inverse()).normalize()))
            .collect()
    }

    /// (FACE) The Face (eye) slot index of [`Animator::hold_face`] / [`Animator::release_face`].
    pub const FACE_EYE: usize = EYE;
    /// (FACE) The Mouth slot index.
    pub const FACE_MOUTH: usize = MOUTH;

    /// (FACE) A host face layer (crate::face_layer) takes face slot `slot` (FACE_EYE / FACE_MOUTH): the body notifies'
    /// plays on it are kept, not played, until [`Animator::release_face`]. Its own plays ([`Animator::play_face_row`],
    /// [`Animator::play_face_clip`]) still go on it.
    pub fn hold_face(&mut self, slot: usize) {
        if slot < 2 && !self.face_hold[slot] {
            self.face_hold[slot] = true;
            self.face_deferred[slot] = None;
        }
    }

    /// (FACE) Lets slot `slot` go: the latest body notify play it kept (if any) plays now, at least `blend_in` long.
    /// Returns whether one was kept.
    pub fn release_face(&mut self, slot: usize, blend_in: f32) -> bool {
        if slot >= 2 || !self.face_hold[slot] {
            return false;
        }
        self.face_hold[slot] = false;
        match self.face_deferred[slot].take() {
            Some((name, kind, _same, blend)) => {
                // the slot shows the layer's clip now: the kept play always starts (no same check)
                self.face_play(name, &kind, false, blend.max(blend_in), &[slot]);
                true
            }
            None => false,
        }
    }

    /// (FACE) The slots a host face layer holds (Face, Mouth).
    pub fn face_held(&self) -> [bool; 2] {
        self.face_hold
    }

    /// (FACE) The slots a body notify asked to play in the latest step (held or not).
    pub fn face_notified(&self) -> [bool; 2] {
        self.face_notified
    }

    /// (FACE) A host's face play by ROW name (`Some("TA_DE_M_01")`) or by type (`None`, `kind` = "NORMAL_AUTO",
    /// "MOUTH_OPEN_01" ...) on the slots of `slots` (Face, Mouth), as a notify would play it (the row lookup `name`,
    /// `name_EL`, `name_EYE` / `name_LIP`; `blend_in` < 0 = the clip's own interpolation time). Held slots take it too.
    /// Returns whether a row of that name exists for every slot asked.
    pub fn play_face_row(&mut self, name: Option<&str>, kind: &str, slots: [bool; 2], blend_in: f32, same_check: bool) -> bool {
        let list: Vec<usize> = [EYE, MOUTH].into_iter().filter(|s| slots[*s]).collect();
        let found = self.face_row(name, kind, &list);
        self.face_play(name.map(str::to_string), kind, same_check, blend_in, &list);
        found
    }

    /// (FACE) Whether the pack has a face row for `name` / type `kind` on every slot of `slots` (`face_play`'s lookup).
    pub fn has_face_row(&self, name: Option<&str>, kind: &str, slots: [bool; 2]) -> bool {
        let list: Vec<usize> = [EYE, MOUTH].into_iter().filter(|s| slots[*s]).collect();
        self.face_row(name, kind, &list)
    }

    fn face_row(&self, name: Option<&str>, kind: &str, slots: &[usize]) -> bool {
        let base = match name {
            Some(n) => n.to_string(),
            None => match self.types.iter().find(|(t, _)| *t == kind) {
                Some((_, row)) => row.clone(),
                None => return false,
            },
        };
        slots.iter().all(|&s| self.find_row(&base, s).is_some_and(|(_, clip)| self.clips.contains_key(&clip)))
    }

    /// (FACE) The length (s) of the clip that row `name` plays on slot `slot` (FACE_EYE / FACE_MOUTH).
    pub fn face_row_length(&self, name: &str, slot: usize) -> Option<f32> {
        let (_, clip) = self.find_row(name, slot.min(1))?;
        self.clips.get(&clip).map(|b| b.info.length)
    }

    /// (FACE) Whether body clip `sequence` carries KH3's own face for a voice line that starts in clip time [`from`, `to`]:
    /// the clip has a voice notify (`*PlayVoice*` / `*PlayActorVoice*`) and either a lip play in that window
    /// (`TresAnimNotify_FaceAnimLip`, or a `TresAnimNotify_FaceAnim` of a `MOUTH_OPEN_*` type: Donald's M_1301, MagicFire)
    /// or a face play of a NAMED row up to `to` (the damage mouth DA_0_Lip of M_0211, the team attack's whole face
    /// COO_FF_0_EL of FForceFinish). Such a line is lip-synced by the clip.
    pub fn voiced_lips(&self, sequence: &str, from: f32, to: f32) -> bool {
        let Some(b) = self.clips.get(sequence) else { return false };
        let n = &b.info.notifies;
        let voiced = n.iter().any(|x| x.class.contains("PlayVoice") || x.class.contains("PlayActorVoice"));
        voiced
            && n.iter().any(|x| {
                if !x.class.starts_with("TresAnimNotify_FaceAnim") || x.class == "TresAnimNotify_FaceAnimEye" {
                    return false;
                }
                let named = ["FaceAnimName", "m_AnimName"].iter().any(|k| x.properties.get(k).as_str().is_some_and(|s| !s.is_empty() && s != "None"));
                let lip = x.class == "TresAnimNotify_FaceAnimLip" || ["FaceAnimType", "m_AnimType"].iter().any(|k| x.properties.get(k).as_str().is_some_and(|t| t.starts_with("MOUTH_OPEN")));
                (lip && x.trigger >= from && x.trigger <= to) || (named && x.trigger <= to)
            })
    }

    /// (FACE) The clip times of body clip `sequence`'s voice notifies (`*PlayVoice*` / `*PlayActorVoice*`), and the last
    /// lip play of the clip (s; 0 without one).
    pub fn voice_triggers(&self, sequence: &str) -> (Vec<f32>, f32) {
        let Some(b) = self.clips.get(sequence) else { return (Vec::new(), 0.0) };
        let n = &b.info.notifies;
        let voices = n.iter().filter(|x| x.class.contains("PlayVoice") || x.class.contains("PlayActorVoice")).map(|x| x.trigger).collect();
        let last_lip = n.iter().filter(|x| x.class == "TresAnimNotify_FaceAnimLip" || ["FaceAnimType", "m_AnimType"].iter().any(|k| x.properties.get(k).as_str().is_some_and(|t| t.starts_with("MOUTH_OPEN")))).map(|x| x.trigger).fold(0.0f32, f32::max);
        (voices, last_lip)
    }

    /// (IMPL_ER17, er-host: another game's cutscene poses the body) A play on the Face slot asked for by the host: a row by name
    /// (`Some("NO_1")` = the game-over scene's closed eyes) or the row of a type (`None`, e.g. "NORMAL_AUTO"). A row that is
    /// already the slot's last one is not started again.
    pub fn play_face(&mut self, name: Option<&str>, kind: &str, blend_in: f32) {
        self.face_play(name.map(str::to_string), kind, true, blend_in, &[EYE]);
    }

    /// (party eyes) A face play by name on BOTH face slots from an anim set the pack's face rows do not hold: the
    /// camp UI actor's `Idle_CM_Face` / `IdleBreak_CM_Face` (n_ex00x_CM_Face_AnimSet). The UI actor's anim instance
    /// plays it with the body (FUN_1415440E0: body vt+0x390, then, while `m_bEnableFaceAnim` (ctor 0x141531AC0: true),
    /// vt+0x440 0x14056BA40 -> face by name vt+0x448 0x14055DD00 = eye play 0x14055D270 + lip play 0x14055D790, with
    /// SameCheck false, priority 0, blend-in 0, rate 1). `row` names the row, `clip` its sequence (`face/<clip>.glb`).
    /// Returns false when the clip is not in the pack.
    pub fn play_face_clip(&mut self, row: &str, clip: &str, blend_in: f32) -> bool {
        let Some(info) = self.clips.get(clip).map(|b| &b.info) else { return false };
        let length = info.length;
        for slot in [EYE, MOUTH] {
            self.face[slot] = Some(Montage { row: row.to_string(), clip: clip.to_string(), time: 0.0, blend_time: 0.0, blend_in: blend_in.max(0.0), length, first: true });
            self.last_row[slot] = Some(row.to_string());
        }
        true
    }

    /// (ZETTA) What the Face and Mouth slots play now, to be put back by [`Animator::face_restore`] after a clip a host
    /// played over them (kh-partyhost's face-clip slot: Zettaflare's lip track, the FACE stream's clips).
    pub fn face_save(&self) -> FaceSave {
        FaceSave { face: self.face.clone(), last_row: self.last_row.clone() }
    }

    /// (ZETTA) Puts back what [`Animator::face_save`] kept, blended in over `blend_in` seconds from the face shown now. A
    /// slot that played nothing gets the neutral face row (`NO_0`), else it would hold the clip's last frame.
    pub fn face_restore(&mut self, saved: FaceSave, blend_in: f32) {
        for slot in [EYE, MOUTH] {
            self.face[slot] = match saved.face[slot].clone() {
                Some(mut m) => {
                    m.blend_time = 0.0;
                    m.blend_in = blend_in.max(0.0);
                    Some(m)
                }
                None => self.find_row("NO_0", slot).and_then(|(row, clip)| {
                    let length = self.clips.get(&clip).map(|b| b.info.length)?;
                    Some(Montage { row, clip, time: 0.0, blend_time: 0.0, blend_in: blend_in.max(0.0), length, first: true })
                }),
            };
        }
        self.last_row = saved.last_row;
    }

    fn face_play(&mut self, name: Option<String>, kind: &str, same_check: bool, blend_in: f32, slots: &[usize]) {
        let base = match name {
            Some(name) => name,
            None => match self.types.iter().find(|(t, _)| *t == kind) {
                Some((_, row)) => row.clone(),
                // A type this pack has no row for (not used by the stage-1 clips).
                None => return,
            },
        };
        for &slot in slots {
            let Some((row, clip)) = self.find_row(&base, slot) else { continue };
            if same_check && self.last_row[slot].as_deref() == Some(row.as_str()) {
                continue;
            }
            let Some(info) = self.clips.get(&clip).map(|b| &b.info) else { continue };
            let blend_in = if blend_in < 0.0 { info.interpolation_time } else { blend_in };
            self.face[slot] = Some(Montage { row: row.clone(), clip, time: 0.0, blend_time: 0.0, blend_in, length: info.length, first: true });
            self.last_row[slot] = Some(row);
        }
    }

    /// (party A2) The ground for the Bonamik ground clamp of the next steps (world point and normal; `None`: none).
    pub fn set_bonamik_ground(&mut self, ground: Option<(glam::Vec3, glam::Vec3)>) {
        self.bonamik_ground = ground;
    }

    /// (party A2) The friends' Bonamik notify states by the stock order (ends of the states no longer listed, begins of
    /// the new ones, then the ticks of all active ones). Natives (decompiled C): EnableGroundCollision begin 0x140562500
    /// / end 0x140564EB0; SpeedRate begin 0x140562780 / end 0x140565150 (ctor 0x140559D00 defaults); ChangeGroupParams
    /// and CharaWind as Sora's (bnmsub_notifies_physics.md 1b). ScaleGroupParams (0x140562650 -> KBD 0x143b5f770 /
    /// 0x143b5f9b0 / 0x143b60440 / 0x143b5fbf0 / 0x143b5fe30), Weight (0x140562840 -> 0x143b5f4b0 -> core 0x143b07270)
    /// and Enable: their core writes were not read: NOT applied (listed in `unported_cloth`). CharaWind's tick scale
    /// (Sora's 0033: the clip's `wind` curve) stays 1: the friends' clips' curves are not in the pack [?].
    fn party_cloth_states(&mut self, listed: Vec<(String, usize)>) {
        let Some(mut solver) = self.bonamik.take() else {
            self.party_cloth = listed;
            return;
        };
        let notify = |this: &Animator, key: &(String, usize)| this.clips.get(&key.0).map(|b| b.info.notifies[key.1].clone());
        let groups = |n: &Notify| -> Vec<String> { n.properties.get("m_Groups").items().iter().filter_map(|g| g.as_str().map(str::to_string)).collect() };
        let ended: Vec<(String, usize)> = self.party_cloth.iter().filter(|k| !listed.contains(k)).cloned().collect();
        let begun: Vec<(String, usize)> = listed.iter().filter(|k| !self.party_cloth.contains(k)).cloned().collect();
        for (phase, keys) in [(0u8, &ended), (1u8, &begun)] {
            for key in keys {
                let Some(n) = notify(self, key) else { continue };
                let p = &n.properties;
                let begin = phase == 1;
                match n.class.as_str() {
                    "TresAnimNotifyState_Bonamik_EnableGroundCollision" => {
                        let enable = p.get("m_Enable").as_bool().unwrap_or(false);
                        solver.set_ground_collision(&groups(&n), if begin { enable } else { !enable });
                    }
                    "TresAnimNotifyState_Bonamik_SpeedRate" => {
                        if begin {
                            self.party_speed_saved.retain(|(k, _)| k != key);
                            self.party_speed_saved.push((key.clone(), solver.speed_rate()));
                            solver.set_speed_rate(p.get("m_BeginSpeedRate").as_f32().unwrap_or(1.0), p.get("m_BeginBlendTime").as_f32().unwrap_or(0.0));
                        } else {
                            let saved = self.party_speed_saved.iter().find(|(k, _)| k == key).map(|(_, r)| *r);
                            let rate = if p.get("m_RestoreOriginalSpeedRate").as_bool().unwrap_or(true) { saved.unwrap_or(1.0) } else { p.get("m_EndSpeedRate").as_f32().unwrap_or(1.0) };
                            solver.set_speed_rate(rate, p.get("m_EndBlendTime").as_f32().unwrap_or(0.0));
                            self.party_speed_saved.retain(|(k, _)| k != key);
                        }
                    }
                    "TresAnimNotifyState_Bonamik_ChangeGroupParams" => match crate::bonamik::notify::group_params_from_json(p.get("m_GroupParams")) {
                        Ok(rows) if begin => solver.set_group_param(&rows),
                        Ok(rows) => solver.restore_group_param(&rows),
                        Err(e) => self.warnings.push(format!("Bonamik ChangeGroupParams of {}: {e}", key.0)),
                    },
                    "TresAnimNotifyState_Bonamik_CharaWind" => {
                        if begin {
                            match crate::bonamik::notify::chara_wind_from_json(p) {
                                Ok(wind) => solver.set_chara_wind(&wind),
                                Err(e) => self.warnings.push(format!("Bonamik CharaWind of {}: {e}", key.0)),
                            }
                        } else {
                            solver.clear_chara_wind();
                        }
                    }
                    "TresAnimNotifyState_Bonamik_ScaleGroupParams" => {
                        // ctor 0x140559C90: every m_Change* true, every scale 1.0; m_ApplyToAttachments false
                        let on = |flag: &str| p.get(flag).as_bool().unwrap_or(true);
                        let value = |key: &str| if begin { p.get(key).as_f32().unwrap_or(1.0) } else { 1.0 };
                        solver.set_group_scales(
                            &groups(&n),
                            on("m_ChangeInnerCone").then(|| value("m_InnerConeScale")),
                            on("m_ChangeOuterCone").then(|| value("m_OuterConeScale")),
                            on("m_ChangeLocalForce").then(|| value("m_LocalForceScale")),
                            on("m_ChangeWindScale").then(|| value("m_GroupWindScale")),
                            on("m_ChangeMassScale").then(|| value("m_GroupMassScale")),
                        );
                    }
                    "TresAnimNotifyState_Bonamik_Enable" => {
                        // begin 0x1405623A0 / end 0x140564D50: TurnOn with m_On (else TurnOff), the opposite at the end
                        let on = p.get("m_On").as_bool().unwrap_or(false);
                        let blend = p.get(if begin { "m_BeginBlendTime" } else { "m_EndBlendTime" }).as_f32().unwrap_or(0.0);
                        solver.turn_groups(&groups(&n), if begin { on } else { !on }, blend);
                    }
                    "TresAnimNotifyState_Bonamik_Weight" => {
                        // ctor 0x140559D40: begin weight 0, begin time 0.5, end weight 1, end time 0
                        let (w, t) = if begin { (p.get("m_BeginBonamikWeight").as_f32().unwrap_or(0.0), p.get("m_BeginTime").as_f32().unwrap_or(0.5)) } else { (p.get("m_EndBonamikWeight").as_f32().unwrap_or(1.0), p.get("m_EndTime").as_f32().unwrap_or(0.0)) };
                        solver.set_group_weight(&groups(&n), w, t);
                    }
                    other => {
                        if !self.unported_cloth.iter().any(|c| c == other) {
                            self.unported_cloth.push(other.to_string());
                        }
                    }
                }
            }
        }
        self.bonamik = Some(solver);
        self.party_cloth = listed;
    }

    /// The notifies of the body's update window that concern the pose.
    fn notifies(&mut self, body: &BodySlot) {
        let (from, to) = body.notify_window;
        let pieces: &[(f32, f32)] = if from == 0.0 && to == 0.0 {
            &[]
        } else if to < from {
            &[(from, f32::INFINITY), (0.0, to)]
        } else {
            &[(from, to)]
        };
        let instant = |trigger: f32| pieces.iter().any(|&(a, b)| trigger > a && trigger <= b);
        let state = |begin: f32, end: f32| pieces.iter().any(|&(a, b)| begin <= b && end > a);
        let mut listed: Vec<(String, usize)> = Vec::new();
        let mut cloth: Vec<(String, usize)> = Vec::new();
        let mut plays: Vec<Notify> = Vec::new();
        let mut resets: Vec<(bool, bool, i32)> = Vec::new();
        let mut party_cloth: Vec<(String, usize)> = Vec::new();
        let mut looks: Vec<(String, usize)> = Vec::new();
        for sequence in body.notify_sequences {
            let Some(bound) = self.clips.get(*sequence) else { continue };
            for (k, notify) in bound.info.notifies.iter().enumerate() {
                if notify.class == "TresAnimNotifyState_IK" {
                    if state(notify.trigger, notify.end) {
                        listed.push((sequence.to_string(), k));
                    }
                } else if self.look_at.is_some() && notify.class == "TresAnimNotifyState_LookAt" {
                    // (party I2) the friends' look-at windows (most with m_ReverseTrg: look-at off for the window)
                    if state(notify.trigger, notify.end) {
                        looks.push((sequence.to_string(), k));
                    }
                } else if self.party && notify.class.starts_with("TresAnimNotifyState_Bonamik_") {
                    // (party A2) every Bonamik notify state of every clip (the friends' swim / reaction / team clips)
                    if state(notify.trigger, notify.end) {
                        party_cloth.push((sequence.to_string(), k));
                    }
                } else if !self.party && *sequence == BONAMIK_NOTIFY_CLIP && (notify.class == BONAMIK_GROUP_PARAMS || notify.class == BONAMIK_CHARA_WIND) {
                    // Only clip 0033's Bonamik notify states have an effect with this asset
                    // (tools/lib/bonamik/notify.js; manifest runtime.bonamik.clip_notifies).
                    if state(notify.trigger, notify.end) {
                        cloth.push((sequence.to_string(), k));
                    }
                } else if notify.class.starts_with("TresAnimNotify_FaceAnim") && instant(notify.trigger) {
                    plays.push(notify.clone());
                } else if self.party && notify.class == BONAMIK_RESET_AND_PRE_ROLL && instant(notify.trigger) {
                    // ctor 0x14052BD40: m_Reset true, m_ResetPose false, m_PreRoll 0, m_PreRollForAttachment 0
                    let p = &notify.properties;
                    resets.push((p.get("m_Reset").as_bool().unwrap_or(true), p.get("m_ResetPose").as_bool().unwrap_or(false), p.get("m_PreRoll").as_f32().unwrap_or(0.0) as i32));
                }
            }
        }
        // Instant notifies are triggered in queue order; then the Ends of the states no longer
        // listed, then the Begins of the new ones (stock TriggerAnimNotifies, 0x143F2DC50).
        if self.settings.face {
            for notify in &plays {
                self.face_notify(notify);
            }
        }
        // (party A2) `TresAnimNotify_Bonamik_ResetAndPreRoll` Notify 0x140548a20 [C]: `if (m_Reset)` Reset(mesh KBD,
        // m_ResetPose) (0x1405d6040 -> 0x143b5ea20: per core +0x33F4 = 1, +0x33F5 = m_ResetPose, then the forced
        // evaluation 0x143b5c470); `if (m_PreRoll > 0 || m_PreRollForAttachment > 0)` 0x1405d60f0: SetPreRoll(mesh KBD,
        // m_PreRoll) (0x143b5ff60 -> core 0x143b06ef0), attached components get m_PreRoll + m_PreRollForAttachment (the
        // weapons' Bonamik is not simulated by the port: not applied).
        if let Some(solver) = &mut self.bonamik {
            for (reset, reset_pose, pre_roll) in resets {
                if reset {
                    solver.request_reset(reset_pose);
                    solver.force_evaluation();
                }
                if pre_roll > 0 {
                    solver.set_pre_roll(pre_roll);
                }
            }
        }
        if self.party {
            self.party_cloth_states(party_cloth);
        }
        let property = |this: &Animator, key: &(String, usize), name: &str, default: f32| this.clips.get(&key.0).and_then(|b| b.info.notifies[key.1].properties.get(name).as_f32()).unwrap_or(default);
        for old in self.ik_states.clone() {
            if !listed.contains(&old) {
                let time = property(self, &old, "m_EndInterpolationTime", 0.0);
                self.leg_alpha.end(time);
            }
        }
        for new in &listed {
            if !self.ik_states.contains(new) {
                let (time, target) = (property(self, new, "m_StartInterpolationTime", 0.0), property(self, new, "m_TargetAlpha", 1.0));
                self.leg_alpha.begin(time, target);
            }
        }
        self.ik_states = listed;
        // (party I2) TresAnimNotifyState_LookAt: NotifyEnd 0x140565C90 (m_ReverseTrg ? on : off, m_EndInterpolationTime)
        // of the states no longer listed, then NotifyBegin 0x140563C90 (m_ReverseTrg ? off : on, m_StartInterpolationTime;
        // the time is +0x48 for both calls, asm 0x140563CE5) of the new ones.
        if self.look_at.is_some() {
            let flag = |this: &Animator, key: &(String, usize)| this.clips.get(&key.0).and_then(|b| b.info.notifies[key.1].properties.get("m_ReverseTrg").as_bool()).unwrap_or(false);
            for old in self.look_states.clone() {
                if !looks.contains(&old) {
                    let (time, reverse) = (property(self, &old, "m_EndInterpolationTime", 0.0), flag(self, &old));
                    if let Some(node) = &mut self.look_at {
                        node.notify(reverse, time);
                    }
                }
            }
            for new in &looks {
                if !self.look_states.contains(new) {
                    let (time, reverse) = (property(self, new, "m_StartInterpolationTime", 0.0), flag(self, new));
                    if let Some(node) = &mut self.look_at {
                        node.notify(!reverse, time);
                    }
                }
            }
            self.look_states = looks;
        }
        // The Bonamik notify states, by the same rule: Ends of the states no longer listed, Begins
        // of the new ones, then the Tick of every active one (the new ones included).
        let class = |this: &Animator, key: &(String, usize)| this.clips.get(&key.0).map(|b| b.info.notifies[key.1].class.clone()).unwrap_or_default();
        for old in self.bonamik_states.clone() {
            if !cloth.contains(&old) {
                let class = class(self, &old);
                if let Some(solver) = &mut self.bonamik {
                    if class == BONAMIK_GROUP_PARAMS {
                        solver.notify_group_params_end_0033();
                    } else {
                        solver.notify_chara_wind_end_0033();
                    }
                }
            }
        }
        for new in &cloth {
            if !self.bonamik_states.contains(new) {
                let class = class(self, new);
                if let Some(solver) = &mut self.bonamik {
                    if class == BONAMIK_GROUP_PARAMS {
                        solver.notify_group_params_begin_0033();
                    } else {
                        solver.notify_chara_wind_begin_0033();
                    }
                }
            }
        }
        for active in &cloth {
            if class(self, active) == BONAMIK_CHARA_WIND {
                if let Some(solver) = &mut self.bonamik {
                    solver.notify_chara_wind_tick_0033();
                }
            }
        }
        self.bonamik_states = cloth;
    }

    /// One simulation step. Returns the pose to show for it.
    pub fn step(&mut self, frame: &Frame, world: &dyn IkWorld) -> &Pose {
        self.step_with(frame, world, None)
    }

    /// (IMPL_ER16) `step` with an OVERLAY: after the saved pose (never fed back into it) and before leg IK, KineDriver
    /// and Bonamik, `overlay` may rewrite the local pose (a pose taken from elsewhere: a retargeted cutscene figure).
    /// The helper bones and the cloth then follow the overlaid body.
    pub fn step_with(&mut self, frame: &Frame, world: &dyn IkWorld, overlay: Option<&mut dyn FnMut(&Skeleton, &mut [Trs])>) -> &Pose {
        let dt = frame.dt;
        // The face montages that exist advance: weight in real time from the first update on;
        // position as the body's (spec 8).
        for montage in self.face.iter_mut().flatten() {
            montage.blend_time += dt;
            let first = std::mem::take(&mut montage.first);
            if !(first && self.settings.first_update_holds_position) && montage.length > 0.0 {
                montage.time = (montage.time + dt) % montage.length;
            }
        }
        // A montage created by this update's notifies has weight 0 in this frame's evaluation.
        let face_weights = [EYE, MOUTH].map(|s| self.face[s].as_ref().map(|m| m.weight()));
        let face_before = self.face.clone();
        // The alpha the anim instance read in this frame's native update: the interpolator as the
        // previous pawn tick left it (native_D_legik 3.1, 3.2).
        let alpha = self.leg_alpha.value;
        // (party I2) the pawn tick advances the look-at interpolators before the anim update reads LookAtRate
        if let Some(node) = &mut self.look_at {
            node.update(dt);
        }
        self.face_notified = [false, false]; // (FACE)
        self.notifies(&frame.body);

        // ---- the saved-pose recurrence (spec 6)
        let mut next = self.saved.clone();
        if frame.body.active && frame.body.weight > crate::blend::ZERO_WEIGHT {
            let montage = self.body_pose(&frame.body);
            // (ER83) the share of the outfit's own clips follows the slot's blend
            let w = frame.body.weight.clamp(0.0, 1.0);
            self.own_share = self.own_share_of(&frame.body) * w + self.own_share * (1.0 - w);
            for (out, m) in next.iter_mut().zip(&montage) {
                *out = slot(m, frame.body.weight, out);
            }
        }
        let mut grid_weight = 0.0f32; // (FACE) how much of the eye slot is an eye-look grid this step
        for s in [EYE, MOUTH] {
            // Whatever the body slot did to these bones is replaced: the face layers feed them
            // from their own slots, whose source is the previous saved pose as well.
            for &bone in &self.subtree[s] {
                next[bone] = self.saved[bone];
            }
            if !self.settings.face {
                // Without face animation: frame 0 of the default face clip (spec 12).
                if let Some((_, clip)) = self.find_row("NO_0", s) {
                    let pose = self.sample(&clip, 0.0);
                    for &bone in &self.subtree[s] {
                        next[bone] = pose[bone];
                    }
                }
                continue;
            }
            // A slot whose montage was replaced in this update shows its source (the old
            // instance is cut, the new one has weight 0).
            let unchanged = self.face[s] == face_before[s];
            if let (Some(montage), Some(weight), true) = (&face_before[s], face_weights[s], unchanged) {
                // (party I2) an eye-look blend space row (NO_0_Eye / BA_00_Eye): its grid at the eye input
                let eye = if s == EYE { self.look_at.as_ref().and_then(|l| l.eye_weights(&montage.row)) } else { None };
                if eye.is_some() {
                    grid_weight = weight; // (FACE)
                }
                let pose = match eye {
                    Some(list) if list.len() > 1 => {
                        let poses: Vec<Vec<Trs>> = list.iter().map(|(c, _)| self.sample(c, montage.time)).collect();
                        (0..self.reference.len()).map(|bone| weighted(&poses.iter().zip(&list).map(|(p, (_, w))| (p[bone], *w)).collect::<Vec<_>>())).collect()
                    }
                    Some(list) if list.len() == 1 => self.sample(&list[0].0, montage.time),
                    _ => self.sample(&montage.clip, montage.time),
                };
                for &bone in &self.subtree[s] {
                    next[bone] = slot(&pose[bone], weight, &self.saved[bone]);
                }
            }
        }
        self.saved = next;

        // ---- after the saved pose: leg IK, then KineDriver; never fed back
        let mut local = self.saved.clone();
        // (FACE) the eyes look around with ANY eye row: an eye row that is not an eye-look grid (BA_0_EL of the battle
        // idle, a damage face ...) gets the grid's eyeball turn at the eye input on top (not fed back)
        if grid_weight < 1.0 {
            for (bone, dq) in self.eye_overlay() {
                let q = glam::Quat::IDENTITY.slerp(dq, 1.0 - grid_weight);
                local[bone].rotation = (q * local[bone].rotation).normalize();
            }
        }
        if let Some(overlay) = overlay {
            overlay(&self.skeleton, &mut local);
        }
        self.body_override.clear();
        if self.settings.leg_ik {
            if let Some(node) = &mut self.leg_ik {
                let component = component_space(&self.skeleton, &local);
                node.evaluate(&self.skeleton, &mut local, &component, alpha, dt, &frame.component_to_world, world);
            }
            // (party A2) Donald's graph: CollTwoBoneIK R_ashi1 then L_ashi1, each on the previous node's output (node
            // 8 <- L2C 24, node 9 <- node 8; n_ex001_Anim), both with Alpha <- m_LegIKAlpha (not capped).
            for node in &mut self.coll_ik {
                let component = component_space(&self.skeleton, &local);
                node.evaluate(&self.skeleton, &mut local, &component, alpha, dt, &frame.component_to_world, world);
            }
        }
        // (party I2) ExLookAt (#12) after the leg IK, before the ModifyBone pair (pass-throughs: blend weights 0) and
        // the KineDriver / Bonamik
        if let Some(node) = &mut self.look_at {
            node.evaluate(&self.skeleton, &mut local, &frame.component_to_world, dt);
        }
        let mut component = component_space(&self.skeleton, &local);
        if self.settings.kine_driver {
            if let Some(driver) = &mut self.kine {
                driver.evaluate(&mut component, &local);
                for &bone in driver.written() {
                    if let Some(parent) = self.skeleton.bones[bone].parent {
                        local[bone] = component[bone].relative_to(&component[parent]);
                    }
                }
            }
        }
        // Bonamik: the 30 Hz evaluation and its interpolation, stepped with the simulation dt.
        if self.settings.bonamik {
            if let Some(solver) = &mut self.bonamik {
                // (party A2) the friends' ground (host-set); Sora's host gives none, as before
                let host = BonamikHost { ground: self.bonamik_ground, ..BonamikHost::default() };
                solver.frame(dt, &mut component, &frame.component_to_world, &host);
                for &bone in &self.bonamik_bones {
                    if let Some(parent) = self.skeleton.bones[bone].parent {
                        local[bone] = component[bone].relative_to(&component[parent]);
                    } else {
                        local[bone] = component[bone];
                    }
                }
            }
        }
        self.pose = Pose { local, component };

        // The pawn's tick, after the movement tick: the interpolator advances.
        self.leg_alpha.update(dt);
        &self.pose
    }
}

/// Component-space transforms from local ones (a parent comes before its children).
pub fn component_space(skeleton: &Skeleton, local: &[Trs]) -> Vec<Trs> {
    let mut out: Vec<Trs> = Vec::with_capacity(local.len());
    for (bone, transform) in skeleton.bones.iter().zip(local) {
        let value = match bone.parent {
            Some(parent) => transform.then(&out[parent]),
            None => *transform,
        };
        out.push(value);
    }
    out
}
