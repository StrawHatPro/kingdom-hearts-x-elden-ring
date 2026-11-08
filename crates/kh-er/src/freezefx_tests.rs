//! (FREEZE-FX, the user 2026-11-07: "make sure frozen effects are applied to anything that gets frozen by blizzard") Elden Ring's
//! bodies frozen by any KH3 FREEZE row wear KH3's frozen look for exactly the freeze, the break at its end; generic for every
//! body (small, large, boss-sized), no table of enemies.

use glam::Vec3;
use kh_core::combat::freeze_look::{scale_for, Change, FreezeLook, Frozen, Victim};
use kh_core::enums::ETresDamageKind as Kind;
use kh_core::host::{EffectHandle, EffectSpawn, Effects};
use kh_core::{HitRecord, TargetId};
use kh_react::{Body, BodyClass, EnemyReaction, FlatWorld, Phase, Pose, ReactionRequest};

use crate::ermap::{Params, Weight};
use crate::puppet::Puppets;
use crate::targets::Enemy;

fn freeze_hit(e: &Enemy, seconds: f32) -> HitRecord {
    let mut h = HitRecord { damage: 30.0, damage_kind: Kind::Small, direction: Vec3::X, ..HitRecord::empty(e.id) };
    h.position = e.pos + Vec3::Z * 50.0;
    (h.info.bad_status, h.info.bad_time) = ("FREEZE", seconds);
    h
}

/// A FREEZE row's hit on an Elden Ring body (any size the puppet throws about) puts it on the frozen look's list; a plain hit
/// does not.
#[test]
fn freezefx_a_freeze_hit_puts_any_body_on_the_frozen_list() {
    let params = Params::default();
    for (k, (r, hh, w)) in [(20.0, 40.0, Weight::Small), (45.0, 95.0, Weight::Medium), (120.0, 320.0, Weight::Large)].into_iter().enumerate() {
        let e = Enemy::stub(2 + k as u32, Vec3::new(300.0, 0.0, 0.0), r, hh, 2500, w);
        let mut p = Puppets::default();
        let _ = p.hit(&params, &e, &HitRecord { damage: 10.0, damage_kind: Kind::Small, direction: Vec3::X, ..HitRecord::empty(e.id) }, true, false);
        assert!(!p.frozen().contains(&e.id), "{w:?}: a plain hit freezes nothing");
        let o = p.hit(&params, &e, &freeze_hit(&e, 2.0), true, false);
        assert!(p.frozen().contains(&e.id), "{w:?}: frozen by the FREEZE row ({o:?})");
    }
}

#[derive(Default)]
struct Rec {
    spawned: Vec<(usize, EffectSpawn)>,
    stopped: Vec<(usize, EffectHandle)>,
    k: usize,
}

impl Effects for Rec {
    fn spawn(&mut self, _h: EffectHandle, s: &EffectSpawn) {
        self.spawned.push((self.k, *s));
    }
    fn stop(&mut self, h: EffectHandle, _immediate: bool) {
        self.stopped.push((self.k, h));
    }
}

/// THE LENGTH: KH3's Hold keeps a body Frozen for the row's time (kh-react); the look, fed the frozen list every step, is on
/// for exactly those steps and breaks the step it ends. Small, large and boss-sized bodies: the same, sized to each.
#[test]
fn freezefx_the_look_lasts_exactly_the_freeze_on_every_size() {
    const DT: f32 = 1.0 / 60.0;
    for (class, hh) in [(BodyClass::Light, 40.0), (BodyClass::Medium, 95.0), (BodyClass::Giant, 600.0)] {
        let mut react = EnemyReaction::new(Body::of(class));
        let world = FlatWorld::default();
        let mut req = ReactionRequest::new(Kind::Small, Vec3::X);
        (req.bad_status, req.bad_time) = (kh_react::BadStatus::Freeze, 3.0);
        let _ = react.hit(&req, Pose { position: Vec3::ZERO, yaw: 0.0, grounded: true });
        let (mut look, mut fx, mut n) = (FreezeLook::default(), Rec::default(), 0u32);
        let mut handle = || {
            n += 1;
            EffectHandle(n)
        };
        let id = TargetId(9);
        let (mut frozen_steps, mut broke) = (0usize, None);
        for k in 0..400 {
            fx.k = k;
            let frozen = react.is_active() && react.phase() == Phase::Frozen;
            frozen_steps += usize::from(frozen);
            let list: Vec<Frozen> = if frozen { vec![Frozen::target(id, Vec3::Z * hh, Vec3::ZERO, hh)] } else { Vec::new() };
            for c in look.step(&list, DT, &mut handle, &mut fx) {
                if let Change::Broke(Victim::Target(t)) = c {
                    assert_eq!(t, id);
                    broke = Some(k);
                }
            }
            let _ = react.step_dt(&world, DT);
        }
        assert!(((frozen_steps as f32 * DT) - 3.0).abs() <= 2.0 * DT, "{class:?}: KH3's hold is the row's 3 s ({frozen_steps} steps)");
        assert_eq!(broke, Some(frozen_steps), "{class:?}: the break the step after the last frozen one");
        let ice: Vec<_> = fx.spawned.iter().filter(|s| s.1.persistent).collect();
        assert_eq!(ice.iter().map(|s| (s.0, s.1.name)).collect::<Vec<_>>(), vec![(0, "vs_abn_frz0"), (0, "vs_abn_frz_in")]);
        assert!(ice.iter().all(|s| s.1.scale == scale_for(hh)), "{class:?}: sized to its body");
        let brk: Vec<_> = fx.spawned.iter().filter(|s| !s.1.persistent).map(|s| (s.0, s.1.name)).collect();
        assert_eq!(brk, vec![(frozen_steps, "vs_abn_frz3"), (frozen_steps, "vs_abn_frz_out0")]);
        assert!(fx.stopped.len() == 2 && fx.stopped.iter().all(|s| s.0 == frozen_steps), "{class:?}: the ice stopped as it breaks");
    }
}
