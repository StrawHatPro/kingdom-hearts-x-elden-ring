//! (HUD-REVIVE, THE USER 2026-11-07: "the background element of the HUD party leader picture starts flashing red when low
//! health, this is correct, but it keeps flashing red low health even when healed past low health, fix this so it only
//! displays when low health") The portrait's base plate (`pcFaceDodai1`, the friends' `npcDodai`) and the face's own red
//! effect (`faceEff.effFaceDying`) flash only while HP is at or below KH3's pinch (`hp <= maxHP * 25 / 100`, 0x1405AD600),
//! stop the moment HP is above it, whatever raised it, and start again at the next pinch. Every leader face, the friends'.

use kh_ui::hud_status::{HudStatus, SlotValues, StatusDriver, StatusValues};
use kh_ui::{Matrix, Movie, Pack, Rect, Renderer, UiSound};

const DT: f32 = 1.0 / 60.0;
const PC_PLATE: &str = "sttsPc.pcHp.pcHpPack.pcFaceDodai.pcFaceDodai1";
const PC_FACE: &str = "sttsPc.pcHp.pcHpPack.poPcFace";
const NPC1_PLATE: &str = "sttsNpc1.sttsNpc.npcDodai";

fn pack() -> Option<Pack> {
    match Pack::open_default() {
        Ok(pack) => Some(pack),
        Err(_) => {
            eprintln!("SKIPPED: no build/sora_pack/ui");
            None
        }
    }
}

fn slot(hp: i32, hp_max: i32) -> SlotValues {
    SlotValues { active: true, hp, hp_max, mp: 100, mp_max: 100, ..SlotValues::default() }
}

fn values(hp: i32, hp_max: i32) -> StatusValues {
    let mut v = StatusValues { focus: 50, focus_max: 100, mp_gauge_unlocked: true, focus_gauge_unlocked: true, ..StatusValues::default() };
    v.slots[0] = slot(hp, hp_max);
    v
}

struct Status {
    hud: HudStatus,
    driver: StatusDriver,
    sounds: Vec<UiSound>,
}

impl Status {
    fn new(pack: &mut Pack, leader_face: &str) -> Status {
        let mut hud = HudStatus::new(pack.movie("HudStatus").unwrap());
        let mut driver = StatusDriver::default();
        driver.faces[0] = Some(pack.movie(leader_face).unwrap());
        hud.set_status(0, 0);
        Status { hud, driver, sounds: Vec::new() }
    }

    fn run(&mut self, v: &StatusValues, frames: usize) {
        for _ in 0..frames {
            self.driver.update(&mut self.hud, v, DT, &mut self.sounds);
            self.hud.tick(DT);
        }
    }

    fn movie(&self) -> &Movie {
        &self.hud.movie
    }
}

/// The plate flashes: on its `dying` loop (frames 21 .. 69 of `McPcFaceDodai1_62`, 39 .. 87 of `McNpcDodai_3`), playing.
fn flashing(m: &Movie, plate: &str) -> bool {
    m.current_label(plate) == Some("dying") && m.sprite(plate).is_some_and(|s| s.playing)
}

/// The plate is at rest on the look `label` (stopped).
fn resting(m: &Movie, plate: &str, label: &str) -> bool {
    m.current_label(plate) == Some(label) && m.sprite(plate).is_some_and(|s| !s.playing)
}

/// The face's own red pulse (`faceEff.effFaceDying`): `loop` while dying, `off` otherwise.
fn face_pulse(m: &Movie, face: &str) -> Option<String> {
    m.current_label(&format!("{PC_FACE}.{face}.faceEff.effFaceDying")).map(str::to_string)
}

/// Down to a pinch, then healed by `heal` (the next values), checked at once and after a second at rest; then down again.
fn pinch_then(s: &mut Status, face: &str, pinch: &StatusValues, healed: &StatusValues, rest: &str, route: &str) {
    s.run(pinch, 90);
    assert!(flashing(s.movie(), PC_PLATE), "{route}: the plate flashes at a pinch");
    assert_eq!(face_pulse(s.movie(), face).as_deref(), Some("loop"), "{route}: the face pulses at a pinch");
    s.run(healed, 1);
    assert!(!flashing(s.movie(), PC_PLATE), "{route}: the flash stops on the first frame above the pinch");
    s.run(healed, 60);
    assert!(resting(s.movie(), PC_PLATE, rest), "{route}: the plate rests on `{rest}`, got {:?}", s.movie().current_label(PC_PLATE));
    assert_eq!(face_pulse(s.movie(), face).as_deref(), Some("off"), "{route}: the face pulse is off");
    let frame = s.movie().current_frame(PC_PLATE);
    s.run(healed, 120);
    assert_eq!(s.movie().current_frame(PC_PLATE), frame, "{route}: no flash two seconds later");
    s.run(pinch, 30);
    assert!(flashing(s.movie(), PC_PLATE), "{route}: a new pinch starts the flash again");
}

#[test]
fn the_leaders_flash_ends_with_every_heal_route() {
    let Some(mut pack) = pack() else { return };
    for (face, inst) in [("FacePcExSora01", "facePcExSora01"), ("FacePcBxSora01", "facePcBxSora01"), ("FacePcTsSora01", "facePcTsSora01"), ("FacePcMiSora01", "facePcMiSora01"), ("FacePcCaSora01", "facePcCaSora01"), ("FacePcEx1Sora01", "facePcEx1Sora01"), ("FacePcEx2Sora01", "facePcEx2Sora01")] {
        let mut s = Status::new(&mut pack, face);
        s.run(&values(250, 250), 30);
        let pinch = values(62, 250);
        // KH3's line: 62 of 250 is a pinch, 63 is not (integer 25 %).
        pinch_then(&mut s, inst, &pinch, &values(63, 250), "nomal", &format!("{face}: one HP over the line"));
        pinch_then(&mut s, inst, &pinch, &values(150, 250), "nomal", &format!("{face}: Cure"));
        pinch_then(&mut s, inst, &pinch, &values(250, 250), "nomal", &format!("{face}: save point / full heal"));
        // A level up: max HP grows and HP is refilled.
        pinch_then(&mut s, inst, &pinch, &values(280, 280), "nomal", &format!("{face}: level up"));
        // Healed in battle: the battle plate.
        let mut battle = values(200, 250);
        battle.game_in_battle = true;
        pinch_then(&mut s, inst, &pinch, &battle, "battle", &format!("{face}: healed in battle"));
        // The battle ends after the heal: battle -> normal.
        s.run(&values(200, 250), 30);
        assert!(resting(s.movie(), PC_PLATE, "nomal"), "{face}: battle over");
    }
}

#[test]
fn the_flash_ends_on_a_leader_swap_and_a_load() {
    let Some(mut pack) = pack() else { return };
    let donald = pack.movie("FaceNpcExDonald").unwrap();
    let goofy = pack.movie("FaceNpcExGoofy").unwrap();
    let sora = pack.movie("FacePcExSora01").unwrap();
    let mut s = Status::new(&mut pack, "FacePcExSora01");
    s.run(&values(250, 250), 30);
    // Sora at a pinch hands the lead to a healthy Donald (party P: the main slot's face is replaced).
    s.run(&values(30, 250), 90);
    assert!(flashing(s.movie(), PC_PLATE));
    s.driver.faces[0] = Some(donald.clone());
    s.driver.reload_face(0);
    s.run(&values(180, 180), 60);
    assert!(resting(s.movie(), PC_PLATE, "nomal"), "swap to a healthy Donald: {:?}", s.movie().current_label(PC_PLATE));
    // Donald leading: his own pinch flashes and his heal ends it; Goofy the same.
    for (face, inst) in [(donald, "faceNpcExDonald"), (goofy, "faceNpcExGoofy")] {
        s.driver.faces[0] = Some(face);
        s.driver.reload_face(0);
        s.run(&values(180, 180), 30);
        pinch_then(&mut s, inst, &values(45, 180), &values(46, 180), "nomal", inst);
        pinch_then(&mut s, inst, &values(45, 180), &values(180, 180), "nomal", inst);
    }
    // A load: the HUD is built again (new driver, same movie) with the loaded, healthy values.
    s.run(&values(10, 180), 90);
    assert!(flashing(s.movie(), PC_PLATE));
    s.driver = StatusDriver::default();
    s.driver.faces[0] = Some(sora);
    s.run(&values(250, 250), 60);
    assert!(resting(s.movie(), PC_PLATE, "nomal"), "load: {:?}", s.movie().current_label(PC_PLATE));
}

#[test]
fn a_friends_plate_leaves_the_pinch_and_the_ko() {
    let Some(mut pack) = pack() else { return };
    let mut s = Status::new(&mut pack, "FacePcExSora01");
    s.driver.faces[1] = Some(pack.movie("FaceNpcExGoofy").unwrap());
    let with_friend = |hp: i32| {
        let mut v = values(250, 250);
        v.slots[1] = slot(hp, 200);
        v
    };
    s.run(&with_friend(200), 30);
    assert!(resting(s.movie(), NPC1_PLATE, "nomal"));
    s.run(&with_friend(50), 60);
    assert_eq!(s.movie().current_label(NPC1_PLATE), Some("dying"), "friend at a pinch");
    s.run(&with_friend(51), 60);
    assert!(resting(s.movie(), NPC1_PLATE, "nomal"), "friend healed: {:?}", s.movie().current_label(NPC1_PLATE));
    // KO, then revived at full HP (the save point's revive): the death plate goes.
    s.run(&with_friend(0), 60);
    assert_eq!(s.movie().current_label(NPC1_PLATE), Some("death"), "friend KO");
    s.run(&with_friend(200), 60);
    assert!(resting(s.movie(), NPC1_PLATE, "nomal"), "friend revived: {:?}", s.movie().current_label(NPC1_PLATE));
}

#[test]
fn the_panel_is_redrawn_when_the_flash_ends() {
    let Some(mut pack) = pack() else { return };
    let fonts = kh_ui::Fonts::load(&mut pack).unwrap();
    let renderer = Renderer::new(fonts);
    let mut s = Status::new(&mut pack, "FacePcExSora01");
    s.run(&values(250, 250), 30);
    s.run(&values(40, 250), 120);
    let plate_parts = |s: &Status| renderer.parts(s.movie(), &Matrix::IDENTITY);
    let screen = Rect::new(0.0, 0.0, 1920.0, 1080.0);
    // The flashing area: what changes frame to frame at a pinch (the plate's and the face's red pulse).
    let mut flash = Rect::EMPTY;
    let mut last = plate_parts(&s);
    for _ in 0..60 {
        s.run(&values(40, 250), 1);
        let now = plate_parts(&s);
        flash = flash.union(&kh_ui::render::dirty_rect(&last, &now));
        last = now;
    }
    let flash = flash.intersect(&screen);
    assert!(!flash.is_empty(), "the pinch flashes");
    // The pulse as the last pinch frame drew it (what changed on that frame) is inside the heal frame's redraw.
    let prev = plate_parts(&s);
    s.run(&values(40, 250), 1);
    let before = plate_parts(&s);
    let pulse = kh_ui::render::dirty_rect(&prev, &before).intersect(&screen);
    s.run(&values(250, 250), 1);
    let after = plate_parts(&s);
    let dirty = kh_ui::render::dirty_rect(&before, &after);
    assert!(!dirty.is_empty(), "the heal redraws the panel");
    assert_eq!(dirty.union(&pulse), dirty, "the heal redraws the pulse: dirty {dirty:?}, pulse {pulse:?}");
    // Healed and at rest: nothing in the flashing area changes any more.
    s.run(&values(250, 250), 60);
    let mut last = plate_parts(&s);
    for _ in 0..60 {
        s.run(&values(250, 250), 1);
        let now = plate_parts(&s);
        let moved = kh_ui::render::dirty_rect(&last, &now).intersect(&flash);
        assert!(moved.is_empty(), "the healed panel still changes in the flash area: {moved:?}");
        last = now;
    }
    assert!(resting(s.movie(), PC_PLATE, "nomal"));
}
