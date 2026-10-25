//! (DON-PROG) THE TESTS of the abilities' rest (description / AP / level), learning by level, the test loadout, the
//! KH3-standard wiring, the X3D kit's migration, the aggro rules, the auras and AI Donald's decisions (pure).

use super::abilities::DonAbilities;
use super::aggro::{self, AggroCtx, Cand, Stillness};
use super::ai::{self, AiAlly, AiAsk, AiChoice, AiEnemy, AiInput};
use super::attune::Slot;
use super::aura::{aura_of, PartyAura};
use super::cast::{CastTarget, SORA_MEMBER};
use super::*;
use crate::ids::{Chara, MemberId};
use crate::status::FriendStatus;
use glam::Vec3;
use kh_core::enums::ETresAbilityKind as A;

fn donald_at(level: u32) -> FriendStatus {
    let exp = crate::tables::PARTY_LEVELS.iter().find(|r| r.0 == Chara::Donald.uid() && r.1 == level).map(|r| r.2).unwrap();
    FriendStatus::new(Chara::Donald, exp)
}

fn abilities(s: &FriendStatus) -> DonAbilities {
    s.donald.as_ref().unwrap().abilities
}

// ------------------------------------------------------------------------------------------------ abilities (9, 10)

#[test]
fn every_ability_has_a_description_an_ap_and_a_source() {
    for k in DonAbility::ALL {
        assert!(k.description().len() > 20 && k.description().ends_with('.'), "{k:?}: {}", k.description());
        assert!(k.ap() >= 0, "{k:?}");
        assert!(k.level().is_some() || k == DonAbility::AeroFloat, "{k:?} has no level and no boss");
        assert!(super::ability_info::prog().row(&format!("ability.{}", k.key())).is_some(), "{k:?}: no sheet row");
    }
    assert_eq!(DonAbility::AeroFloat.level(), None, "the Red Wolf of Radagon's");
    assert_eq!(DonAbility::AeroFloat.ap(), 0);
    // KH3's AP where KH3 has the ability (Sora's sheet)
    let kh3 = [(DonAbility::Scan, 1), (DonAbility::MpHaste, 3), (DonAbility::MpHastera, 4), (DonAbility::MpHastega, 5), (DonAbility::SecondChance, 4), (DonAbility::WithstandCombo, 4), (DonAbility::LuckyLucky, 5), (DonAbility::TreasureMagnet, 1), (DonAbility::ItemBoost, 2), (DonAbility::ComboPlus, 4), (DonAbility::AirComboPlus, 4), (DonAbility::FireBoost, 4)];
    for (k, ap) in kh3 {
        assert_eq!(k.ap(), ap, "{k:?}");
    }
    assert_eq!(DonAbility::ComboPlus2.key(), "combo_plus_2");
}

#[test]
fn the_level_tree_is_the_docs() {
    let lv = |k: DonAbility| k.level().unwrap();
    // 10: the start, then the named levels
    assert_eq!(lv(DonAbility::Scan), 1, "Scan from the start");
    assert_eq!(lv(DonAbility::LowProfile), 10);
    assert_eq!(lv(DonAbility::HideBehind), 25);
    for k in [DonAbility::ComboPlus, DonAbility::SwiftAttune, DonAbility::Distraction, DonAbility::ElementalInsight] {
        assert!((30..=50).contains(&lv(k)), "{k:?} {}", lv(k));
    }
    assert!((38..=42).contains(&lv(DonAbility::Distraction)), "5.4: ~40");
    for k in [DonAbility::TauntPass, DonAbility::OutOfSight, DonAbility::SpellEcho] {
        assert!((50..=70).contains(&lv(k)), "{k:?} {}", lv(k));
    }
    assert!((53..=57).contains(&lv(DonAbility::TauntPass)), "5.4: ~55");
    assert!((68..=72).contains(&lv(DonAbility::OutOfSight)), "5.4: ~70");
    assert!(lv(DonAbility::InstantAttune) >= 70);
    assert!((78..=82).contains(&lv(DonAbility::DualAttunement)), "~80");
    // Swift before Instant, Combo Plus before Combo Plus 2, MP Haste < Hastera < Hastega, Scan before Elemental Insight
    assert!(lv(DonAbility::SwiftAttune) < lv(DonAbility::InstantAttune));
    assert!(lv(DonAbility::ComboPlus) < lv(DonAbility::ComboPlus2));
    assert!(lv(DonAbility::MpHaste) < lv(DonAbility::MpHastera) && lv(DonAbility::MpHastera) < lv(DonAbility::MpHastega));
    assert!(lv(DonAbility::Scan) < lv(DonAbility::ElementalInsight));
    // "as many as Sora's": 42 by level, spread over the levels (no level teaches two)
    let order = DonAbility::level_order();
    assert_eq!(order.len(), 42);
    let mut levels: Vec<u32> = order.iter().map(|k| lv(*k)).collect();
    levels.dedup();
    assert_eq!(levels.len(), 42, "one ability a level");
    assert!(order.windows(2).all(|w| lv(w[0]) < lv(w[1])));
    assert_eq!(DonAbility::learned_between(9, 10), vec![DonAbility::LowProfile]);
    assert!(DonAbility::learned_between(99, 99).is_empty());
}

#[test]
fn he_learns_by_level_in_level_order_and_a_late_join_wears_the_early_ones() {
    // THE START (doc 10): Scan, worn from the status's making (the enemy HP gauge while he leads)
    let mut s = donald_at(1);
    assert!(abilities(&s).has(DonAbility::Scan), "worn at once (1 AP of 14)");
    assert_eq!(abilities(&s).list(), vec![(DonAbility::Scan, true)]);
    assert!(s.don_sync_level().is_empty(), "once");
    // a level-up to 12: Quick Cast, MP Haste, Wand Bonk+, Low Profile, Fire Boost
    let to = donald_at(12);
    s.set_total_exp(to.total_exp);
    let got = s.don_sync_level();
    assert_eq!(got, vec![DonAbility::QuickCast, DonAbility::MpHaste, DonAbility::WandBonkPlus, DonAbility::LowProfile, DonAbility::FireBoost]);
    assert!(s.ap_in_use() <= s.max_ap());
    // A JOIN AT LEVEL 60: everything up to 60 owned; worn in level order while the AP lasts
    let mut j = donald_at(60);
    assert!(j.don_sync_level().is_empty(), "given at the status's making");
    let got: Vec<DonAbility> = DonAbility::level_order().into_iter().filter(|k| abilities(&j).owned(*k)).collect();
    assert_eq!(got, DonAbility::level_order().into_iter().filter(|k| k.level().unwrap() <= 60).collect::<Vec<_>>());
    let a = abilities(&j);
    assert!(j.ap_in_use() <= j.max_ap(), "{} > {}", j.ap_in_use(), j.max_ap());
    // the first ones are worn, and a worn one is never later in the order than one that fitted and was skipped first
    assert!(a.has(DonAbility::Scan) && a.has(DonAbility::QuickCast) && a.has(DonAbility::MpHaste) && a.has(DonAbility::LowProfile));
    let worn: Vec<DonAbility> = got.iter().copied().filter(|k| a.has(*k)).collect();
    let mut ap = 0;
    for k in &got {
        if ap + k.ap() <= j.max_ap() - (j.ap_in_use() - worn.iter().map(|w| w.ap()).sum::<i32>()) {
            ap += k.ap();
            assert!(a.has(*k), "{k:?} fitted in level order");
        }
    }
    assert!(!a.has(DonAbility::TauntPass) || a.has(DonAbility::HideBehind));
    // his KH3 spell abilities are gone (the overhaul replaced them); his tree's requests are gated by his state, with
    // KH3's m_UseMP: Fire 10, Cure all MP (any)
    let fire = A::from_game_name("DONALD_FIRE").unwrap();
    assert_eq!(j.abilities.owned(fire), 0);
    assert_eq!((j.don_tree_gate("DONALD_FIRE"), j.don_tree_gate("DONALD_CURE"), j.don_tree_gate("GOOFY_TURBO")), (Some(true), Some(true), None));
    assert_eq!(j.don_has_ability(fire), Some(true));
    let mut k = donald_at(1);
    k.mp = 9;
    assert_eq!((k.don_tree_gate("DONALD_FIRE"), k.don_tree_gate("DONALD_CURE")), (Some(false), Some(true)));
    k.mp = 0;
    k.mp_charge_left = 5.0;
    assert_eq!(k.don_tree_gate("DONALD_CURE"), Some(false), "recharging MP");
    // an old save's entries: dropped once, the AP unchanged (they cost none with the overhaul)
    let ap = k.ap_in_use();
    k.abilities.set(fire, 1, 1);
    assert_eq!((k.ap_in_use(), k.don_tree_gate("DONALD_FIRE")), (ap, None), "while held: KH3's own gate");
    assert_eq!(k.don_drop_kh3_spells(), vec!["DONALD_FIRE"]);
    assert!(k.don_drop_kh3_spells().is_empty());
    assert_eq!((k.abilities.owned(fire), k.ap_in_use()), (0, ap));
    // Goofy: none of this
    let mut g = FriendStatus::new(Chara::Goofy, 1_000_000);
    assert!(g.don_sync_level().is_empty() && g.donald.is_none());
}

#[test]
fn the_test_levels_loadout_is_everything_within_his_ap() {
    let s = FriendStatus::test_level99(Chara::Donald);
    let a = abilities(&s);
    assert!(DonAbility::ALL.iter().all(|k| a.owned(*k)), "every ability owned");
    assert!(s.ap_in_use() <= s.max_ap(), "{} > {}", s.ap_in_use(), s.max_ap());
    for k in [DonAbility::Scan, DonAbility::QuickCast, DonAbility::ComboPlus, DonAbility::LowProfile, DonAbility::HideBehind, DonAbility::Distraction, DonAbility::TauntPass, DonAbility::OutOfSight, DonAbility::InstantAttune, DonAbility::DualAttunement, DonAbility::AeroFloat] {
        assert!(a.has(k), "{k:?} worn in the test level");
    }
    assert_eq!(s.kit, Default::default(), "no X3D kit");
    assert!(s.max_ap() - s.ap_in_use() < 4, "the AP is used up: {} of {}", s.ap_in_use(), s.max_ap());
}

#[test]
fn the_kh3_standard_ones_are_kh3s_effects() {
    let mut s = donald_at(99);
    let d = s.donald.as_mut().unwrap();
    for k in [DonAbility::SecondChance, DonAbility::WithstandCombo, DonAbility::LuckyLucky, DonAbility::TreasureMagnet, DonAbility::ItemBoost, DonAbility::Scan, DonAbility::MpHaste, DonAbility::MpHastega, DonAbility::MpRage] {
        d.abilities.give(k);
        d.abilities.set_equipped(k, true);
    }
    assert_eq!((s.count(A::LuckUp), s.count(A::Libra), s.count(A::ItemUp), s.count(A::LastLeave)), (1, 1, 1, 1));
    assert_eq!(s.count(A::PrizeDraw), 2, "his KH3 start copy + Treasure Magnet");
    // unequipped: not counted
    s.donald.as_mut().unwrap().abilities.set_equipped(DonAbility::LuckyLucky, false);
    assert_eq!(s.count(A::LuckUp), 0);
    // Second Chance: 1 HP from 2 or more; Withstand Combo: also from 1 HP inside a combo
    assert_eq!(s.don_survive(10, 0, false), 1);
    assert_eq!(s.don_survive(1, 0, false), 0);
    assert_eq!(s.don_survive(1, 0, true), 1);
    assert_eq!(s.don_survive(10, 4, false), 4);
    // MP Haste (0.1) + Hastega (0.3) on his KH3 MP_HASTE? none here: 1.4
    assert!((s.don_mp_haste() - 1.4).abs() < 1e-5, "{}", s.don_mp_haste());
    // the charge runs 1.4 x as fast
    s.mp = 0;
    s.mp_charge_left = 14.0;
    s.tick(1.0);
    assert!((s.mp_charge_left - 12.6).abs() < 1e-4, "{}", s.mp_charge_left);
    // MP Rage: 0.5 MP per HP lost, not while charging
    assert_eq!(s.don_mp_rage(10), 0, "charging");
    s.mp_charge_left = 0.0;
    s.mp = 5;
    assert_eq!(s.don_mp_rage(10), 5);
    // Goofy keeps KH3's own rules (none of these)
    let g = FriendStatus::new(Chara::Goofy, 0);
    assert_eq!((g.don_mp_haste(), g.don_survive(5, 0, true)), (1.0, 0));
}

#[test]
fn the_x3d_kit_migrates_once() {
    use crate::pad::donald_kit::{KitAbilities, KitAbility};
    let mut s = donald_at(1);
    let mut kit = KitAbilities::default();
    for k in [KitAbility::SpellWeaver, KitAbility::CounterBlast, KitAbility::AeroFloat, KitAbility::UnisonFire] {
        kit.give(k);
    }
    kit.set_equipped(KitAbility::SpellWeaver, true);
    kit.set_equipped(KitAbility::CounterBlast, true);
    s.kit = kit;
    let got = s.kit_migrate();
    assert_eq!(got, vec![DonAbility::SpellWeaver, DonAbility::AeroFloat]);
    let a = abilities(&s);
    assert!(a.has(DonAbility::SpellWeaver), "its equipped bit carried over");
    assert!(a.owned(DonAbility::AeroFloat) && !a.equipped(DonAbility::AeroFloat));
    assert_eq!(s.kit, KitAbilities::default(), "the kit is empty: Counter Blast and the Unison dropped");
    assert!(s.kit_migrate().is_empty(), "once");
    // an old reward token: Aero Float through the shim
    let mut t = donald_at(5);
    assert_eq!(t.kit_grant(KitAbility::AeroFloat), (true, true));
    assert!(abilities(&t).has(DonAbility::AeroFloat));
    assert_eq!(t.kit_grant(KitAbility::BlinkStep), (false, false));
    assert!(t.kit_sync_level().is_empty() && KitAbility::OWN.is_empty());
}

// ------------------------------------------------------------------------------------------------ aggro (5.4)

fn with(list: &[DonAbility]) -> DonAbilities {
    let mut a = DonAbilities::default();
    for k in list {
        a.give(*k);
        a.set_equipped(*k, true);
    }
    a
}

fn party_ctx(nearest: f32) -> AggroCtx {
    AggroCtx { others: 1, nearest_ally: nearest, still: false, casting: false }
}

#[test]
fn aggro_numbers_and_never_alone() {
    let lp = with(&[DonAbility::LowProfile]);
    let both = with(&[DonAbility::LowProfile, DonAbility::HideBehind]);
    let hb = with(&[DonAbility::HideBehind]);
    // alone: nothing applies
    assert_eq!(aggro::pick_weight(&both, &AggroCtx::ALONE, 500.0), 1.0);
    assert!(!aggro::switch_on_target(&with(&[DonAbility::Distraction]), &AggroCtx::ALONE, 0.0));
    // Low Profile -20 %; Hide Behind -40 % within 8 m (both: x0.48); no ally near: Low Profile only
    assert!((aggro::pick_weight(&lp, &party_ctx(2000.0), 500.0) - 0.8).abs() < 1e-6);
    assert!((aggro::pick_weight(&hb, &party_ctx(799.0), 500.0) - 0.6).abs() < 1e-6);
    assert!((aggro::pick_weight(&both, &party_ctx(800.0), 500.0) - 0.48).abs() < 1e-6);
    assert!((aggro::pick_weight(&both, &party_ctx(801.0), 500.0) - 0.8).abs() < 1e-6);
    assert_eq!(aggro::pick_weight(&DonAbilities::default(), &party_ctx(100.0), 500.0), 1.0, "without the abilities");
    // Distraction 30 %
    let dis = with(&[DonAbility::Distraction]);
    assert!(aggro::switch_on_target(&dis, &party_ctx(900.0), 0.29) && !aggro::switch_on_target(&dis, &party_ctx(900.0), 0.31));
    // Taunt Pass: enemies within 8 m of him
    let tp = with(&[DonAbility::TauntPass]);
    assert!(aggro::taunt_pass(&tp, &party_ctx(500.0), 799.0) && !aggro::taunt_pass(&tp, &party_ctx(500.0), 801.0));
    assert!(!aggro::taunt_pass(&tp, &AggroCtx::ALONE, 100.0));
    // Out of Sight: still AND casting AND over 15 m
    let oos = with(&[DonAbility::OutOfSight, DonAbility::LowProfile]);
    let quiet = AggroCtx { still: true, casting: true, ..party_ctx(500.0) };
    assert_eq!(aggro::pick_weight(&oos, &quiet, 1501.0), 0.0);
    assert!((aggro::pick_weight(&oos, &quiet, 1499.0) - 0.8).abs() < 1e-6, "inside 15 m: the other rules");
    assert!(aggro::pick_weight(&oos, &AggroCtx { casting: false, ..quiet }, 3000.0) > 0.0, "not casting");
    assert!(aggro::pick_weight(&oos, &AggroCtx { still: false, ..quiet }, 3000.0) > 0.0, "moving");
}

#[test]
fn the_choice_keeps_him_by_his_weight_and_distraction_moves_it_to_his_nearest_ally() {
    let a = with(&[DonAbility::LowProfile, DonAbility::Distraction]);
    let ctx = party_ctx(300.0);
    // Donald nearest (0), Goofy (1), Sora (2, nearest to Donald)
    let c = [Cand { dist: 200.0, donald: true, to_donald: 0.0 }, Cand { dist: 400.0, donald: false, to_donald: 600.0 }, Cand { dist: 900.0, donald: false, to_donald: 300.0 }];
    assert_eq!(aggro::choose(&c, &a, &ctx, 0.85, 0.9), Some(1), "keep roll above 0.8: the next nearest");
    assert_eq!(aggro::choose(&c, &a, &ctx, 0.5, 0.9), Some(0), "kept, no distraction");
    assert_eq!(aggro::choose(&c, &a, &ctx, 0.5, 0.1), Some(2), "Distraction: his nearest ally");
    assert_eq!(aggro::choose(&c, &a, &AggroCtx::ALONE, 0.99, 0.0), Some(0), "alone: the host's own rule");
    // over many rolls the share of picks on him is his weight x (1 - 0.3)
    let mut on_him = 0;
    let n = 20_000u64;
    for k in 0..n {
        if aggro::choose(&c, &a, &ctx, aggro::roll(k, 1, 7), aggro::roll(k, 2, 9)) == Some(0) {
            on_him += 1;
        }
    }
    let share = on_him as f32 / n as f32;
    assert!((share - 0.8 * 0.7).abs() < 0.015, "{share}");
    // nobody else: still him (an enemy is never sent nowhere)
    assert_eq!(aggro::choose(&c[..1], &a, &ctx, 0.99, 0.0), Some(0));
}

#[test]
fn out_of_sight_clocks_are_frame_rate_independent() {
    for dt in [1.0 / 30.0, 1.0 / 144.0] {
        let mut st = Stillness::new();
        assert!(!st.casting(), "no cast yet");
        st.tick(300.0, true, dt);
        let mut t = dt;
        while t < 0.49 {
            st.tick(10.0, false, dt);
            t += dt;
        }
        assert!(!st.still() && st.casting(), "{dt}: below 50 cm/s for under 0.5 s");
        while t < 0.53 {
            st.tick(10.0, false, dt);
            t += dt;
        }
        assert!(st.still(), "{dt}");
        while t < 1.6 {
            st.tick(0.0, false, dt);
            t += dt;
        }
        assert!(!st.casting(), "the cast is 1.5 s old: {dt}");
        st.tick(300.0, false, dt);
        assert!(!st.still());
    }
}

// ------------------------------------------------------------------------------------------------ auras (4.2)

fn attuned(list: &[(Element, f32)]) -> DonaldState {
    let mut d = DonaldState::new();
    d.learned = DonLearned::all();
    for (k, (e, res)) in list.iter().enumerate() {
        d.core.attune.slots[k] = Some(Slot { element: *e, res: *res, period: k as u32 + 1, ult_used: false, full_at: Some(0.0) });
    }
    d
}

#[test]
fn every_element_gives_its_aura_and_only_in_a_party() {
    let close = |a: f32, b: f32| (a - b).abs() < 1e-5;
    let a = aura_of(&attuned(&[(Element::Fire, 0.0)]));
    assert!(close(a.attack_mult, 1.10) && close(a.radius, 1000.0));
    assert!(close(aura_of(&attuned(&[(Element::Blizzard, 0.0)])).enemy_speed_mult, 0.85));
    assert!(close(aura_of(&attuned(&[(Element::Thunder, 0.0)])).move_mult, 1.10));
    assert!(close(aura_of(&attuned(&[(Element::Water, 0.0)])).mp_regen_mult, 1.25));
    let aero = aura_of(&attuned(&[(Element::Aero, 0.0)]));
    assert!(aero.evasion > 0.0 && aero.recovery_mult > 1.0);
    assert!(aura_of(&attuned(&[(Element::Light, 0.0)])).regen_pct_per_s > 0.0);
    assert!(aura_of(&DonaldState::new()).is_none(), "no attunement: no aura");
    // Dual: both
    let dual = aura_of(&attuned(&[(Element::Fire, 0.0), (Element::Water, 0.0)]));
    assert!(close(dual.attack_mult, 1.10) && close(dual.mp_regen_mult, 1.25));
    // Aura Range+
    let mut d = attuned(&[(Element::Fire, 0.0)]);
    d.abilities = with(&[DonAbility::AuraRangePlus]);
    assert!(close(aura_of(&d).radius, 1500.0));
    // the party's snapshot: alone none; in a party the members in reach
    let fire = attuned(&[(Element::Fire, 0.0), (Element::Blizzard, 0.0)]);
    let mut p = PartyAura::default();
    p.make(&[(MemberId(0), Vec3::ZERO, &fire)], 1);
    assert!(p.is_empty(), "alone");
    p.make(&[(MemberId(0), Vec3::ZERO, &fire)], 2);
    assert!(close(p.at(Vec3::new(900.0, 0.0, 0.0)).attack_mult, 1.10));
    assert!(close(p.at(Vec3::new(1100.0, 0.0, 0.0)).attack_mult, 1.0), "out of reach");
    assert!(close(p.at(Vec3::ZERO).attack_mult, 1.10), "Donald himself is in the party");
    assert!(close(p.enemy_speed(Vec3::new(500.0, 0.0, 0.0)), 0.85) && close(p.enemy_speed(Vec3::new(5000.0, 0.0, 0.0)), 1.0));
}

#[test]
fn water_speeds_the_mp_charge_and_light_regenerates_frame_rate_independently() {
    let water_light = attuned(&[(Element::Water, 0.0), (Element::Light, 0.0)]);
    for dt in [1.0 / 30.0, 1.0 / 144.0] {
        let mut p = PartyAura::default();
        p.make(&[(MemberId(0), Vec3::ZERO, &water_light)], 2);
        let mut goofy = FriendStatus::new(Chara::Goofy, 0);
        goofy.mp = 0;
        goofy.mp_charge_left = 10.0;
        let max = goofy.max_hp();
        goofy.hp = 1;
        let mut t = 0.0;
        while t < 4.0 - 1e-4 {
            p.apply_friends(std::iter::once((MemberId(1), Vec3::new(300.0, 0.0, 0.0), &mut goofy)), dt);
            t += dt;
        }
        // the extra 25 % of 4 s (the status's own tick is not run here)
        assert!((goofy.mp_charge_left - 9.0).abs() < 0.01, "{dt}: {}", goofy.mp_charge_left);
        // 0.5 % of max HP a second for 4 s
        let want = 1 + (max as f32 * 0.005 * 4.0).floor() as i32;
        assert!((goofy.hp - want).abs() <= 1, "{dt}: {} vs {want}", goofy.hp);
    }
}

// ------------------------------------------------------------------------------------------------ AI Donald (11)

fn enemy(id: u32, x: f32) -> AiEnemy {
    AiEnemy { id, pos: Vec3::new(x, 0.0, 0.0), weak: ElemSet::NONE, resist: ElemSet::NONE, wet: false, chilled: false, frozen: false, burned: false, big: false }
}

fn ally(m: u8, x: f32, hp: i32) -> AiAlly {
    AiAlly { member: MemberId(m), pos: Vec3::new(x, 300.0, 0.0), hp, hp_max: 100, ko: false, melee: m == 1, attacked: false, barrier_left: 0.0, orbs: 0, thunder: false }
}

fn input<'a>(me: &'a DonaldState, enemies: &'a [AiEnemy], allies: &'a [AiAlly]) -> AiInput<'a> {
    AiInput { me, id: MemberId(0), pos: Vec3::ZERO, hp: 100, hp_max: 100, mp: 300, mp_max: 300, mp_charging: false, in_battle: true, barrier_left: 30.0, orbs: 3, enemies, allies, refused: &[] }
}

fn cast_of(c: Option<AiChoice>) -> Option<(SpellRef, CastTarget)> {
    match c {
        Some(AiChoice::Cast { plan, target }) => Some((plan.spell, target)),
        _ => None,
    }
}

#[test]
fn ai_cures_by_the_share_missing() {
    let me = attuned(&[(Element::Fire, 0.0)]);
    let allies = [ally(1, 900.0, 20), ally(2, 900.0, 90)];
    let i = input(&me, &[], &allies);
    // the MetaAI asked: Goofy at 20 %: the -ga (Curaza only at full-power Light)
    assert_eq!(cast_of(ai::decide(&i, AiAsk::Cure(MemberId(1)))), Some((SpellRef::Line1(Element::Light, 3), CastTarget::Member(MemberId(1)))));
    assert_eq!(cast_of(ai::decide(&i, AiAsk::Cure(MemberId(2)))), Some((SpellRef::Line1(Element::Light, 1), CastTarget::Member(MemberId(2)))));
    // full-power Light: Curaza
    let light = attuned(&[(Element::Light, 3.0)]);
    let i = input(&light, &[], &allies);
    assert_eq!(cast_of(ai::decide(&i, AiAsk::Cure(MemberId(1)))).map(|c| c.0), Some(SpellRef::Line1(Element::Light, 4)));
    // only Cure known: Cure
    let mut basic = attuned(&[(Element::Fire, 0.0)]);
    basic.learned.tier[Element::Light.index()] = 1;
    let i = input(&basic, &[], &allies);
    assert_eq!(cast_of(ai::decide(&i, AiAsk::Cure(MemberId(1)))).map(|c| c.0), Some(SpellRef::Line1(Element::Light, 1)));
    // the whole party low: Light first (then the Cure line attuned)
    let low = [ally(1, 900.0, 20), ally(2, 900.0, 20)];
    let i = AiInput { hp: 20, ..input(&me, &[], &low) };
    assert_eq!(ai::decide(&i, AiAsk::Cure(MemberId(1))), Some(AiChoice::Attune(Element::Light)));
    let light = attuned(&[(Element::Light, 1.0)]);
    let i = AiInput { hp: 20, ..input(&light, &[], &low) };
    assert!(matches!(cast_of(ai::decide(&i, AiAsk::Cure(MemberId(1)))), Some((SpellRef::Line1(Element::Light, _), _))));
    // a refused Curaga (he was busy) is not replaced by a weaker Cure: he waits for the next request
    let refused = [(SpellRef::Line1(Element::Light, 3), CastTarget::Member(MemberId(1)))];
    let i = AiInput { refused: &refused, ..input(&me, &[], &allies) };
    assert_eq!(ai::decide(&i, AiAsk::Cure(MemberId(1))), None);
    // no MP: nothing (the request fails; no KH3 Cure)
    let i = AiInput { mp: 0, ..input(&me, &[], &allies) };
    assert_eq!(ai::decide(&i, AiAsk::Cure(MemberId(1))), None);
}

#[test]
fn ai_aero_barrier_kh1_style_and_its_renewal() {
    let me = attuned(&[(Element::Fire, 0.0)]);
    let far = [enemy(9, 1500.0)];
    // a friend being attacked: the barrier on him
    let mut allies = [ally(1, 1400.0, 90), ally(2, 600.0, 90)];
    allies[0].attacked = true;
    let i = input(&me, &far, &allies);
    assert_eq!(cast_of(ai::decide(&i, AiAsk::Attack(Some(9)))), Some((SpellRef::Line2(Element::Aero, 3), CastTarget::Member(MemberId(1)))));
    // he wears one with 10 s left: not again
    allies[0].barrier_left = 10.0;
    let i = input(&me, &far, &allies);
    assert_ne!(cast_of(ai::decide(&i, AiAsk::Attack(Some(9)))), Some((SpellRef::Line2(Element::Aero, 3), CastTarget::Member(MemberId(1)))));
    // 3 s left in a fight: renewed
    allies[0].barrier_left = 3.0;
    let i = input(&me, &far, &allies);
    assert_eq!(cast_of(ai::decide(&i, AiAsk::Attack(Some(9)))).map(|c| c.1), Some(CastTarget::Member(MemberId(1))));
    // a low friend (not attacked)
    let low = [ally(2, 600.0, 30)];
    let i = input(&me, &far, &low);
    assert_eq!(cast_of(ai::decide(&i, AiAsk::Attack(Some(9)))).map(|c| c.1), Some(CastTarget::Member(MemberId(2))));
    // an enemy closes in on him: on himself
    let near = [enemy(9, 300.0)];
    let i = AiInput { barrier_left: 0.0, orbs: 0 /* (DON-SPELLB one worn) */, ..input(&me, &near, &[]) };
    assert_eq!(cast_of(ai::decide(&i, AiAsk::Attack(Some(9)))), Some((SpellRef::Line2(Element::Aero, 3), CastTarget::Member(MemberId(0)))));
    // no barrier learned: none of this
    let mut no = me.clone();
    no.learned.line2[Element::Aero.index()] = 0;
    let i = AiInput { barrier_left: 0.0, ..input(&no, &near, &[]) };
    assert_ne!(cast_of(ai::decide(&i, AiAsk::Attack(Some(9)))).map(|c| c.0), Some(SpellRef::Line2(Element::Aero, 3)));
}

#[test]
fn ai_ball_lightning_on_himself_when_cornered_else_on_the_melee_friend() {
    let me = attuned(&[(Element::Fire, 0.0)]);
    let two = [enemy(8, 300.0), enemy(9, 350.0)];
    let i = AiInput { orbs: 0, barrier_left: 0.0 /* (DON-SPELLB one worn: never over his barrier) */, ..input(&me, &two, &[]) };
    // (DON-SPELLB one worn) wearing a barrier, cornered: no Ball Lightning over it
    let worn = AiInput { orbs: 0, barrier_left: 30.0, ..input(&me, &two, &[]) };
    assert_ne!(cast_of(ai::decide(&worn, AiAsk::Attack(Some(9)))).map(|c| c.0), Some(SpellRef::Line2(Element::Thunder, 3)));
    assert_eq!(cast_of(ai::decide(&i, AiAsk::Attack(Some(9)))), Some((SpellRef::Line2(Element::Thunder, 3), CastTarget::Member(MemberId(0)))));
    // Goofy fighting up close, no orbs: on Goofy
    let goofy = [ally(1, 1500.0, 90)];
    let foes = [enemy(9, 1700.0)];
    let i = input(&me, &foes, &goofy);
    assert_eq!(cast_of(ai::decide(&i, AiAsk::Attack(Some(9)))), Some((SpellRef::Line2(Element::Thunder, 3), CastTarget::Member(MemberId(1)))));
    // Sora too (the leader in melee)
    let sora = [AiAlly { member: SORA_MEMBER, melee: true, ..ally(3, 1500.0, 90) }];
    let i = input(&me, &foes, &sora);
    assert_eq!(cast_of(ai::decide(&i, AiAsk::Attack(Some(9)))).map(|c| c.1), Some(CastTarget::Member(SORA_MEMBER)));
}

#[test]
fn ai_attunes_by_situation_and_never_releases() {
    let foes = [AiEnemy { weak: ElemSet::one(Element::Blizzard), ..enemy(9, 1500.0) }];
    // the target's weakness
    let fire = attuned(&[(Element::Fire, 1.0)]);
    assert_eq!(ai::decide(&input(&fire, &foes, &[]), AiAsk::Attack(Some(9))), Some(AiChoice::Attune(Element::Blizzard)));
    // Sora used Thunder: Water
    let thunder = [AiAlly { member: SORA_MEMBER, thunder: true, orbs: 3, barrier_left: 30.0, ..ally(3, 600.0, 90) }];
    assert_eq!(ai::decide(&input(&fire, &foes, &thunder), AiAsk::Attack(Some(9))), Some(AiChoice::Attune(Element::Water)));
    // the party low: Light
    let low = [AiAlly { barrier_left: 30.0, orbs: 3, ..ally(2, 600.0, 10) }];
    let i = AiInput { hp: 30, ..input(&fire, &foes, &low) };
    assert_eq!(ai::decide(&i, AiAsk::Attack(Some(9))), Some(AiChoice::Attune(Element::Light)));
    // AT RESONANCE 3 he never switches (a Release would fire): he casts in his element instead
    let full = attuned(&[(Element::Fire, 3.0)]);
    let c = ai::decide(&input(&full, &foes, &[]), AiAsk::Attack(Some(9)));
    assert!(!matches!(c, Some(AiChoice::Attune(_))), "{c:?}");
    assert!(cast_of(c).is_some_and(|c| c.0.element() == Some(Element::Fire)), "{c:?}");
    // nothing attuned: the element he knows best (not Light)
    let none = attuned(&[]);
    assert!(matches!(ai::decide(&input(&none, &[enemy(9, 1500.0)], &[]), AiAsk::Attack(Some(9))), Some(AiChoice::Attune(e)) if e != Element::Light));
}

#[test]
fn ai_ultimate_once_per_period_and_easy_reactions() {
    let foes = [enemy(9, 1500.0)];
    let full = attuned(&[(Element::Blizzard, 3.0)]);
    assert_eq!(cast_of(ai::decide(&input(&full, &foes, &[]), AiAsk::Attack(Some(9)))).map(|c| c.0), Some(SpellRef::Ultimate(Element::Blizzard)));
    // cast in this period: the -za instead
    let mut spent = full.clone();
    spent.core.attune.slots[0].as_mut().unwrap().ult_used = true;
    assert_eq!(cast_of(ai::decide(&input(&spent, &foes, &[]), AiAsk::Attack(Some(9)))).map(|c| c.0), Some(SpellRef::Line1(Element::Blizzard, 4)));
    // below full power: the -ga (the menu's pick), no ultimate
    let half = attuned(&[(Element::Blizzard, 1.5)]);
    assert_eq!(cast_of(ai::decide(&input(&half, &foes, &[]), AiAsk::Attack(Some(9)))).map(|c| c.0), Some(SpellRef::Line1(Element::Blizzard, 3)));
    // Wet target: Thunder (Conduct); never Fire on it (Fizzle)
    let wet = [AiEnemy { wet: true, ..enemy(9, 1500.0) }];
    let fire = attuned(&[(Element::Fire, 1.0)]);
    let c = cast_of(ai::decide(&input(&fire, &wet, &[]), AiAsk::Attack(Some(9))));
    assert_eq!(c.map(|c| c.0.element()), Some(Some(Element::Thunder)));
    let mut only_fire = fire.clone();
    only_fire.learned = DonLearned::start();
    only_fire.learned.tier[Element::Water.index()] = 0;
    only_fire.learned.line2[Element::Water.index()] = 0;
    let c = cast_of(ai::decide(&input(&only_fire, &wet, &[]), AiAsk::Attack(Some(9))));
    assert!(c.is_none_or(|c| c.0.element() != Some(Element::Fire)), "{c:?}");
    // Frozen: Aero or Thunder (Shatter)
    let frozen = [AiEnemy { frozen: true, ..enemy(9, 1500.0) }];
    let c = cast_of(ai::decide(&input(&fire, &frozen, &[]), AiAsk::Attack(Some(9))));
    assert_eq!(c.map(|c| c.0.element()), Some(Some(Element::Aero)));
    // the ultimate is off by the sheet row: the -za instead
    let mut p = super::ability_info::ProgData::sheet();
    assert!(p.set("don.ai.ultimate", 0.0));
    assert!(p.set("ability.combo_plus.ap", 5.0) && !p.set("ability.combo_plus.nope", 1.0) && !p.set("nope", 1.0));
    assert_eq!((p.value("ai.ultimate"), p.row("ability.combo_plus").unwrap().ap), (0.0, 5));
}

#[test]
fn ai_keeps_his_range() {
    let me = attuned(&[(Element::Fire, 1.0)]);
    let near = [enemy(9, 500.0)];
    match ai::range_move(&input(&me, &near, &[])) {
        Some(AiChoice::Move { to, retreat: true }) => assert!((to.x - (500.0 - 1200.0)).abs() < 1.0, "{to:?}: 12 m from it, away"),
        other => panic!("{other:?}"),
    }
    assert_eq!(ai::range_move(&input(&me, &[enemy(9, 900.0)], &[])), None, "8-20 m: his place");
    // low MP: only heals and protection, no attack
    let far = [enemy(9, 1500.0)];
    let i = AiInput { mp: 10, ..input(&me, &far, &[]) };
    assert_eq!(ai::decide(&i, AiAsk::Attack(Some(9))), None);
}
