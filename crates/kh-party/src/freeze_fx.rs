//! (FREEZE-FX, THE USER 2026-11-07: "make sure frozen effects are applied to anything that gets frozen by blizzard") KH3'S
//! FROZEN LOOK ON A PLAYABLE CHARACTER: every member pawn (Donald, Goofy, a friend leader, the Mimic boss's friend copies, any
//! future character: one pawn type) whose `BadStatFreeze` state runs wears KH3's ice row on his own `vfx_abn_root` for exactly
//! that long, and its break the step it ends, by the shared route (kh-core `combat::freeze_look`): KH3's `SetFrozen`
//! (0x1405B77B0) of the freeze state's Begin / End (`TresNpcState_BadStatFreeze` Begin 0x1404330F0, the pawn's +0x868 bit 1),
//! the same rows kh-core puts on Sora. `vs_abn_frz_in`'s "Ice" drive reaches HIS OWN materials (the effect side's drives,
//! owner = member + 1): his body encased in ice, his copy's too.

use kh_core::combat::freeze_look::Frozen;
use kh_core::host::PartyMember;

use crate::host::PartyHost;
use crate::pawn::NpcPawn;
use crate::states::NpcStateId;

/// Is the member frozen this step (his freeze state runs; KO / death ends it)?
pub fn frozen(p: &NpcPawn) -> bool {
    p.state == NpcStateId::BadStatFreeze
}

/// Once a step, right after his own step (Party::step, with the host): the ice on while he is frozen, the break when it ends.
pub(crate) fn flush(p: &mut NpcPawn, host: &mut PartyHost, dt: f32) {
    let list = if frozen(p) { vec![Frozen::member(PartyMember(p.id.0))] } else { Vec::new() };
    if list.is_empty() && p.freeze_look.on().next().is_none() {
        return;
    }
    let seam = &mut p.seam;
    let _ = p.freeze_look.step(&list, dt, &mut || seam.effect(), host.effects);
}

#[cfg(test)]
mod tests {
    use glam::Vec3;
    use kh_core::enums::ETresDamageKind as Kind;
    use kh_core::host::{Attach, EffectHandle, EffectSpawn, HitRecord, NullAudio, NullTargets, TargetId};
    use kh_core::test_world::TestWorld;

    use crate::host::PartyHost;
    use crate::ids::Chara;
    use crate::states::NpcStateId;
    use crate::{Party, PartyConfig, SoraView};

    const DT: f32 = 1.0 / 60.0;

    #[derive(Default)]
    struct Fx {
        spawned: Vec<(usize, EffectHandle, EffectSpawn)>,
        stopped: Vec<(usize, EffectHandle)>,
    }
    impl kh_core::host::Effects for Fx {
        fn spawn(&mut self, h: EffectHandle, s: &EffectSpawn) {
            self.spawned.push((usize::MAX, h, *s));
        }
        fn stop(&mut self, h: EffectHandle, _immediate: bool) {
            self.stopped.push((usize::MAX, h));
        }
    }

    /// A member hit by a FREEZE row (MB3-FREEZE's 2 s, the copies' Blizzard on the party): KH3's ice on his own root for
    /// exactly as long as his freeze state runs, the break once, the step it ends. Every playable character, the same.
    #[test]
    fn freeze_fx_a_member_wears_kh3s_ice_for_exactly_his_freeze() {
        for chara in [Chara::Donald, Chara::Goofy] {
            let world = TestWorld::floor();
            let (mut targets, mut audio, mut fx) = (NullTargets, NullAudio, Fx::default());
            let mut party = Party::new(PartyConfig::test_level99());
            let id = party.spawn(chara, Vec3::ZERO, 0.0);
            let sora = SoraView { position: Vec3::new(-3000.0, 0.0, 0.0), ..SoraView::default() };
            let mut step = |party: &mut Party, fx: &mut Fx, k: usize| {
                let mut host = PartyHost { world: &world, targets: &mut targets, audio: &mut audio, effects: fx };
                let _ = party.step(DT, &sora, &mut host);
                for s in fx.spawned.iter_mut().filter(|s| s.0 == usize::MAX) {
                    s.0 = k;
                }
                for s in fx.stopped.iter_mut().filter(|s| s.0 == usize::MAX) {
                    s.0 = k;
                }
            };
            for k in 0..20 {
                step(&mut party, &mut fx, k);
            }
            fx.spawned.clear();
            let mut h = HitRecord::empty(TargetId(7));
            (h.damage, h.damage_kind, h.position, h.direction, h.attack_name) = (1.0, Kind::Small, Vec3::Z * 90.0, Vec3::new(-1.0, 0.0, 0.0), "test.blizzard");
            (h.info.bad_status, h.info.bad_time) = ("FREEZE", 2.0);
            let _ = party.receive_hit(id, &h);
            let mut frozen_steps = Vec::new();
            for k in 20..20 + 600 {
                step(&mut party, &mut fx, k);
                if party.member(id).unwrap().state == NpcStateId::BadStatFreeze {
                    frozen_steps.push(k);
                }
            }
            let (first, last) = (*frozen_steps.first().expect("he froze"), *frozen_steps.last().unwrap());
            assert_eq!(frozen_steps.len(), last - first + 1, "{chara:?}: one freeze");
            assert!(((frozen_steps.len() as f32 * DT) - 2.0).abs() <= 2.0 * DT, "{chara:?}: the freeze is the row's 2 s ({} steps)", frozen_steps.len());
            let ice: Vec<_> = fx.spawned.iter().filter(|s| s.2.name.starts_with("vs_abn_frz")).collect();
            let names: Vec<&str> = ice.iter().map(|s| s.2.name).collect();
            assert_eq!(names, vec!["vs_abn_frz0", "vs_abn_frz_in", "vs_abn_frz3", "vs_abn_frz_out0"], "{chara:?}");
            // the ice from his first frozen step, persistent, on his own root; the break the step after his last
            assert!(ice[..2].iter().all(|s| s.0 == first && s.2.persistent), "{chara:?}: {:?}", ice.iter().map(|s| s.0).collect::<Vec<_>>());
            assert!(ice[2..].iter().all(|s| s.0 == last + 1 && !s.2.persistent));
            assert!(ice.iter().all(|s| s.2.at == Attach::Party { member: kh_core::host::PartyMember(id.0), bone: Some("vfx_abn_root"), offset: Vec3::ZERO }));
            let stopped: Vec<_> = fx.stopped.iter().filter(|s| s.1 == ice[0].1 || s.1 == ice[1].1).collect();
            assert!(stopped.len() == 2 && stopped.iter().all(|s| s.0 == last + 1), "{chara:?}: the ice ends the step the freeze does");
        }
    }
}
