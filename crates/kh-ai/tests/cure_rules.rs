//! The cure / item decision as transcribed from 0x141264E30 and its helpers (crate::cure).

mod common;
use common::*;
use glam::Vec3;
use kh_ai::cure::*;
use kh_ai::*;

fn stat(actor: ActorRef, hp: i32, hp_max: i32) -> TargetStat {
    TargetStat { actor, position: Vec3::ZERO, hp, hp_max, mp: Some((100, 100)), mp_charging: false, mp_charge_left: f32::INFINITY, dying: false, bad_status: false, is_player: actor == ActorRef::Player, focus: None }
}

fn curer<'a>(pack: &'a AiPack, style: CureStyle, in_battle: bool, can_cure: bool, items: &[&str]) -> Curer<'a> {
    let mut commands: Vec<String> = items.iter().map(|i| format!("TRES_CMD_KIND_{i}")).collect();
    if can_cure {
        commands.push(CMD_CURE.into());
    }
    Curer { pack, style, in_battle, can_cure, cure_amount: if can_cure { 120 + 30 } else { 0 }, commands, item_up: 0, focus_break: false }
}

/// 0x14126AC70: OFTEN < 50 % (35 % in battle), PINCH <= 25 % or dying, ALWAYS < 80 % (50 % in battle).
#[test]
fn hp_thresholds_per_style() {
    let pack = pack();
    let t = |hp| stat(ActorRef::Player, hp, 100);
    for (style, out_thr, in_thr) in [(CureStyle::Often, 50, 35), (CureStyle::Always, 80, 50)] {
        let o = curer(&pack, style, false, true, &[]);
        assert!(o.hp_flag(&t(out_thr - 1)) && !o.hp_flag(&t(out_thr)), "{style:?} out of battle");
        let b = curer(&pack, style, true, true, &[]);
        assert!(b.hp_flag(&t(in_thr - 1)) && !b.hp_flag(&t(in_thr)), "{style:?} in battle");
    }
    let p = curer(&pack, CureStyle::Pinch, true, true, &[]);
    assert!(p.hp_flag(&t(25)) && !p.hp_flag(&t(26)));
    let mut ko = t(0);
    ko.dying = true;
    assert!(p.hp_flag(&ko));
    // integer pinch rule: 220 * 25 / 100 = 55
    let g = stat(ActorRef::Member(1), 55, 220);
    assert!(p.pinch_ok(&g));
}

trait PinchOk {
    fn pinch_ok(&self, t: &TargetStat) -> bool;
}
impl PinchOk for Curer<'_> {
    fn pinch_ok(&self, t: &TargetStat) -> bool {
        self.hp_flag(t) && t.pinch() && t.hp * 100 / t.hp_max <= 25
    }
}

/// Out of battle only Cure answers an HP need; items need battle (the command list is emptied).
#[test]
fn items_only_in_battle_cure_first() {
    let pack = pack();
    let low = [stat(ActorRef::Player, 20, 100)];
    let goofy_out = curer(&pack, CureStyle::Pinch, false, false, &["POTION", "POTION"]);
    assert_eq!(goofy_out.decide(&low), None);
    let goofy_in = curer(&pack, CureStyle::Pinch, true, false, &["POTION", "POTION"]);
    let (i, cmd, score, flags) = goofy_in.decide(&low).expect("a potion in battle");
    assert_eq!((i, cmd.as_str(), flags[0]), (0, "TRES_CMD_KIND_POTION", true));
    // HP evaluation 3 -> score 30 + command index 0
    assert_eq!(score, 30);
    // Donald with Cure and items: Cure first (style PINCH), out of battle too
    let donald_out = curer(&pack, CureStyle::Pinch, false, true, &["POTION", "ETHER"]);
    assert_eq!(donald_out.decide(&low).map(|d| d.1), Some(CMD_CURE.to_string()));
}

/// ALWAYS in battle: no Cure-first; the fitting item / Cure by the missing HP (0x14125F380).
#[test]
fn always_in_battle_fits_the_missing_hp() {
    let pack = pack();
    let d = curer(&pack, CureStyle::Always, true, true, &["POTION", "HIPOTION"]);
    // 40 % of 400 HP: 240 missing, Cure 150 does not cover (HP% < 50 but 240 > 150); Hi-Potion
    // 160 does not cover either and is the larger -> Hi-Potion
    let t = stat(ActorRef::Player, 160, 400);
    assert_eq!(d.hp_pick(&t), "TRES_CMD_KIND_HIPOTION");
    // 45 % of 200: 110 missing <= Cure 150 -> Cure
    let t2 = stat(ActorRef::Player, 90, 200);
    assert_eq!(d.hp_pick(&t2), CMD_CURE);
    // 60 missing at 70 %: Potion 80 covers with the smallest overshoot (Cure needs HP% < 50)
    let t3 = stat(ActorRef::Player, 140, 200);
    assert_eq!(d.hp_pick(&t3), "TRES_CMD_KIND_POTION");
    // but in battle ALWAYS asks only below 50 %: no need at 70 %
    assert_eq!(d.decide(&[t3]), None);
    // the decision takes the command that fits its best target: Hi-Potion at 40 %
    let pick = d.decide(&[t]).expect("something");
    assert_eq!((pick.1.as_str(), pick.2), ("TRES_CMD_KIND_HIPOTION", 1 + 30));
}

/// MP (0x14126AF20): OFTEN MP <= 24 %, an Ether in battle.
#[test]
fn ether_on_low_mp() {
    let pack = pack();
    let mut t = stat(ActorRef::Member(0), 200, 200);
    t.mp = Some((20, 100));
    let d = curer(&pack, CureStyle::Often, true, false, &["ETHER"]);
    assert!(d.mp_flag(&t));
    assert_eq!(d.decide(&[t]).map(|x| x.1), Some("TRES_CMD_KIND_ETHER".to_string()));
    t.mp = Some((25, 100));
    assert!(!d.mp_flag(&t));
    // recharging with less than 8 s left: no
    t.mp = Some((0, 100));
    t.mp_charging = true;
    t.mp_charge_left = 5.0;
    assert!(!d.mp_flag(&t));
    t.mp_charge_left = 9.0;
    assert!(d.mp_flag(&t));
}

/// The MetaAI tick (0x14123E440): records every 2 s; the near curer wins; the record survives
/// while needed and drops once healthy.
#[test]
fn meta_ai_period_and_record() {
    let mut p = Party::new();
    p.world.add_dummy(7, Vec3::new(700.0, 0.0, 0.0));
    p.set_battle(true);
    p.world.player.hp = 20;
    // no record before the 2 s period ran out
    p.run(1.9);
    assert!(p.shared.cure_info.is_none() || p.pawns[0].log.iter().any(|c| matches!(c, AiCommand::Cure { .. })));
    p.run(0.3);
    let rec_or_cast = p.shared.cure_info.is_some() || p.pawns[0].log.iter().any(|c| matches!(c, AiCommand::Cure { .. }));
    assert!(rec_or_cast, "no record after 2 s");
    // both friends build a record (Donald: Cure, Goofy: a Potion, Sora is in a pinch, in battle);
    // the MetaAI keeps the pair with the smaller curer-target distance
    if let Some(r) = &p.shared.cure_info {
        println!("record {r:?}");
        assert_eq!(r.target, ActorRef::Player);
        assert!((r.curer, r.command.as_str()) == (0, CMD_CURE) || (r.curer, r.command.as_str()) == (1, "TRES_CMD_KIND_POTION"), "{r:?}");
    }
    // healthy again: a pending record drops (its HP flag no longer holds)
    p.world.player.hp = 100;
    p.run(0.1);
    assert!(p.shared.cure_info.is_none());
}
