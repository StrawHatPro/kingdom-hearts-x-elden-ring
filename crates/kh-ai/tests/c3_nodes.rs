//! C3 (2026-10-15): the node bodies transcribed from the decompiled C. Pure checks on the action
//! definitions' validation (ValidateAction 0x14047FB20 -> CDO slots 125 / 127 / 129) with the
//! game's own definition classes from the pack, plus tree-level checks of the field follow.
mod common;
use common::*;
use glam::Vec3;
use kh_ai::api::{ActorRef, ActorView};
use kh_ai::nodes::validate::{norm, ActionCheck, CheckWorld, Me};

struct W {
    abilities: Vec<&'static str>,
    path: bool,
    los: bool,
}
impl CheckWorld for W {
    fn in_viewport(&self, _p: Vec3) -> bool {
        true
    }
    fn direct_path(&self, _a: Vec3, _b: Vec3) -> bool {
        self.path
    }
    fn line_of_sight(&self, _a: Vec3, _b: Vec3) -> bool {
        self.los
    }
    fn has_ability(&self, k: &str) -> bool {
        self.abilities.contains(&k)
    }
}

fn check(name: &str) -> ActionCheck {
    let pack = pack();
    let d = pack.class(name).unwrap_or_else(|| panic!("{name} in the pack"));
    ActionCheck::from_class(d.native.as_deref().unwrap(), &d.p)
}

fn me() -> Me {
    // Donald's capsule centre 50 cm up, facing +X
    Me { center: Vec3::new(0.0, 0.0, 50.0), half_height: 50.0, yaw: 0.0, swimming: false, bad_status_bound: false }
}

fn target(x: f32, y: f32, z_center: f32, radius: f32, half: f32) -> ActorView {
    ActorView { actor: ActorRef::Enemy(1), position: Vec3::new(x, y, z_center - half), center: Vec3::new(x, y, z_center), radius, half_height: half, alive: true, hostile: true, ..Default::default() }
}

#[test]
fn magic_fire_window_pitch_and_ability() {
    let c = check("n_ex001_Attack4_MagFire");
    // Ranged ctor 0x14123F0D0 + NpcBase 0x14121BBF0 + the asset: 100..5000 cm, yaw 30, pitch 0..45,
    // no direct path, LOS on
    assert!(c.use_min_dist && c.use_max_dist && c.min_dist == 100.0 && c.max_dist == 5000.0);
    assert!(c.ranged && c.ranged_pitch && c.pitch_up == 45.0 && c.pitch_down == 0.0 && c.yaw_tolerance == 30.0);
    assert!(!c.direct_path && c.line_of_sight && c.target_required);
    // (DON-PROG) KH3's tree data as read (the AI engine itself): Donald's abilities are gone in play since the overhaul;
    // the party answers HasAbility:DONALD_* from his overhaul magic (kh-party FriendStatus::don_has_ability)
    assert_eq!(c.ability.as_deref(), Some("DONALD_FIRE"));
    let w = W { abilities: vec!["DONALD_FIRE"], path: true, los: true };
    let m = me();
    // 2D distance minus the target radius: 140 - 50 = 90 < 100 fails, 200 - 50 passes
    assert!(!c.location(&m, Some(&target(140.0, 0.0, 60.0, 50.0, 60.0)), &w));
    assert!(c.location(&m, Some(&target(200.0, 0.0, 60.0, 50.0, 60.0)), &w));
    // LOS blocked fails
    assert!(!c.location(&m, Some(&target(200.0, 0.0, 60.0, 50.0, 60.0)), &W { los: false, ..W { abilities: vec![], path: true, los: true } }));
    // pitch: a target centre below the pawn's centre fails (Down 0), above within 45 passes
    assert!(!c.orientation(&m, Some(&target(500.0, 0.0, 20.0, 50.0, 20.0))));
    assert!(c.orientation(&m, Some(&target(500.0, 0.0, 120.0, 50.0, 60.0))));
    // yaw 30: 40 deg off fails
    let off = Vec3::new(40f32.to_radians().cos(), 40f32.to_radians().sin(), 0.0) * 500.0;
    assert!(!c.orientation(&m, Some(&target(off.x, off.y, 120.0, 50.0, 60.0))));
    // execution: needs a target and the ability equipped
    assert!(c.execution(&m, Some(&target(500.0, 0.0, 60.0, 50.0, 60.0)), &w));
    assert!(!c.execution(&m, None, &w));
    assert!(!c.execution(&m, Some(&target(500.0, 0.0, 60.0, 50.0, 60.0)), &W { abilities: vec![], path: true, los: true }));
}

#[test]
fn melee_height_window_and_no_ability() {
    let c = check("n_ex002_Attack1_Push");
    // Melee ctor 0x14123F050: height window on; the asset 50 / 135, max 230 cm, no direct path
    assert!(c.use_min_height && c.use_max_height && c.min_height == 50.0 && c.max_height == 135.0);
    assert!(!c.use_min_dist && c.use_max_dist && c.max_dist == 230.0 && !c.direct_path);
    let w = W { abilities: vec![], path: false, los: true };
    let m = me();
    // target top relative to the pawn's centre: centre 60, half 60 -> bottom -50, top 70: passes
    assert!(c.location(&m, Some(&target(150.0, 0.0, 60.0, 40.0, 60.0)), &w));
    // a flier whose bottom is 200 above the pawn's centre fails the max height
    assert!(!c.location(&m, Some(&target(150.0, 0.0, 310.0, 40.0, 60.0)), &w));
    // a short target whose top is below +50 fails the min height
    assert!(!c.location(&m, Some(&target(150.0, 0.0, 10.0, 40.0, 20.0)), &w));
    // normal attack: no ability needed
    assert!(c.execution(&m, Some(&target(150.0, 0.0, 60.0, 40.0, 60.0)), &w));
}

#[test]
fn goofy_specials_take_the_ctor_ability_and_direct_path() {
    let t = check("n_ex002_Attack6_Tornado");
    assert_eq!(t.ability.as_deref(), Some("GOOFY_TORNADE"));
    // base ctor 0x14042FD30 direct path on (the asset leaves it)
    assert!(t.direct_path && t.line_of_sight);
    let m = me();
    let near = target(400.0, 0.0, 60.0, 40.0, 60.0);
    assert!(!t.location(&m, Some(&near), &W { abilities: vec![], path: false, los: true }));
    assert!(t.location(&m, Some(&near), &W { abilities: vec![], path: true, los: true }));
    assert!(!t.execution(&m, Some(&near), &W { abilities: vec![], path: true, los: true }));
    assert!(t.execution(&m, Some(&near), &W { abilities: vec!["GOOFY_TORNADE"], path: true, los: true }));
    // swim attacks need the pawn swimming (0x14121E9F0 -> FUN_1405ad820)
    let s = check("n_ex002_Attack5_WaterRush");
    assert!(s.swim_attack);
    assert!(!s.execution(&m, Some(&near), &W { abilities: vec![], path: true, los: true }));
    // the cure: target not required for location / orientation, the ability for execution
    let cure = check("n_ex001_Action2_MagCure");
    assert!(!cure.target_required && cure.location(&m, None, &W { abilities: vec![], path: true, los: true }));
    assert!(!cure.execution(&m, Some(&near), &W { abilities: vec![], path: true, los: true }));
}

#[test]
fn normalised_angles() {
    assert_eq!(norm(190.0), -170.0);
    assert_eq!(norm(-190.0), 170.0);
    assert_eq!(norm(360.0), 0.0);
}

#[test]
fn standing_member_walks_around_the_player() {
    // TimeMotionlessCheck (0x14046EE10) now passes while the member stands: n_Common_NonFollowPlayerLand
    // runs its random walk (RunEQSQuery n_Common_RandomWalkAroundPlayer -> MoveTo ActionDestination)
    let mut p = Party::new();
    p.run(1.0);
    p.world.set_player(Vec3::new(800.0, 0.0, 0.0), 0.0);
    p.run(6.0);
    let walked = p.ais.iter().any(|a| a.trace().log.iter().any(|l| l.contains("n_Common_NonFollowPlayerLand:TresBTTask_MoveTo")) || a.trace().path.iter().any(|l| l.starts_with("n_Common_NonFollowPlayerLand:")));
    assert!(walked, "{}", p.path(0));
    // and the battle flag still wins over the field loops (request merge fix in bt.rs)
    p.set_battle(true);
    p.run(1.0);
    for i in 0..2 {
        assert!(p.ais[i].trace().path.iter().any(|l| l.starts_with("BT_n_Common_Battle:")), "member {i}: {}", p.path(i));
    }
}

#[test]
fn cure_focus_need_and_item_up() {
    use kh_ai::api::CureStyle;
    use kh_ai::cure::{eval_row, Curer, TargetStat};
    let pack = pack();
    let mk = |style, item_up| Curer { pack: &pack, style, in_battle: true, can_cure: false, cure_amount: 0, commands: vec!["TRES_CMD_KIND_POTION".into(), "TRES_CMD_KIND_HIPOTION".into()], item_up, focus_break: false };
    let player = |hp: i32, focus: Option<(i32, i32)>| TargetStat { actor: ActorRef::Player, position: Vec3::ZERO, hp, hp_max: 1000, mp: Some((10, 10)), mp_charging: false, mp_charge_left: f32::INFINITY, dying: false, bad_status: false, is_player: true, focus };
    // FUN_141269f60: focus% <= 24 with OFTEN; ALWAYS also needs a fitting focus item (C4:
    // FUN_14125ef60; none in this list); PINCH = the focus-break bit (off here); unknown gauge never
    const NONE: &str = "TRES_CMD_KIND_NONE";
    assert!(mk(CureStyle::Often, 0).focus_need(&player(1000, Some((24, 100))), NONE));
    assert!(!mk(CureStyle::Often, 0).focus_need(&player(1000, Some((25, 100))), NONE));
    assert!(!mk(CureStyle::Always, 0).focus_need(&player(1000, Some((0, 100))), NONE));
    assert!(mk(CureStyle::Always, 0).focus_flag(&player(1000, Some((0, 100)))));
    assert!(!mk(CureStyle::Pinch, 0).focus_need(&player(1000, Some((0, 100))), NONE));
    assert!(!mk(CureStyle::Often, 0).focus_need(&player(1000, None), NONE));
    // FUN_141263950: a POTION heals ceil(R * 1.5) with one ITEM_UP: a target missing exactly that
    // takes the POTION (exact fit) instead of the HIPOTION
    let r = eval_row(&pack, "TRES_CMD_KIND_POTION", false).unwrap().recovery;
    let missing = (r as f32 * 1.5).ceil() as i32;
    let t = player(1000 - missing, None);
    assert_eq!(mk(CureStyle::Often, 1).hp_pick(&t), "TRES_CMD_KIND_POTION");
    assert_ne!(mk(CureStyle::Often, 0).hp_pick(&t), "TRES_CMD_KIND_POTION");
}
