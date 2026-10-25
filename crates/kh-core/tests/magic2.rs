//! The second magic pass (build/sora_tmp/IMPL_MAGIC_STATE.md, 2026-10-11): casts and items out
//! of the action states (magic spec A 2.2 / A 5.4 / E I2), grand magic from the formchange gauge
//! (I 7), the airstep's follow-ups (H 4.4 - 4.7, actions II.5) with the flowmotion attack choice,
//! and the camp menu's writes (`command::customize`; UI spec 10.5 / 10.6). Everything is driven
//! through the pad (`Input::pad`) or `Sora::command_request` (the same `ExecuteCommand`).

use kh_core::command::{customize, CommandRequest, MenuEvent};
use kh_core::magic::spells::cmd;
use kh_core::magic::View;
use kh_core::status::{ItemKind, LoadoutValue};
use kh_core::test_world::TestWorld;
use kh_core::{AnimId, Input, Loadout, Output, PadButton, PadButtons, PadState, Pose, RecordingHost, Sora, StateId, Status, TargetFlags, TargetId, TargetInfo, Tuning, Vec2, Vec3, SORA_PARAMS};

const DT: f32 = 1.0 / 60.0;

fn cell(json: &serde_json::Value) -> Option<LoadoutValue> {
    match json {
        serde_json::Value::Number(n) => n.as_f64().map(LoadoutValue::Number),
        serde_json::Value::String(s) => Some(LoadoutValue::Text(s.clone())),
        serde_json::Value::Array(items) => items.iter().map(|i| i.as_str().map(str::to_string)).collect::<Option<Vec<_>>>().map(LoadoutValue::List),
        serde_json::Value::Object(map) => map.iter().map(|(k, v)| v.as_f64().map(|n| (k.clone(), n))).collect::<Option<Vec<_>>>().map(LoadoutValue::Map),
        _ => None,
    }
}

/// The level-99 loadout with exactly these abilities equipped.
fn level99(equipped: &[&str]) -> Status {
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap();
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let key = row[0].as_str().unwrap();
        let value = if key == "abilities.equipped" { LoadoutValue::List(equipped.iter().map(|s| s.to_string()).collect()) } else { cell(&row[1]).unwrap() };
        loadout.set(key, &value).unwrap_or_else(|e| panic!("{e}"));
    }
    loadout.resolve().unwrap();
    Status::from_loadout(&loadout)
}

fn dummy(id: u32, x: f32, y: f32) -> TargetInfo {
    TargetInfo { id: TargetId(id), position: Vec3::new(x, y, 0.0), center: Vec3::new(x, y, 90.0), velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 }
}

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
    held: PadButtons,
    stick: Vec2,
    view: Option<View>,
}

impl Bench {
    fn new(status: Status, dummies: &[TargetInfo]) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(status);
        let mut host = RecordingHost::new();
        host.targets.extend_from_slice(dummies);
        let mut bench = Bench { sora, host, world: TestWorld::floor(), held: PadButtons::NONE, stick: Vec2::ZERO, view: Some(View::from_camera(Vec3::new(-300.0, 0.0, 150.0), 0.0, 0.0, 90.0, 1280, 720)) };
        bench.run(5);
        bench
    }

    fn step(&mut self, held: &[PadButton]) -> Output {
        let now = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let pad = PadState::from_held(now, self.held);
        self.held = now;
        let mut input = Input::default().with_pad(pad);
        input.view = self.view;
        input.move_stick = self.stick;
        let (sora, world) = (&mut self.sora, &self.world);
        self.host.step(world, |h| sora.step_hosted(DT, &input, h))
    }

    fn run(&mut self, steps: usize) -> Output {
        let mut out = self.sora.output();
        for _ in 0..steps {
            out = self.step(&[]);
        }
        out
    }

    fn hold(&mut self, held: &[PadButton], steps: usize) -> Output {
        let mut out = self.sora.output();
        for _ in 0..steps {
            out = self.step(held);
        }
        out
    }

    /// Asks for a command and steps once; true when it was refused.
    fn ask(&mut self, kind: u8, held: &[PadButton]) -> bool {
        self.sora.command_request(CommandRequest::Execute(kind));
        self.step(held);
        self.sora.ui_snapshot().command.events.contains(&MenuEvent::Refused)
    }

    fn until_state(&mut self, id: StateId, max: usize) -> usize {
        for n in 0..max {
            if self.step(&[]).state == id {
                return n + 1;
            }
        }
        panic!("the state never became {id:?} (it is {:?})", self.sora.output().state);
    }

    /// Stick forward + the guard button: a dodge roll.
    fn roll(&mut self) {
        self.stick = Vec2::new(0.0, 1.0);
        self.step(&[PadButton::GUARD]);
        self.stick = Vec2::ZERO;
        self.until_state(StateId::Dodgeroll, 3);
    }
}

/// A 2.2 (`CanMagic` rows), A 2.3 (the Glide row of `TryMagic`), E I2 (`CanUseItem` rows) and the
/// reverse rows of the dodge gate (A 5.4).
#[test]
fn casts_and_items_out_of_the_action_states() {
    let abilities = ["DODGE", "REFLECT_GUARD", "GLIDE", "HIGHJUMP"];
    // DODGEROLL: refused before TresEnableChange (0.4659 s), taken after it.
    let mut b = Bench::new(level99(&abilities), &[dummy(1, 800.0, 0.0)]);
    b.sora.status_mut().set_mp(100);
    b.roll();
    b.run(5);
    assert!(b.ask(cmd::FIRE, &[]), "a cast 0.1 s into the roll is refused");
    assert_eq!(b.sora.output().state, StateId::Dodgeroll);
    b.run(24);
    assert!(b.sora.state_flags().enable_change && b.sora.output().state == StateId::Dodgeroll);
    assert!(!b.ask(cmd::FIRE, &[]));
    b.until_state(StateId::Firemagic, 3);
    assert_eq!(b.sora.cast().unwrap().combo_index, 0);
    // ... and the reverse: out of the cast, a roll is refused before the cast's change point
    // and taken at it (the shot at 0.3 s, TresEnableChange at 0.733 s).
    b.run(10);
    b.stick = Vec2::new(0.0, 1.0);
    b.step(&[PadButton::GUARD]);
    b.step(&[]);
    assert_eq!(b.sora.output().state, StateId::Firemagic, "no roll before the cast's change point");
    b.stick = Vec2::ZERO;
    for _ in 0..60 {
        if b.sora.state_flags().enable_change {
            break;
        }
        b.step(&[]);
    }
    assert!(b.sora.cast().is_some_and(|c| c.bullets == 1));
    b.roll();
    // An item out of the roll, at its change point.
    let mut b = Bench::new(level99(&abilities), &[]);
    b.roll();
    b.run(5);
    assert!(b.ask(cmd::POTION, &[]));
    b.run(24);
    assert!(!b.ask(cmd::POTION, &[]));
    b.until_state(StateId::UseItem, 3);
    // GUARD: a player guard that caught nothing refuses the cast and the item (Guard+0xDC == 0).
    let mut b = Bench::new(level99(&abilities), &[dummy(1, 800.0, 0.0)]);
    b.sora.status_mut().set_mp(100);
    b.step(&[PadButton::GUARD]);
    b.until_state(StateId::Guard, 3);
    assert!(b.ask(cmd::FIRE, &[]) && b.ask(cmd::POTION, &[]));
    assert_eq!(b.sora.output().state, StateId::Guard);
    // GLIDE: always; the cast is an air cast with the move flag forced (flags 1 | 8 -> 2).
    let mut b = Bench::new(level99(&abilities), &[dummy(1, 3000.0, 0.0)]);
    b.sora.status_mut().set_mp(100);
    b.step(&[PadButton::JUMP]);
    b.run(30);
    for _ in 0..200 {
        if b.step(&[PadButton::JUMP]).state == StateId::Glide {
            break;
        }
    }
    assert_eq!(b.sora.output().state, StateId::Glide);
    b.hold(&[PadButton::JUMP], 10);
    assert!(!b.ask(cmd::FIRE, &[PadButton::JUMP]));
    let out = b.step(&[]);
    assert_eq!(out.state, StateId::Firemagic);
    let out = b.step(&[]);
    let flags = b.sora.cast().unwrap().flags;
    assert_eq!(flags & 0xB, 0xB, "air (1), moving (2), move request (8): {flags:#x}");
    assert_eq!(out.anim.name, AnimId::MagicAirFireMF);
}

/// I 7: Fire hits fill the formchange gauge; at 100 the roll offers S_FIRA (one tier above the
/// highest Fire tier cast) instead of the formchange, with the lifetime 20 s x (1 + 0.2 x 3
/// Grand Magic Extenders); Triangle (Y) casts it for no MP and the tallies are cleared.
#[test]
fn grand_magic_from_the_gauge() {
    let mut b = Bench::new(level99(&["MAGIC_TIME x3"]), &[dummy(1, 800.0, 0.0)]);
    let mut casts = 0;
    while !b.sora.combat().situation.has(110) {
        assert!(casts < 14, "no offer after {casts} casts (gauge {})", b.sora.combat().style.gauge);
        b.sora.status_mut().set_mp(100);
        assert!(!b.ask(cmd::FIRE, &[]));
        b.until_state(StateId::Firemagic, 5);
        if casts == 0 {
            // The first hit's points are in the Fire tally while the gauge holds them.
            for _ in 0..60 {
                b.step(&[]);
                if b.sora.magic().grand.element_points[1] > 0.0 {
                    break;
                }
            }
            assert_eq!((b.sora.magic().grand.element_points[1], b.sora.magic().grand.last_cmd[1]), (15.0, cmd::FIRE));
        }
        b.until_state(StateId::Idle, 200);
        casts += 1;
    }
    // 15 points a hit: the seventh cast fills it.
    assert_eq!(casts, 7);
    let queue = &b.sora.combat().situation;
    assert!(!queue.has(0x94), "the formchange is not offered when a grand magic was rolled");
    assert_eq!(queue.find(110).unwrap().1.total, 32.0);
    assert_eq!((b.sora.combat().style.gauge, b.sora.magic().grand.element_points, b.sora.magic().grand.offers), (0.0, [0.0; 8], 1));
    // Triangle uses it: TryMagic(bGrand): the Shoot clip, no MP, the entry is gone.
    b.run(40);
    b.sora.status_mut().set_mp(37);
    b.step(&[PadButton::SITUATION]);
    b.until_state(StateId::Firemagic, 5);
    let out = b.step(&[]);
    assert_eq!((b.sora.cast().unwrap().cmd, out.anim.name), (cmd::S_FIRA, AnimId::MagicShoot));
    assert!(!b.sora.combat().situation.has(110));
    b.until_state(StateId::Idle, 200);
    assert_eq!(b.sora.status().mp, 37);
}

/// Aims at the dummy / point ahead and presses the airstep button; returns once the state runs.
fn airstep(b: &mut Bench) {
    b.hold(&[PadButton::RightShoulder], 40);
    assert!(b.sora.aim().can_airstep, "no airstep target");
    b.step(&[PadButton::RightShoulder, PadButton::GUARD]);
    b.until_state(StateId::Athleticflow, 3);
}

/// H 4.7 + Attack slot 124: the attack press after an airstep arrival at an enemy starts the
/// flow attack: Sonic Slash (kind 0x36, clip 1181) with SONIC_SLASH, Buzz Saw (0x38, clip 1183)
/// with two enemies and TURN_CUTTER, an ordinary air attack without the ability.
#[test]
fn airstep_flow_attack() {
    for (abilities, dummies, kind, clip) in [
        (&["SONIC_SLASH", "TURN_CUTTER"][..], &[dummy(1, 2000.0, 0.0)][..], 0x36u8, Some(AnimId::from_name("1181").unwrap())),
        (&["SONIC_SLASH", "TURN_CUTTER"][..], &[dummy(1, 2000.0, 0.0), dummy(2, 2100.0, 150.0)][..], 0x38, Some(AnimId::from_name("1183").unwrap())),
        (&[][..], &[dummy(1, 2000.0, 0.0)][..], 0x10, None),
    ] {
        let mut b = Bench::new(level99(abilities), dummies);
        airstep(&mut b);
        // A press during the flight is not accepted (the lock is taken at the arrival).
        b.step(&[PadButton::ATTACK]);
        b.step(&[]);
        assert!(!b.sora.airstep().unwrap().attack_latched);
        let mut arrived = false;
        for _ in 0..200 {
            if b.step(&[]).anim.name == AnimId::AthleticSlideEnd {
                arrived = true;
                break;
            }
        }
        assert!(arrived);
        let view = b.sora.airstep().unwrap();
        assert_eq!((view.mode, view.flags & 0x10), (4, 0x10), "the enemy stop with the lock taken");
        b.step(&[PadButton::ATTACK]);
        assert!(b.sora.airstep().unwrap().attack_latched);
        b.until_state(StateId::Attack, 40);
        let attack = b.sora.attack().unwrap();
        assert_eq!(attack.kind, kind, "abilities {abilities:?}");
        assert_eq!(attack.target, TargetId(1));
        if let Some(clip) = clip {
            assert_eq!(b.sora.output().anim.name, clip);
        }
        // The flow attack reaches the dummy and hits it with its own rows.
        if kind == 0x36 {
            b.until_state(StateId::Idle, 300);
            let rows: Vec<&str> = b.host.hits().into_iter().map(|h| h.0.attack_name).collect();
            assert!(rows.contains(&"FF_SONIC_1") && rows.contains(&"FF_SONIC_2"), "{rows:?}");
        }
    }
}

/// H 4.4 - 4.6: a chain of two level points: the first hop flies with world collision off and
/// ends in the kick flourish (`WallKickF` from clip time 0.2), the last hop with collision on and
/// `SlideEnd`; a jump press on the first hop aborts it and collision is back on.
#[test]
fn airstep_chain_of_points() {
    let points = |b: &mut Bench| {
        b.sora.magic_mut().airstep_points.push(kh_core::magic::AirstepPoint::wall(7, Vec3::new(1000.0, 0.0, 300.0)));
        b.sora.magic_mut().airstep_points.push(kh_core::magic::AirstepPoint::wall(8, Vec3::new(2000.0, 0.0, 300.0)));
        // The camera looks at the far point: the near one is inside the 160 px circle.
        b.view = Some(View::from_camera(Vec3::new(-300.0, 0.0, 150.0), 0.0, 3.73, 90.0, 1280, 720));
    };
    let mut b = Bench::new(level99(&[]), &[]);
    points(&mut b);
    b.hold(&[PadButton::RightShoulder], 40);
    assert_eq!(b.sora.aim().airstep, vec![(TargetId::NONE, 7), (TargetId::NONE, 8)]);
    b.step(&[PadButton::RightShoulder, PadButton::GUARD]);
    b.until_state(StateId::Athleticflow, 3);
    let view = b.sora.airstep().unwrap();
    assert_eq!((view.leg, view.legs, view.world_collision_off), (0, 2, true));
    let (mut kicked, mut last_leg, mut ended) = (false, false, false);
    for _ in 0..400 {
        let out = b.step(&[]);
        let Some(view) = b.sora.airstep() else {
            ended = true;
            break;
        };
        if out.anim.name == AnimId::WallKickF && !kicked {
            kicked = true;
            assert_eq!(view.mode, 1);
            assert!(out.anim.time >= 0.2 && out.anim.time < 0.25, "the kick clip starts at 0.2 s: {}", out.anim.time);
            assert!(!view.world_collision_off && (out.position - Vec3::new(1000.0, 0.0, 300.0)).length() < 130.0, "{:?}", out.position);
        }
        if view.leg == 1 && view.mode == 0 {
            last_leg = true;
            assert!(!view.world_collision_off, "the last hop collides");
            if out.anim.name == AnimId::SlideEnd {
                assert!((out.position - Vec3::new(2000.0, 0.0, 300.0)).length() < 5.0, "{:?}", out.position);
            }
        }
    }
    assert!(kicked && last_leg && ended);
    assert!(!b.sora.magic().projectile_views().any(|p| p.class == "TresProj_AthleticKick_C"), "no kick projectile without the attack latch");
    // The abort.
    let mut b = Bench::new(level99(&[]), &[]);
    points(&mut b);
    b.hold(&[PadButton::RightShoulder], 40);
    b.step(&[PadButton::RightShoulder, PadButton::GUARD]);
    b.until_state(StateId::Athleticflow, 3);
    b.run(4);
    b.step(&[PadButton::JUMP]);
    let out = b.step(&[]);
    let view = b.sora.airstep().unwrap();
    assert_eq!((out.anim.name, view.sub, view.world_collision_off), (AnimId::SlideEnd, 2, false));
    b.until_state(StateId::Jump, 80);
}

/// UI spec 10.5 / 10.6 through `command::customize`: what the camp menu writes shows in the
/// command menu's snapshot; the pouch / stock rules of the Equipment and Items screens.
#[test]
fn camp_menu_writes() {
    let mut b = Bench::new(level99(&[]), &[]);
    let s = &mut b.sora;
    assert_eq!(customize::magic_slots(s), [cmd::FIRAGA, cmd::WATAGA, cmd::CURAGA, cmd::BLIZZAGA, cmd::THUNDAGA, cmd::AEROGA]);
    assert_eq!(customize::magic_tiers(s, 0), Some([(cmd::FIRE, true), (cmd::FIRA, true), (cmd::FIRAGA, true)]));
    assert!(customize::set_magic_tier(s, 0, cmd::FIRE) && !customize::set_magic_tier(s, 0, cmd::BLIZZARD));
    assert!(customize::swap_magic_slots(s, 0, 1));
    assert_eq!((customize::magic_slots(s)[..2].to_vec(), s.command().tier_chosen[..2].to_vec()), (vec![cmd::WATAGA, cmd::FIRE], vec![false, true]));
    // The list a shortcut button opens: 18 magic entries in asset order, then the pouch's items.
    let list = customize::assignable(s);
    assert_eq!(list.iter().filter(|a| a.tab == customize::Tab::Magic).count(), 18);
    assert_eq!((list[0].kind, list[3].kind, list[17].kind), (cmd::FIRE, cmd::BLIZZARD, cmd::CURAGA));
    assert_eq!(list[18..].iter().map(|a| a.kind).collect::<Vec<_>>(), vec![cmd::POTION, cmd::HIPOTION, cmd::ETHER]);
    assert!(customize::set_shortcut(s, 2, 3, cmd::THUNDER) && !customize::set_shortcut(s, 2, 0, cmd::ELIXIR) && customize::clear_shortcut(s, 0, 0));
    assert_eq!((customize::shortcuts(s, 2), customize::shortcuts(s, 0)[0]), ([0, 0, 0, cmd::THUNDER], 0));
    // The next step's snapshot has them.
    b.run(1);
    let menu = b.sora.ui_snapshot().command;
    assert_eq!((menu.lists[1][0].kind, menu.lists[1][1].kind, menu.shortcut[0].kind), (cmd::WATAGA, cmd::FIRE, 0));
    // Equipment: an item leaves the stock into a slot and the old one returns; auto-reload is
    // inherited from the other slots with the same item; the toggle acts on every such slot.
    let s = &mut b.sora;
    assert_eq!(customize::equip_item(s, 6, Some(ItemKind::Elixir)), Err(customize::Refusal::NoStock));
    s.magic_mut().pouch.stock_add(ItemKind::Elixir, 2);
    s.magic_mut().pouch.stock_add(ItemKind::Potion, 1);
    assert_eq!(customize::equip_item(s, 6, Some(ItemKind::Potion)), Ok(()));
    assert_eq!(customize::equip_item(s, 0, Some(ItemKind::Elixir)), Ok(()));
    let slots = customize::pouch_slots(s);
    assert_eq!((slots[0].item, slots[0].auto_reload, slots[6].item, slots[6].auto_reload), (Some(ItemKind::Elixir), false, Some(ItemKind::Potion), true));
    assert_eq!((s.magic().pouch.stock_of(ItemKind::Elixir), s.magic().pouch.stock_of(ItemKind::Potion)), (1, 1));
    assert_eq!(customize::toggle_auto_reload(s, ItemKind::Potion), Some(false));
    assert_eq!(customize::equip_item(s, 7, None), Ok(()));
    assert_eq!(customize::equip_item(s, 9, None), Err(customize::Refusal::Slot));
    // Items screen: one from the stock, the effect at once; refused without benefit / stock.
    assert_eq!(customize::use_item_from_menu(s, ItemKind::Potion), Err(customize::Refusal::NoBenefit));
    let hp = s.status().max_hp();
    s.status_mut().set_hp(hp - 100);
    assert_eq!(customize::use_item_from_menu(s, ItemKind::Potion).map(|o| o.hp), Ok(80));
    assert_eq!(customize::use_item_from_menu(s, ItemKind::Potion), Err(customize::Refusal::NoStock));
    assert_eq!(customize::use_item_from_menu(s, ItemKind::Panacea), Err(customize::Refusal::NotUsable));
}

/// A 1.4 step 11, literal (`fire_magic::FALLING_AT_GROUND_SHOT`): the shot of a ground cast sets
/// Falling; he is back on the floor a few frames later, without having moved.
#[test]
fn ground_shot_sets_falling_briefly() {
    let mut b = Bench::new(level99(&[]), &[dummy(1, 800.0, 0.0)]);
    b.sora.status_mut().set_mp(100);
    let z = b.sora.output().position.z;
    assert!(!b.ask(cmd::FIRE, &[]));
    b.until_state(StateId::Firemagic, 5);
    while b.sora.cast().is_some_and(|c| c.bullets == 0) {
        assert!(b.step(&[]).on_floor || b.sora.cast().is_some_and(|c| c.bullets == 1));
    }
    let mut off = 0;
    while !b.sora.output().on_floor {
        off += 1;
        assert!(off <= 6, "still falling {off} steps after the shot");
        let out = b.step(&[]);
        assert!((out.position.z - z).abs() < 2.5 && out.state == StateId::Firemagic, "{:?}", out.position);
    }
    assert!(off >= 1, "the shot did not set Falling");
    println!("off the floor for {off} steps after a ground shot");
    b.until_state(StateId::Idle, 120);
    // (He lands inside the walking floor-distance band, not necessarily at the old height.)
    let dz = b.sora.output().position.z - z;
    println!("height change after the landing: {dz} cm");
    assert!(dz.abs() < 0.6, "{dz}");
}

/// A 2.2 GUARD row, the accepting side: out of a GUARDED HIT (Guard+0xDC) the cast is taken from
/// the hit clip's `TresEnableInput` (0.2667 s), not before.
#[test]
fn cast_out_of_a_guarded_hit() {
    let mut b = Bench::new(level99(&["REFLECT_GUARD"]), &[dummy(1, 300.0, 0.0)]);
    b.sora.status_mut().set_mp(100);
    b.step(&[PadButton::GUARD]);
    b.until_state(StateId::Guard, 3);
    b.run(3);
    let hp = b.sora.status().hp;
    b.sora.take_hit(kh_core::states::HitOnPlayer::new(kh_core::enums::ETresDamageKind::Small, 30.0, TargetId(1), Vec3::new(300.0, 0.0, 0.0)));
    b.run(3);
    assert_eq!((b.sora.status().hp, b.sora.output().state, b.sora.output().anim.name), (hp, StateId::Guard, AnimId::GuardHit));
    assert!(b.ask(cmd::FIRE, &[]), "before TresEnableInput of the hit clip");
    b.run(16);
    assert!(!b.ask(cmd::FIRE, &[]));
    b.until_state(StateId::Firemagic, 3);
}
