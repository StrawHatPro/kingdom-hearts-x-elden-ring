//! (FREEZE-FX, THE USER 2026-11-07: "make sure frozen effects are applied to anything that gets frozen by blizzard") KH3'S
//! FROZEN LOOK ON ANY VICTIM: the one shared route every host runs for whatever it has frozen (an Elden Ring enemy of any
//! size, a party member, a Mimic copy's pawn, any future character), exactly as KH3's `SetFrozen` (0x1405B77B0) does on its
//! own pawns (kh-core [`super::badstat::set_frozen`] for Sora and every Sora core):
//!
//! - FROZEN: the ICE row of `ead_com_abn0` (group 0xC1C, sheet kh3-freeze-look `look.ice_group`): `vs_abn_frz0` (the frost
//!   decal and the cold smoke spawned on the victim's own mesh, `SQEX_VertexSpawn` bUseAttachMeshVertex) and `vs_abn_frz_in`
//!   (`SQEX_Material_Scalar` "Ice" 0 -> 1 over 0.3 s on the victim's character materials: the ice encasing), on
//!   `vfx_abn_root`, kept (persistent) for exactly as long as the victim is frozen;
//! - THE BREAK, the step the freeze ends (thawed, broken by a hit, shattered, killed while frozen): the ice row stopped (KH3's
//!   `set_effect(None)`, a fade) and the ICE_BREAK row (group 0xC1F, `look.break_group`) played once: `vs_abn_frz3` (smoke,
//!   the ice shards `fmd_stn_c00..04` in `smt_flz_ice_dst_diz_lit_00o`, the melt water; its SE se00002_116) and
//!   `vs_abn_frz_out0` ("Ice" 1 -> 0 over 0.1 s).
//!
//! The host says, once a step, WHO is frozen ([`Frozen`]); this route decides the starts and the breaks. Sized to each body:
//! the rows are authored on Sora's 89 cm half-height capsule, a victim's look is scaled by its own half height
//! ([`scale_for`]); its shards and smoke spawn on its own bones (the hosts hand the effect side the victim's bones: kh-host
//! `EffectHost::set_target_bones`, the party members' taps). A victim that is gone (despawned, unloaded) is [`FreezeLook::forget`]
//! -ed: its look ends at once, no break. Sora's own look stays kh-core's (`badstat::set_frozen`, the same rows).

use glam::Vec3;

use super::badstat::effect;
use crate::host::{Attach, EffectHandle, EffectId, EffectSpawn, Effects, PartyMember, TargetId};

include!(concat!(env!("KH_GEN_DIR"), "/kh3_freeze_look.rs"));

/// The rows of sheet kh3-freeze-look.
pub const P: Kh3FreezeLookParams = KH3_FREEZE_LOOK_PARAMS;

/// Whoever is frozen: an enemy (a target of the host) or a party member (his own skeleton, `Attach::Party`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Victim {
    Target(TargetId),
    Member(PartyMember),
}

/// One victim frozen this step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frozen {
    pub victim: Victim,
    /// A target: the world offset from its centre (the host's target anchor) down to its feet (`vfx_abn_root` is the root).
    /// A member: unused (his own `vfx_abn_root` bone).
    pub feet: Vec3,
    /// The look's size ([`scale_for`] of its half height; 1 = Sora's size).
    pub scale: f32,
}

impl Frozen {
    /// An enemy frozen: its centre and feet and its half height as the host knows them.
    pub fn target(id: TargetId, center: Vec3, feet: Vec3, half_height: f32) -> Frozen {
        Frozen { victim: Victim::Target(id), feet: feet - center, scale: scale_for(half_height) }
    }

    /// A party member frozen (any playable character, copies included): KH3's rows on his own `vfx_abn_root`.
    pub fn member(member: PartyMember) -> Frozen {
        Frozen { victim: Victim::Member(member), feet: Vec3::ZERO, scale: 1.0 }
    }
}

/// THE SIZE RULE: the victim's half height over the half height the rows were authored on (Sora's), within the sheet's clamp.
pub fn scale_for(half_height: f32) -> f32 {
    if !half_height.is_finite() || half_height <= 0.0 {
        return 1.0;
    }
    (half_height / P.size_authored_half_height).clamp(P.size_scale_min, P.size_scale_max)
}

/// THE SIZE OF A BODY READ FROM ITS OWN BONES, as a half height for [`scale_for`]: the larger of its capsule's half height, half
/// its bones' height and its bones' longest horizontal extent x `size.wide_share` (a winged or long body is bigger than its
/// capsule says).
pub fn half_height_of(capsule_half_height: f32, bones: &[Vec3]) -> f32 {
    let Some(first) = bones.first() else { return capsule_half_height };
    let (lo, hi) = bones.iter().fold((*first, *first), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
    let d = hi - lo;
    capsule_half_height.max(0.5 * d.z).max(d.x.max(d.y) * P.size_wide_share)
}

/// A BODY'S CAPSULE AS ITS BONES (KH3 space, cm), for a body whose own bones cannot be read: `capsule.rings` rings from its
/// feet to its top, `capsule.ring_points` points each on its surface (its hemispheres narrowing toward the ends).
pub fn capsule_bones(center: Vec3, radius: f32, half_height: f32) -> Vec<(String, Vec3)> {
    let (rings, per) = (P.capsule_rings.max(1) as usize, P.capsule_ring_points.max(1) as usize);
    let bottom = center.z - half_height;
    let straight = (half_height - radius).max(0.0);
    let mut out = Vec::with_capacity(rings * per);
    for i in 0..rings {
        let z = bottom + (i as f32 + 0.5) / rings as f32 * 2.0 * half_height;
        let from_mid = (z - center.z).abs();
        let r = if from_mid <= straight { radius } else { (radius * radius - (from_mid - straight).powi(2)).max(0.0).sqrt() };
        for k in 0..per {
            let a = (k as f32 + 0.5 * (i % 2) as f32) / per as f32 * std::f32::consts::TAU;
            out.push((format!("capsule_{i}_{k}"), Vec3::new(center.x + r * a.cos(), center.y + r * a.sin(), z)));
        }
    }
    out
}

/// (FREEZE-FX ice shell) THE ENCASING'S AMOUNT, as KH3 ramps its characters' "Ice": `vs_abn_frz_in`'s curve 0 -> 1 over
/// `shell.ice_in_s` from the freeze's start (its keys' flat ends: a smooth step), then held; after the break `vs_abn_frz_out0`
/// takes it from where it was to 0 over `shell.ice_out_s`. `frozen_for`: seconds since the freeze began; `broke_for`:
/// seconds since it ended (None: still frozen).
pub fn shell_ice(frozen_for: f32, broke_for: Option<f32>) -> f32 {
    let smooth = |x: f32| {
        let x = x.clamp(0.0, 1.0);
        x * x * (3.0 - 2.0 * x)
    };
    let held = smooth(frozen_for / P.shell_ice_in_s.max(1e-3));
    match broke_for {
        None => held,
        Some(t) => held * (1.0 - smooth(t / P.shell_ice_out_s.max(1e-3))),
    }
}

/// What changed this step (for the hosts' logs and tests).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Change {
    /// The ice row started on it.
    Froze(Victim),
    /// Its freeze ended: the ice row stopped, the break row played.
    Broke(Victim),
}

/// The effect names of a row (the pack must have each: the hosts' preload lists).
pub fn row_names(row: u32) -> Vec<&'static str> {
    effect::particles(row).iter().map(|p| p.0).collect()
}

/// Every effect this route spawns.
pub fn effect_names() -> Vec<&'static str> {
    let mut v = row_names(P.look_ice_group as u32);
    v.extend(row_names(P.look_break_group as u32));
    v
}

#[derive(Clone, Debug, PartialEq)]
struct Live {
    frozen: Frozen,
    handles: Vec<EffectHandle>,
    /// Seconds it has been frozen.
    seconds: f32,
}

/// The frozen looks one host keeps (one per victim).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FreezeLook {
    live: Vec<Live>,
}

fn spawn_row(row: u32, f: &Frozen, persistent: bool, handle: &mut dyn FnMut() -> EffectHandle, fx: &mut dyn Effects) -> Vec<EffectHandle> {
    let mut out = Vec::new();
    for &(name, bone, offset) in effect::particles(row) {
        let at = match f.victim {
            Victim::Target(target) => Attach::Target { target, offset: f.feet + Vec3::from(offset) * f.scale },
            Victim::Member(member) => Attach::Party { member, bone: Some(bone), offset: Vec3::from(offset) },
        };
        let h = handle();
        fx.spawn(h, &EffectSpawn { id: EffectId(row), name, at, rotation: Vec3::ZERO, scale: f.scale, persistent });
        out.push(h);
    }
    out
}

impl FreezeLook {
    /// ONCE A STEP: `frozen` = every victim frozen now (`dt` the step). A victim new to the list gets the ice row; one that
    /// left it has its ice stopped and the break played. `handle` gives the host's own effect handles.
    pub fn step(&mut self, frozen: &[Frozen], dt: f32, handle: &mut dyn FnMut() -> EffectHandle, fx: &mut dyn Effects) -> Vec<Change> {
        let mut changes = Vec::new();
        let mut keep = Vec::with_capacity(self.live.len());
        for mut l in self.live.drain(..) {
            match frozen.iter().find(|f| f.victim == l.frozen.victim) {
                Some(f) => {
                    // (its size and feet followed: a body standing up, a boss whose capsule the host re-reads)
                    l.frozen = *f;
                    l.seconds += dt.max(0.0);
                    keep.push(l);
                }
                None => {
                    for h in &l.handles {
                        fx.stop(*h, false);
                    }
                    let _ = spawn_row(P.look_break_group as u32, &l.frozen, false, handle, fx);
                    changes.push(Change::Broke(l.frozen.victim));
                }
            }
        }
        self.live = keep;
        for f in frozen {
            if self.live.iter().any(|l| l.frozen.victim == f.victim) {
                continue;
            }
            let handles = spawn_row(P.look_ice_group as u32, f, true, handle, fx);
            self.live.push(Live { frozen: *f, handles, seconds: 0.0 });
            changes.push(Change::Froze(f.victim));
        }
        changes
    }

    /// A victim that is gone (despawned, unloaded, out of the host's list): its look ends at once, no break.
    pub fn forget(&mut self, victim: Victim, fx: &mut dyn Effects) {
        self.live.retain(|l| {
            if l.frozen.victim != victim {
                return true;
            }
            for h in &l.handles {
                fx.stop(*h, true);
            }
            false
        });
    }

    /// Every look ends at once, no break (the host stops: a load, the party off).
    pub fn clear(&mut self, fx: &mut dyn Effects) {
        for l in self.live.drain(..) {
            for h in &l.handles {
                fx.stop(*h, true);
            }
        }
    }

    /// The victims whose ice is on, with how long it has been.
    pub fn on(&self) -> impl Iterator<Item = (Victim, f32)> + '_ {
        self.live.iter().map(|l| (l.frozen.victim, l.seconds))
    }

    pub fn is_on(&self, victim: Victim) -> bool {
        self.live.iter().any(|l| l.frozen.victim == victim)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Rec {
        spawned: Vec<(EffectHandle, EffectSpawn)>,
        stopped: Vec<(EffectHandle, bool)>,
    }

    impl Effects for Rec {
        fn spawn(&mut self, handle: EffectHandle, spawn: &EffectSpawn) {
            self.spawned.push((handle, *spawn));
        }
        fn stop(&mut self, handle: EffectHandle, immediate: bool) {
            self.stopped.push((handle, immediate));
        }
    }

    fn names(r: &Rec) -> Vec<&'static str> {
        r.spawned.iter().map(|s| s.1.name).collect()
    }

    /// KH3's rows: the ice is frz0 + frz_in, the break frz3 + frz_out0, on vfx_abn_root.
    #[test]
    fn freeze_look_the_rows_are_kh3s_set_frozen_rows() {
        assert_eq!(effect_names(), vec!["vs_abn_frz0", "vs_abn_frz_in", "vs_abn_frz3", "vs_abn_frz_out0"]);
        assert_eq!((P.look_ice_group as u32, P.look_break_group as u32), (effect::ICE, effect::ICE_BREAK));
    }

    /// The look lasts exactly as long as the freeze (to the step), and the break plays once, the step it ends.
    #[test]
    fn freeze_look_lasts_exactly_the_freeze_and_breaks_at_its_end() {
        let dt = 1.0 / 60.0;
        let id = TargetId(7);
        let f = Frozen::target(id, Vec3::new(0.0, 0.0, 150.0), Vec3::ZERO, 150.0);
        let (mut look, mut fx, mut next) = (FreezeLook::default(), Rec::default(), 0u32);
        let mut handle = || {
            next += 1;
            EffectHandle(0x7C00_0000 + next)
        };
        let frozen_steps = 120; // a 2 s freeze
        let mut froze_at = None;
        let mut broke_at = None;
        for step in 0..200 {
            let list: Vec<Frozen> = if (10..10 + frozen_steps).contains(&step) { vec![f] } else { Vec::new() };
            for c in look.step(&list, dt, &mut handle, &mut fx) {
                match c {
                    Change::Froze(_) => froze_at = Some(step),
                    Change::Broke(_) => broke_at = Some(step),
                }
            }
            assert_eq!(look.is_on(Victim::Target(id)), (10..10 + frozen_steps).contains(&step), "on exactly while frozen (step {step})");
        }
        assert_eq!((froze_at, broke_at), (Some(10), Some(10 + frozen_steps)));
        assert_eq!(names(&fx), vec!["vs_abn_frz0", "vs_abn_frz_in", "vs_abn_frz3", "vs_abn_frz_out0"]);
        // the ice persistent, the break one-shot; the ice stopped (faded, as KH3's set_effect(None)) the step it broke
        assert!(fx.spawned[..2].iter().all(|s| s.1.persistent) && fx.spawned[2..].iter().all(|s| !s.1.persistent));
        assert_eq!(fx.stopped, vec![(fx.spawned[0].0, false), (fx.spawned[1].0, false)]);
    }

    /// Fitted to the body: a boss's look is bigger, a rat's smaller; on its feet (vfx_abn_root), not its centre.
    #[test]
    fn freeze_look_is_sized_to_each_body_and_sits_at_its_feet() {
        assert_eq!(scale_for(89.0), 1.0);
        assert!((scale_for(300.0) - 300.0 / 89.0).abs() < 1e-5);
        assert_eq!((scale_for(10.0), scale_for(5000.0), scale_for(f32::NAN)), (P.size_scale_min, P.size_scale_max, 1.0));
        // a body read from its bones: a 20 m wingspan, 4 m tall dragon on a 190 cm capsule is a 500 cm body; a man stays his capsule
        let dragon = [Vec3::new(0.0, -1000.0, 0.0), Vec3::new(0.0, 1000.0, 100.0), Vec3::new(300.0, 0.0, 400.0)];
        assert_eq!(half_height_of(190.0, &dragon), 2000.0 * P.size_wide_share);
        let man = [Vec3::new(0.0, -40.0, 0.0), Vec3::new(10.0, 40.0, 178.0)];
        assert_eq!((half_height_of(90.0, &man), half_height_of(90.0, &[])), (90.0, 90.0));
        let mut fx = Rec::default();
        let mut n = 0;
        let mut handle = || {
            n += 1;
            EffectHandle(n)
        };
        let boss = Frozen::target(TargetId(3), Vec3::new(100.0, 0.0, 400.0), Vec3::new(100.0, 0.0, 0.0), 400.0);
        let _ = FreezeLook::default().step(&[boss], 0.016, &mut handle, &mut fx);
        for (_, s) in &fx.spawned {
            assert_eq!(s.scale, scale_for(400.0));
            assert_eq!(s.at, Attach::Target { target: TargetId(3), offset: Vec3::new(0.0, 0.0, -400.0) });
        }
    }

    /// A body whose bones cannot be read gets points on ITS capsule: from its feet to its top, within its width, whatever its
    /// size (a rat, a knight, a dragon).
    #[test]
    fn freeze_look_capsule_bones_cover_each_body() {
        for (r, hh) in [(15.0, 20.0), (40.0, 90.0), (300.0, 600.0)] {
            let c = Vec3::new(100.0, -50.0, hh);
            let pts = capsule_bones(c, r, hh);
            assert_eq!(pts.len(), (P.capsule_rings * P.capsule_ring_points) as usize);
            let (lo, hi) = pts.iter().fold((f32::MAX, f32::MIN), |a, p| (a.0.min(p.1.z), a.1.max(p.1.z)));
            let ring = 2.0 * hh / P.capsule_rings as f32;
            assert!(lo > 0.0 && lo < ring && hi < 2.0 * hh && hi > 2.0 * hh - ring, "{r} {hh}: {lo} .. {hi}");
            assert!(pts.iter().all(|p| (Vec3::new(p.1.x, p.1.y, 0.0) - Vec3::new(100.0, -50.0, 0.0)).length() <= r + 1e-3));
        }
    }

    /// The ice shell's amount follows KH3's "Ice": 0 -> 1 over 0.3 s, held, 0 again 0.1 s after the break.
    #[test]
    fn freeze_look_the_shell_ramps_as_kh3s_ice() {
        assert_eq!((shell_ice(0.0, None), shell_ice(P.shell_ice_in_s, None), shell_ice(5.0, None)), (0.0, 1.0, 1.0));
        assert!((shell_ice(P.shell_ice_in_s * 0.5, None) - 0.5).abs() < 1e-5);
        assert_eq!((shell_ice(5.0, Some(0.0)), shell_ice(5.0, Some(P.shell_ice_out_s))), (1.0, 0.0));
        // a freeze broken before the ice was full breaks from where it was
        assert!(shell_ice(0.1, Some(0.0)) < 0.5);
    }

    /// A party member's look is on his own vfx_abn_root (his materials take the "Ice" drive: his own body encased).
    #[test]
    fn freeze_look_a_member_wears_it_on_his_own_root() {
        let mut fx = Rec::default();
        let mut n = 0;
        let mut handle = || {
            n += 1;
            EffectHandle(n)
        };
        let mut look = FreezeLook::default();
        let _ = look.step(&[Frozen::member(PartyMember(1))], 0.016, &mut handle, &mut fx);
        assert!(fx.spawned.iter().all(|s| s.1.at == Attach::Party { member: PartyMember(1), bone: Some("vfx_abn_root"), offset: Vec3::ZERO }));
        // gone: ends at once, no break
        look.forget(Victim::Member(PartyMember(1)), &mut fx);
        assert_eq!(fx.spawned.len(), 2);
        assert!(fx.stopped.iter().all(|s| s.1));
    }
}
