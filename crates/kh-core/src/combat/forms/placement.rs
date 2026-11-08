//! (AUDIT-A PLACEMENT) WHERE KH3 PUTS THE FORM WEAPONS while their own clips (or a throw) move them off their sockets: one
//! generic reader for every form of every package (sheets/port/sora-form-placement.json, tools/sora_form_placement.js).
//!
//! 1. [`part_parent`]: `TresAnimNotifyState_ChangeAttachParent` of a form weapon's OWN clip (the launchers w_so024 5417 /
//!    w_so044 H417 for FINISH_BZ, the staff w_so072 A416, Nano Arms' w_so172 clips): for the window KH3 re-parents the
//!    weapon actor from its socket to Sora's `m_SocketName` (kh_root) and the weapon's clip poses it from there. The clip is
//!    the weapon's row for Sora's sequence, chosen as kh-er-render `keyblade::rig_of` chooses it (a logical-name match
//!    first, `M_` prefixes ignored, then the clip file's stem), at Sora's clip time (the weapon plays in step with him).
//! 2. [`ThrownWeapon`]: `TresAnimNotify_ThrowWeapon` of Sora's form clips (Counter Shield's / Frying Pan's raid B413,
//!    Storm Flag's F412): ::Notify 0x140561C20 -> the state's interface slot 120 0x141345A60 (TresPlayerState_Attack's,
//!    shared by ComboSH / ComboFL): velocity = his actor forward x state+0x250 (2000 after ComboSH Reset 0x141352E90 /
//!    ComboFL Reset 0x141352E20), pawn vt+0x13A8 -> equipment vt+0x550 0x14178F2E0 -> every equipped weapon's vt+0x10C0
//!    0x14178F080: MyMovement InitialSpeed = |v|, Velocity = v, homing toward the attack's target (state+0x118; movement
//!    vt+0x590 0x1414826C0), the weapon plays m_WeaponAnimName (looped unless m_bWeaponAnimLoop is false), weapon state 5
//!    FreeMove (begin 0x14177DB30: its root ABSOLUTE at its world place, turned along the velocity when the movement has
//!    bRotationFollowsVelocity; the movement then moves it: TresProjectileMovementComponent's homing 0x1414751E0 /
//!    0x141481160, `magic::projectile::steer`, m_HomingStartDelay / m_HomingTurnMax / MaxSpeed of the weapon blueprint's
//!    TresWeaponMovement0). The recall (pawn vt+0x13B8(7) -> equipment vt+0x560(7) -> weapon vt+0x10D0 0x14177F050: state
//!    5 -> 0, FreeMove end 0x14177E920: back on its socket) comes when Sora's play leaves the throw clip and the weapon's
//!    clip of Sora's (ComboSH slot 125 0x141366990 before M_B415, 0x141350750 while M_B414; ComboFL slot 125 0x141366630
//!    before M_F414) or the state ends (Attack End 0x141335120). THE DRAWING HOSTS draw a thrown part where this puts it
//!    (kh-er sora.rs `submit`, the effects harness), the effect side its own-clip effects on it (kh-host vfx.rs).
//!
//! StyleFinish's interface slot 120 is an empty function (0x14041F120): Drill Punch's 6421 ThrowWeapon throws nothing;
//! its rows say `ignored`. Sora's base BackRaid (1297, the hand keyblade, Attack kind 41) is not a form part: its row says
//! `base` and is not thrown here.

use crate::host::{TargetFlags, TargetId};
use crate::pawn::Ctx;
use crate::states::StateData;
use glam::{Quat, Vec3};

include!(concat!(env!("KH_GEN_DIR"), "/sora_form_placement.rs"));

/// The weapon clip rows of a model (`parent` / `clip`), in the renderer's `rig_of` order.
fn clip_rows(model: &str) -> impl Iterator<Item = &'static FormPlacementRow> + '_ {
    FORM_PLACEMENT.iter().filter(move |r| r.model == model && (r.what == "parent" || r.what == "clip"))
}

/// The weapon's clip for Sora's sequence `seq`, as kh-er-render `keyblade::rig_of` picks it: a logical name equal to it
/// (either side's `M_` prefix ignored) first, else the clip whose file stem is it.
fn weapon_clip(model: &str, seq: &str) -> Option<&'static FormPlacementRow> {
    let named = |n: &&str| *n == seq || n.strip_prefix("M_") == Some(seq) || seq.strip_prefix("M_") == Some(*n);
    clip_rows(model).find(|r| r.names.iter().any(named)).or_else(|| clip_rows(model).find(|r| r.clip == seq))
}

/// THE PART'S PARENT NOW: Sora's bone / socket that form `cmd`'s part `model` hangs on while Sora plays `sora_seq` at
/// `time` (s), when its own clip's TresAnimNotifyState_ChangeAttachParent window is open (None: its own socket). The
/// hosts place the part on that bone of his drawn pose (`component[bone] x his component`), its own clip as usual.
pub fn part_parent(cmd: i32, model: &str, sora_seq: &str, time: f32) -> Option<&'static str> {
    let r = weapon_clip(model, sora_seq).filter(|r| r.what == "parent" && r.cmds.contains(&cmd))?;
    (r.values.len() >= 2 && time >= r.values[0] && time < r.values[1]).then_some(r.logical)
}

/// (AUDIT-B W-HANDOVER) THE SAME WINDOW WHOEVER DRAWS THE PART: KH3's TresAnimNotifyState_ChangeAttachParent is a notify of
/// the WEAPON'S OWN clip. Its begin 0x1405633F0 calls the weapon's anim interface vt+0x370(m_SocketName), the weapon's
/// 0x1417873F0: `AttachToComponent(m_EquipAttachParent (weapon+0x680), rules, m_SocketName)`. Its end 0x140565500 calls
/// vt+0x378, the weapon's 0x141785E90: `AttachToComponent(m_EquipAttachParent, rules, weapon+0x688)`, the socket the
/// equipment attached it to (0x14177E4E0 stores both). The pose of bone 1 is carried over for that frame, so nothing
/// jumps; the clip poses the weapon from its socket again from the next frame on. No form command is read on that path, so
/// a shotlock's equip set (the Zone Connector's w_so172 M_4481 .. M_4483, gate.rs) re-parents the part as a form does.
/// [`part_parent`] keys by the form command; this reader keys only by the model and Sora's clip.
/// A window that opens at 0.0001 s is open from the clip's first tick (UE fires a notify state's begin on the tick whose
/// range covers its trigger time; tools/sora_form_nano.js `inWindow` bakes the weapon's effects by the same rule); it closes
/// exactly at its end trigger (the end's AttachToComponent runs on the tick that passes it).
pub fn clip_parent(model: &str, sora_seq: &str, time: f32) -> Option<&'static str> {
    let r = weapon_clip(model, sora_seq).filter(|r| r.what == "parent")?;
    let open = if r.values.first().is_some_and(|b| *b <= 1.0e-3) { 0.0 } else { r.values.first().copied().unwrap_or(0.0) };
    (r.values.len() >= 2 && time >= open && time < r.values[1]).then_some(r.logical)
}

/// Sora's SOCKET a part is re-parented to (`kh_root`: a SkeletalMeshSocket of his skeleton, not a bone): (its bone, its
/// RelativeLocation cm, its RelativeRotation (pitch, yaw, roll) degrees). The hosts place the part at bone x socket.
pub fn socket(name: &str) -> Option<(&'static str, Vec3, Vec3)> {
    let r = FORM_PLACEMENT.iter().find(|r| r.what == "socket" && r.logical == name)?;
    Some((r.clip, Vec3::new(tv(r, 0), tv(r, 1), tv(r, 2)), Vec3::new(tv(r, 3), tv(r, 4), tv(r, 5))))
}

/// `FRotator(pitch, yaw, roll).Quaternion()` (UE's formula, as tools/sora_form_placement.js `rotQ`), degrees.
fn ue_quat(pitch: f32, yaw: f32, roll: f32) -> Quat {
    let h = std::f32::consts::PI / 360.0;
    let (sp, cp) = (pitch * h).sin_cos();
    let (sy, cy) = (yaw * h).sin_cos();
    let (sr, cr) = (roll * h).sin_cos();
    Quat::from_xyzw(cr * sp * sy - sr * cp * cy, -cr * sp * cy - sr * cp * sy, cr * cp * sy - sr * sp * cy, cr * cp * cy + sr * sp * sy)
}

/// The rotator of a direction (`FVector::Rotation`: roll 0) as a UE quaternion.
fn along(v: Vec3) -> Quat {
    let yaw = v.y.atan2(v.x).to_degrees();
    let pitch = v.z.atan2(v.truncate().length()).to_degrees();
    ue_quat(pitch, yaw, 0.0)
}

// The numbers of a `throw` row (tools/sora_form_placement.js): [0] the notify time, then these.
const T_SPEED: usize = 1;
const T_DELAY: usize = 2;
const T_TURN: usize = 3;
const T_MAX: usize = 4;
const T_POS: usize = 5;
const T_ROT: usize = 8;

fn tv(r: &FormPlacementRow, i: usize) -> f32 {
    r.values.get(i).copied().unwrap_or(0.0)
}

/// A FORM PART KH3 HAS THROWN (weapon state 5, FreeMove): where it is in the world (KH3 space, cm) and how it is turned
/// (a UE quaternion: the weapon actor's, its mesh component's frame), its own throw clip and how long since the throw.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThrownWeapon {
    /// The `throw` row (FORM_PLACEMENT index).
    pub row: usize,
    pub model: &'static str,
    pub position: Vec3,
    pub velocity: Vec3,
    pub rotation: Quat,
    /// Seconds since the throw (the weapon's clip clock).
    pub time: f32,
    target: TargetId,
    homing_delay: f32,
    state: std::mem::Discriminant<StateData>,
}

impl ThrownWeapon {
    fn data(&self) -> &'static FormPlacementRow {
        &FORM_PLACEMENT[self.row]
    }

    /// The weapon's throw clip: (its logical name `M_B414`, its sequence `B414`).
    pub fn clip(&self) -> (&'static str, &'static str) {
        let r = self.data();
        (r.names.first().copied().unwrap_or("-"), r.names.get(1).copied().unwrap_or("-"))
    }

    /// The weapon clip's time now (looped by m_bWeaponAnimLoop, else held at its end).
    pub fn clip_time(&self) -> f32 {
        let r = self.data();
        if r.length <= 0.0 {
            0.0
        } else if r.flags.get(2) == Some(&"loop") {
            self.time.rem_euclid(r.length)
        } else {
            self.time.min(r.length)
        }
    }

    /// Where the part left his hand, in his ACTOR frame at the throw (cm from his origin, a UE quaternion): his socket at
    /// the notify time (for the effect side: its own-clip effects baked in that frame, kh-host vfx.rs).
    pub fn socket_at_throw(&self) -> (Vec3, Quat) {
        let r = self.data();
        (Vec3::new(tv(r, T_POS), tv(r, T_POS + 1), tv(r, T_POS + 2)), Quat::from_xyzw(tv(r, T_ROT), tv(r, T_ROT + 1), tv(r, T_ROT + 2), tv(r, T_ROT + 3)).normalize())
    }
}

/// The throw rows of form `cmd` at grow level `grow` for Sora's sequence `seq` (the parts KH3 throws there).
fn throw_rows(cmd: i32, grow: u8, seq: &str) -> impl Iterator<Item = (usize, &'static FormPlacementRow)> + '_ {
    let level = grow.max(1).to_string();
    FORM_PLACEMENT.iter().enumerate().filter(move |(_, r)| r.what == "throw" && r.clip == seq && r.cmds.contains(&cmd) && r.flags.first() == Some(&level.as_str()) && r.flags.get(4) == Some(&"thrown"))
}

/// THE RECALL (pawn vt+0x13B8(7) -> equipment vt+0x560(7) -> weapon vt+0x10D0 0x14177F050: FreeMove ends, the part is back
/// on its socket): when Sora's play is no longer the throw clip nor the weapon clip's row of his (ComboSH slot 125 0x141366990
/// recalls right before it plays M_B415, ComboFL's 0x141366630 before M_F414), or his state is another (Attack End
/// 0x141335120). After the state's tick (sora.rs, the same step its next play starts) and at the pose tick.
pub(crate) fn recall(state: &StateData, cx: &mut Ctx) {
    if cx.pawn.combat.thrown.is_empty() {
        return;
    }
    let seq = cx.pawn.anim.request().body_samples()[0].0;
    let here = std::mem::discriminant(state);
    cx.pawn.combat.thrown.retain(|t| {
        let r = &FORM_PLACEMENT[t.row];
        t.state == here && (r.clip == seq || r.names.get(2) == Some(&seq))
    });
}

/// After the pose tick's notify handlers (sora.rs), every tick: the ThrowWeapon notify throws the form's parts and the thrown
/// parts fly (their recall: [`recall`]).
pub(crate) fn on_pose_tick(state: &StateData, cx: &mut Ctx, notifies: &crate::anim::Notifies, dt: f32) {
    recall(state, cx);
    let seq = cx.pawn.anim_ticked.body_samples()[0].0;
    let here = std::mem::discriminant(state);
    // the flight (FreeMove: the movement component; homing as magic/projectile.rs ports TresProjectileMovementComponent)
    let mut thrown = std::mem::take(&mut cx.pawn.combat.thrown);
    for t in &mut thrown {
        fly(cx, t, dt);
    }
    cx.pawn.combat.thrown = thrown;
    // the throw: the state's interface slot 120 (the Attack class's 0x141345A60; StyleFinish's is empty)
    if !notifies.fired(crate::anim::NotifyClass::TresAnimNotifyThrowWeapon) {
        return;
    }
    let Some(a) = state.attack() else { return };
    let (cmd, grow) = (cx.pawn.combat.form.cmd, cx.pawn.combat.form.grow);
    let (sin, cos) = cx.pawn.yaw.to_radians().sin_cos();
    let (ps, pc) = cx.pawn.pitch.to_radians().sin_cos();
    let forward = Vec3::new(pc * cos, pc * sin, ps);
    let yaw_q = ue_quat(0.0, cx.pawn.yaw, 0.0);
    let location = cx.pawn.cmc.location;
    for (row, r) in throw_rows(cmd, grow, seq) {
        if cx.pawn.combat.thrown.iter().any(|t| t.model == r.model) {
            continue; // vt+0x10C0: a weapon already in state 5 is not thrown again
        }
        let (p, q) = (Vec3::new(tv(r, T_POS), tv(r, T_POS + 1), tv(r, T_POS + 2)), Quat::from_xyzw(tv(r, T_ROT), tv(r, T_ROT + 1), tv(r, T_ROT + 2), tv(r, T_ROT + 3)).normalize());
        let velocity = forward * tv(r, T_SPEED);
        // FreeMove begin 0x14177DB30: at its world place; along its velocity when its movement turns it so
        let rotation = if r.flags.get(1) == Some(&"follow") && velocity.length_squared() > 0.0 { along(velocity) } else { yaw_q * q };
        let homing = r.flags.get(5) == Some(&"homing");
        cx.pawn.combat.thrown.push(ThrownWeapon { row, model: r.model, position: location + Vec3::new(p.x * cos - p.y * sin, p.x * sin + p.y * cos, p.z), velocity, rotation, time: 0.0, target: if homing { a.target } else { TargetId::NONE }, homing_delay: tv(r, T_DELAY), state: here });
    }
}

/// (AUDIT-A THROW) THE THROWN PART'S HIT SHAPE of group `group`: (end 1, end 2, radius) in the world, or None when no thrown
/// part has that group. KH3: TresAnimNotifyState_Attack begin 0x140561F40 -> the pawn's interface slot 64 (0x1405B3FE0)
/// activates the group on EVERY equipped weapon's own component (weapon+0x578 MyAtkColl, Activate 0x140625500) and on the
/// pawn's; each primitive's Update 0x140647950 places its shape at SocketTransform 0x14064BA60 of the component's attach
/// parent, the weapon mesh (NORMAL). FreeMove 0x14177DB30 does not detach the weapon (its root goes absolute at its world
/// place), so its shapes fly with it, posed by its own throw clip. The `tshape` rows (tools/sora_form_placement.js) carry
/// the shape per frame of that clip in the part's frame (`position` / `rotation`); every frame between is interpolated as
/// the in-hand tracks are (combat/forms/shield.rs `track_shape`).
pub fn thrown_shape(thrown: &[ThrownWeapon], group: &str) -> Option<(Vec3, Vec3, f32)> {
    thrown.iter().find_map(|t| {
        let id = FORM_PLACEMENT[t.row].id;
        let r = FORM_PLACEMENT.iter().find(|r| r.what == "tshape" && r.logical == group && r.names.first() == Some(&id))?;
        let frames = r.values.len() / 7;
        let last = frames.checked_sub(1)?;
        let f = if r.length > 0.0 { (t.clip_time() / r.length).clamp(0.0, 1.0) * last as f32 } else { 0.0 };
        let (i, k) = ((f.floor() as usize).min(last), f - f.floor());
        let j = (i + 1).min(last);
        let at = |n: usize| r.values[i * 7 + n] + (r.values[j * 7 + n] - r.values[i * 7 + n]) * k;
        let world = |v: Vec3| t.position + t.rotation * v;
        Some((world(Vec3::new(at(0), at(1), at(2))), world(Vec3::new(at(3), at(4), at(5))), at(6)))
    })
}

/// One tick of a thrown part's movement component.
fn fly(cx: &Ctx, t: &mut ThrownWeapon, dt: f32) {
    let r = &FORM_PLACEMENT[t.row];
    t.time += dt;
    if t.velocity == Vec3::ZERO {
        return; // no movement component (or no speed): it stays where it left his hand
    }
    if t.target != TargetId::NONE {
        match cx.target(t.target).filter(|x| x.flags.has(TargetFlags::ALIVE)) {
            Some(target) => {
                if t.homing_delay > 0.0 {
                    t.homing_delay = (t.homing_delay - dt).max(0.0);
                }
                if t.homing_delay <= 0.0 {
                    let turn = if tv(r, T_TURN) > 0.0 { (tv(r, T_TURN).to_radians() * dt).min(std::f32::consts::PI) } else { std::f32::consts::PI };
                    t.velocity = crate::magic::projectile::steer(t.velocity, target.center - t.position, turn, -1.0);
                }
            }
            None => t.target = TargetId::NONE,
        }
    }
    let max = tv(r, T_MAX);
    if max > 0.0 && t.velocity.length() > max {
        t.velocity = t.velocity.normalize() * max;
    }
    t.position += t.velocity * dt;
    if r.flags.get(1) == Some(&"follow") && t.velocity.length_squared() > 1.0e-8 {
        t.rotation = along(t.velocity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every ChangeAttachParent instance of every form weapon: the launchers' FINISH_BZ, the staff's braid, Nano Arms.
    #[test]
    fn the_change_attach_parent_windows() {
        // Magic Launcher (0x83) w_so024 5417 and Hunny Launcher (0x8F) w_so044 H417 (its logical M_5417): kh_root 0 .. 3 s
        assert_eq!(part_parent(0x83, "w_so024", "5417", 0.0), Some("kh_root"));
        assert_eq!(part_parent(0x83, "w_so024", "5417", 2.99), Some("kh_root"));
        assert_eq!(part_parent(0x83, "w_so024", "5417", 3.0), None);
        assert_eq!(part_parent(0x83, "w_so024", "5413", 0.5), None, "the combo clip keeps it on R_wep1_skt");
        assert_eq!(part_parent(0x8F, "w_so044", "5417", 1.0), Some("kh_root"), "Sora's 5417 picks the launcher's H417 by its logical M_5417");
        assert_eq!(part_parent(0x8F, "w_so044", "H417", 1.0), Some("kh_root"), "or by the file stem");
        assert_eq!(part_parent(0x8F, "w_so044", "5417", 0.0), None, "its window opens at 0.0001 s");
        assert_eq!(part_parent(0x83, "w_so044", "5417", 1.0), None, "not a part of Magic Launcher");
        // Mirage Staff (0x88) w_so072 A416: kh_root 0 .. 3.1 s (STAFF's former staff::part_parent)
        assert_eq!(part_parent(0x88, "w_so072", "A416", 1.5), Some("kh_root"));
        assert_eq!(part_parent(0x88, "w_so072", "M_A416", 3.05), Some("kh_root"));
        assert_eq!(part_parent(0x88, "w_so072", "A415", 0.5), None);
        // Nano Arms (0x86) w_so172: 15 clips; 8415's window ends at 1.1667 of its 1.7333 s
        assert_eq!(part_parent(0x86, "w_so172", "8415", 1.0), Some("kh_root"));
        assert_eq!(part_parent(0x86, "w_so172", "8415", 1.2), None);
        assert_eq!(part_parent(0x86, "w_so172", "DodgeRollB", 0.5), Some("kh_root"), "its 8051 by the logical DodgeRollB");
        assert_eq!(part_parent(0x86, "w_so172", "4481", 0.5), Some("kh_root"), "its 8481 is the logical M_4481");
        // (AUDIT-B W-HANDOVER) the tails: 8415's window closes at 1.1667 of its 1.7333 s, 8483's at 0.6666 of 0.7 s; then
        // the weapon is back on the socket its equipment gave it; the same window for a shotlock's equip set
        assert_eq!(clip_parent("w_so172", "8415", 1.16), Some("kh_root"));
        assert_eq!(clip_parent("w_so172", "8415", 1.17), None, "8415's tail: back on R_wep_skt");
        assert_eq!(clip_parent("w_so172", "4483", 0.66), Some("kh_root"), "the Zone Connector's M_4483 (no form command)");
        assert_eq!(clip_parent("w_so172", "4483", 0.667), None, "8483's tail");
        assert_eq!(clip_parent("w_so172", "8301", 0.2), None, "the combo clips have no weapon clip: the socket");
        assert_eq!(clip_parent("w_so024", "5417", 1.0), part_parent(0x83, "w_so024", "5417", 1.0));
        assert_eq!(clip_parent("w_so172", "8411", 0.0), Some("kh_root"), "open from the clip's first tick (its trigger 0.0001)");
        let windows = FORM_PLACEMENT.iter().filter(|r| r.what == "parent").count();
        assert_eq!(windows, 18, "w_so024 1 + w_so044 1 + w_so072 1 + w_so172 15");
        assert!(FORM_PLACEMENT.iter().filter(|r| r.what == "parent").all(|r| r.logical == "kh_root" && r.values.len() == 2 && r.values[0] < r.values[1]));
        // kh_root is a socket of his skeleton: on kh_sk, turned roll -90 (it undoes kh_sk's own quarter turn)
        assert_eq!(socket("kh_root"), Some(("kh_sk", Vec3::ZERO, Vec3::new(0.0, 0.0, -90.0))));
    }

    /// Every ThrowWeapon of the forms: the raid's shield / pan and the flag throw; Drill Punch's is ignored, BackRaid is base.
    #[test]
    fn the_throw_rows() {
        let thrown: Vec<&str> = FORM_PLACEMENT.iter().filter(|r| r.what == "throw" && r.flags.get(4) == Some(&"thrown") && r.flags.first() == Some(&"1")).map(|r| r.model).collect();
        assert_eq!(thrown, ["w_so032", "w_so142", "w_so093"]);
        let shield = FORM_PLACEMENT.iter().find(|r| r.id == "throw.SH.1.B413.w_so032").expect("the raid");
        assert_eq!((tv(shield, 0), tv(shield, T_SPEED), tv(shield, T_DELAY), tv(shield, T_TURN), tv(shield, T_MAX)), (0.3005, 2000.0, 0.1, 360.0, 6000.0));
        assert_eq!((shield.names, shield.flags[1], shield.flags[2]), (&["M_B414", "B414", "B414"][..], "follow", "loop"));
        let flag = FORM_PLACEMENT.iter().find(|r| r.id == "throw.FL.1.F412.w_so093").expect("the flag throw");
        assert_eq!((flag.names, flag.flags[1]), (&["M_F413", "F413", "F413"][..], "keep"), "w_so093's movement does not turn it");
        assert!(FORM_PLACEMENT.iter().filter(|r| r.clip == "6421").all(|r| r.flags.get(4) == Some(&"ignored")), "StyleFinish's slot 120 is empty");
        assert!(FORM_PLACEMENT.iter().any(|r| r.id == "throw.base.1297" && r.flags.get(4) == Some(&"base")));
        assert_eq!(throw_rows(0x89, 1, "B413").count(), 1);
        assert_eq!(throw_rows(0x89, 2, "B413").count(), 2, "the grown units: w_so033 and w_so037");
        assert_eq!(throw_rows(0x8D, 1, "F412").count(), 1);
    }

    /// (AUDIT-A THROW) every thrown part's attack groups of its throw / flight clips fly with it: one row per (throw, group),
    /// no group on two parts of one grow level (KH3 gives each component its own record: one window each), placed in the
    /// part's frame.
    #[test]
    fn the_thrown_parts_hit_shapes() {
        let rows: Vec<_> = FORM_PLACEMENT.iter().filter(|r| r.what == "tshape").collect();
        let ids: Vec<&str> = rows.iter().map(|r| r.id).collect();
        assert_eq!(ids, ["tshape.SH.1.B413.w_so032.None", "tshape.SH.2.B413.w_so033.None", "tshape.SH.3.B413.w_so034.None", "tshape.SH2.1.B413.w_so142.None", "tshape.SH2.2.B413.w_so143.None", "tshape.SH2.3.B413.w_so144.None", "tshape.FL.1.F412.w_so093.KB_StickL", "tshape.FL.2.F412.w_so093.KB_StickL", "tshape.FL.3.F412.w_so093.KB_StickL"]);
        for r in &rows {
            assert!(FORM_PLACEMENT.iter().any(|t| t.what == "throw" && Some(&t.id) == r.names.first()), "{}: its throw row", r.id);
            assert!(!r.values.is_empty() && r.values.len() % 7 == 0, "{}: [a, b, radius] per frame", r.id);
            let same = rows.iter().filter(|o| o.logical == r.logical && o.cmds == r.cmds && o.flags.first() == r.flags.first()).count();
            assert_eq!(same, 1, "{}: one part per grow level has the group", r.id);
        }
        // the shield's SPHERE 120 on ATK_01 (21 cm out along `wep`), placed on the flying part
        let row = FORM_PLACEMENT.iter().position(|r| r.id == "throw.SH.1.B413.w_so032").unwrap();
        let mut t = ThrownWeapon { row, model: "w_so032", position: Vec3::new(500.0, 0.0, 100.0), velocity: Vec3::ZERO, rotation: Quat::from_rotation_z(std::f32::consts::FRAC_PI_2), time: 0.0, target: TargetId::NONE, homing_delay: 0.0, state: std::mem::discriminant(&StateData::DEFAULT) };
        let (a, b, radius) = thrown_shape(&[t], "None").unwrap();
        assert_eq!((a, radius), (b, 120.0));
        let local = Vec3::new(7.2, 0.0, 19.7);
        assert!((a - (t.position + t.rotation * local)).length() < 1.0e-3, "{a}");
        t.time = 0.65;
        assert_ne!(thrown_shape(&[t], "None").unwrap().0, a, "posed by its own M_B414");
        assert_eq!(thrown_shape(&[t], "KB_StickL"), None, "not a group of the shield");
        assert_eq!(thrown_shape(&[], "None"), None);
    }

    #[test]
    fn a_rotator_of_a_direction() {
        let q = along(Vec3::new(0.0, 1.0, 0.0));
        assert!((q * Vec3::X - Vec3::Y).length() < 1.0e-5, "yaw 90: X -> Y");
        let q = along(Vec3::new(1.0, 0.0, 1.0));
        assert!((q * Vec3::X - Vec3::new(1.0, 0.0, 1.0).normalize()).length() < 1.0e-5, "pitch 45 up: X -> (1, 0, 1)");
    }
}
