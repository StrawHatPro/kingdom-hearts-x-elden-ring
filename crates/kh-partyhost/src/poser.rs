//! THE PARTY POSER (contract ER.4; the engine-free half of kh-app party_draw.rs, stream A + A2 / B5 / I2 / eyes):
//! Donald and Goofy posed by the engine-free `kh-anim` exactly as Sora (saved pose and face slots, KineDriver with
//! the character's own data, Bonamik hat / coat / ears with the character's own asset, leg IK from the pack's
//! `data/legik.json`), the equipped weapon (one of the 14 staves / 14 shields) on the character's weapon socket
//! (Donald `R_wep_skt`, Goofy `R_wep`, both on bone `R_buki`: TresEquipWeaponData `AttachBoneName`).
//!
//! WHO DRIVES IT: the party runtime once per fixed step ([`crate::PartyRuntime::pose`]), from B's `AnimView` (the
//! anim-set rows resolved at the blend input) or a pad-driven friend's speed-matched loop. The host reads the pack's
//! files (manifest, party.json, the weapon manifests, `kh_anim::Pack::load`) and hands them in; it draws the latest
//! two steps ([`PosedMember::previous`] / [`PosedMember::current`]) interpolated, or takes [`PartyPoser::pose`].
//!
//! Every step yields a [`MemberTap`]: the member's bones, sockets and weapon sockets in the world (stream E's attach
//! resolver, the audio side's footsteps).

use std::collections::{BTreeMap, HashMap};

use glam::{Quat, Vec3};
use kh_anim::{Animator, BodySlot, Frame, NoIkWorld, Pose, Trs};
use serde_json::Value;

use crate::say;

pub const DONALD: u8 = 1;
pub const GOOFY: u8 = 2;

/// The draw id (1 Donald, 2 Goofy) of a party character.
pub fn draw_id(c: kh_host::party::PartyChara) -> u8 {
    if c == kh_host::party::PartyChara::Donald {
        DONALD
    } else {
        GOOFY
    }
}

pub fn npc_of(id: u8) -> Option<&'static str> {
    match id {
        DONALD => Some("n_ex001"),
        GOOFY => Some("n_ex002"),
        _ => None,
    }
}

/// (FACE) THE FRIENDS' HEAD LOOK-UP CAP (degrees of the head's own pitch; OUR DESIGN after the user's rule 2026-10-22: "Limit
/// how high Donald's head can look up to a reasonable amount, then he uses his eyes to look the rest of the way up"; KH3's
/// limits tip Donald's head 84 deg and Goofy's 92 deg up). The rest of a look up goes to the eyes (kh-anim lookat.rs).
/// kh-er keys `party.look_head_up.donald` / `.goofy` (deg; 0 = KH3's own limit).
pub const LOOK_HEAD_UP_MAX_DEG: f32 = 30.0;

/// (FACE) The hosts' override of [`LOOK_HEAD_UP_MAX_DEG`] per member (1 Donald, 2 Goofy): f32 bits, NaN = the default.
static HEAD_UP: [std::sync::atomic::AtomicU32; 3] = [const { std::sync::atomic::AtomicU32::new(0x7FC0_0000) }; 3];

/// (FACE) A host's head look-up cap for member `id` (deg; Some(0) = KH3's own limit; None = [`LOOK_HEAD_UP_MAX_DEG`]).
pub fn set_look_head_up_max(id: u8, deg: Option<f32>) {
    if let Some(slot) = HEAD_UP.get(id as usize) {
        slot.store(deg.unwrap_or(f32::NAN).to_bits(), std::sync::atomic::Ordering::Relaxed);
    }
}

/// (FACE) TEST / DEBUG: a look focus forced on member `id` (world cm; the debug word `party look-at`), over the
/// controller's focus.
static FORCED_LOOK: std::sync::Mutex<[Option<Vec3>; 3]> = std::sync::Mutex::new([None; 3]);

/// (FACE) Forces (or with None frees) member `id`'s look focus (world cm).
pub fn set_forced_look(id: u8, point: Option<Vec3>) {
    if let Ok(mut f) = FORCED_LOOK.lock() {
        if let Some(slot) = f.get_mut(id as usize) {
            *slot = point;
        }
    }
}

fn forced_look(id: u8) -> Option<Vec3> {
    FORCED_LOOK.lock().ok()?.get(id as usize).copied().flatten()
}

fn look_head_up_max(id: u8) -> Option<f32> {
    let v = f32::from_bits(HEAD_UP.get(id as usize)?.load(std::sync::atomic::Ordering::Relaxed));
    Some(if v.is_nan() { LOOK_HEAD_UP_MAX_DEG } else { v })
}

/// (FACE) TEST AID: the environment variable KH_FACE_LOG (any value but "0") turns on the per-step face log.
fn face_log_on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("KH_FACE_LOG").is_ok_and(|v| v != "0"))
}

/// What the party simulation gives the poser per fixed step.
pub struct PoseInput<'a> {
    /// Actor origin, KH world (cm, Z up).
    pub position: Vec3,
    /// Actor yaw, degrees (KH).
    pub yaw: f32,
    /// The fixed step.
    pub dt: f32,
    /// The FullBody slot after the member's pose tick (sequence names = `anims/<seq>.glb` of the pack).
    pub body: BodySlot<'a>,
    pub weapon_visible: bool,
    /// (for stream E) the playing anim-set row and its time; movement mode walking.
    pub anim: (&'a str, f32),
    pub grounded: bool,
    /// B's MemberId.0 (0 Donald, 1 Goofy): the id the tap is published under (E's Attach::Party).
    pub member: u8,
}

/// One member as of the latest step, KH3 space (kh-app party_fx `MemberTap` without `visible`).
#[derive(Clone, Debug, Default)]
pub struct MemberTap {
    /// B's MemberId.0.
    pub member: u8,
    /// The member's steps so far.
    pub step: u64,
    /// Actor origin (at the feet), cm.
    pub position: Vec3,
    /// Facing, degrees about Z from +X toward +Y.
    pub yaw: f32,
    /// Bone and socket name -> (world position cm, world rotation); the weapon's sockets as `wep:<name>` (and by
    /// their own name when the skeleton has none of that name).
    pub bones: HashMap<String, (Vec3, Quat)>,
    pub grounded: bool,
    /// The playing anim-set row and its time.
    pub anim: (String, f32),
    /// The equipped weapon's model name ("w_do010", "w_gu010" ...).
    pub weapon: String,
}

/// One equip item of the character (party.json).
#[derive(Clone, Debug)]
pub struct WeaponDef {
    pub item: String,
    pub item_key: String,
    pub model: String,
    pub socket: String,
}

/// A socket of a skeleton: its bone and its transform relative to the bone (UE).
#[derive(Clone, Debug)]
pub struct Socket {
    pub name: String,
    pub bone: usize,
    pub local: Trs,
}

/// The weapon on the character's socket.
#[derive(Clone, Debug)]
pub struct WeaponMount {
    pub model: String,
    pub item: String,
    /// The character's socket the weapon rides on.
    pub socket: Socket,
    /// The weapon's own sockets in its root's space (reference pose; UE): ATK_00, ATK_01, vfx_* ...
    pub sockets: Vec<(String, Trs)>,
}

/// A member playing an anim-set row by itself (debug words; no simulation behind it).
#[derive(Clone, Debug)]
pub struct DebugPose {
    pub row: String,
    /// (sequence, weight): a blend space at its axes' origin.
    pub samples: Vec<(String, f32)>,
    pub time: f32,
    pub length: f32,
    /// > 0: the member walks a circle of 250 cm about `centre` at this speed (cm/s).
    pub speed: f32,
    pub centre: Vec3,
    pub angle: f32,
    pub yaw: f32,
}

/// One posed member.
pub struct PosedMember {
    pub id: u8,
    pub npc: String,
    pub name: String,
    pub animator: Animator,
    pub previous: Pose,
    pub current: Pose,
    pub position: (Vec3, Vec3),
    pub yaw: (f32, f32),
    /// (party B5) the body pitch of the previous / current step (deg, UE sign: kh-party MemberView.pitch).
    pub pitch: (f32, f32),
    pub sockets: Vec<Socket>,
    pub weapons: Vec<WeaponDef>,
    pub weapon: Option<WeaponMount>,
    pub weapon_visible: bool,
    pub debug: Option<DebugPose>,
    /// The manifest's body rows (name -> row JSON).
    pub rows: HashMap<String, Value>,
    pub steps: u64,
    pub drawn: String,
    /// set_pose_view: the clip and time of the previous call (its notify window).
    pub last_clip: Option<(String, f32)>,
    /// (SM) the drawn height's smoothing over steps and kerbs (`HeightSmooth`).
    pub sm_height: HeightSmooth,
    /// (ZETTA) The face-clip slot drawn (kh_party::face): the serial of the play on the face now (0: none) and the face
    /// slots' plays from before it (put back when the view's clip goes).
    pub face_clip: (u32, Option<kh_anim::pose::FaceSave>),
    /// (FACE) The host face layer (kh_anim::face_layer): the voice line's lip-sync, the talk line's face (KH3's field
    /// voice: rows, the NPC blink timer), on top of the body clips' own face plays.
    pub face_layer: kh_anim::face_layer::FaceLayer,
    /// (FACE) The member's voice line now (the host's audio side: [`PosedMember::set_voice`]).
    pub voice: Option<kh_anim::face_layer::VoiceInput>,
    /// (FACE) The talk line of the latest view (kh_party::face::FaceView::talk).
    pub face_talk: Option<kh_party::face::TalkFace>,
    /// (FACE) The host feeds this member's voice lines (`set_voice` was called): the face layer's voice gate is on.
    pub voice_fed: bool,
}

/// (SM) THE DRAWN HEIGHT ON UNEVEN GROUND (OUR DESIGN; KH3 / UE draw the mesh at the capsule, whose step-up lifts it by a
/// whole stair at once: the Elden Ring harness showed 12.6 cm pops on 15 cm steps). The mesh's height follows the
/// capsule's at a limited climb rate: at least SM_Z_SLOPE x the ground speed (a stair taken as a ramp of that slope), at
/// least SM_Z_FOLLOW x the capsule's own smoothed climb rate (a long slope or a long flight of stairs is followed without
/// a growing lag) and at least SM_Z_MIN_RATE; it never overshoots. The leg IK (which traces from the drawn root) still
/// puts the feet on the real ground. Never more than SM_Z_MAX from the capsule; off the ground (a jump, a fall, a climb)
/// or after a jump of the capsule (a warp, a reseat) the mesh is at the capsule at once. Flat ground: exactly the capsule.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HeightSmooth {
    /// (the drawn height, the capsule's smoothed climb rate, the capsule's last place); None = not started.
    pub state: Option<(f32, f32, Vec3)>,
}

/// (SM) The stair-as-ramp slope: the drawn height climbs at least this x the ground speed.
pub const SM_Z_SLOPE: f32 = 0.25;
/// (SM) ... and at least this x the capsule's smoothed climb rate (no growing lag on a long slope or flight).
pub const SM_Z_FOLLOW: f32 = 1.25;
/// (SM) ... and at least this (cm/s): a kerb taken standing still.
pub const SM_Z_MIN_RATE: f32 = 120.0;
/// (SM) The climb rate's low-pass time (s).
pub const SM_Z_RATE_TAU: f32 = 0.15;
/// (SM) At most this far from the capsule's height (cm).
pub const SM_Z_MAX: f32 = 20.0;
/// (SM) A move of the capsule this long in one step is a warp: no smoothing across it (cm).
pub const SM_Z_WARP: f32 = 120.0;

impl HeightSmooth {
    /// The drawn place for the capsule at `at` this step.
    pub fn step(&mut self, at: Vec3, dt: f32, grounded: bool) -> Vec3 {
        let Some((mut x, mut climb, last)) = self.state.filter(|s| grounded && dt > 0.0 && s.2.distance(at) <= SM_Z_WARP) else {
            self.state = Some((at.z, 0.0, at));
            return at;
        };
        let raw = (at.z - last.z) / dt;
        climb += (raw - climb) * (dt / (SM_Z_RATE_TAU + dt));
        let ground = Vec3::new(at.x - last.x, at.y - last.y, 0.0).length() / dt;
        let rate = (ground * SM_Z_SLOPE).max(climb.abs() * SM_Z_FOLLOW).max(SM_Z_MIN_RATE);
        x += (at.z - x).clamp(-rate * dt, rate * dt);
        x = at.z + (x - at.z).clamp(-SM_Z_MAX, SM_Z_MAX);
        self.state = Some((x, climb, at));
        Vec3::new(at.x, at.y, x)
    }
}

/// What one member's pose is for a renderer (contract ER.4 `PartyPoser::pose(id) -> PosedChar`).
#[derive(Clone, Debug)]
pub struct PosedChar {
    /// The mesh component in the world (actor origin, yaw - 90, the body pitch).
    pub component_to_world: Trs,
    /// Per bone: local (parent space) and component space, the skeleton's order.
    pub local: Vec<Trs>,
    pub component: Vec<Trs>,
    /// The weapon: (model, its root in component space, visible).
    pub weapon: Option<(String, Trs, bool)>,
}

/// A UE FRotator-free socket record of a manifest (`quaternion_ue_xyzw`, `relative_location_ue_cm`, `relative_scale`).
pub fn socket_trs(s: &Value) -> Trs {
    let f = |v: &Value, k: usize, d: f32| v.get(k).and_then(Value::as_f64).map_or(d, |x| x as f32);
    let (q, t, sc) = (&s["quaternion_ue_xyzw"], &s["relative_location_ue_cm"], &s["relative_scale"]);
    Trs { translation: Vec3::new(f(t, 0, 0.0), f(t, 1, 0.0), f(t, 2, 0.0)), rotation: Quat::from_xyzw(f(q, 0, 0.0), f(q, 1, 0.0), f(q, 2, 0.0), f(q, 3, 1.0)).normalize(), scale: Vec3::new(f(sc, 0, 1.0), f(sc, 1, 1.0), f(sc, 2, 1.0)) }
}

/// The mesh component in the world: at the actor origin, turned by the actor's yaw minus 90 degrees (as Sora's).
pub fn component_to_world(position: Vec3, yaw: f32) -> Trs {
    Trs { translation: position, rotation: Quat::from_rotation_z((yaw - 90.0).to_radians()), scale: Vec3::ONE }
}

/// (party B5) `component_to_world` with the actor pitched: actor rotation (pitch, yaw, 0) about its origin, then the
/// mesh's -90 deg yaw.
pub fn component_to_world_pitched(position: Vec3, yaw: f32, pitch: f32) -> Trs {
    if pitch == 0.0 {
        return component_to_world(position, yaw);
    }
    Trs { translation: position, rotation: ue_rotator(pitch, yaw, 0.0) * Quat::from_rotation_z((-90.0f32).to_radians()), scale: Vec3::ONE }
}

/// A UE rotator (pitch, yaw, roll, degrees) as a core-space quaternion: yaw about Z, then pitch about Y (nose up),
/// then roll about X [D: the roll sign].
pub fn ue_rotator(pitch: f32, yaw: f32, roll: f32) -> Quat {
    Quat::from_rotation_z(yaw.to_radians()) * Quat::from_rotation_y(-pitch.to_radians()) * Quat::from_rotation_x(-roll.to_radians())
}

impl PosedMember {
    /// A member (1 Donald, 2 Goofy) from its pack: the animator (`kh_anim::Pack::load` + `Animator::new`), the
    /// pack's `manifest.json` and its `party.json` entry (`members[]` row of that member: name, weapons). In its
    /// reference pose until the first step.
    pub fn new(id: u8, animator: Animator, manifest: &Value, entry: &Value) -> Result<PosedMember, String> {
        let npc = npc_of(id).ok_or(format!("party_draw: no member {id}"))?;
        let skeleton = animator.skeleton();
        let sockets = manifest["sockets"].as_array().into_iter().flatten().filter_map(|s| Some(Socket { name: s["name"].as_str()?.to_string(), bone: skeleton.index_of(s["bone"].as_str()?)?, local: socket_trs(s) })).collect();
        let weapons = entry["weapons"].as_array().into_iter().flatten().filter_map(|w| Some(WeaponDef { item: w["item"].as_str()?.to_string(), item_key: w["item_key"].as_str().unwrap_or("").to_string(), model: w["model"].as_str()?.to_string(), socket: w["socket"].as_str()?.to_string() })).collect::<Vec<_>>();
        let rows = manifest["logical_animations"].as_array().into_iter().flatten().filter_map(|r| Some((r["name"].as_str()?.to_string(), r.clone()))).collect();
        let pose = animator.pose().clone();
        let name = entry["name"].as_str().unwrap_or(npc).to_string();
        Ok(PosedMember { id, npc: npc.to_string(), name, animator, previous: pose.clone(), current: pose, position: (Vec3::ZERO, Vec3::ZERO), yaw: (0.0, 0.0), pitch: (0.0, 0.0), sockets, weapons, weapon: None, weapon_visible: true, debug: None, rows, steps: 0, drawn: String::new(), last_clip: None, sm_height: HeightSmooth::default(), face_clip: (0, None) /* (ZETTA) */, face_layer: Default::default(), voice: None, face_talk: None, voice_fed: false /* (FACE) */ })
    }

    /// The weapon definition of an equip item id ("WEP_DONALD00", "WEP_DONALD_00") or a weapon model ("w_do010").
    pub fn weapon_def(&self, item: &str) -> Result<WeaponDef, String> {
        let norm = |s: &str| s.to_ascii_uppercase().replace('_', "");
        self.weapons.iter().find(|w| w.model.eq_ignore_ascii_case(item) || norm(&w.item) == norm(item) || norm(&w.item_key) == norm(item)).cloned().ok_or(format!("party_draw: {} has no weapon {item}", self.name))
    }

    /// `def` on its socket, with the weapon pack's `manifest.json` (its bones at the reference pose and its sockets).
    pub fn mount_weapon(&mut self, def: &WeaponDef, weapon_manifest: &Value) -> Result<&WeaponMount, String> {
        let socket = self.sockets.iter().find(|s| s.name == def.socket).cloned().ok_or(format!("party_draw: {} has no socket {}", self.name, def.socket))?;
        // the weapon's bones at their reference pose (component space of the weapon mesh, UE), and its sockets on them
        let bones: Vec<(String, Option<usize>, Trs)> = weapon_manifest["skeleton"]["bones"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|b| {
                let f = |k: &str, i: usize, d: f32| b[k].get(i).and_then(Value::as_f64).map_or(d, |x| x as f32);
                let parent = b["parent"].as_i64().filter(|p| *p >= 0).map(|p| p as usize);
                (b["name"].as_str().unwrap_or("").to_string(), parent, Trs { translation: Vec3::new(f("translation_ue", 0, 0.0), f("translation_ue", 1, 0.0), f("translation_ue", 2, 0.0)), rotation: Quat::from_xyzw(f("rotation_ue", 0, 0.0), f("rotation_ue", 1, 0.0), f("rotation_ue", 2, 0.0), f("rotation_ue", 3, 1.0)).normalize(), scale: Vec3::new(f("scale_ue", 0, 1.0), f("scale_ue", 1, 1.0), f("scale_ue", 2, 1.0)) })
            })
            .collect();
        let mut cs: Vec<Trs> = Vec::new();
        for (_, parent, local) in &bones {
            let v = match parent {
                Some(p) => local.then(&cs[*p]),
                None => *local,
            };
            cs.push(v);
        }
        let sockets = weapon_manifest["sockets"].as_array().into_iter().flatten().filter_map(|s| { let b = bones.iter().position(|(n, _, _)| Some(n.as_str()) == s["bone"].as_str())?; Some((s["name"].as_str()?.to_string(), socket_trs(s).then(&cs[b]))) }).collect();
        say!("party_draw: {}: weapon {} ({}) on {} (bone {})", self.name, def.model, def.item, def.socket, self.animator.skeleton().bones[socket.bone].name);
        self.weapon = Some(WeaponMount { model: def.model.clone(), item: def.item.clone(), socket, sockets });
        Ok(self.weapon.as_ref().unwrap())
    }

    /// (F's request) One fixed step from B's `AnimView` (anim-set ROW names): the row resolved to its sequences at the
    /// view's blend input, the notify window from the previous call (a new clip: its first update). The previous clip
    /// is in the animator's saved pose: `weight` blends the new one over it, as the game's slot does.
    #[allow(clippy::too_many_arguments)]
    pub fn set_pose_view(&mut self, member: u8, position: Vec3, yaw: f32, dt: f32, anim: &kh_party::AnimView, weapon_visible: bool, grounded: bool, world: &dyn kh_anim::IkWorld) -> Option<MemberTap> {
        // a debug pose (`party-word A pose ...`) owns the member until `pose <who> off`
        if anim.clip.is_empty() || self.debug.is_some() {
            return None;
        }
        // (party A2) a 2D row: B's AnimView has one axis and pawn.rs:552 puts the SPEED in it, which is the Y axis of
        // WalkRunN/W/B (140..680): evaluated at (0, blend_axis) until B2 adds `blend_axis_y` (contracts, Requests A2 -> B2).
        let two = self.rows.get(anim.clip).is_some_and(|r| r["class"].as_str() == Some("BlendSpace"));
        let (x, y) = if two { (0.0, anim.blend_axis) } else { (anim.blend_axis, 0.0) };
        let samples = row_samples(self, anim.clip, x, y).unwrap_or_else(|_| vec![(anim.sequence.to_string(), 1.0)]);
        if samples.len() > 2 {
            self.animator.set_body_samples(samples.clone());
        }
        // (FACE) a clip played BACKWARDS (the port's back walk while A is held, DON-KIT `charge.walk_back_reverse`): an
        // empty notify window. Read as a wrap it fired every notify of the clip each step (the beak froze open).
        let length = self.animator.clip_info(&samples[0].0).map_or(0.0, |i| i.length);
        let backwards = matches!(&self.last_clip, Some((clip, t)) if clip == anim.clip && *t > anim.time + 1.0e-6 && *t - anim.time < 0.5 * length);
        let from = match &self.last_clip {
            _ if backwards => anim.time, // (FACE)
            Some((clip, t)) if clip == anim.clip && *t <= anim.time + 1.0e-6 => *t,
            Some((clip, t)) if clip == anim.clip => *t, // looped: to < from, the wrap is read by kh-anim
            _ => -1.0,
        };
        self.last_clip = Some((anim.clip.to_string(), anim.time));
        self.apply_face_view(&anim.face); // (ZETTA) the member's face-clip slot over the body
        let names: Vec<&str> = samples.iter().map(|(s, _)| s.as_str()).collect();
        let body = BodySlot { samples: [(names[0], samples[0].1), (names.get(1).copied().unwrap_or(names[0]), samples.get(1).map_or(0.0, |s| s.1))], blend_with: None, notify_sequences: &names, time: anim.time, weight: anim.weight, active: true, notify_window: (from, anim.time) };
        Some(self.step(&PoseInput { position, yaw, dt, body, weapon_visible, anim: (anim.clip, anim.time), grounded, member }, world))
    }

    /// (ZETTA) THE FACE-CLIP SLOT (kh_party::face, `AnimView::face`): a new play (its serial) starts the clip on both face
    /// slots (kh-anim `play_face_clip`, over the face's current pose by its blend-in), the face's previous plays kept; when
    /// the view has no clip any more they are put back (`face_restore`, by the view's blend-out). A clip missing from the
    /// pack is said once per play and leaves the face as it was.
    pub fn apply_face_view(&mut self, face: &kh_party::face::FaceView) {
        self.face_talk = face.talk; // (FACE) the talk line for the face layer
        match face.clip {
            Some(c) if c.serial != self.face_clip.0 => {
                let over = self.face_clip.1.is_some();
                let saved = self.face_clip.1.take().unwrap_or_else(|| self.animator.face_save());
                if self.animator.play_face_clip(c.sequence, c.sequence, c.blend_in) {
                    self.face_clip = (c.serial, Some(saved));
                    // (FACE) the clip owns both slots while it plays: the body's face plays wait (KH3's play priority)
                    self.animator.hold_face(kh_anim::Animator::FACE_EYE);
                    self.animator.hold_face(kh_anim::Animator::FACE_MOUTH);
                } else {
                    say!("party_draw: {}: face clip {} is not in the pack", self.npc, c.sequence);
                    if over {
                        // the clip before it goes: the face it had before that one comes back
                        self.animator.face_restore(saved, c.blend_in);
                    }
                    self.face_clip = (c.serial, None);
                }
            }
            Some(_) => {}
            None => {
                if let Some(saved) = self.face_clip.1.take() {
                    self.animator.face_restore(saved, face.blend_out);
                    // (FACE) then what the body asked for meanwhile, on the slots it asked (it is newer than the saved)
                    self.animator.release_face(kh_anim::Animator::FACE_EYE, face.blend_out);
                    self.animator.release_face(kh_anim::Animator::FACE_MOUTH, face.blend_out);
                }
                self.face_clip.0 = 0;
            }
        }
    }

    /// (party I2) The member's head look-at focus for the next pose step (world, cm; `None` = no focus): the AI
    /// controller's UpdateControlRotation 0x140527460 (SetLookAt(valid, 0.5) + SetLookAtLocation) for kh-anim's
    /// TresAnimNode_ExLookAt (kh-anim src/lookat.rs). Set it before each step.
    pub fn set_look_focus(&mut self, point: Option<Vec3>) {
        self.animator.set_look_focus(point);
    }

    /// (FACE) The cast look's hold (kh_anim::Animator::set_look_hold): his head looks at the cast's target over the cast
    /// clips' own LookAt notify states.
    pub fn set_look_hold(&mut self, on: bool) {
        self.animator.set_look_hold(on);
    }

    /// (FACE) The face of the latest step on his posed skeleton: the eyelid gap (both eyes, cm: RU/LU_eyelid_e to
    /// RD/LD_eyelid_b) and the beak / mouth opening (CU_jaw_b to CD_jaw_b, cm); 0 for a bone the skeleton lacks.
    pub fn face_numbers(&self) -> (f32, f32) {
        let s = self.animator.skeleton();
        let c = &self.current.component;
        let d = |a: &str, b: &str| match (s.index_of(a), s.index_of(b)) {
            (Some(a), Some(b)) if a < c.len() && b < c.len() => (c[a].translation - c[b].translation).length(),
            _ => 0.0,
        };
        (d("RU_eyelid_e", "RD_eyelid_b") + d("LU_eyelid_e", "LD_eyelid_b"), d("CU_jaw_b", "CD_jaw_b"))
    }

    /// (FACE) The member's voice line for the next pose steps (the host's audio side: kh_audio `Engine::party_voice` of
    /// the member, handed on each step; None = silent): the face layer lip-syncs it unless his body clip has KH3's own
    /// lips for it.
    pub fn set_voice(&mut self, voice: Option<kh_anim::face_layer::VoiceInput>) {
        self.voice = voice;
        self.voice_fed = true;
    }

    /// (party B5) The member's body pitch for the next pose step (kh-party `MemberView.pitch`: the SlopeSlide tick's
    /// rotation 0x14126C950); set it before each step. 0 = upright.
    pub fn set_body_pitch(&mut self, pitch: f32) {
        self.pitch = (self.pitch.1, pitch);
    }

    /// One fixed step of the pose (B's slot output).
    pub fn step(&mut self, input: &PoseInput, world: &dyn kh_anim::IkWorld) -> MemberTap {
        // (SM) the drawn place: the capsule's, its height smoothed over steps and kerbs (HeightSmooth)
        let at = self.sm_height.step(input.position, input.dt, input.grounded);
        let input = &PoseInput { position: at, ..*input };
        // (party B5) the body pitch set for this step
        let c2w = component_to_world_pitched(input.position, input.yaw, self.pitch.1);
        // (party A2) the Bonamik ground (owner vf10 0x143B57E80): the movement component's floor when it has one, else a
        // trace along gravity from 10 cm above the root bone to 1000 cm below it (0x143B5BB90, ignored when it starts
        // inside). The port answers both with that trace against the IK world [D: B's floor hit is not in its view].
        let root = input.position;
        let ground = world.sweep_sphere(root + Vec3::Z * 10.0, root - Vec3::Z * 1000.0, 0.0).filter(|h| !h.start_penetrating).map(|h| (h.impact_point, h.impact_normal));
        self.animator.set_bonamik_ground(ground);
        // (FACE) the head's look-up cap (the rest of a look up goes to the eyes)
        if let (Some(cap), Some(l)) = (look_head_up_max(self.id), self.animator.look_at_mut()) {
            l.set_head_up_max(Some(cap));
        }
        // (FACE) the host face layer before the step (its plays show from this step on, as a notify's): the voice line's
        // lips, the talk line's face; a face-clip slot's clip owns the face while it plays
        {
            let talk = self.face_talk.map(|t| kh_anim::face_layer::TalkInput { serial: t.serial, eye: t.eye, blink: t.blink, lip: t.lip, time: t.time, length: t.length });
            let fi = kh_anim::face_layer::FaceInput { voice: self.voice.as_ref(), talk, clip: self.face_clip.1.is_some(), body: input.body.notify_sequences, body_time: input.body.time, gate: self.voice_fed };
            self.face_layer.step(&mut self.animator, &fi, input.dt);
        }
        let pose = self.animator.step(&Frame { dt: input.dt, body: input.body, component_to_world: c2w }, world).clone();
        let first = self.steps == 0;
        self.previous = if first { pose.clone() } else { std::mem::replace(&mut self.current, pose.clone()) };
        self.current = pose;
        self.position = (if first { input.position } else { self.position.1 }, input.position);
        self.yaw = (if first { input.yaw } else { self.yaw.1 }, input.yaw);
        self.weapon_visible = input.weapon_visible;
        self.steps += 1;
        let [(a, wa), (b, wb)] = input.body.samples;
        let face = self.animator.face_playing().map(|p| p.map_or("-".to_string(), |(c, w)| format!("{c} x {w:.2}")));
        self.drawn = format!("{} {}: {a} x {wa:.2}{} t {:.2}, face {} / {}", self.name, input.anim.0, if wb > 0.0 { format!(" + {b} x {wb:.2}") } else { String::new() }, input.body.time, face[0], face[1]);
        // (party I2) the head look-at feed: LookAtRate and the current (pitch, yaw)
        if let Some(l) = self.animator.look_at() {
            self.drawn.push_str(&format!(", look {:.2} ({:.0}, {:.0})", l.rate, l.current.x, l.current.y));
        }
        // (FACE) the face layer's last word (lip track / talk)
        if !self.face_layer.said.is_empty() {
            self.drawn.push_str(&format!(", face layer: {}", self.face_layer.said));
        }
        // (FACE) TEST AID: KH_FACE_LOG=1 prints the face of every step (eye gap, beak opening, the face slots' clips)
        if face_log_on() {
            let (eyes, beak) = self.face_numbers();
            let f = self.animator.face_playing().map(|p| p.map_or("-".to_string(), |(c, w)| format!("{c}x{w:.2}")));
            let look = self.animator.look_at().map_or(String::new(), |l| format!(" look {:.2} head ({:.1}, {:.1}) eyes ({:.1}, {:.1}) x {:.2} cap {:.0}{} at ({:.0}, {:.0}, {:.0})", l.rate, l.current.x, l.current.y, l.eye.x, l.eye.y, l.eye_rate(), l.head_up_max.map_or(0.0, |c| c.to_degrees()), if l.hold { " CAST" } else { "" }, l.point.x, l.point.y, l.point.z));
            say!("face-log: {} step {} row {} t {:.2} eyes {eyes:.1} beak {beak:.1} | {} / {} | {}{look}", self.name, self.steps, input.anim.0, input.anim.1, f[0], f[1], self.face_layer.said);
        }
        // (party A2) leg IK: node kind, alpha, hits
        let coll = self.animator.coll_ik_reports();
        let hits = if coll.is_empty() { let r = self.animator.leg_ik_report(); format!("hit {}{} drop {:.1}", r.hit[0] as u8, r.hit[1] as u8, r.hips_drop) } else { coll.iter().map(|r| format!("hit {} pen {:.1}", r.hit as u8, r.penetration_depth)).collect::<Vec<_>>().join(", ") };
        self.drawn.push_str(&format!(", ik {} a {:.2} {hits}", self.animator.leg_ik_kind(), self.animator.leg_alpha().value));
        // stream E's tap: every bone and socket in the world, the weapon's sockets
        let mut bones: HashMap<String, (Vec3, Quat)> = HashMap::new();
        for (i, b) in self.animator.skeleton().bones.iter().enumerate() {
            let w = self.current.component[i].then(&c2w);
            bones.insert(b.name.clone(), (w.translation, w.rotation));
        }
        for s in &self.sockets {
            let w = s.local.then(&self.current.component[s.bone]).then(&c2w);
            bones.entry(s.name.clone()).or_insert((w.translation, w.rotation));
        }
        let mut weapon_model = String::new();
        if let Some(wp) = &self.weapon {
            weapon_model = wp.model.clone();
            let root = wp.socket.local.then(&self.current.component[wp.socket.bone]).then(&c2w);
            for (name, local) in &wp.sockets {
                let w = local.then(&root);
                bones.insert(format!("wep:{name}"), (w.translation, w.rotation));
                bones.entry(name.clone()).or_insert((w.translation, w.rotation));
            }
        }
        MemberTap { member: input.member, step: self.steps, position: input.position, yaw: input.yaw, bones, grounded: input.grounded, anim: (input.anim.0.to_string(), input.anim.1), weapon: weapon_model }
    }

    /// The latest step's world transform of a bone or of one of the character's sockets (KH space, cm).
    pub fn bone_world(&self, bone: &str) -> Option<Trs> {
        let c2w = component_to_world_pitched(self.position.1, self.yaw.1, self.pitch.1);
        if let Some(i) = self.animator.skeleton().index_of(bone) {
            return Some(self.current.component.get(i)?.then(&c2w));
        }
        let s = self.sockets.iter().find(|s| s.name == bone)?;
        Some(s.local.then(self.current.component.get(s.bone)?).then(&c2w))
    }

    /// The latest step's world transform of a socket of the equipped weapon ("ATK_00", "vfx_top" ...).
    /// (ZETTA 2026-10-23) The weapon's own place in the world (its root: the hand socket's), the latest step: Donald's staff
    /// where it falls in Zettaflare's down (kh_party::donald::zettaflare::feed_staff).
    pub fn weapon_world(&self) -> Option<Trs> {
        let w = self.weapon.as_ref()?;
        let c2w = component_to_world_pitched(self.position.1, self.yaw.1, self.pitch.1);
        Some(w.socket.local.then(self.current.component.get(w.socket.bone)?).then(&c2w))
    }

    pub fn weapon_socket_world(&self, socket: &str) -> Option<Trs> {
        let w = self.weapon.as_ref()?;
        let c2w = component_to_world_pitched(self.position.1, self.yaw.1, self.pitch.1);
        let root = w.socket.local.then(self.current.component.get(w.socket.bone)?).then(&c2w);
        Some(w.sockets.iter().find(|(n, _)| n == socket)?.1.then(&root))
    }

    /// The latest step's pose for a renderer; `alpha` 0..1 between the previous and the latest step (1 = latest).
    pub fn posed(&self, alpha: f32) -> PosedChar {
        // (INT) a jump of more than 3 m between the steps (a warp to the leader, a reseat) is never blended
        let alpha = if self.position.0.distance(self.position.1) > 300.0 || !self.position.0.is_finite() { 1.0 } else { alpha };
        let pose = if alpha >= 1.0 { self.current.clone() } else { self.previous.lerp(&self.current, alpha) };
        let yaw = self.yaw.0 + (((self.yaw.1 - self.yaw.0 + 540.0) % 360.0) - 180.0) * alpha;
        let pitch = self.pitch.0 + (self.pitch.1 - self.pitch.0) * alpha;
        let c2w = component_to_world_pitched(self.position.0.lerp(self.position.1, alpha), yaw, pitch);
        let weapon = self.weapon.as_ref().and_then(|w| Some((w.model.clone(), w.socket.local.then(pose.component.get(w.socket.bone)?), self.weapon_visible)));
        PosedChar { component_to_world: c2w, local: pose.local, component: pose.component, weapon }
    }

    /// A row of the member's body set at its axes' origin, looping (the debug words).
    pub fn debug_pose(&self, row: &str, speed: f32) -> Result<DebugPose, String> {
        // a 2D row at (0, the second axis' value for the circling speed): straight locomotion at that speed
        let y = if speed > 0.0 { speed } else { self.rows.get(row).and_then(|r| r["axis_y"]["min"].as_f64()).unwrap_or(0.0) as f32 };
        let samples = row_samples(self, row, 0.0, y)?;
        let length = samples.iter().filter_map(|(s, _)| self.animator.clip_info(s).map(|i| i.length)).fold(0.0f32, f32::max);
        Ok(DebugPose { row: row.to_string(), samples, time: 0.0, length: length.max(1.0e-3), speed, centre: self.position.1, angle: 0.0, yaw: self.yaw.1 })
    }

    /// The debug pose's steps (`new` fixed steps of `dt`): the row loops, the member circles when it has a speed.
    pub fn step_debug(&mut self, new: u64, dt: f32, world: &dyn kh_anim::IkWorld) -> Vec<MemberTap> {
        let mut taps = Vec::new();
        let Some(mut d) = self.debug.take() else { return taps };
        for _ in 0..new {
            let from = if self.steps == 0 { -1.0 } else { d.time };
            d.time += dt;
            if d.time >= d.length {
                d.time -= d.length;
            }
            let (mut position, mut yaw) = (d.centre, d.yaw);
            if d.speed > 0.0 {
                const RADIUS: f32 = 250.0;
                d.angle += d.speed / RADIUS * dt;
                position = d.centre + Vec3::new(d.angle.cos(), d.angle.sin(), 0.0) * RADIUS;
                yaw = d.angle.to_degrees() + 90.0;
            }
            let names: Vec<&str> = d.samples.iter().map(|(s, _)| s.as_str()).collect();
            let samples = [(names[0], d.samples[0].1), (names.get(1).copied().unwrap_or(names[0]), d.samples.get(1).map_or(0.0, |s| s.1))];
            let body = BodySlot { samples, blend_with: None, notify_sequences: &names, time: d.time, weight: 1.0, active: true, notify_window: (from, d.time) };
            let row = d.row.clone();
            if d.samples.len() > 2 {
                self.animator.set_body_samples(d.samples.clone());
            }
            let member = self.id - 1;
            taps.push(self.step(&PoseInput { position, yaw, dt, body, weapon_visible: true, anim: (&row, d.time), grounded: true, member }, world));
        }
        self.debug = Some(d);
        taps
    }
}

/// (party A2) A 2D `BlendSpace` row's samples at (x, y): stock `UBlendSpace::GetSamplesFromBlendInput` over the asset's
/// grid (manifest `grid_samples`: (grid_num + 1) x (grid_num_y + 1) points, index = x_index * (grid_num_y + 1) + y_index,
/// each point up to three (sample, weight) entries); the four points around the input mixed bilinearly, entries of the
/// same sequence merged, the result normalised. Every sample with a weight > 0, heaviest first.
pub fn grid_samples_2d(r: &Value, x: f32, y: f32) -> Option<Vec<(String, f32)>> {
    let (ax, ay) = (&r["axis"], &r["axis_y"]);
    let range = |a: &Value| -> Option<(f32, f32, usize)> { Some((a["min"].as_f64()? as f32, a["max"].as_f64()? as f32, a["grid_num"].as_u64()? as usize)) };
    let ((x0, x1, nx), (y0, y1, ny)) = (range(ax)?, range(ay)?);
    let samples: Vec<String> = r["samples"].as_array()?.iter().filter_map(|s| s["animation_name"].as_str().map(str::to_string)).collect();
    let grid = r["grid_samples"].as_array()?;
    let unit = |v: f32, lo: f32, hi: f32, n: usize| if hi > lo { (v.clamp(lo, hi) - lo) / (hi - lo) * n as f32 } else { 0.0 };
    let (gx, gy) = (unit(x, x0, x1, nx), unit(y, y0, y1, ny));
    let (ix, iy) = ((gx.floor() as usize).min(nx.saturating_sub(1)), (gy.floor() as usize).min(ny.saturating_sub(1)));
    let (fx, fy) = (gx - ix as f32, gy - iy as f32);
    let mut weights: Vec<(String, f32)> = Vec::new();
    for (dx, dy, w) in [(0, 0, (1.0 - fx) * (1.0 - fy)), (1, 0, fx * (1.0 - fy)), (0, 1, (1.0 - fx) * fy), (1, 1, fx * fy)] {
        let (gxi, gyi) = ((ix + dx).min(nx), (iy + dy).min(ny));
        for e in grid.get(gxi * (ny + 1) + gyi).and_then(|p| p.as_array()).into_iter().flatten() {
            let (Some(i), Some(ew)) = (e["sample_index"].as_u64(), e["weight"].as_f64()) else { continue };
            let Some(name) = samples.get(i as usize) else { continue };
            match weights.iter_mut().find(|(n, _)| n == name) {
                Some(slot) => slot.1 += w * ew as f32,
                None => weights.push((name.clone(), w * ew as f32)),
            }
        }
    }
    weights.retain(|(_, w)| *w > 0.0);
    let total: f32 = weights.iter().map(|(_, w)| w).sum();
    if !(total > 0.0) {
        return None;
    }
    weights.iter_mut().for_each(|(_, w)| *w /= total);
    weights.sort_by(|a, b| b.1.total_cmp(&a.1));
    Some(weights)
}

/// The sequences an anim-set row plays at a blend-space input (x, y) (a plain row: its sequence): a 1D blend space's two
/// samples around x, linear (UBlendSpace1D's grid rule without the input filter); a 2D blend space's full grid result
/// (`grid_samples_2d`; more than two samples go to kh-anim through `Animator::set_body_samples`).
pub fn row_samples(m: &PosedMember, row: &str, axis: f32, axis_y: f32) -> Result<Vec<(String, f32)>, String> {
    let r = m.rows.get(row).ok_or(format!("{} has no row {row}", m.name))?;
    let stem = |f: &str| f.rsplit('/').next().unwrap_or(f).trim_end_matches(".glb").to_string();
    Ok(match r["samples"].as_array() {
        Some(samples) if !samples.is_empty() => {
            let two = r["class"].as_str() == Some("BlendSpace");
            let mut list: Vec<(f32, f32, String)> = samples.iter().filter_map(|s| Some((s["value"].as_f64()? as f32, s["value_y"].as_f64().unwrap_or(0.0) as f32, s["animation_name"].as_str()?.to_string()))).collect();
            if two {
                match grid_samples_2d(r, axis, axis_y) {
                    Some(weights) => weights,
                    None => {
                        list.sort_by(|a, b| ((a.0 - axis).hypot(a.1 - axis_y)).total_cmp(&(b.0 - axis).hypot(b.1 - axis_y)));
                        vec![(list[0].2.clone(), 1.0)]
                    }
                }
            } else {
                list.sort_by(|a, b| a.0.total_cmp(&b.0));
                let x = axis.clamp(list[0].0, list[list.len() - 1].0);
                let after = list.iter().position(|s| s.0 >= x).unwrap_or(list.len() - 1);
                if after == 0 || list[after].0 == x {
                    vec![(list[after].2.clone(), 1.0)]
                } else {
                    let (lo, hi) = (&list[after - 1], &list[after]);
                    let f = (x - lo.0) / (hi.0 - lo.0);
                    vec![(lo.2.clone(), 1.0 - f), (hi.2.clone(), f)]
                }
            }
        }
        _ => vec![(stem(r["file"].as_str().ok_or(format!("row {row}: no file"))?), 1.0)],
    })
}

/// (F's request) A friend's copy for the camp menu's panels: its own pose evaluator playing the camp anim set
/// (`n_ex00x_CM_Body_AnimSet`: Idle_CM = 0001CM, IdleBreak_CM = 0002CM; the UI actor
/// `UI/Blueprints/CampTop/n_ex00x_UIActor` holds no weapon).
pub struct MenuPoser {
    pub npc: String,
    pub animator: Animator,
    /// (sequence, length) of the playing camp row, and its position.
    pub clip: (String, f32),
    pub time: f32,
    pub stepped: bool,
    pub rows: Vec<(String, String, f32)>,
    /// (party eyes) The camp face rows (`n_ex00x_CM_Face_AnimSet`: Idle_CM_Face = face/0001CM, IdleBreak_CM_Face =
    /// face/0002CM): (row, sequence), and the one paired with the playing body row (the UI actor anim instance's
    /// m_AnimNameIdle / m_AnimNameIdleFace and m_AnimNameIdleBreak / m_AnimNameIdleBreakFace, ctor 0x141531AC0).
    pub faces: Vec<(String, String)>,
    pub face: Option<(String, String)>,
}

impl MenuPoser {
    /// The copy of `npc` from its animator (leg IK off) and the pack's manifest (`camp_animations`); playing the
    /// first camp row with its face row.
    pub fn new(npc: &str, animator: Animator, manifest: &Value) -> Result<MenuPoser, String> {
        let rows: Vec<(String, String, f32)> = manifest["camp_animations"]["body"].as_array().into_iter().flatten().filter_map(|r| { let seq = r["sequence"].as_str()?.to_string(); let len = animator.clip_info(&seq)?.length; Some((r["name"].as_str()?.to_string(), seq, len)) }).collect();
        let first = rows.first().cloned().ok_or(format!("party_draw: {npc} has no camp rows"))?;
        // (party eyes) the camp face rows: without them the face subtrees held IdleN's frame 0 (closed eyes)
        let faces: Vec<(String, String)> = manifest["camp_animations"]["face"].as_array().into_iter().flatten().filter_map(|r| Some((r["name"].as_str()?.to_string(), r["sequence"].as_str()?.to_string()))).collect();
        let mut copy = MenuPoser { npc: npc.to_string(), animator, clip: (first.1, first.2), time: 0.0, stepped: false, rows, faces, face: None };
        copy.play_face_of(&first.0);
        Ok(copy)
    }

    /// (party eyes) The face row of body row `row` ("<row>_Face"), started on both face slots with the body, as
    /// FUN_1415440E0 does at every body play (the idle's restart at each loop end, 0x141544440, included).
    pub fn play_face_of(&mut self, row: &str) {
        let wanted = format!("{row}_Face");
        self.face = self.faces.iter().find(|(n, _)| n.eq_ignore_ascii_case(&wanted)).cloned();
        if let Some((name, seq)) = &self.face {
            if !self.animator.play_face_clip(name, seq, 0.0) {
                say!("party_draw: {}: camp face clip {seq} is not in the pack", self.npc);
            }
        }
    }

    fn replay_face(&mut self) {
        if let Some((name, seq)) = self.face.clone() {
            self.animator.play_face_clip(&name, &seq, 0.0);
        }
    }

    /// The camp row the copy plays from its start ("Idle_CM", "IdleBreak_CM"); its length in seconds.
    pub fn play_row(&mut self, row: &str) -> Option<f32> {
        let (_, seq, len) = self.rows.iter().find(|(n, _, _)| n == row)?.clone();
        self.clip = (seq, len);
        self.time = 0.0;
        // (party eyes) the body row's face row with it
        self.play_face_of(row);
        Some(len)
    }

    /// Advances the copy's camp clip by `dt` (looping) and poses it (Bonamik and KineDriver run as in the level).
    /// Returns true when the clip wrapped.
    pub fn step(&mut self, dt: f32) -> bool {
        let from = if self.stepped { self.time } else { -1.0 };
        self.stepped = true;
        self.time += dt;
        let wrapped = self.time >= self.clip.1;
        if wrapped {
            self.time -= self.clip.1;
            // (party eyes) the clip's end restarts body and face (0x141544440 -> FUN_1415440E0)
            self.replay_face();
        }
        let seq = self.clip.0.clone();
        let sequences = [seq.as_str()];
        let body = BodySlot { samples: [(&seq, 1.0), (&seq, 0.0)], blend_with: None, notify_sequences: &sequences, time: self.time, weight: 1.0, active: true, notify_window: (from, self.time) };
        self.animator.step(&Frame { dt, body, component_to_world: Trs { translation: Vec3::ZERO, rotation: Quat::from_rotation_z((-90.0f32).to_radians()), scale: Vec3::ONE } }, &NoIkWorld);
        wrapped
    }
}

/// The party's posers: the members (1 Donald, 2 Goofy) and their camp-menu copies.
#[derive(Default)]
pub struct PartyPoser {
    pub members: BTreeMap<u8, PosedMember>,
    pub menu: BTreeMap<u8, MenuPoser>,
}

impl PartyPoser {
    pub fn has_member(&self, id: u8) -> bool {
        self.members.contains_key(&id)
    }

    pub fn member(&self, id: u8) -> Option<&PosedMember> {
        self.members.get(&id)
    }

    pub fn member_mut(&mut self, id: u8) -> Option<&mut PosedMember> {
        self.members.get_mut(&id)
    }

    /// The latest step's world transform of a bone or socket of member `id`.
    pub fn bone_world(&self, id: u8, bone: &str) -> Option<Trs> {
        self.members.get(&id)?.bone_world(bone)
    }

    /// Contract ER.4: member `id`'s latest pose (`alpha` 1) for a renderer.
    pub fn pose(&self, id: u8, alpha: f32) -> Option<PosedChar> {
        Some(self.members.get(&id)?.posed(alpha))
    }

    /// (FACE) The members' voice lines for their faces' lip-sync, by the audio side's member index (0 Donald, 1 Goofy:
    /// kh_audio `Engine::party_voice`, kh_host::audio::party_voice_inputs). Call once per step before the pose.
    pub fn set_voices(&mut self, voices: &[(u8, Option<kh_anim::face_layer::VoiceInput>)]) {
        for (member, voice) in voices {
            if let Some(m) = self.members.get_mut(&(member + 1)) {
                m.set_voice(voice.clone());
            }
        }
    }
}

/// (party P) The speed-matched locomotion pose of a pad-driven friend (dec016): the run samples by the lean, the walk
/// cycle on its own time, blended by the speed (kh-anim's `BodySlot::blend_with`, as Sora's speed-matched loop).
#[allow(clippy::too_many_arguments)]
pub fn pose_loco(loco_time: &mut [f32; 3], m: &mut PosedMember, v: &kh_party::MemberView, loco: &kh_party::pad::LocoPose, dt: f32, weapon_visible: bool, world: &dyn kh_anim::IkWorld) -> MemberTap {
    let i = (v.chara as usize + 1).min(2);
    let from = loco_time[i];
    loco_time[i] = loco.run_time;
    let names = [loco.run[0].0, loco.run[1].0, loco.walk];
    let body = BodySlot {
        samples: [loco.run[0], loco.run[1]],
        blend_with: (loco.run_weight < 1.0).then_some((loco.walk, loco.walk_time, 1.0 - loco.run_weight)),
        notify_sequences: &names,
        time: loco.run_time,
        weight: v.anim.weight,
        active: true,
        notify_window: (from, loco.run_time),
    };
    let input = PoseInput { position: v.position, yaw: v.yaw, dt, body, weapon_visible, anim: (v.anim.clip, v.anim.time), grounded: v.on_ground, member: v.id.0 };
    m.apply_face_view(&v.anim.face); // (FACE) the face-clip slot and the talk line in the speed-matched loop too
    m.step(&input, world)
}

impl crate::runtime::PartyRuntime {
    /// (FACE) The world point of a cast's look (kh_party::donald::cast_look): an enemy's centre, a member's head, Sora's look
    /// point, a point; ahead = 5 m along his facing at his eyes' height (also when a target is gone).
    fn cast_point(&self, c: kh_party::donald::cast_look::CastLook, v: &kh_party::MemberView, poser: &PartyPoser) -> Vec3 {
        use kh_party::donald::cast_look::CastLook;
        let (s, co) = v.yaw.to_radians().sin_cos();
        let ahead = v.position + Vec3::new(co, s, 0.0) * 500.0 + Vec3::Z * 90.0;
        let head = |m: kh_party::MemberId| self.members.iter().find(|x| x.1 == m).and_then(|x| poser.bone_world(draw_id(x.0), "atama")).map(|t| t.translation);
        match c {
            CastLook::Enemy(t) => self.enemy_centre(t),
            CastLook::Member(m) => head(m),
            CastLook::Sora => self.sora_look_point,
            CastLook::Point(p) => Some(p),
            CastLook::Ahead => None,
        }
        .unwrap_or(ahead)
    }

    /// (party I2) The focal point of a member's focus actor (world, cm): Sora (the player) = his "t_camera" bone, or the
    /// leading friend's head when a friend leads (he is the trees' player); another member = his "atama" bone (the
    /// friends' pawns name no MyLookAtTargetBoneName [A n_ex00x_PawnBase]: their lock-on component's own bone is not
    /// read, the head is taken [D]); an enemy = its centre (the host's look-at point); a point as it is.
    fn look_point(&self, v: &kh_party::MemberView, poser: &PartyPoser) -> Option<Vec3> {
        let head = |m: kh_party::MemberId| self.members.iter().find(|x| x.1 == m).and_then(|x| poser.bone_world(draw_id(x.0), "atama")).map(|t| t.translation);
        match v.focus {
            kh_party::TargetRef::None => None,
            kh_party::TargetRef::Sora => match self.leader {
                Some(l) if l == v.id => None,
                Some(l) => head(l),
                None => self.sora_look_point,
            },
            kh_party::TargetRef::Member(m) if m == v.id => None,
            kh_party::TargetRef::Member(m) => head(m),
            kh_party::TargetRef::Enemy(t) => self.enemy_centre(t),
            kh_party::TargetRef::Point(p) => Some(p),
        }
    }

    /// ONCE PER FIXED STEP after the party's step (contract A.3 / ER.4): each member's pose from B's anim slot (the
    /// sequence in the FullBody montage slot, its time and blend-in weight; the notify window from the last call).
    /// Returns the members' taps of this step (the host publishes them for its effect / sound side).
    pub fn pose(&mut self, poser: &mut PartyPoser, world: &dyn kh_anim::IkWorld, dt: f32) -> Vec<MemberTap> {
        let mut taps = Vec::new();
        for v in self.current.clone().iter() {
            let c = crate::runtime::party_chara(v.chara);
            let id = draw_id(c);
            if !poser.has_member(id) || v.anim.clip.is_empty() {
                continue;
            }
            let weapon = self.party.status(v.id).map(|s| s.weapon).unwrap_or("");
            // (party B5) the slides' body pitch (MemberView.pitch) for this step's pose: the actor rotation (pitch, yaw, 0)
            poser.members.get_mut(&id).unwrap().set_body_pitch(v.pitch);
            // (party I2) the head look-at: the controller focus's focal point (GetFocalPointOnActor 0x141263BF0 = the
            // actor's look-at target point vt+0x6F0; the own pawn / none = no look-at)
            // (FACE) a debug focus over everything; then, while Donald casts, the cast's target and nothing else (the user:
            // "Donald should look at his target when he's casting magic not anything else"; not Zettaflare); else the
            // controller's focus
            let cast = self.party.member(v.id).and_then(kh_party::donald::cast_look::cast_look);
            let focus = forced_look(id).or_else(|| match cast {
                Some(c) => Some(self.cast_point(c, v, poser)),
                None => self.look_point(v, poser),
            });
            let m = poser.members.get_mut(&id).unwrap();
            m.set_look_hold(cast.is_some()); // (FACE) first: the focus comes in at the hold's blend
            m.set_look_focus(focus);
            // (party P) a pad-driven friend in the speed-matched loop (dec016): both cycles on one phase
            // (party K1) and an AI member in the Move state's speed-matched loop (kh-party locomotion)
            if let Some(loco) = self.party.pad_state(v.id).and_then(|p| p.loco).or_else(|| self.party.ai_loco(v.id)) {
                taps.push(pose_loco(&mut self.loco_time, m, v, &loco, dt, !weapon.is_empty(), world));
                continue;
            }
            // A resolves the anim-set rows (blend spaces at the axis) and keeps the notify window (contract A.3)
            if let Some(tap) = m.set_pose_view(v.id.0, v.position, v.yaw, dt, &v.anim, !weapon.is_empty(), v.on_ground, world) {
                taps.push(tap);
            }
        }
        taps
    }
}
