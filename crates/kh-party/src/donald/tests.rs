//! (DON1) The foundation's rules against the doc (documentation/DONALD_OVERHAUL.md), section by section.

use super::abilities::*;
use super::arcane::*;
use super::combat::*;
use super::element::*;
use super::learn::*;
use super::react::*;
use super::spells::*;
use super::status::*;
use super::tune::*;

fn d() -> DonData {
    DonData::sheet()
}

const MAX_MP: i32 = 200;

fn mods() -> DonMods {
    DonMods::default()
}

/// Attunes `e` at once (Instant Attune) and runs the step.
fn attune_now(c: &mut DonCore, e: Element, l: &DonLearned, m: &DonMods, d: &DonData) {
    let m2 = DonMods { instant_attune: true, ..*m };
    c.begin_attune(e, l, &m2, d).expect("attune");
}

/// Casts until `e` is at Resonance 3 (attuned base-tier casts).
fn fill(c: &mut DonCore, e: Element, l: &DonLearned, d: &DonData) {
    for _ in 0..10 {
        if c.full_power(e, d) {
            return;
        }
        let plan = c.check_cast(SpellRef::Line1(e, 1), l, MAX_MP, MAX_MP, false, d).expect("base tier");
        c.commit_cast(&plan, &mods(), d);
    }
    assert!(c.full_power(e, d), "{e:?} never reached full power");
}

// ---------------------------------------------------------------- tune (2: "tuned without a rebuild")
#[test]
fn every_core_row_is_tunable_at_run_time() {
    let mut data = d();
    for key in core_keys() {
        let v = data.get(key).unwrap_or_else(|| panic!("{key} has no getter"));
        assert!(data.set(key, v + 1.0), "{key} has no setter");
        assert_eq!(data.get(key), Some(v + 1.0));
        assert!(data.set(&format!("don.{key}"), v));
    }
    assert!(!data.set("res.nope", 1.0));
    assert!(data.set("spell.l1.fire.4.cost_att", 11.0));
    assert_eq!(data.row("l1.fire.4").unwrap().cost_att, 11.0);
    assert!(!data.set("spell.l1.fire.4.nope", 1.0));
    assert!(!data.set("spell.l9.fire.4.cost_att", 1.0));
    assert_eq!(data.apply([("attune.cast", 0.4), ("x.y", 1.0)]), vec!["x.y"]);
    assert_eq!(data.tune.attune_cast, 0.4);
}

// ---------------------------------------------------------------- spells (6.1, 6.2, 6.4, 6.3)
#[test]
fn the_sheet_has_every_spell_and_level_of_the_doc() {
    let data = d();
    assert_eq!(data.spells.len(), 24 + 18 + 6 + 15 + 10 + 6);
    for r in &data.spells {
        let s = SpellRef::from_id(r.id).unwrap_or_else(|| panic!("{} is no spell id", r.id));
        assert_eq!(s.id(), r.id);
    }
    let names: Vec<&str> = Element::ALL.iter().map(|e| SpellRef::Line2(*e, 1).name(&data)).collect();
    assert_eq!(names, ["Flare", "Freeze", "Ball Lightning", "Flood", "Aero Barrier", "Banish"]);
    let ult: Vec<&str> = Element::ALL.iter().map(|e| SpellRef::Ultimate(*e).name(&data)).collect();
    assert_eq!(ult, ["Inferno", "Diamond Dust", "Judgment Bolt", "Tsunami", "Aerial Blast", "Holy"]);
    assert_eq!(SpellRef::Line1(Element::Light, 4).name(&data), "Curaza");
    assert_eq!(SpellRef::Line2(Element::Light, 3).name(&data), "Banish III");
    assert_eq!(SpellRef::Arcane(ArcaneLine::Gravity, 3).name(&data), "Graviga");
    assert_eq!(hybrid_pairs().count(), 15);
    let hy: Vec<&str> = hybrid_pairs().map(|(a, b)| SpellRef::Hybrid(a, b).name(&data)).collect();
    for n in ["Thermal Shock", "Plasma", "Scald", "Fire Whirl", "Sunburst", "Hailstorm", "Glacier", "Whiteout", "Prism Frost", "Thunderstorm", "Tempest", "Divine Spark", "Maelstrom", "Shining Rain", "Radiant Gale"] {
        assert!(hy.contains(&n), "{n} missing");
    }
    assert_eq!(SpellRef::hybrid(Element::Water, Element::Thunder), Some(SpellRef::Hybrid(Element::Thunder, Element::Water)));
    assert_eq!(SpellRef::Hybrid(Element::Fire, Element::Water).name(&data), "Scald");
}

#[test]
fn the_mp_table_is_the_docs() {
    let data = d();
    let att = |s: SpellRef, quick: bool| price_pct(s.row(&data).unwrap(), PriceCtx { attuned: true, quick, echo_free: false });
    let free = |s: SpellRef| price_pct(s.row(&data).unwrap(), PriceCtx::default());
    // fixed MP like Sora's KH3 spells (user, 2026-10-22): line 1 tiers 1-3 = Sora's KH3 costs unattuned, about x0.75 attuned
    let l1 = [
        (Element::Fire, [(10.0, 8.0), (14.0, 11.0), (20.0, 15.0)]),
        (Element::Blizzard, [(12.0, 9.0), (17.0, 13.0), (24.0, 18.0)]),
        (Element::Thunder, [(14.0, 11.0), (20.0, 15.0), (28.0, 21.0)]),
        (Element::Water, [(12.0, 9.0), (15.0, 11.0), (22.0, 17.0)]),
        (Element::Aero, [(13.0, 10.0), (18.0, 14.0), (26.0, 20.0)]),
        (Element::Light, [(13.0, 10.0), (17.0, 12.0), (23.0, 17.0)]),
    ];
    for (e, tiers) in l1 {
        for (i, (f, a)) in tiers.into_iter().enumerate() {
            let s = SpellRef::Line1(e, i as u8 + 1);
            assert_eq!((free(s), att(s, false)), (Some(f), Some(a)), "{e:?} tier {}", i + 1);
        }
        let za = SpellRef::Line1(e, 4);
        assert_eq!((free(za), att(za, false), att(za, true)), (None, Some(27.0), Some(27.0)), "-za: no quick discount");
    }
    // four -za casts per full bar: Donald's start max MP (Balance 110 x 1.5 = 165) at the x1.5 base cost (user, 2026-10-22: no difficulty select)
    assert_eq!(mp_of_cost(27.0, 1.5), 41);
    assert_eq!(165 / mp_of_cost(27.0, 1.5), 4, "only four -za from a full bar (user, 2026-10-22)");
    for e in Element::ALL {
        for (lv, f, a) in [(1, 10.0, 7.0), (2, 13.0, 10.0), (3, 17.0, 12.0)] {
            assert_eq!((free(SpellRef::Line2(e, lv)), att(SpellRef::Line2(e, lv), false)), (Some(f), Some(a)), "{e:?} line 2 Lv {lv}");
        }
        assert_eq!((free(SpellRef::Ultimate(e)), att(SpellRef::Ultimate(e), false), att(SpellRef::Ultimate(e), true)), (None, Some(20.0), Some(13.0)));
    }
    for (a, b) in hybrid_pairs() {
        assert_eq!(att(SpellRef::Hybrid(a, b), false), Some(15.0));
        assert_eq!(free(SpellRef::Hybrid(a, b)), None);
    }
    let y: Vec<(u32, f32)> = ArcaneLine::ALL.iter().map(|l| SpellRef::Arcane(*l, 1).row(&data).map(|r| (r.arcane, r.cost_att)).unwrap()).collect();
    assert_eq!(y, [(2, 40.0), (1, 45.0), (3, 55.0), (4, 70.0), (4, 0.0)], "set MP costs for the Y spells; Comet 2 stars, Gravity 1 (user 2026-10-23); Zettaflare none in the row: all of his current MP (ZETTA, user 2026-10-23)");
    for e in Element::ALL {
        assert_eq!(free(SpellRef::Release(e)), Some(0.0), "a Release is free");
    }
    assert_eq!(mp_of_pct(200, 3.0, 1.0), 6);
    assert_eq!(mp_of_pct(150, 3.0, 1.0), 5); // 4.5 -> 5
    assert_eq!(mp_of_pct(100, 0.0, 1.0), 0);
    assert_eq!(mp_of_pct(10, 3.0, 1.0), 1);
}

#[test]
fn command_ids_round_trip_and_old_blocks_migrate() {
    let mut seen = Vec::new();
    for id in CMD_FIRST..=CMD_LAST {
        let c = DonCmd::from_id(id).unwrap_or_else(|| panic!("{id:#x}"));
        assert_eq!(c.id(), Some(id));
        seen.push(c);
    }
    assert_eq!(seen.len(), 24 + 6 + 6 + 15 + 6);
    assert!(!(58..=68).any(|i| DonCmd::from_id(i).is_some()), "item commands stay items");
    assert_eq!(DonCmd::Spell(SpellRef::Line2(Element::Thunder, 3)).id(), DonCmd::Spell(SpellRef::Line2(Element::Thunder, 1)).id());
    assert_eq!(DonCmd::from_sora_cmd(29), Some(DonCmd::Spell(SpellRef::Line1(Element::Fire, 1))));
    assert_eq!(DonCmd::from_sora_cmd(51), Some(DonCmd::Spell(SpellRef::Line1(Element::Light, 3))));
    assert_eq!(DonCmd::from_sora_cmd(118), Some(DonCmd::Spell(SpellRef::Line1(Element::Thunder, 4))));
    assert_eq!(DonCmd::from_sora_cmd(60), None);
}

// ---------------------------------------------------------------- learning (9, 10)
#[test]
fn the_start_and_one_tier_per_boss() {
    let data = d();
    let mut l = DonLearned::start();
    assert_eq!((l.tier(Element::Fire), l.tier(Element::Water), l.line2(Element::Fire), l.line2(Element::Water)), (1, 1, 1, 1));
    assert_eq!(l.arcane(ArcaneLine::Gravity), 1);
    assert_eq!(l.elements(), ElemSet::of(&[Element::Fire, Element::Water]));
    assert!(!l.attune_unlocked(Element::Thunder));
    // Fire: Fira + Flare Lv 2, Firaga + Flare Lv 3 from base-game bosses; (DON-PROG) a further base boss past -ga: Max
    // MP +3; the DLC boss: Firaza + Inferno; again: Max MP +3
    assert_eq!(l.apply(DonStep::Element(Element::Fire)), vec![Grant::Tier(Element::Fire, 2), Grant::Line2(Element::Fire, 2)]);
    assert_eq!(l.apply(DonStep::Element(Element::Fire)), vec![Grant::Tier(Element::Fire, 3), Grant::Line2(Element::Fire, 3)]);
    assert_eq!(l.apply(DonStep::Element(Element::Fire)), vec![Grant::MaxMp(3)], "(DON-PROG) no base boss gives -za");
    assert_eq!(l.apply(DonStep::ElementDlc(Element::Fire)), vec![Grant::Tier(Element::Fire, 4), Grant::Ultimate(Element::Fire)]);
    assert!(l.knows(SpellRef::Ultimate(Element::Fire)) && l.knows(SpellRef::Line1(Element::Fire, 4)));
    assert_eq!(l.apply(DonStep::ElementDlc(Element::Fire)), vec![Grant::MaxMp(3)]);
    assert_eq!(l.mp_bonus(), 6);
    // (DON-PROG) the DLC boss first: every missing tier, then -za + the ultimate (nothing skipped)
    let mut n = DonLearned::start();
    assert_eq!(n.apply(DonStep::ElementDlc(Element::Blizzard)), vec![Grant::Tier(Element::Blizzard, 1), Grant::Attunement(Element::Blizzard), Grant::Line2(Element::Blizzard, 1), Grant::Tier(Element::Blizzard, 2), Grant::Line2(Element::Blizzard, 2), Grant::Tier(Element::Blizzard, 3), Grant::Line2(Element::Blizzard, 3), Grant::Tier(Element::Blizzard, 4), Grant::Ultimate(Element::Blizzard)]);
    assert_eq!(n.apply(DonStep::Element(Element::Blizzard)), vec![Grant::MaxMp(3)], "a base boss after it");
    assert_eq!(DonStep::parse("dlc:fire"), Some(DonStep::ElementDlc(Element::Fire)));
    assert_eq!(DonStep::ElementDlc(Element::Light).token(), "dlc:light");
    assert_eq!(sora_gain(DonStep::ElementDlc(Element::Fire), 1), SoraGain::Tier(2));
    // Thunder from nothing: Thunder + its Attunement + Ball Lightning Lv 1
    assert_eq!(l.apply(DonStep::Element(Element::Thunder)), vec![Grant::Tier(Element::Thunder, 1), Grant::Attunement(Element::Thunder), Grant::Line2(Element::Thunder, 1)]);
    let words: Vec<String> = [Grant::Tier(Element::Fire, 4), Grant::Line2(Element::Fire, 2), Grant::Attunement(Element::Light), Grant::MaxMp(3)].iter().map(|g| g.label(&data)).collect();
    assert_eq!(words, ["Firaza", "Flare Lv 2", "Light Attunement", "Max MP +3"]);
}

#[test]
fn banish_levels_in_any_order_and_light_attunement() {
    // the doc's example: two Light bosses first -> Banish Lv 1 from the second; the Tree Sentinel later gives Lv 2
    let mut l = DonLearned::start();
    assert_eq!(l.apply(DonStep::Element(Element::Light)), vec![Grant::Tier(Element::Light, 1), Grant::Attunement(Element::Light)]);
    assert_eq!(l.apply(DonStep::Element(Element::Light)), vec![Grant::Tier(Element::Light, 2), Grant::Line2(Element::Light, 1)]);
    assert_eq!(l.apply(DonStep::Banish), vec![Grant::Line2(Element::Light, 2)]);
    assert_eq!(l.apply(DonStep::Element(Element::Light)), vec![Grant::Tier(Element::Light, 3), Grant::Line2(Element::Light, 3)]);
    assert_eq!(l.apply(DonStep::ElementDlc(Element::Light)), vec![Grant::Tier(Element::Light, 4), Grant::Ultimate(Element::Light)]); // (DON-PROG) its DLC boss
    assert_eq!(l.apply(DonStep::Banish), vec![Grant::MaxMp(3)], "past Lv 3: Max MP +3");
    // the Tree Sentinel first (as intended): Banish Lv 1 unlocks Light Attunement
    let mut l = DonLearned::start();
    assert_eq!(l.apply(DonStep::Banish), vec![Grant::Line2(Element::Light, 1), Grant::Attunement(Element::Light)]);
    assert!(l.attune_unlocked(Element::Light) && l.tier(Element::Light) == 0);
    assert_eq!(l.apply(DonStep::Element(Element::Light)), vec![Grant::Tier(Element::Light, 1)]);
}

#[test]
fn arcane_lines_and_soras_share() {
    let mut l = DonLearned::start();
    assert_eq!(l.apply(DonStep::Arcane(ArcaneLine::Gravity)), vec![Grant::Arcane(ArcaneLine::Gravity, 2)]);
    assert_eq!(l.apply(DonStep::Arcane(ArcaneLine::Gravity)), vec![Grant::Arcane(ArcaneLine::Gravity, 3)]);
    assert_eq!(l.apply(DonStep::Arcane(ArcaneLine::Gravity)), vec![Grant::MaxMp(3)]);
    assert_eq!(l.apply(DonStep::Arcane(ArcaneLine::Comet)), vec![Grant::Arcane(ArcaneLine::Comet, 1)]);
    assert_eq!(l.apply(DonStep::Arcane(ArcaneLine::Ultima)), vec![Grant::Arcane(ArcaneLine::Ultima, 1)]);
    assert_eq!(l.apply(DonStep::Arcane(ArcaneLine::Zettaflare)), vec![Grant::Arcane(ArcaneLine::Zettaflare, 1)]);
    assert_eq!(l.apply(DonStep::Arcane(ArcaneLine::Zettaflare)), vec![Grant::MaxMp(3)]);
    assert_eq!(sora_gain(DonStep::Element(Element::Fire), 1), SoraGain::Tier(2));
    assert_eq!(sora_gain(DonStep::Element(Element::Thunder), 0), SoraGain::Tier(1));
    assert_eq!(sora_gain(DonStep::Element(Element::Fire), 3), SoraGain::MaxMp(3));
    assert_eq!(sora_gain(DonStep::Banish, 0), SoraGain::Nothing);
    assert_eq!(sora_gain(DonStep::Arcane(ArcaneLine::Meteor), 3), SoraGain::Nothing);
    assert_eq!(DonStep::parse("magic:cure"), Some(DonStep::Element(Element::Light)));
    assert_eq!(DonStep::parse("arcane:zettaflare"), Some(DonStep::Arcane(ArcaneLine::Zettaflare)));
    assert_eq!(DonStep::parse("banish"), Some(DonStep::Banish));
    for s in [DonStep::Element(Element::Aero), DonStep::Banish, DonStep::Arcane(ArcaneLine::Comet)] {
        assert_eq!(DonStep::parse(&s.token()), Some(s));
    }
}

#[test]
fn the_record_parts_round_trip_and_clamp() {
    let mut l = DonLearned::start();
    l.apply(DonStep::Element(Element::Aero));
    l.apply(DonStep::Banish);
    l.apply(DonStep::Arcane(ArcaneLine::Meteor));
    let (t, l2, a, mp) = l.parts();
    let back = DonLearned::from_parts(&t.map(u64::from), &l2.map(u64::from), &a.map(u64::from), mp as u64);
    assert_eq!(back, l);
    let odd = DonLearned::from_parts(&[9, 9], &[7], &[9, 0, 9, 9, 9, 9], 999);
    assert_eq!((odd.tier[0], odd.tier[2], odd.line2[0], odd.arcane, odd.mp_steps), (4, 0, 3, [2, 1, 3, 1, 1], 255));
}

// ---------------------------------------------------------------- attunement (4.1 - 4.4, 6.1)
#[test]
fn attuning_takes_its_cast_and_can_be_interrupted() {
    let data = d();
    let l = DonLearned::start();
    let mut c = DonCore::default();
    assert_eq!(c.begin_attune(Element::Thunder, &l, &mods(), &data), Err(Refusal::NotLearned), "no Thunder spell: locked");
    // (DON-KIT) the sheet's attune.cast (1.0 s since the user's 2026-10-22 "make attuning take a bit longer")
    let cast = data.tune.attune_cast;
    assert_eq!(c.begin_attune(Element::Fire, &l, &mods(), &data), Ok(cast));
    assert_eq!(c.check_cast(SpellRef::Line1(Element::Fire, 1), &l, MAX_MP, MAX_MP, false, &data).unwrap_err(), Refusal::Attuning);
    let steps = ((cast - 0.1) / 0.1).round() as usize;
    for _ in 0..steps {
        c.tick(0.1, true, &mods(), &data);
    }
    assert!(!c.attuned(Element::Fire), "just before its cast: not yet");
    c.tick(0.11, true, &mods(), &data);
    assert!(c.attuned(Element::Fire));
    assert_eq!(c.begin_attune(Element::Fire, &l, &mods(), &data), Err(Refusal::NotCastable), "already attuned");
    // interrupted: nothing changes
    c.begin_attune(Element::Water, &l, &mods(), &data).unwrap();
    c.tick(0.3, true, &mods(), &data);
    assert!(c.interrupt_attune());
    c.tick(1.0, true, &mods(), &data);
    assert_eq!(c.attune.primary(), Some(Element::Fire));
    let ev = c.take_events();
    assert!(ev.iter().any(|e| matches!(e, DonEvent::AttuneInterrupted { element: Element::Water })));
    // Swift halves, Instant removes
    let mut c = DonCore::default();
    assert_eq!(c.begin_attune(Element::Fire, &l, &DonMods { swift_attune: true, ..mods() }, &data), Ok(cast * data.tune.attune_swift_mult)); // (DON-KIT) the sheet's cast x the Swift row
    let mut c = DonCore::default();
    assert_eq!(c.begin_attune(Element::Fire, &l, &DonMods { instant_attune: true, ..mods() }, &data), Ok(0.0));
    assert!(c.attuned(Element::Fire));
}

#[test]
fn resonance_pips_refund_and_full_power_opens_the_za_tier() {
    let data = d();
    let mut l = DonLearned::start();
    l.apply(DonStep::ElementDlc(Element::Fire)); // (DON-PROG) up to -za: the DLC boss
    let mut c = DonCore::default();
    attune_now(&mut c, Element::Fire, &l, &mods(), &data);
    c.take_events();
    assert_eq!(c.check_cast(SpellRef::Line1(Element::Fire, 4), &l, MAX_MP, MAX_MP, false, &data).unwrap_err(), Refusal::NeedsFullPower);
    assert_eq!(c.line1_pick(Element::Fire, &l, &data), Some(SpellRef::Line1(Element::Fire, 3)), "the menu casts -ga until full power");
    assert_eq!(c.ultimate_mark(Element::Fire, &l, &data), UltMark::Waiting);
    // attuned Fire: a fixed 8 MP (Sora's KH3 Fire is 10; attuned about x0.75)
    let plan = c.check_cast(SpellRef::Line1(Element::Fire, 1), &l, MAX_MP, MAX_MP, false, &data).unwrap();
    assert_eq!((plan.mp, plan.attuned), (8, true));
    fill(&mut c, Element::Fire, &l, &data);
    let ev = c.take_events();
    let refunds: Vec<u8> = ev.iter().filter_map(|e| if let DonEvent::PipFilled { pips, refund_pct, .. } = e { assert_eq!(*refund_pct, 2.0); Some(*pips) } else { None }).collect();
    assert_eq!(refunds, [1, 2, 3], "each pip refunds 2 % of max MP as it fills");
    assert!(ev.contains(&DonEvent::FullPower { element: Element::Fire }));
    assert_eq!(c.line1_pick(Element::Fire, &l, &data), Some(SpellRef::Line1(Element::Fire, 4)));
    assert_eq!(c.ultimate_mark(Element::Fire, &l, &data), UltMark::Ready);
    // -za: a fixed 27 MP (four per Donald's start bar at Standard's x1.5, see the_mp_table_is_the_docs); no quick discount
    let za = c.check_cast(SpellRef::Line1(Element::Fire, 4), &l, MAX_MP, MAX_MP, false, &data).unwrap();
    assert_eq!((za.pct, za.mp), (27.0, 27));
    assert_eq!(za.mp, 27, "fixed MP, whatever his max MP");
    c.tick(3.1, true, &mods(), &data);
    let za = c.check_cast(SpellRef::Line1(Element::Fire, 4), &l, MAX_MP, MAX_MP, false, &data).unwrap();
    assert_eq!((za.pct, za.mp, za.quick), (27.0, 27, false));
    // off-element: full price, no Resonance
    let water = c.check_cast(SpellRef::Line1(Element::Water, 1), &l, MAX_MP, MAX_MP, false, &data).unwrap();
    assert_eq!((water.pct, water.attuned), (12.0, false));
    // not enough MP
    assert_eq!(c.check_cast(SpellRef::Line1(Element::Fire, 4), &l, 26, MAX_MP, false, &data).unwrap_err(), Refusal::NotEnoughMp { need: 27 });
}

#[test]
fn the_ultimate_once_per_attunement_period() {
    let data = d();
    let mut l = DonLearned::start();
    l.apply(DonStep::ElementDlc(Element::Fire)); // (DON-PROG) up to -za: the DLC bosses
    l.apply(DonStep::ElementDlc(Element::Water));
    let mut c = DonCore::default();
    attune_now(&mut c, Element::Fire, &l, &mods(), &data);
    fill(&mut c, Element::Fire, &l, &data);
    let ult = c.check_cast(SpellRef::Ultimate(Element::Fire), &l, MAX_MP, MAX_MP, false, &data).unwrap();
    assert_eq!(ult.pct, 13.0, "within 3 s of Resonance 3: 13 MP");
    c.commit_cast(&ult, &mods(), &data);
    assert_eq!(c.check_cast(SpellRef::Ultimate(Element::Fire), &l, MAX_MP, MAX_MP, false, &data).unwrap_err(), Refusal::UltimateSpent);
    assert_eq!(c.ultimate_mark(Element::Fire, &l, &data), UltMark::Spent);
    assert!(c.check_cast(SpellRef::Line1(Element::Fire, 4), &l, MAX_MP, MAX_MP, false, &data).is_ok(), "the -za tier has no such limit");
    // switch to Water (Release), power it up: Tsunami opens; back to Fire later: a fresh period
    attune_now(&mut c, Element::Water, &l, &mods(), &data);
    fill(&mut c, Element::Water, &l, &data);
    assert!(c.check_cast(SpellRef::Ultimate(Element::Water), &l, MAX_MP, MAX_MP, false, &data).is_ok());
    attune_now(&mut c, Element::Fire, &l, &mods(), &data);
    assert_eq!(c.ultimate_mark(Element::Fire, &l, &data), UltMark::Waiting, "a fresh period");
    fill(&mut c, Element::Fire, &l, &data);
    assert!(c.check_cast(SpellRef::Ultimate(Element::Fire), &l, MAX_MP, MAX_MP, false, &data).is_ok());
}

#[test]
fn the_release_bursts_the_old_element_and_starts_the_new_at_one() {
    let data = d();
    let l = DonLearned::start();
    let mut c = DonCore::default();
    attune_now(&mut c, Element::Fire, &l, &mods(), &data);
    assert_eq!(c.would_release(Element::Water, &mods(), &data), None, "below 3: no Release");
    attune_now(&mut c, Element::Water, &l, &mods(), &data);
    assert_eq!(c.resonance(Element::Water), Some(0.0));
    assert!(!c.take_events().iter().any(|e| matches!(e, DonEvent::Released { .. })));
    fill(&mut c, Element::Water, &l, &data);
    assert_eq!(c.would_release(Element::Fire, &mods(), &data), Some(Element::Water));
    c.take_events();
    attune_now(&mut c, Element::Fire, &l, &mods(), &data);
    let ev = c.take_events();
    assert!(ev.contains(&DonEvent::Released { element: Element::Water }));
    assert!(ev.iter().any(|e| matches!(e, DonEvent::ArcaneGained { reason: ArcaneGain::Release, .. })));
    assert_eq!(c.resonance(Element::Fire), Some(1.0), "a Release starts the new element at 1 pip");
    assert!(!ev.iter().any(|e| matches!(e, DonEvent::PipFilled { element: Element::Fire, .. })), "the starting pip is not a fill (no refund)");
    assert_eq!(c.arcane.charges(), 1);
}

#[test]
fn dual_attunement_holds_two_and_opens_the_hybrid() {
    let data = d();
    let mut l = DonLearned::start();
    l.apply(DonStep::Element(Element::Thunder));
    let m = DonMods { dual: true, ..mods() };
    let mut c = DonCore::default();
    attune_now(&mut c, Element::Water, &l, &m, &data);
    let hy = SpellRef::Hybrid(Element::Thunder, Element::Water);
    assert_eq!(c.check_cast(hy, &l, MAX_MP, MAX_MP, true, &data).unwrap_err(), Refusal::NeedsBothAttuned);
    attune_now(&mut c, Element::Thunder, &l, &m, &data);
    assert_eq!(c.elements(), ElemSet::of(&[Element::Water, Element::Thunder]));
    assert_eq!((c.missile_element(0), c.missile_element(1), c.missile_element(2)), (Some(Element::Thunder), Some(Element::Water), Some(Element::Thunder)));
    assert_eq!(c.check_cast(hy, &l, MAX_MP, MAX_MP, false, &data).unwrap_err(), Refusal::NeedsBothAttuned, "without the ability: no hybrid");
    let plan = c.check_cast(hy, &l, MAX_MP, MAX_MP, true, &data).unwrap();
    assert_eq!(plan.mp, 15, "a hybrid: 15 MP");
    c.commit_cast(&plan, &m, &data);
    let g = data.tune.res_gain_spell; // (DON-KIT) the sheet's res.gain_spell
    assert_eq!((c.resonance(Element::Water), c.resonance(Element::Thunder)), (Some(g), Some(g)), "a hybrid fills both meters");
    // a third element replaces the OLDER attunement (Water)
    attune_now(&mut c, Element::Fire, &l, &m, &data);
    assert_eq!(c.elements(), ElemSet::of(&[Element::Fire, Element::Thunder]));
    // the ability taken off: the older one goes
    c.tick(0.01, true, &mods(), &data);
    assert_eq!(c.elements(), ElemSet::one(Element::Fire));
}

#[test]
fn heavy_hits_cost_a_pip_and_the_meter_drains_only_out_of_combat() {
    let data = d();
    let l = DonLearned::start();
    let mut c = DonCore::default();
    attune_now(&mut c, Element::Fire, &l, &mods(), &data);
    fill(&mut c, Element::Fire, &l, &data);
    c.take_events();
    c.on_heavy_hit(&data);
    assert_eq!(c.resonance(Element::Fire), Some(2.0));
    let ev = c.take_events();
    assert!(ev.contains(&DonEvent::FullPowerLost { element: Element::Fire }) && ev.contains(&DonEvent::PipLost { element: Element::Fire, pips: 2 }));
    for _ in 0..600 {
        c.tick(1.0 / 60.0, true, &mods(), &data);
    }
    assert_eq!(c.resonance(Element::Fire), Some(2.0), "no drain in combat");
    // out of combat: 2 s delay, then 0.25 pip/s (frame-rate independent)
    let run = |dt: f32, keeper: bool| {
        let mut k = c.clone();
        let n = (6.0 / dt).round() as usize;
        for _ in 0..n {
            k.tick(dt, false, &DonMods { resonance_keeper: keeper, ..mods() }, &data);
        }
        k.resonance(Element::Fire).unwrap()
    };
    let (a, b) = (run(1.0 / 30.0, false), run(1.0 / 144.0, false));
    assert!((a - 1.0).abs() < 0.02 && (a - b).abs() < 0.01, "6 s out: 2 - 4 x 0.25 = 1 ({a} / {b})");
    assert!((run(1.0 / 60.0, true) - 1.5).abs() < 0.02, "Resonance Keeper halves the drain");
}

// ---------------------------------------------------------------- Arcane Flow (6.3, 6.5)
#[test]
fn arcane_flow_gains_decays_and_the_y_list() {
    let data = d();
    let mut l = DonLearned::start();
    l.apply(DonStep::Element(Element::Thunder));
    l.apply(DonStep::Arcane(ArcaneLine::Comet));
    l.apply(DonStep::Arcane(ArcaneLine::Meteor));
    let mut c = DonCore::default();
    c.tick(0.1, true, &mods(), &data);
    // three elements within 10 s: +1
    for (i, e) in [Element::Fire, Element::Water, Element::Thunder].into_iter().enumerate() {
        let p = c.check_cast(SpellRef::Line1(e, 1), &l, MAX_MP, MAX_MP, false, &data).unwrap();
        c.commit_cast(&p, &mods(), &data);
        assert_eq!(c.arcane.charges(), (i == 2) as u8);
        c.tick(4.0, true, &mods(), &data);
    }
    // slower than 10 s: nothing
    for e in [Element::Fire, Element::Water, Element::Thunder] {
        let p = c.check_cast(SpellRef::Line1(e, 1), &l, MAX_MP, MAX_MP, false, &data).unwrap();
        c.commit_cast(&p, &mods(), &data);
        c.tick(5.5, true, &mods(), &data);
    }
    assert_eq!(c.arcane.charges(), 0, "the first charge expired after 20 s and no new one came");
    // perfect blocks: +1 each; the Y list
    c.on_perfect_block(&mods(), &data);
    c.on_perfect_block(&mods(), &data);
    let names = |o: &[YOffer]| o.iter().map(|y| y.spell).collect::<Vec<_>>();
    let offers = c.y_offers(&l, MAX_MP, MAX_MP, &data);
    assert_eq!(names(&offers), [SpellRef::Arcane(ArcaneLine::Comet, 1), SpellRef::Arcane(ArcaneLine::Gravity, 1)]);
    assert_eq!(offers.iter().map(|o| (o.affordable, o.mp)).collect::<Vec<_>>(), [(true, 40), (true, 45)], "set MP: Comet 40, Gravity 45");
    let poor = c.y_offers(&l, 39, MAX_MP, &data);
    assert!(poor.iter().all(|o| !o.affordable), "below its set MP: greyed with the MP it needs");
    // (user 2026-10-23: "make gravity cost 1 star and comet cost 2 stars") casting Gravity (1) with 2 leaves 1
    let p = c.check_cast(SpellRef::Arcane(ArcaneLine::Gravity, 1), &l, MAX_MP, MAX_MP, false, &data).unwrap();
    c.commit_cast(&p, &mods(), &data);
    assert_eq!(c.arcane.charges(), 1);
    assert_eq!(c.check_cast(SpellRef::Arcane(ArcaneLine::Comet, 1), &l, MAX_MP, MAX_MP, false, &data).unwrap_err(), Refusal::NotEnoughCharges { need: 2 });
    // Arcane Flow+: 30 s
    let mut c = DonCore::default();
    c.on_perfect_block(&DonMods { arcane_flow_plus: true, ..mods() }, &data);
    c.tick(25.0, true, &mods(), &data);
    assert_eq!(c.arcane.charges(), 1);
    c.tick(5.1, true, &mods(), &data);
    assert_eq!(c.arcane.charges(), 0);
    // out of combat it empties (at the fight's end)
    let mut c = DonCore::default();
    c.on_perfect_block(&mods(), &data);
    c.tick(1.0, false, &mods(), &data);
    assert_eq!(c.arcane.charges(), 1);
    c.tick(2.5, false, &mods(), &data);
    assert_eq!(c.arcane.charges(), 0);
    assert!(c.take_events().contains(&DonEvent::FightEnded));
}

#[test]
fn ultima_and_zettaflare_conditions() {
    let data = d();
    let mut l = DonLearned::start();
    l.apply(DonStep::Arcane(ArcaneLine::Ultima));
    l.apply(DonStep::Arcane(ArcaneLine::Zettaflare));
    let mut c = DonCore::default();
    c.tick(0.1, true, &mods(), &data);
    for _ in 0..4 {
        c.on_perfect_block(&mods(), &data);
    }
    let ys = |c: &DonCore, mp: i32| c.y_offers(&l, mp, MAX_MP, &data).iter().map(|y| y.spell).collect::<Vec<_>>();
    assert!(!ys(&c, MAX_MP).contains(&SpellRef::Arcane(ArcaneLine::Ultima, 1)), "Ultima: only at full power");
    attune_now(&mut c, Element::Fire, &l, &mods(), &data);
    fill(&mut c, Element::Fire, &l, &data);
    assert!(ys(&c, MAX_MP).contains(&SpellRef::Arcane(ArcaneLine::Ultima, 1)));
    assert!(!ys(&c, MAX_MP).contains(&SpellRef::Arcane(ArcaneLine::Zettaflare, 1)), "Water has not reached 3 in this fight");
    assert_eq!(c.zettaflare_progress(&l), (ElemSet::one(Element::Fire), ElemSet::of(&[Element::Fire, Element::Water])));
    attune_now(&mut c, Element::Water, &l, &mods(), &data); // the Release refreshes a charge (meter full)
    fill(&mut c, Element::Water, &l, &data);
    assert!(c.zettaflare_ready(&l, &data));
    // (ZETTA 2026-10-23, the user: "Zettaflare needs to use up all of your magic (it needs all of your current MP to be useable) and then put you into the red MP bar mode with a 30% slower recharge rate"; asked, the user chose "Requires a full MP bar") offered and castable only with a FULL bar, all of it spent
    let zy = |c: &DonCore, mp: i32| c.y_offers(&l, mp, MAX_MP, &data).into_iter().find(|y| y.spell == SpellRef::Arcane(ArcaneLine::Zettaflare, 1));
    let z = zy(&c, MAX_MP).unwrap();
    assert!(z.affordable && z.mp == MAX_MP, "his whole bar ({})", z.mp);
    assert!(zy(&c, MAX_MP - 1).is_none() && zy(&c, 1).is_none() && zy(&c, 0).is_none(), "not offered short of a full bar");
    for mp in [0, 1, 7, MAX_MP - 1] {
        assert_eq!(c.check_cast(SpellRef::Arcane(ArcaneLine::Zettaflare, 1), &l, mp, MAX_MP, false, &data).unwrap_err(), Refusal::NotEnoughMp { need: MAX_MP }, "{mp} MP: refused");
    }
    let p = c.check_cast(SpellRef::Arcane(ArcaneLine::Zettaflare, 1), &l, MAX_MP, MAX_MP, false, &data).unwrap();
    assert_eq!(p.mp, MAX_MP);
    assert!(!c.zetta_slow);
    c.commit_cast(&p, &mods(), &data);
    assert_eq!((c.arcane.charges(), c.resonance(Element::Water)), (0, Some(0.0)), "it empties Arcane Flow and Resonance");
    assert!(c.zetta_slow, "his MP regen slows (until his MP has recharged)");
}

#[test]
fn reactions_fill_a_pip_and_arcane_flow_with_cooldowns() {
    let data = d();
    let l = DonLearned::start();
    let mut c = DonCore::default();
    attune_now(&mut c, Element::Fire, &l, &mods(), &data);
    c.tick(0.1, true, &mods(), &data);
    c.on_reaction(Reaction::SteamBurst, 7, ElemSet::one(Element::Fire), &mods(), &data);
    assert_eq!((c.resonance(Element::Fire), c.arcane.charges()), (Some(1.0), 1));
    c.on_reaction(Reaction::SteamBurst, 7, ElemSet::one(Element::Fire), &mods(), &data);
    assert_eq!(c.resonance(Element::Fire), Some(1.0), "the same reaction on the same target: once per second");
    c.on_reaction(Reaction::Exposed, 8, ElemSet::NONE, &mods(), &data);
    assert_eq!((c.resonance(Element::Fire), c.arcane.charges()), (Some(2.0), 1), "another target fills; Arcane Flow waits 3 s");
    c.tick(3.05, true, &mods(), &data);
    c.on_reaction(Reaction::Conduct, 8, ElemSet::one(Element::Thunder), &mods(), &data);
    assert_eq!((c.resonance(Element::Fire), c.arcane.charges()), (Some(3.0), 2), "a hit of an unattuned element fills the primary meter");
    let before = c.clone();
    c.on_reaction(Reaction::Fizzle, 9, ElemSet::one(Element::Fire), &mods(), &data);
    assert_eq!(c.arcane.charges(), before.arcane.charges(), "Fizzle gives nothing");
}

#[test]
fn spell_echo_makes_the_next_base_tier_free() {
    let data = d();
    let mut l = DonLearned::start();
    l.apply(DonStep::ElementDlc(Element::Fire)); // (DON-PROG) up to -za: the DLC boss
    let m = DonMods { spell_echo: true, ..mods() };
    let mut c = DonCore::default();
    attune_now(&mut c, Element::Fire, &l, &m, &data);
    fill(&mut c, Element::Fire, &l, &data);
    let za = c.check_cast(SpellRef::Line1(Element::Fire, 4), &l, MAX_MP, MAX_MP, false, &data).unwrap();
    c.commit_cast(&za, &m, &data);
    let fire = c.check_cast(SpellRef::Line1(Element::Fire, 1), &l, MAX_MP, MAX_MP, false, &data).unwrap();
    assert_eq!((fire.mp, fire.echo), (0, true));
    c.commit_cast(&fire, &m, &data);
    let again = c.check_cast(SpellRef::Line1(Element::Fire, 1), &l, MAX_MP, MAX_MP, false, &data).unwrap();
    assert_eq!(again.mp, 8, "used up");
}

// ---------------------------------------------------------------- statuses and reactions (4.2, 4.5, 6.1)
fn hit(s: &mut TargetStatus, elems: &[Element], applies: &str) -> HitOutcome {
    resolve(s, HitTag::of(ElemSet::of(elems)), &Apply::parse_list(applies), FullFlags::default(), &d().tune, false)
}

#[test]
fn the_reaction_table() {
    let t = d().tune;
    // Wet + Thunder = Conduct (Wet stays)
    let mut s = TargetStatus::new(Size::Small);
    hit(&mut s, &[Element::Water], "wet");
    let o = hit(&mut s, &[Element::Thunder], "-");
    assert_eq!((o.reactions.as_slice(), o.damage_mult, o.conduct_radius), (&[Reaction::Conduct][..], 2.0, Some(1500.0)));
    assert!(s.is_wet());
    // Wet + Blizzard = Flash Freeze
    let o = hit(&mut s, &[Element::Blizzard], "chill:1");
    assert_eq!(o.reactions, [Reaction::FlashFreeze]);
    assert!(s.is_frozen() && !s.is_wet() && (s.frozen - 2.0).abs() < 1e-6);
    // Frozen + Aero = Shatter
    let o = hit(&mut s, &[Element::Aero], "-");
    assert_eq!((o.reactions.as_slice(), o.damage_mult), (&[Reaction::Shatter][..], 3.0));
    assert!(!s.is_frozen());
    // Chill + Fire = Steam Burst + Blind
    let mut s = TargetStatus::new(Size::Medium);
    hit(&mut s, &[Element::Blizzard], "chill:2");
    let o = hit(&mut s, &[Element::Fire], "burn");
    assert_eq!((o.reactions.as_slice(), o.damage_mult), (&[Reaction::SteamBurst][..], 1.75));
    assert!(s.is_blind() && !s.is_chilled() && s.is_burned());
    // Burn + Aero = Wildfire
    let o = hit(&mut s, &[Element::Aero], "-");
    assert_eq!((o.reactions.as_slice(), o.wildfire_radius), (&[Reaction::Wildfire][..], Some(600.0)));
    // Wet + Fire = Fizzle: Wet gone, weak, no Burn
    let mut s = TargetStatus::new(Size::Small);
    hit(&mut s, &[Element::Water], "wet");
    let o = hit(&mut s, &[Element::Fire], "burn");
    assert_eq!((o.reactions.as_slice(), o.damage_mult), (&[Reaction::Fizzle][..], 0.5));
    assert!(!s.is_wet() && !s.is_burned());
    // Scald (Fire + Water) never Fizzles; Plasma (Fire + Thunder) on Wet Conducts
    let mut s = TargetStatus::new(Size::Small);
    hit(&mut s, &[Element::Water], "wet");
    assert!(hit(&mut s, &[Element::Fire, Element::Water], "burn,blind").reactions.is_empty());
    assert!(s.is_burned() && s.is_wet());
    assert_eq!(hit(&mut s, &[Element::Fire, Element::Thunder], "burn").reactions, [Reaction::Conduct]);
    // Revealed + any = Exposed, once per Reveal; Light alone only Reveals
    let mut s = TargetStatus::new(Size::Small);
    hit(&mut s, &[Element::Light], "reveal");
    assert!(hit(&mut s, &[Element::Light], "reveal").reactions.is_empty());
    let o = hit(&mut s, &[], "-");
    assert_eq!((o.reactions.as_slice(), o.damage_mult), (&[Reaction::Exposed][..], t.exposed_mult));
    let o = hit(&mut s, &[Element::Fire], "-");
    assert!(o.reactions.is_empty() && o.damage_mult == t.exposed_mult, "+25 % from everyone for the rest of the Reveal");
    // the Charged Missile shatters whatever its element
    let mut s = TargetStatus::new(Size::Small);
    hit(&mut s, &[Element::Blizzard], "chill:5");
    assert!(s.is_frozen(), "5 Chill stacks = Frozen");
    let o = resolve(&mut s, HitTag { elements: ElemSet::NONE, breaker: true }, &[], FullFlags::default(), &t, true);
    assert_eq!((o.reactions.as_slice(), o.damage_mult), (&[Reaction::Shatter][..], 1.0 + 2.0 * 1.3), "Reaction Plus: +30 % on the bonus");
}

#[test]
fn frozen_rules_bosses_and_full_power_lengths() {
    let t = d().tune;
    let mut ev = Vec::new();
    // full-power Blizzard: 4 s; full-power Water: Wet x2
    let mut s = TargetStatus::new(Size::Small);
    s.apply(Apply::Frozen, FullFlags { blizzard: true, ..FullFlags::default() }, &t, &mut ev);
    assert_eq!(s.frozen, 4.0);
    s.apply(Apply::Wet, FullFlags { water: true, ..FullFlags::default() }, &t, &mut ev);
    assert_eq!(s.wet, 16.0);
    // other hits shorten the ice
    let before = s.frozen;
    hit(&mut s, &[Element::Water], "-");
    assert!((before - s.frozen - 0.5).abs() < 1e-6);
    // thaw, then the re-arm: a freeze gives one Chill stack instead
    let mut ev = Vec::new();
    for _ in 0..240 {
        s.tick(1.0 / 60.0, &t, &mut ev);
    }
    assert!(!s.is_frozen() && ev.contains(&StatusEvent::Thawed));
    s.apply(Apply::Frozen, FullFlags::default(), &t, &mut ev);
    assert!(!s.is_frozen() && s.chill == 1);
    // a boss never freezes: 3 Chill stacks and -30 % for 4 s
    let mut b = TargetStatus::new(Size::Boss);
    let mut ev = Vec::new();
    b.apply(Apply::Chill(5), FullFlags::default(), &t, &mut ev);
    assert!(!b.is_frozen() && b.chill == 3 && b.big_slow == 4.0 && ev.contains(&StatusEvent::BigSlowed));
    assert!((b.slow(&t) - 0.3).abs() < 1e-6);
    // Aero missiles launch a small enemy on the 3rd hit; a medium one only at full power
    let mut s = TargetStatus::new(Size::Medium);
    assert!(!(0..3).any(|_| s.aero_missile_hit(false, &t)));
    assert!((0..3).map(|_| s.aero_missile_hit(true, &t)).last() == Some(true));
}

#[test]
fn burn_ticks_are_frame_rate_independent() {
    let t = d().tune;
    let ticks = |dt: f32| {
        let mut s = TargetStatus::new(Size::Small);
        let mut ev = Vec::new();
        s.apply(Apply::Burn, FullFlags::default(), &t, &mut ev);
        let n = (10.0 / dt).round() as usize;
        for _ in 0..n {
            s.tick(dt, &t, &mut ev);
        }
        ev.iter().filter(|e| matches!(e, StatusEvent::BurnTick(_))).count()
    };
    assert_eq!(ticks(1.0 / 30.0), 6);
    assert_eq!(ticks(1.0 / 144.0), 6);
    assert_eq!(ticks(0.25), 6);
}

#[test]
fn the_status_book_keeps_enemies_in_order_and_drops_clear_ones() {
    let t = d().tune;
    let mut b = StatusBook::default();
    let mut ev = Vec::new();
    b.entry(9, Size::Small).apply(Apply::Wet, FullFlags::default(), &t, &mut ev);
    b.entry(3, Size::Large).apply(Apply::Reveal, FullFlags::default(), &t, &mut ev);
    assert_eq!(b.entries.iter().map(|e| e.0).collect::<Vec<_>>(), [3, 9]);
    assert_eq!(b.wet_ids().collect::<Vec<_>>(), [9]);
    for _ in 0..10 {
        b.tick(1.0, &t);
    }
    assert!(b.entries.is_empty());
}

// ---------------------------------------------------------------- abilities, the status hook
#[test]
fn abilities_have_unique_save_names_and_records() {
    let mut names: Vec<String> = DonAbility::ALL.iter().map(|a| a.name()).collect();
    assert_eq!(names[0], "DON_COMBO_PLUS");
    assert!(names.contains(&"DON_COMBO_PLUS_2".to_string()) && names.contains(&"DON_MP_HASTERA".to_string()));
    names.sort();
    names.dedup();
    assert_eq!(names.len(), DonAbility::ALL.len());
    for a in DonAbility::ALL {
        assert_eq!(DonAbility::from_name(&a.name()), Some(a));
    }
    let mut set = DonAbilities::default();
    set.give(DonAbility::DualAttunement);
    set.set_equipped(DonAbility::DualAttunement, true);
    set.give(DonAbility::SwiftAttune);
    assert!(set.mods().dual && !set.mods().swift_attune);
    let mut back = DonAbilities::default();
    for (n, o, m) in set.record() {
        assert!(back.apply_record(&n, o, m));
    }
    assert_eq!(back, set);
    assert!(!back.apply_record("KIT_SPELL_WEAVER", 1, 1), "an X3D name is not ours");
}

#[test]
fn donald_has_the_state_and_goofy_not() {
    let s = crate::FriendStatus::new(crate::Chara::Donald, 0);
    let st = s.donald.as_ref().expect("Donald's state");
    assert_eq!(st.learned, DonLearned::start());
    assert!(crate::FriendStatus::new(crate::Chara::Goofy, 0).donald.is_none());
    assert_eq!(data().tune, DonData::sheet().tune);
}

// ---------------------------------------------------------------- the party's side (hooks.rs)
#[test]
fn the_party_step_pays_refunds_and_a_hit_fills_the_casters_meter() {
    let data = d();
    let mut party = crate::Party::new(crate::PartyConfig::default());
    let id = party.spawn(crate::Chara::Donald, glam::Vec3::ZERO, 0.0);
    assert!(crate::Party::new(crate::PartyConfig::test_level99()).spawn(crate::Chara::Donald, glam::Vec3::ZERO, 0.0) == crate::MemberId(0));
    party.set_in_battle(true);
    let l = DonLearned::start();
    let (max_mp, mp0) = {
        let s = party.status_mut(id).unwrap();
        s.mp = 10;
        let st = s.donald.as_mut().unwrap();
        attune_now(&mut st.core, Element::Water, &l, &mods(), &data);
        fill(&mut st.core, Element::Water, &l, &data);
        (s.max_mp(), s.mp)
    };
    super::hooks::step(&mut party, 1.0 / 60.0);
    let s = party.status(id).unwrap();
    assert_eq!(s.mp, (mp0 + 3 * mp_of_pct(max_mp, 2.0, 1.0)).min(max_mp), "three pips: 3 x 2 % of max MP");
    assert!(party.don.events.iter().any(|(m, e)| *m == id && *e == DonEvent::FullPower { element: Element::Water }));
    // a Wet enemy, then the caster's Thunder... Donald is at full Water: the Wet lasts twice as long
    let o = party.don_hit(Some(id), 42, Size::Small, HitTag::of(ElemSet::one(Element::Water)), &[Apply::Wet]);
    assert!(o.reactions.is_empty());
    assert_eq!(party.don.status.get(42).unwrap().wet, 16.0);
    // Sora's Thunder (no caster) conducts too, but fills nobody's meter
    let o = party.don_hit(None, 42, Size::Small, HitTag::of(ElemSet::one(Element::Thunder)), &[]);
    assert_eq!(o.reactions, [Reaction::Conduct]);
    let charges = party.status(id).unwrap().donald.as_ref().unwrap().core.arcane.charges();
    assert_eq!(charges, 0);
    party.don.status.entry(43, Size::Small).reveal = 5.0;
    let o = party.don_hit(Some(id), 43, Size::Small, HitTag::default(), &[]);
    assert_eq!(o.reactions, [Reaction::Exposed]);
    assert_eq!(party.status(id).unwrap().donald.as_ref().unwrap().core.arcane.charges(), 1, "the caster's reaction: +1 Arcane Flow");
    assert_eq!(party.don_damage_taken(43), data.tune.exposed_mult);
    party.don_forget(43);
    assert_eq!(party.don_damage_taken(43), 1.0);
}
