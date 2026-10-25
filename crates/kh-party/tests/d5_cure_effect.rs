//! (party D5) Donald's Cure plays the cure-target effect on every pawn it heals, as Recover 0x1405B57D0 ->
//! 0x1405B8C30(+0x440, 4, kind) does: kinds 14 (the target) and 15 (the others within 300 cm) -> group 0x58F = 1423
//! of ead_com_abn0 = vm_cur_bds1 at vfx_abn_center, whose effect-attached SE (se00301_040) rides the effect seam.
//! Members get it from `Party::step` (their own seam handles); Sora's heal leaves it to the host (`SoraHeal::effect`
//! -> `Party::heal_effect_on_sora`).

use glam::Vec3;
use kh_core::host::{Attach, EffectHandle, EffectSpawn, Effects, NullTargets};
use kh_core::test_world::TestWorld;
use kh_party::{Chara, CommandStatus, MemberCommand, Party, PartyConfig, PartyHost, SoraView, TargetRef};

const DT: f32 = 1.0 / 60.0;

#[derive(Default)]
struct Rec(Vec<(EffectHandle, String, Attach, u32)>);

impl Effects for Rec {
    fn spawn(&mut self, handle: EffectHandle, spawn: &EffectSpawn) {
        self.0.push((handle, spawn.name.to_string(), spawn.at, spawn.id.0));
    }
    fn stop(&mut self, _handle: EffectHandle, _immediate: bool) {}
}

#[test]
fn d5_donalds_cure_plays_vm_cur_bds1_on_each_pawn_it_heals() {
    assert_eq!(kh_party::actions::recover_effect(14), Some(("vm_cur_bds1", "vfx_abn_center", 1423)));
    assert_eq!(kh_party::actions::recover_effect(15), kh_party::actions::recover_effect(14));
    assert_eq!(kh_party::actions::recover_effect(1), None);
    let world = TestWorld::floor();
    let (mut targets, mut audio, mut effects) = (NullTargets, kh_core::host::audio::NullAudio, Rec::default());
    let mut party = Party::new(PartyConfig::default());
    party.don_prog.kh3_ai = true; // (DON-PROG) KH3's Cure machinery (the overhaul's Cure line replaces it in play)
    let d = party.spawn(Chara::Donald, Vec3::new(0.0, 180.0, 0.0), 0.0);
    let g = party.spawn(Chara::Goofy, Vec3::new(0.0, -180.0, 0.0), 0.0);
    // (DON-PROG) KH3's Cure machinery: his KH3 DONALD_CURE (gone in play since the overhaul)
    party.status_mut(d).unwrap().abilities.set(kh_core::enums::ETresAbilityKind::DonaldCure, 1, 1);
    let sora = SoraView::default();
    let mut heals = Vec::new();
    let mut step = |party: &mut Party, effects: &mut Rec, heals: &mut Vec<kh_party::SoraHeal>| {
        let mut host = PartyHost { world: &world, targets: &mut targets, audio: &mut audio, effects };
        let out = party.step(DT, &sora, &mut host);
        heals.extend(out.sora_heals.iter().copied());
    };
    for _ in 0..2 {
        step(&mut party, &mut effects, &mut heals);
    }
    party.status_mut(g).unwrap().hp = 10;
    party.status_mut(d).unwrap().hp = 10;
    let t = party.command(d, MemberCommand::Cure { target: TargetRef::Sora });
    for _ in 0..(5.0 / DT) as usize {
        step(&mut party, &mut effects, &mut heals);
        if party.command_status(d, t) != CommandStatus::Running {
            break;
        }
    }
    assert_eq!(party.command_status(d, t), CommandStatus::Succeeded);
    let cure: Vec<_> = effects.0.iter().filter(|e| e.1 == "vm_cur_bds1").collect();
    // Goofy and Donald (both 180 cm from Sora, within 300) get theirs from the party, at their vfx_abn_center
    let on = |m: kh_party::MemberId| cure.iter().any(|e| e.3 == 1423 && matches!(e.2, Attach::Party { member, bone: Some("vfx_abn_center"), .. } if member.0 == m.0));
    assert!(on(d) && on(g), "{cure:?}");
    assert_eq!(cure.len(), 2, "Sora's is the host's: {cure:?}");
    // Sora's heal carries kind 14: the host's call puts the effect on him with the healer's handles
    let h = heals.iter().find(|h| h.effect == 14).copied().expect("the heal on Sora, kind 14");
    party.heal_effect_on_sora(h.from, h.effect, &mut effects);
    let last = effects.0.last().unwrap();
    assert_eq!(last.1, "vm_cur_bds1");
    assert!(matches!(last.2, Attach::Sora { bone: Some("vfx_abn_center"), .. }));
    assert!(kh_core::host::party::is_party_handle(last.0 .0), "a party-range handle");
}
