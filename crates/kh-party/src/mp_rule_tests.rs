//! (THE USER 2026-11-02: "let Goofy use his last MP and go into MP reserve, make sure Donald and his spells (including
//! Zettaflare) have this as well. Zettaflare should instantly put Donald into MP reserve") The party's MP rule: the rule
//! itself, Goofy's Arts and Taunt, Donald's spells, Zettaflare, a member with no overhaul state of his own (any later
//! character), the AI's choices and the HUD's charge value.

use super::*;
use crate::donald::cast::CastTarget;
use crate::donald::{ArcaneLine, DonCmd, DonLearned, DonaldState, Element, SpellRef};
use crate::host::PartyHost;
use crate::ids::{Chara, MemberId};
use crate::pad::goofy_arts::{self as arts, Art};
use crate::pad::PadFrame;
use crate::{Party, PartyConfig, SoraView};
use glam::{Vec2, Vec3};
use kh_core::command::MenuEvent;
use kh_core::host::{HitRecord, HitResult, HurtShape, TargetDefense, TargetFlags, TargetId, TargetInfo, Targets};
use kh_core::input::{PadButton, PadButtons, PadState};
use kh_core::test_world::TestWorld;
use kh_core::Tuning;

const DT: f32 = 1.0 / 60.0;
const LB: PadButton = PadButton::LeftShoulder;
const A: PadButton = PadButton::FaceBottom;
const B: PadButton = PadButton::FaceRight;
const X: PadButton = PadButton::FaceLeft;
const ENEMY: Vec3 = Vec3::new(400.0, 0.0, 0.0);
const T: TargetId = TargetId(1);

// ------------------------------------------------------------------------------------------------ the rule itself

#[test]
fn any_mp_above_zero_pays_and_a_dearer_price_empties_it_into_mp_charge() {
    let mut s = FriendStatus::new(Chara::Goofy, 0);
    s.mp = 8;
    assert_eq!(gate(&s), Gate::Ready);
    assert!(may_pay(&s, 15) && drains(8, 15));
    assert_eq!(spend(&mut s, 15), Some(Spent { mp: 8, charge_started: true }), "8 MP buys a 15-MP move");
    assert_eq!(s.mp, 0);
    assert_eq!(s.mp_charge_left, charge_time(&s), "MP Charge at once, of his own row");
    assert_eq!(charge_time(&s), crate::tables::cparam(Chara::Goofy, "mp_charge_time"));
    assert_eq!(gate(&s), Gate::Charging);
    assert_eq!(spend(&mut s, 3), None, "nothing that costs MP in MP Charge");
    assert_eq!(spend(&mut s, 0), Some(Spent::default()), "a free move is never refused by MP");
    // landing exactly on 0 starts it as well (his next move needs MP above 0)
    let mut s = FriendStatus::new(Chara::Goofy, 0);
    s.mp = 15;
    assert_eq!(spend(&mut s, 15), Some(Spent { mp: 15, charge_started: true }));
    assert!(s.mp_charge_left > 0.0);
    // a price below what he has: just paid
    let mut s = FriendStatus::new(Chara::Goofy, 0);
    s.mp = 40;
    assert_eq!(spend(&mut s, 15), Some(Spent { mp: 15, charge_started: false }));
    assert_eq!((s.mp, s.mp_charge_left), (25, 0.0));
    // 0 MP with no charge running (a fresh status a host emptied): refused, as the error sound's rule wants
    s.mp = 0;
    assert_eq!(gate(&s), Gate::Empty);
    assert!(!may_pay(&s, 1) && spend(&mut s, 1).is_none());
}

#[test]
fn the_charge_runs_its_length_then_the_bar_is_full() {
    let mut s = FriendStatus::new(Chara::Goofy, 0);
    let full = s.max_mp();
    s.mp = 5;
    spend(&mut s, 20);
    let t = charge_time(&s);
    let steps = (t / DT).ceil() as usize;
    for _ in 0..steps - 2 {
        s.tick(DT);
    }
    assert_eq!(s.mp, 0, "still charging");
    assert!(charge_progress(&s) > 0.95 && charge_progress(&s) < 1.0);
    for _ in 0..4 {
        s.tick(DT);
    }
    assert_eq!((s.mp, s.mp_charge_left), (full, 0.0));
    assert!(can_spend(&s));
}

#[test]
fn a_later_character_with_no_overhaul_state_gets_the_same_rule_from_the_helper() {
    // the helper reads only the status: a member without Goofy's or Donald's own state (any character added later) runs it
    // from his own `mp_charge_time` row
    let mut s = FriendStatus::new(Chara::Goofy, 0);
    s.goofy = None;
    s.guard = None;
    s.mp = 3;
    assert_eq!(spend_all(&mut s), Some(Spent { mp: 3, charge_started: true }));
    assert_eq!((s.mp, s.mp_charge_left), (0, crate::tables::cparam(Chara::Goofy, "mp_charge_time")));
    // KH3's friend ability gate (`FriendStatus::pay`, the KH3 tree's abilities) is the same rule
    let mut d = FriendStatus::new(Chara::Donald, 0);
    d.abilities.set(kh_core::enums::ETresAbilityKind::DonaldFire, 1, 1);
    d.mp = 3;
    assert!(d.can_pay("DONALD_FIRE") && d.pay("DONALD_FIRE"), "3 MP buys a 10-MP KH3 Fire");
    assert_eq!((d.mp, d.mp_charge_left), (0, charge_time(&d)));
    assert!(!d.can_pay("DONALD_FIRE") && !d.pay("DONALD_FIRE"));
    // Donald's length is his R7 one, not the friends' 30 s
    assert_eq!(charge_time(&d), crate::pad::donald::mp_charge_time(&d));
}

#[test]
fn the_hud_value_is_kh3s_mp_still_to_refill() {
    assert_eq!(hud_charge_value(false, 0.3, 100), 0.0);
    assert_eq!(hud_charge_value(true, 0.0, 100), 100.0, "a charge just begun: the whole bar to refill");
    assert!((hud_charge_value(true, 0.75, 80) - 20.0).abs() < 1e-4);
    // from a member's view, as the party host hands it to the status HUD
    let mut s = FriendStatus::new(Chara::Donald, 0);
    s.mp = 2;
    spend(&mut s, 30);
    assert_eq!(charge_progress(&s), 0.0, "his own length: the gauge starts empty (not part-filled by the friends' 30 s)");
    assert!((hud_charge_value(true, charge_progress(&s), s.max_mp()) - s.max_mp() as f32).abs() < 1e-3);
}

// ------------------------------------------------------------------------------------------------ a pad member's rig

struct Dummies(Vec<(TargetId, Vec3)>);

impl Dummies {
    fn info_of(&self, (id, p): (TargetId, Vec3)) -> TargetInfo {
        TargetInfo { id, position: p, center: p + Vec3::Z * 90.0, velocity: Vec3::ZERO, yaw: 180.0, radius: 50.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 }
    }
}

impl Targets for Dummies {
    fn enumerate(&self, out: &mut Vec<TargetInfo>) {
        for &t in &self.0 {
            out.push(self.info_of(t));
        }
    }
    fn info(&self, id: TargetId) -> Option<TargetInfo> {
        self.0.iter().find(|t| t.0 == id).map(|&t| self.info_of(t))
    }
    fn hurt_shapes(&self, id: TargetId, out: &mut Vec<HurtShape>) {
        if let Some(i) = self.info(id) {
            out.push(HurtShape { a: i.position + Vec3::Z * 30.0, b: i.position + Vec3::Z * 150.0, radius: 60.0, part: 0 });
        }
    }
    fn apply_hit(&mut self, hit: &HitRecord) -> HitResult {
        HitResult { accepted: true, damage_dealt: hit.damage, hp_after: 1.0e6, killed: false, guarded: false }
    }
    fn defense(&self, _id: TargetId) -> Option<TargetDefense> {
        Some(TargetDefense { defense: 10, max_hp: 1_000_000, hp_percent: 100.0, enemy_kind: true, damage_min: 0, damage_max: 0, resist: [0; 8], bio_type: 0 })
    }
    // (AUDIT-B) no bones here: a rider attached to a hit follows the target's origin and yaw
    fn hit_bone(&self, _id: kh_core::TargetId, _at: kh_core::Vec3) -> Option<kh_core::host::TargetBone> {
        None
    }
    fn bone_pose(&self, _id: kh_core::TargetId, _bone: kh_core::host::TargetBone) -> Option<kh_core::host::BonePose> {
        None
    }
}

struct Rig {
    world: TestWorld,
    dummies: Dummies,
    audio: kh_core::host::audio::NullAudio,
    effects: kh_core::host::effects::NullEffects,
    party: Party,
    sora: SoraView,
    id: MemberId,
    held: PadButtons,
}

impl Rig {
    fn new(chara: Chara) -> Rig {
        let mut party = Party::new(PartyConfig::test_level99());
        let id = party.spawn(chara, Vec3::ZERO, 0.0);
        party.set_pad(id, Some(Tuning::USER));
        if chara == Chara::Donald {
            let st = party.status_mut(id).unwrap().donald.as_mut().unwrap();
            st.learned = DonLearned::all();
            st.abilities = Default::default();
        }
        let mut rig = Rig { world: TestWorld::floor(), dummies: Dummies(vec![(T, ENEMY)]), audio: Default::default(), effects: Default::default(), party, sora: SoraView::default(), id, held: PadButtons::NONE };
        rig.sora.position = Vec3::new(-3000.0, 0.0, 0.0);
        rig.party.set_in_battle(true);
        if chara == Chara::Goofy {
            rig.status().growth.ranks = crate::goofy::RankBank::default(); // rank 1: Charge costs its row's 15
        }
        rig.idle(10);
        rig
    }

    fn status(&mut self) -> &mut FriendStatus {
        self.party.status_mut(self.id).unwrap()
    }

    fn s(&self) -> &FriendStatus {
        self.party.status(self.id).unwrap()
    }

    fn step(&mut self, held: &[PadButton]) {
        let mut now = PadButtons::NONE;
        for b in held {
            now.set(*b, true);
        }
        let pad = PadState { held: now, pressed: PadButtons(now.0 & !self.held.0), released: PadButtons(self.held.0 & !now.0) };
        self.held = now;
        self.party.set_pad_frame(self.id, PadFrame { stick: Vec2::ZERO, camera_yaw: 0.0, pad });
        let mut host = PartyHost { world: &self.world, targets: &mut self.dummies, audio: &mut self.audio, effects: &mut self.effects };
        self.party.step(DT, &self.sora, &mut host);
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.step(&[]);
        }
    }

    /// LB, then the button with it (the press step's menu events are left to read), then both let go.
    fn lb(&mut self, b: PadButton) -> bool {
        self.step(&[LB]);
        self.step(&[LB, b]);
        let refused = self.party.pad_menu(self.id).unwrap().events.contains(&MenuEvent::Refused);
        self.step(&[]);
        refused
    }

    fn idle_until_free(&mut self) {
        for _ in 0..600 {
            if self.party.member(self.id).unwrap().state == crate::states::NpcStateId::Idle {
                return;
            }
            self.step(&[]);
        }
        panic!("he never came back to Idle: {:?}", self.party.member(self.id).unwrap().state);
    }
}

// ------------------------------------------------------------------------------------------------ Goofy

#[test]
fn goofy_with_8_mp_uses_a_15_mp_art_and_goes_into_mp_charge_then_is_refused() {
    let mut rig = Rig::new(Chara::Goofy);
    rig.party.set_goofy_arts([Art::Charge.id(), Art::Tornado.id()]);
    assert_eq!(arts::mp(Art::Charge, 1), 15, "the row the user's numbers set");
    rig.status().mp = 8;
    let refused = rig.lb(A);
    assert!(!refused, "no error sound: he had MP");
    assert!(arts::cooldown_left(rig.s(), Art::Charge) > 0.0, "Charge started");
    assert_eq!(rig.s().mp, 0, "his last 8 MP went into it");
    assert!(rig.s().mp_charge_left > 0.0, "MP Charge at once");
    rig.idle_until_free();
    // the other Art is not cooling down: MP Charge alone refuses it, with the refused sound
    assert_eq!(arts::cooldown_left(rig.s(), Art::Tornado), 0.0);
    assert!(rig.lb(B), "the refused SE: MP Charge");
    assert_eq!(arts::cooldown_left(rig.s(), Art::Tornado), 0.0, "Tornado did not start");
    let row = arts::panel_row(rig.s(), Art::Tornado.id());
    assert!(!row.enabled, "greyed in MP Charge");
}

#[test]
fn goofys_taunt_keeps_its_small_price_under_the_same_rule() {
    let mut rig = Rig::new(Chara::Goofy);
    assert_eq!(arts::mp(Art::Taunt, 1), 3);
    rig.status().mp = 10;
    assert!(!rig.lb(X));
    assert_eq!((rig.s().mp, rig.s().mp_charge_left), (7, 0.0), "Taunt pays its 3");
    rig.idle_until_free();
    rig.status().mp = 1;
    assert!(!rig.lb(X));
    assert_eq!(rig.s().mp, 0, "1 MP buys the 3-MP Taunt");
    assert!(rig.s().mp_charge_left > 0.0);
}

#[test]
fn goofys_panel_is_short_of_mp_only_at_zero_or_in_mp_charge() {
    let mut rig = Rig::new(Chara::Goofy);
    rig.status().mp = 1;
    assert!(!arts::panel_row(rig.s(), Art::Bash.id()).mp_short, "1 MP pays any Art now");
    rig.status().mp = 0;
    assert!(arts::panel_row(rig.s(), Art::Bash.id()).mp_short);
}

#[test]
fn ai_goofy_may_spend_his_last_mp_but_keeps_it_for_interpose_when_a_friend_is_low() {
    let mut rig = Rig::new(Chara::Goofy);
    rig.party.set_goofy_arts([Art::Charge.id(), Art::Tornado.id()]);
    let both = [Some(crate::goofy::GoofyMove::GoofyCharge), Some(crate::goofy::GoofyMove::GoofyTornado)];
    rig.status().mp = 8;
    assert_eq!(arts::ai_art_moves(rig.s(), false), both, "nobody low: the last MP may buy an Art");
    assert!(rig.s().gof_rank(crate::goofy::GoofyMove::Interpose) > 0, "the test level knows Interpose");
    assert_eq!(arts::ai_art_moves(rig.s(), true), [None, None], "a friend low: both would empty him, that MP is Interpose's");
    rig.status().mp = 80;
    assert_eq!(arts::ai_art_moves(rig.s(), true), both, "with MP to spare he still uses them");
    // Interpose itself may take the last of it
    rig.party.set_goofy_arts([Art::Interpose.id(), Art::Charge.id()]);
    rig.status().mp = 8;
    assert_eq!(arts::ai_art_moves(rig.s(), true), [Some(crate::goofy::GoofyMove::Interpose), None]);
    // MP Charge: nothing is asked for (no press the kit would refuse)
    rig.status().mp = 0;
    rig.status().mp_charge_left = 5.0;
    assert_eq!(arts::ai_art_moves(rig.s(), false), [None, None]);
}

// ------------------------------------------------------------------------------------------------ Donald

#[test]
fn donald_with_little_mp_casts_a_dearer_spell_and_goes_into_mp_charge_then_is_refused() {
    let mut rig = Rig::new(Chara::Donald);
    let spell = SpellRef::Line1(Element::Fire, 1);
    let price = {
        let s = rig.s();
        let st = s.donald.as_ref().unwrap();
        st.core.check_cast(spell, &st.learned, s.max_mp(), s.max_mp(), false, &crate::donald::data()).unwrap().mp
    };
    assert!(price > 3, "Fire costs more than he will have ({price})");
    rig.status().mp = 3;
    let p = rig.party.member_mut(rig.id).unwrap();
    assert!(crate::pad::donald::wants_cast(p, DonCmd::Spell(spell), CastTarget::Enemy(T)), "3 MP casts a {price}-MP Fire");
    rig.step(&[]);
    assert!(rig.party.member(rig.id).unwrap().don_cast.run.is_some(), "the cast began");
    assert_eq!(rig.s().mp, 0);
    assert!(rig.s().mp_charge_left > 0.0 && rig.s().mp_charge_left <= charge_time(rig.s()) + 1e-3, "MP Charge at once, his R7 length");
    for _ in 0..240 {
        rig.step(&[]);
    }
    let p = rig.party.member_mut(rig.id).unwrap();
    assert!(p.status.mp_charge_left > 0.0);
    assert!(!crate::pad::donald::wants_cast(p, DonCmd::Spell(spell), CastTarget::Enemy(T)), "refused in MP Charge");
    // his menu: the MP-short reason only in MP Charge / at 0 MP
    let s = rig.s();
    let st = s.donald.as_ref().unwrap();
    assert!(st.core.check_cast(spell, &st.learned, 1, s.max_mp(), false, &crate::donald::data()).is_ok(), "1 MP: castable");
    assert_eq!(st.core.check_cast(spell, &st.learned, 0, s.max_mp(), false, &crate::donald::data()).unwrap_err(), crate::donald::Refusal::NotEnoughMp { need: price });
}

/// Zettaflare's own gates met (every learned element at Resonance 3 in this fight, four Arcane Flow charges).
fn zetta_ready(rig: &mut Rig) {
    let d = crate::donald::data();
    let st = rig.status().donald.as_mut().unwrap();
    st.core.fight_full = st.learned.elements();
    for _ in 0..4 {
        st.core.on_perfect_block(&Default::default(), &d);
    }
    assert!(st.core.zettaflare_ready(&st.learned, &d));
}

#[test]
fn zettaflare_with_mp_left_empties_it_and_starts_mp_charge_at_once() {
    for mp in [1, 37] {
        let mut rig = Rig::new(Chara::Donald);
        zetta_ready(&mut rig);
        rig.status().mp = mp;
        let zetta = SpellRef::Arcane(ArcaneLine::Zettaflare, 1);
        let plan = {
            let s = rig.s();
            let st = s.donald.as_ref().unwrap();
            assert!(st.core.y_offers(&st.learned, mp, s.max_mp(), &crate::donald::data()).iter().any(|o| o.spell == zetta && o.affordable), "{mp} MP: offered");
            st.core.check_cast(zetta, &st.learned, mp, s.max_mp(), false, &crate::donald::data()).unwrap()
        };
        assert!(mp < rig.s().max_mp(), "no full bar needed any more");
        let mut host = PartyHost { world: &rig.world, targets: &mut rig.dummies, audio: &mut rig.audio, effects: &mut rig.effects };
        let p = rig.party.member_mut(rig.id).unwrap();
        assert!(crate::donald::zettaflare::begin(p, &plan, CastTarget::Enemy(T), &mut host), "{mp} MP: it begins");
        assert_eq!(p.status.mp, 0, "all of it");
        assert!(p.status.mp_charge_left > 0.0, "MP Charge at its start, not at its end");
        assert!(p.status.donald.as_ref().unwrap().core.zetta_slow, "and its slower refill");
    }
    // 0 MP / MP Charge: refused
    let mut rig = Rig::new(Chara::Donald);
    zetta_ready(&mut rig);
    let s = rig.s();
    let st = s.donald.as_ref().unwrap();
    assert!(st.core.check_cast(SpellRef::Arcane(ArcaneLine::Zettaflare, 1), &st.learned, 0, s.max_mp(), false, &crate::donald::data()).is_err());
}

// ------------------------------------------------------------------------------------------------ AI Donald

/// Everything learned, attuned to Fire (so an attack is his next choice, not an attunement).
fn ai_me() -> DonaldState {
    let mut d = DonaldState::new();
    d.learned = DonLearned::all();
    d.core.attune.slots[0] = Some(crate::donald::Slot { element: Element::Fire, res: 0.0, period: 1, ult_used: false, full_at: Some(0.0) });
    d
}

fn healthy_ally() -> crate::donald::ai::AiAlly {
    crate::donald::ai::AiAlly { member: MemberId(1), pos: Vec3::new(0.0, 300.0, 0.0), hp: 100, hp_max: 100, ko: false, melee: false, attacked: false, barrier_left: 0.0, orbs: 0, orbs_left: 0.0, orbs_since: f32::MAX, thunder: false }
}

fn input<'a>(me: &'a DonaldState, enemies: &'a [crate::donald::ai::AiEnemy], allies: &'a [crate::donald::ai::AiAlly], hp: i32, mp: i32) -> crate::donald::ai::AiInput<'a> {
    crate::donald::ai::AiInput { me, id: MemberId(0), pos: Vec3::ZERO, hp, hp_max: 100, mp, mp_max: 100, mp_charging: false, in_battle: true, barrier_left: 30.0, orbs: 3, orbs_left: 0.0, orbs_since: f32::MAX, enemies, allies, refused: &[] }
}

fn enemy(x: f32) -> crate::donald::ai::AiEnemy {
    crate::donald::ai::AiEnemy { id: 9, pos: Vec3::new(x, 0.0, 0.0), weak: crate::donald::ElemSet::NONE, resist: crate::donald::ElemSet::NONE, wet: false, chilled: false, frozen: false, burned: false, big: false }
}

#[test]
fn ai_donald_never_drains_himself_on_an_attack_but_a_heal_may_take_his_last_mp() {
    use crate::donald::ai::{decide, AiAsk, AiChoice};
    let me = ai_me();
    let far = [enemy(1500.0)];
    // an attack is paid in full, at every MP he may attack with
    let mut attacks = 0;
    for mp in 1..=100 {
        if let Some(AiChoice::Cast { plan, .. }) = decide(&input(&me, &far, &[], 100, mp), AiAsk::Attack(Some(9))) {
            assert!(!drains(mp, plan.mp), "{mp} MP: {:?} for {} would leave him in MP Charge", plan.spell, plan.mp);
            attacks += 1;
        }
    }
    assert!(attacks > 50, "he does attack ({attacks})");
    // a heal takes the last of it: 2 MP buys the Cure line, into MP Charge
    match decide(&input(&me, &far, &[], 60, 2), AiAsk::Cure(MemberId(0))) {
        Some(AiChoice::Cast { plan, .. }) => {
            assert!(matches!(plan.spell, SpellRef::Line1(Element::Light, _)));
            assert!(drains(2, plan.mp), "the heal costs more than his 2 MP ({})", plan.mp);
        }
        other => panic!("2 MP: a heal ({other:?})"),
    }
}

#[test]
fn ai_donald_keeps_his_last_mp_for_a_heal_when_someone_is_low() {
    use crate::donald::ai::{decide, AiAsk, AiChoice};
    let me = ai_me();
    let close = [enemy(100.0)]; // an enemy closing in: the barrier on himself (rule 2)
    let barrier = me.learned.line2_spell(Element::Aero).expect("the test Donald knows Aero Barrier");
    let price = me.core.check_cast(barrier, &me.learned, 100, 100, false, &crate::donald::data()).unwrap().mp;
    let few = (price - 1).max(1);
    let is_barrier = |c: Option<AiChoice>| matches!(c, Some(AiChoice::Cast { plan, .. }) if plan.spell == barrier);
    let allies = [healthy_ally()]; // the party as a whole is not low (no Light attunement first)
    let mut i = input(&me, &close, &allies, 100, few);
    i.barrier_left = 0.0;
    i.orbs = 0;
    assert!(is_barrier(decide(&i, AiAsk::Attack(Some(9)))), "nobody low: protection may take his last {few} MP");
    let mut low = input(&me, &close, &allies, 40, few);
    low.barrier_left = 0.0;
    low.orbs = 0;
    assert!(!is_barrier(decide(&low, AiAsk::Attack(Some(9)))), "he is low: that MP is the heal's");
    assert!(matches!(decide(&low, AiAsk::Cure(MemberId(0))), Some(AiChoice::Cast { plan, .. }) if matches!(plan.spell, SpellRef::Line1(Element::Light, _))), "and the heal gets it");
}
