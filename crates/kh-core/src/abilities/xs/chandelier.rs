//! (XSI) CHANDELIER (XS 2.7; ability CHANDELIER 262, Loretta, Knight of the Haligtree): a Blizzard-family Finish End
//! (KH3's magic combo ends with it on its last cast) becomes Blizzard Blades' falling ice chandelier: `M_7419`
//! (kind 0xE7; SCSK_CHANDELIER 0.5 SMALL blizzard FREEZE at 1.10 s on the group FinishSK, then SCSK_CHANDELIER_END
//! 5.0 BLOW blizzard FREEZE armour 10 rp 5 when it crashes at `chandelier.end_time`: no notify of the clip names that
//! row). MP: the cast's own cost. Power x`chandelier.scale_1/2/3` by the tier cast (ours). The chandelier falls on
//! the target (group FinishSK, `chandelier.radius`, ours).
//!
//! (XW-b) THE CHANDELIER ITSELF: KH3's StylePawn `w_so088` (native ATresStylePawnSKChandelier, whose overrides only
//! reset flags: its life is ATresAccompanyPawnBase's: it appears, plays its anim BP's one row `M_7423`
//! (Character/wep/w_so088/anm/rt/7423: the chandelier hangs above, its lower body falls at 3.93 s and crashes; the attack
//! notify SCSK_CHANDELIER_END 5.0 .. 5.6 s) and goes, `m_Param_MaxTime` 10 s at most). It stands where the move begins
//! to fall (the target's foot point, else `chandelier.ahead` in front of Sora), turned as Sora, and is shown with its clip
//! timed so that its crash notify falls on `chandelier.end_time` of 7419 (`chandelier.pawn_attack_start`): the hosts draw
//! it ([`pawn`]). The two hits of the move are at that spot (the pawn's own collision group FinishSK2 is there in KH3).

use glam::Vec3;

use super::{has, X};
use crate::enums::ETresAbilityKind as A;
use crate::host::TargetId;
use crate::pawn::Ctx;

pub const CHANDELIER: u8 = 0xE7;
pub const END_GROUP: &str = "XS_CHANDELIER_END";

/// The Blizzard family's tier of a command (Blizzard 33, Blizzara 34, Blizzaga 35 / Blizzagan 36), 0 = not Blizzard.
pub fn blizzard_tier(cmd: u8) -> u8 {
    match cmd {
        33 => 1,
        34 => 2,
        35 | 36 => 3,
        _ => 0,
    }
}

/// THE FINISH END (states/fire_magic.rs tick, sub-step 0): true = the cast became the chandelier (the state chains).
pub(crate) fn take_finish_end(cx: &mut Ctx, cmd: u8, mp: crate::status::MpCost, combo: i32, target: TargetId) -> bool {
    let tier = blizzard_tier(cmd);
    if tier == 0 || !has(cx.pawn, A::Chandelier) {
        return false;
    }
    cx.pawn.status.spend_command_mp(mp, combo);
    // (XW-b) where the chandelier stands: the target's foot point (its centre for the hits), else ahead of Sora
    let (ground, center) = (target != TargetId::NONE).then(|| cx.target(target)).flatten().map_or_else(
        || {
            let a = cx.pawn.yaw.to_radians();
            let g = cx.pawn.cmc.location + Vec3::new(a.cos(), a.sin(), 0.0) * X.chandelier_ahead;
            (g, g + Vec3::Z * cx.pawn.params().capsule_center_z)
        },
        |t| (t.position, t.center),
    );
    // on the floor below (an accompany pawn lands: ATresAccompanyPawnBase's land spawn, m_Param_LandSpawnHeight 1200 cm
    // above; a floating target's chandelier stands on the ground under it)
    let ground = cx.world.line_trace(ground + Vec3::Z * 100.0, ground - Vec3::Z * 1200.0).map_or(ground, |h| h.position);
    cx.pawn.combat.xs.chandelier = Some(Chandelier { scale: [X.chandelier_scale_1, X.chandelier_scale_2, X.chandelier_scale_3][tier as usize - 1], crashed: false, ground, center, yaw: cx.pawn.yaw, fx_time: f32::NEG_INFINITY });
    crate::states::attack::xs_forced_attack(cx.pawn, target, CHANDELIER, false);
    cx.pawn.queue.do_chain();
    true
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Chandelier {
    /// The power factor of the tier cast.
    pub scale: f32,
    pub crashed: bool,
    /// (XW-b) Where the chandelier stands (its foot point), the centre of its hits, its turn (Sora's yaw when it began).
    pub ground: Vec3,
    pub center: Vec3,
    pub yaw: f32,
    /// (XW-c) the pawn's clip time its effects were played up to
    pub fx_time: f32,
}

/// (XW-c) THE CHANDELIER'S OWN EFFECTS: the TresAnimNotify(State)_AttachEffect notifies of its clip
/// (KINGDOM HEARTS III/Content/Character/wep/w_so088/anm/rt/7423.uasset, read from the asset: trigger time s, effect,
/// LocationOffset x / y / z cm, RotationOffset roll deg); every one attaches to its owner (SQEX_ATTACH_OBJECT_OWNER, socket
/// None): the pawn's actor (where it stands, turned by its yaw). One-shots (the first, vw_so088_fin_rsn01, is a notify state
/// 0 .. 1.619 s). The pawn is shown from its clip time `pawn_attack_start - end_time` (3.4 s): the ones before are not
/// played (their time is before the move begins).
/// (XW-d) The shatter hahen00..03 (TypeDataSQEX_FragmentMesh, drawn since XW-d) get roll -90 [?]: their node sits at z -300 / -400
/// cm and their fragment emitters are turned roll +90 (the pawn skeleton root kh_sk's Y-up -> Z-up turn, roll -90, undone):
/// in the actor's own frame every piece lay 3-4 m under the ground; in the root bone's frame the pieces assemble the
/// eight-armed chandelier at 2.5-5 m and burst. The attach frame of SQEX_ATTACH_OBJECT_NAME was not traced; the others
/// keep the actor's frame (XW-c).
pub const PAWN_FX: [(f32, &str, [f32; 3], f32); 17] = [
    (0.0001, "vw_so088_fin_rsn01", [0.0, 0.0, 0.0], 0.0),
    (0.590, "vw_so088_fin_kira00", [0.0, 0.0, 0.0], 0.0),
    (0.779, "vw_so088_fin_kira00", [0.0, 0.0, -30.0], 0.0),
    (0.943, "vw_so088_fin_kira00", [0.0, 0.0, 0.0], 0.0),
    (1.187, "vw_so088_fin_kira00", [0.0, 0.0, -30.0], 0.0),
    (1.268, "vw_so088_fin_kira00", [0.0, 0.0, 15.0], 0.0),
    (1.377, "vw_so088_fin_kira01", [0.0, 0.0, 20.0], 0.0),
    (1.606, "vw_so088_fin_kira00", [0.0, 0.0, 0.0], 180.0),
    (4.554, "vw_so088_fin_han00", [0.0, 0.0, 0.0], 0.0),
    (4.554, "vw_so088_fin_han01", [0.0, 0.0, 0.0], 0.0),
    (4.830, "vw_so088_fin_han01", [0.0, 0.0, 0.0], 0.0),
    (4.837, "vw_so088_fin_hahen00", [0.0, 0.0, 0.0], -90.0),
    (4.990, "vw_so088_fin_hahen01", [0.0, 0.0, 0.0], -90.0),
    (5.105, "vw_so088_fin_hahen02", [0.0, 0.0, 0.0], -90.0),
    (5.226, "vw_so088_fin_han01", [0.0, 0.0, 0.0], 0.0),
    (5.233, "vw_so088_fin_hahen03", [0.0, 0.0, 0.0], -90.0),
    (5.494, "vw_so088_fin_mat", [0.0, 0.0, 0.0], 0.0),
];

/// (XW-c) The effects of the pawn's clip between two of its times `(from, to]`: (effect, world point, rotation (pitch, yaw,
/// roll) degrees) for a chandelier standing at `ground` turned `yaw`.
pub fn pawn_effects(ground: Vec3, yaw: f32, from: f32, to: f32) -> Vec<(&'static str, Vec3, Vec3)> {
    let turn = glam::Quat::from_rotation_z(yaw.to_radians());
    PAWN_FX.iter().filter(|(t, ..)| *t > from && *t <= to).map(|(_, name, off, roll)| (*name, ground + turn * Vec3::from(*off), Vec3::new(0.0, yaw, *roll))).collect()
}

/// (XW-b) The chandelier pawn as the hosts draw it: `w_so088` at `ground` turned `yaw` (the core's yaw convention),
/// its clip 7423 at `time` seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChandelierPawn {
    pub ground: Vec3,
    pub yaw: f32,
    pub time: f32,
}

/// (XW-b) The chandelier pawn of this step: while Chandelier runs, its clip time is 7419's + `pawn_attack_start` -
/// `end_time` (its crash notify on the move's crash), shown from 0 to its clip's end (and never past `pawn_max_time`).
pub fn pawn(state: Option<&Chandelier>, clip_time: f32) -> Option<ChandelierPawn> {
    let c = state?;
    let time = clip_time + X.chandelier_pawn_attack_start - X.chandelier_end_time;
    (time >= 0.0 && time <= X.chandelier_pawn_length.min(X.chandelier_pawn_max_time)).then_some(ChandelierPawn { ground: c.ground, yaw: c.yaw, time })
}

/// The crash spot: (XW-b) the chandelier's own (where it stands), else the target / ahead as before.
fn place(cx: &Ctx) -> Vec3 {
    if let Some(c) = cx.pawn.combat.xs.chandelier {
        return c.center;
    }
    let target = cx.pawn.combat.attack_ctx.target;
    (target != TargetId::NONE).then(|| cx.target(target)).flatten().map_or_else(
        || {
            let a = cx.pawn.yaw.to_radians();
            cx.pawn.cmc.location + Vec3::new(a.cos(), a.sin(), 0.0) * X.chandelier_ahead + Vec3::Z * cx.pawn.params().capsule_center_z
        },
        |t| t.center,
    )
}

pub(crate) fn tick(cx: &mut Ctx) {
    let Some(mut c) = cx.pawn.combat.xs.chandelier else { return };
    // (XW-c) the chandelier's own effects at its clip's times
    if let Some(p) = pawn(Some(&c), cx.pawn.anim.time) {
        let from = if c.fx_time.is_finite() { c.fx_time } else { p.time - 1.0e-4 };
        for (name, at, rotation) in pawn_effects(c.ground, c.yaw, from, p.time) {
            cx.spawn_effect(crate::host::EffectSpawn { id: crate::host::EffectId(crate::notify::notify_asset_id(name)), name, at: crate::host::Attach::World(at), rotation, scale: 1.0, persistent: false });
        }
        c.fx_time = p.time;
    }
    if !c.crashed && cx.pawn.anim.time >= X.chandelier_end_time {
        c.crashed = true;
        let at = place(cx);
        crate::combat::hit::spawn_xs_shot(cx, END_GROUP, "SCSK_CHANDELIER_END", at, X.chandelier_radius, X.chandelier_end_life, None); // the crash is part of the notify's effect (M_7419 0.96 s)
    }
    cx.pawn.combat.xs.chandelier = Some(c);
}

pub(crate) fn end(cx: &mut Ctx) {
    cx.pawn.combat.xs.chandelier = None;
}

/// The power factor of the chandelier's hits.
pub(crate) fn scale(cx: &Ctx) -> f32 {
    cx.pawn.combat.xs.chandelier.map_or(1.0, |c| c.scale)
}

pub(crate) fn group_shape(cx: &Ctx, group: &str) -> Option<(Vec3, Vec3, f32)> {
    (group == "FinishSK").then(|| {
        let p = place(cx);
        (p, p, X.chandelier_radius)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (XW-b) the pawn: shown from its clip's start (7419 at end_time - pawn_attack_start, before the move: so from the
    /// move's first step at pawn_attack_start - end_time) to its clip's end; its crash notify on 7419's end_time
    #[test]
    fn the_chandelier_pawn_runs_its_clip_with_its_crash_on_the_moves() {
        let c = Chandelier { scale: 1.0, crashed: false, ground: Vec3::new(100.0, 0.0, 0.0), center: Vec3::new(100.0, 0.0, 90.0), yaw: 30.0, fx_time: f32::NEG_INFINITY };
        assert!(pawn(None, 0.5).is_none());
        let first = pawn(Some(&c), 0.0).unwrap();
        assert!((first.time - (X.chandelier_pawn_attack_start - X.chandelier_end_time)).abs() < 1.0e-5);
        assert!((pawn(Some(&c), X.chandelier_end_time).unwrap().time - X.chandelier_pawn_attack_start).abs() < 1.0e-5);
        let last = X.chandelier_pawn_length - X.chandelier_pawn_attack_start + X.chandelier_end_time;
        assert!(pawn(Some(&c), last - 0.01).is_some() && pawn(Some(&c), last + 0.01).is_none());
        assert_eq!((first.ground, first.yaw), (c.ground, 30.0));
    }

    /// (XW-c) the pawn's effects: in the move's window (pawn times 3.4 .. 5.633) the shatter ones fire once each at their
    /// times, at the pawn's spot; the early sparkles (before the move) never
    #[test]
    fn the_pawn_plays_its_own_effects_in_its_window() {
        let start = X.chandelier_pawn_attack_start - X.chandelier_end_time;
        let all = pawn_effects(Vec3::new(10.0, 20.0, 0.0), 90.0, start, X.chandelier_pawn_length);
        let names: Vec<&str> = all.iter().map(|(n, ..)| *n).collect();
        assert_eq!(names, ["vw_so088_fin_han00", "vw_so088_fin_han01", "vw_so088_fin_han01", "vw_so088_fin_hahen00", "vw_so088_fin_hahen01", "vw_so088_fin_hahen02", "vw_so088_fin_han01", "vw_so088_fin_hahen03", "vw_so088_fin_mat"]);
        assert!(all.iter().all(|(_, at, rot)| *at == Vec3::new(10.0, 20.0, 0.0) && rot.y == 90.0));
        // the kira offsets turn with the pawn
        let k = pawn_effects(Vec3::ZERO, 90.0, 0.7, 0.8);
        assert!((k[0].1 - Vec3::new(0.0, 0.0, -30.0)).length() < 1.0e-3);
        assert!(pawn_effects(Vec3::ZERO, 0.0, 4.554, 4.6).is_empty(), "a time is played once: (from, to]");
    }
}
