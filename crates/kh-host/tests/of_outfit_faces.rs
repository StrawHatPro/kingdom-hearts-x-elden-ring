//! (OF) SORA'S HUD FACE FOLLOWS HIS OUTFIT, WITH ITS OWN BATTLE FACE (THE USER 2026-11-03: "make sure Sora's HUD changes for
//! his costumes and also when he is in battle his battle HUD picture (for each costume) is used"). The real `Hud` (UI pack;
//! skipped without it) is driven as the hosts drive it: Sora stepped with a host that reports engaged enemies (his battle
//! flag), `Hud::set_sora_outfit` as kh-er boc.rs calls it every HUD frame. For every outfit: the face movie KH3's data names
//! for its pawn (hud_outfit.rs) is loaded into the main slot, its `normal` look out of combat, `battle` in combat, the
//! low-HP look at a pinch over both; Sora in a friend slot (Donald leading) wears it too.
//! Pictures: build/sora_tmp/hud/of_<outfit>_{normal,battle,lowhp,friend}.png (the status corner) and of_sheet.png (all
//! outfits: rows in the table's order, columns normal / battle / low HP / friend slot in battle).

use kh_core::host::{NullHost, TargetInfo};
use kh_core::test_world::TestWorld;
use kh_core::{HitRecord, HitResult, HurtShape, Input, PadButtons, Pose, Sora, TargetId, Targets, Vec3, SORA_PARAMS};
use kh_host::hud::{sora_face_movie, Frame, Hud, HudTargets, TargetView, SORA_OUTFIT_FACES};
use kh_host::party::{PartyChara, PartyHudSlot};

const DT: f32 = 1.0 / 60.0;
/// The status corner the pictures keep (stage pixels: x, y, w, h): the leader's portrait and the friend slots.
const CORNER: (usize, usize, usize, usize) = (1500, 860, 420, 220);

struct NoTargets;
impl HudTargets for NoTargets {
    fn target(&self, _id: TargetId) -> Option<TargetView> {
        None
    }
}

struct Engaged(u32);
impl Targets for Engaged {
    fn enumerate(&self, _out: &mut Vec<TargetInfo>) {}
    fn info(&self, _id: TargetId) -> Option<TargetInfo> {
        None
    }
    fn hurt_shapes(&self, _id: TargetId, _out: &mut Vec<HurtShape>) {}
    fn apply_hit(&mut self, _hit: &HitRecord) -> HitResult {
        HitResult::default()
    }
    fn engaged_count(&self) -> u32 {
        self.0
    }
    // (AUDIT-B) no bones here: a rider attached to a hit follows the target's origin and yaw
    fn hit_bone(&self, _id: kh_core::TargetId, _at: kh_core::Vec3) -> Option<kh_core::host::TargetBone> {
        None
    }
    fn bone_pose(&self, _id: kh_core::TargetId, _bone: kh_core::host::TargetBone) -> Option<kh_core::host::BonePose> {
        None
    }
}

fn same(p: Vec3) -> Vec3 {
    p
}

fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

struct Rig {
    hud: Hud,
    fonts: kh_ui::Fonts,
    sora: Sora,
    world: TestWorld,
    calls: Vec<String>,
}

impl Rig {
    fn new() -> Option<Rig> {
        let Ok((hud, fonts)) = Hud::load(&root().join("build/sora_pack/ui")) else {
            eprintln!("no UI pack: skipped");
            return None;
        };
        let origin = Vec3::Z * Sora::origin_height_on_floor(&SORA_PARAMS);
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None)));
        Some(Rig { hud, fonts, sora, world: TestWorld::floor(), calls: Vec::new() })
    }

    /// `n` frames with `engaged` enemies; the outfit is handed to the HUD every frame (as boc.rs does).
    fn frames(&mut self, outfit: &str, engaged: u32, n: usize, words: &[&str]) {
        for k in 0..n {
            let mut null = NullHost::default();
            let mut targets = Engaged(engaged);
            let mut host = null.host(&self.world);
            host.targets = &mut targets;
            let state = self.sora.step_hosted(DT, &Input::default(), &mut host).state;
            let snapshot = self.sora.ui_snapshot();
            let words: Vec<String> = if k == 0 { words.iter().map(|w| w.to_string()).collect() } else { Vec::new() };
            self.hud.frame(&mut Frame { snapshot: &snapshot, sora: &self.sora, events: &[], state, step_dt: DT, play_seconds: 0, targets: &NoTargets, to_view: same, view: None, words: &words, dt: DT, ticks: 1, pressed: PadButtons::NONE, nav: Vec::new() });
            self.hud.set_sora_outfit(outfit);
            self.calls.extend(self.hud.take_status_calls());
        }
    }

    fn look(&self, slot: usize) -> Option<i32> {
        let head = format!("setFaceLook({slot}, ");
        self.calls.iter().rev().find_map(|c| c.strip_prefix(&head).and_then(|r| r.trim_end_matches(')').parse().ok()))
    }

    fn face(&self, slot: usize) -> Option<String> {
        let head = format!("loadFace({slot}, ");
        self.calls.iter().rev().find_map(|c| c.strip_prefix(&head).map(|r| r.split("Some(\"").nth(1).and_then(|r| r.split('"').next()).unwrap_or("").to_string()))
    }

    /// The status corner (CORNER) as RGBA, also written as `name`.png.
    fn corner(&self, name: &str) -> Vec<u8> {
        let (w, h) = (1920usize, 1080usize);
        let mut canvas = kh_ui::Canvas::new(w, h);
        let (stage, _) = kh_host::hud_layers::stage_root(w as f32, h as f32);
        kh_ui::Renderer::new(self.fonts.clone()).draw(&mut canvas, self.hud.movie("HudStatus").unwrap(), &stage);
        let mut rgba = Vec::new();
        canvas.to_rgba8_over(|_, y| [0.30 + 0.08 * (y as f32 / 1080.0), 0.27, 0.24], &mut rgba);
        let (x0, y0, cw, ch) = CORNER;
        let mut out = vec![0u8; cw * ch * 4];
        for y in 0..ch {
            let s = ((y0 + y) * w + x0) * 4;
            out[y * cw * 4..(y + 1) * cw * 4].copy_from_slice(&rgba[s..s + cw * 4]);
        }
        let dir = root().join("build/sora_tmp/hud");
        std::fs::create_dir_all(&dir).ok();
        kh_ui::def::write_png(&dir.join(format!("{name}.png")), cw as u32, ch as u32, &out).unwrap();
        out
    }
}

/// Pixels of the main slot's portrait (in the CORNER picture) that differ between two pictures.
fn portrait_diff(a: &[u8], b: &[u8]) -> usize {
    let (w, x0, x1, y0, y1) = (CORNER.2, 260usize, 360usize, 50usize, 160usize);
    (y0..y1).flat_map(|y| (x0..x1).map(move |x| (y * w + x) * 4)).filter(|i| (0..3).any(|c| a[i + c].abs_diff(b[i + c]) > 24)).count()
}

fn slot(chara: PartyChara, in_battle: bool) -> PartyHudSlot {
    PartyHudSlot { chara, hp: 160, hp_max: 220, mp: 60, mp_max: 100, mp_charging: false, mp_charge_value: 0.0, in_battle, focus: None }
}

#[test]
fn every_outfit_has_its_kh3_face_with_its_battle_and_low_hp_looks_in_every_slot() {
    let Some(_) = Rig::new() else { return };
    let (cw, ch) = (CORNER.2, CORNER.3);
    let mut sheet = vec![0u8; cw * 4 * ch * SORA_OUTFIT_FACES.len() * 4];
    let mut kh3_normal: Option<Vec<u8>> = None;
    for (row, (outfit, movie)) in SORA_OUTFIT_FACES.iter().enumerate() {
        let mut rig = Rig::new().unwrap();
        // (THE USER 2026-11-03: "just use KH3's KH1 sora face") KH3's KH1 face is one picture for every look
        let one_picture = *movie == "FacePcEx1Sora01";
        rig.frames(outfit, 0, 90, &[]);
        assert_eq!(rig.face(0).as_deref(), Some(*movie), "{outfit}: KH3's face movie of its pawn");
        assert_eq!(rig.look(0), Some(0), "{outfit}: out of combat, the normal look");
        let normal = rig.corner(&format!("of_{outfit}_normal"));
        rig.frames(outfit, 1, 90, &[]);
        assert_eq!(rig.look(0), Some(1), "{outfit}: in combat, the battle look");
        let battle = rig.corner(&format!("of_{outfit}_battle"));
        if one_picture {
            assert!(portrait_diff(&normal, &battle) < 40, "{outfit}: KH3's one picture in battle too ({})", portrait_diff(&normal, &battle));
        } else {
            assert!(portrait_diff(&normal, &battle) > 300, "{outfit}: the battle face differs from the normal one ({})", portrait_diff(&normal, &battle));
        }
        rig.frames(outfit, 1, 90, &["status hp 10"]);
        assert_eq!(rig.look(0), Some(2), "{outfit}: at a pinch the low-HP look wins over battle");
        let lowhp = rig.corner(&format!("of_{outfit}_lowhp"));
        rig.frames(outfit, 1, 10, &["status clear"]);
        // Donald leads, Sora in a friend slot, in combat
        rig.hud.set_party(&[slot(PartyChara::Donald, true), slot(PartyChara::Goofy, true)]);
        rig.hud.set_leader(PartyChara::Donald, true);
        rig.frames(outfit, 1, 120, &[]);
        let seat = rig.hud.seat_of(PartyChara::Sora).expect("Sora sits in a friend slot");
        assert_ne!(seat, 0);
        let want = if *outfit == "kh3" || *movie == "FacePcExSora01" { "FaceNpcExSora" } else { movie };
        assert_eq!(rig.face(seat).as_deref(), Some(want), "{outfit}: his face in the friend slot");
        assert_eq!(rig.look(seat), Some(1), "{outfit}: his battle look in the friend slot");
        let friend = rig.corner(&format!("of_{outfit}_friend"));
        if *outfit == "kh3" {
            kh3_normal = Some(normal.clone());
        } else if *movie != "FacePcExSora01" {
            assert!(portrait_diff(kh3_normal.as_ref().unwrap(), &normal) > 300, "{outfit}: not his own face");
        }
        for (col, pic) in [normal, battle, lowhp, friend].iter().enumerate() {
            for y in 0..ch {
                let d = ((row * ch + y) * cw * 4 + col * cw) * 4;
                sheet[d..d + cw * 4].copy_from_slice(&pic[y * cw * 4..(y + 1) * cw * 4]);
            }
        }
        assert!(rig.hud.movie("HudStatus").unwrap().misses.is_empty(), "{outfit}: paths not found: {:?}", rig.hud.movie("HudStatus").unwrap().misses);
    }
    kh_ui::def::write_png(&root().join("build/sora_tmp/hud/of_sheet.png"), (cw * 4) as u32, (ch * SORA_OUTFIT_FACES.len()) as u32, &sheet).unwrap();
}

#[test]
fn the_party_formation_screen_shows_his_bust_in_the_worn_outfit() {
    let Some(mut rig) = Rig::new() else { return };
    let mut busts = Vec::new();
    for outfit in ["kh3", "toybox", "kh1"] {
        rig.frames(outfit, 0, 2, &[]);
        assert!(rig.hud.open_party_formation(&kh_host::formation::Formation::default()));
        rig.frames(outfit, 0, 90, &[]);
        // the leader's card (Sora) of the CampMenu movie on the 1920 x 1080 stage: its bust picture
        let movie = rig.hud.movie("CampMenu").unwrap();
        assert!(movie.root.extra.iter().any(|n| n.name.as_deref() == Some("pf_bust0")), "{outfit}: the screen is up");
        let (stage, _) = kh_host::hud_layers::stage_root(1920.0, 1080.0);
        let mut canvas = kh_ui::Canvas::new(1920, 1080);
        kh_ui::Renderer::new(rig.fonts.clone()).draw(&mut canvas, movie, &stage);
        let mut rgba = Vec::new();
        canvas.to_rgba8_over(|_, _| [0.0, 0.0, 0.0], &mut rgba);
        kh_ui::def::write_png(&root().join(format!("build/sora_tmp/hud/of_formation_{outfit}.png")), 1920, 1080, &rgba).unwrap();
        busts.push(rgba);
        rig.frames(outfit, 0, 30, &["menu cancel"]);
        rig.frames(outfit, 0, 60, &[]);
    }
    let diff = |a: &[u8], b: &[u8]| a.chunks(4).zip(b.chunks(4)).filter(|(p, q)| (0..3).any(|c| p[c].abs_diff(q[c]) > 24)).count();
    assert!(diff(&busts[0], &busts[1]) > 2000 && diff(&busts[0], &busts[2]) > 2000, "the bust follows the outfit ({}, {})", diff(&busts[0], &busts[1]), diff(&busts[0], &busts[2]));
}

#[test]
fn a_change_at_boc_or_a_load_swaps_the_face_at_once_and_keeps_the_battle_look() {
    let Some(mut rig) = Rig::new() else { return };
    rig.frames("kh3", 1, 90, &[]);
    assert_eq!((rig.face(0).as_deref(), rig.look(0)), (Some("FacePcExSora01"), Some(1)));
    // the outfit changes in combat (Boc's screen, or a save with another outfit loaded): the new face, still in battle
    rig.frames("toybox", 1, 30, &[]);
    assert_eq!((rig.face(0).as_deref(), rig.look(0)), (Some("FacePcTsSora01"), Some(1)));
    assert_eq!(rig.hud.sora_outfit(), "toybox");
    // a load: the leader reset, then the loaded save's outfit
    rig.hud.new_character();
    rig.frames("caribbean", 0, 120, &[]);
    assert_eq!((rig.face(0).as_deref(), rig.look(0)), (Some("FacePcCaSora01"), Some(0)));
    // an outfit KH3 gives his own face (Quadratum), and one no table names: his own face
    rig.frames("verumrex", 0, 30, &[]);
    assert_eq!(rig.face(0).as_deref(), Some("FacePcExSora01"));
    assert_eq!(sora_face_movie("a_new_outfit"), "FacePcExSora01");
}
