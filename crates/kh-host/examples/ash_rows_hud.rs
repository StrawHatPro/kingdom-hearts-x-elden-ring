//! (ASH-ROWS, THE USER 2026-11-05: "For the ash summons, make the bubble dynamically extend to show the whole name (only
//! for spirit ashes)") THE SUMMON LIST WITH ELDEN RING'S LONGEST SPIRIT-ASH NAMES, OFFLINE, THROUGH THE REAL HUD: Sora with
//! four ashes equipped at +10 (test data: names and costs as kh-er sends them), Donald and Goofy in the party, the Summon list opened by the pad (D-pad down x3, A), the cursor on each row; the whole cockpit (status, command, target, side HUDs) at 16:9 and at 21:9, 4:3 and 16:10
//! windows (the stage keeps 16:9 inside them), each also as a 2x crop of the bottom-left corner.
//!
//!   cargo run -p kh-host --example ash_rows_hud -- [OUT_DIR]
//!
//! Default OUT_DIR: build/sora_tmp/weapons/ASH-ROWS/shots.

use kh_core::command::summon::{AshSlot, SummonView};
use kh_core::host::NullHost;
use kh_core::test_world::TestWorld;
use kh_core::{Input, PadButton, PadButtons, PadState, Pose, Sora, TargetId, Vec3, SORA_PARAMS};
use kh_host::hud::{spirit_in_slot, Frame, Hud, HudTargets, TargetView};
use kh_host::hud_layers::stage_root;
use kh_host::party::{PartyChara, PartyHudSlot};

const DT: f32 = 1.0 / 60.0;

struct NoTargets;
impl HudTargets for NoTargets {
    fn target(&self, _id: TargetId) -> Option<TargetView> {
        None
    }
}

fn same(p: Vec3) -> Vec3 {
    p
}

fn ash(goods: u32, name: &str, mp: i32) -> AshSlot {
    AshSlot { goods, level: kh_core::command::summon::level_of(goods), name: name.into(), mp, renowned: true, mimic: goods / 1000 == 207, usable: true, icon: None }
}

/// The four longest names among Elden Ring's spirit ashes at +10 (and the Mimic Tear: "All").
fn owned() -> Vec<AshSlot> {
    vec![ash(200010, "Black Knife Tiche +10", 132), ash(207010, "Mimic Tear Ashes +10", 99), ash(212010, "Lhutel the Headless +10", 98), ash(229010, "Nightmaiden & Swordstress Puppets +10", 106)]
}

fn view(sora: &Sora) -> SummonView {
    let mut v = SummonView { allowed: true, bell: true, owned: owned(), ..SummonView::default() };
    for (slot, base) in sora.command().spirit_ashes.iter().enumerate() {
        v.slots[slot] = spirit_in_slot(&v, *base).cloned();
    }
    v
}

struct Rig {
    hud: Hud,
    fonts: kh_ui::Fonts,
    sora: Sora,
    world: TestWorld,
    held: PadButtons,
    out: std::path::PathBuf,
}

impl Rig {
    fn frames(&mut self, n: u32, pad: &[PadButton]) {
        for k in 0..n {
            // (no spirit out: one out turns the row into Unsummon)
            self.sora.command_mut().summon = view(&self.sora);
            let now = if k == 0 { pad.iter().fold(PadButtons::NONE, |all, b| all.with(*b)) } else { PadButtons::NONE };
            let input = Input::default().with_pad(PadState::from_held(now, self.held));
            self.held = now;
            let mut null = NullHost::default();
            let mut host = null.host(&self.world);
            let state = if self.hud.paused() { kh_core::StateId::Idle } else { self.sora.step_hosted(DT, &input, &mut host).state };
            let mut snapshot = self.sora.ui_snapshot();
            snapshot.gauges.hp = snapshot.gauges.hp_max * 7 / 10;
            self.hud.frame(&mut Frame { snapshot: &snapshot, sora: &self.sora, events: &[], state, step_dt: DT, play_seconds: 100, targets: &NoTargets, to_view: same, view: None, words: &[], dt: DT, ticks: 1, pressed: PadButtons::NONE, nav: Vec::new() });
            self.hud.apply_actions(&mut self.sora);
            let _ = self.hud.take_sounds();
        }
    }

    fn press(&mut self, b: PadButton) {
        self.frames(1, &[b]);
        self.frames(14, &[]);
    }

    fn shoot(&self, name: &str, (w, h): (usize, usize)) {
        let (root, (left, top, sw, sh)) = stage_root(w as f32, h as f32);
        let mut canvas = kh_ui::Canvas::new(w, h);
        let mut r = kh_ui::Renderer::new(self.fonts.clone());
        for m in ["HudStatus", "HudCommand", "HudTarget", "HudLeft", "HudRight"] {
            if let Some(movie) = self.hud.movie(m) {
                r.draw(&mut canvas, movie, &root);
            }
        }
        let mut rgba = Vec::new();
        // a mid scene inside the stage, black bars outside it (the window's parts the 16:9 stage does not cover)
        canvas.to_rgba8_over(
            |x, y| {
                let (fx, fy) = (x as f32, y as f32);
                if fx < left || fx >= left + sw || fy < top || fy >= top + sh {
                    return [0.0, 0.0, 0.0];
                }
                let t = (fy - top) / sh;
                [0.42 - 0.12 * t, 0.44 - 0.1 * t, 0.46 - 0.16 * t]
            },
            &mut rgba,
        );
        let tag = format!("{w}x{h}");
        kh_ui::def::write_png(&self.out.join(format!("hud_{name}_{tag}.png")), w as u32, h as u32, &rgba).unwrap();
        if (w, h) == (1920, 1080) {
            let (x0, y0, cw, ch, z) = (0usize, 660usize, 1000usize, 420usize, 2usize);
            let mut big = vec![0u8; cw * z * ch * z * 4];
            for y in 0..ch * z {
                for x in 0..cw * z {
                    let s = ((y0 + y / z) * w + x0 + x / z) * 4;
                    let d = (y * cw * z + x) * 4;
                    big[d..d + 4].copy_from_slice(&rgba[s..s + 4]);
                }
            }
            kh_ui::def::write_png(&self.out.join(format!("hud_{name}_corner2x.png")), (cw * z) as u32, (ch * z) as u32, &big).unwrap();
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = args.first().map(std::path::PathBuf::from).unwrap_or_else(|| root.join("build/sora_tmp/weapons/ASH-ROWS/shots"));
    std::fs::create_dir_all(&out).unwrap();
    let (mut hud, fonts) = Hud::load(&root.join("build/sora_pack/ui")).expect("the UI pack");
    let origin = Vec3::Z * Sora::origin_height_on_floor(&SORA_PARAMS);
    let mut sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None)));
    kh_core::command::customize::ensure(&mut sora);
    sora.command_mut().spirit_ashes = [200000, 207000, 212000, 229000];
    let donald = PartyHudSlot { chara: PartyChara::Donald, hp: 140, hp_max: 200, mp: 70, mp_max: 100, mp_charging: false, mp_charge_value: 0.0, in_battle: true, focus: None };
    let goofy = PartyHudSlot { chara: PartyChara::Goofy, hp: 160, hp_max: 220, mp: 60, mp_max: 100, mp_charging: false, mp_charge_value: 0.0, in_battle: true, focus: None };
    hud.set_party(&[donald, goofy]);
    hud.set_leader(PartyChara::Sora, true);
    let mut r = Rig { hud, fonts, sora, world: TestWorld::floor(), held: PadButtons::NONE, out };
    r.frames(150, &[]);
    for _ in 0..3 {
        r.press(PadButton::DPadDown);
    }
    r.press(PadButton::ATTACK);
    r.frames(40, &[]);
        let movie = r.hud.movie("HudCommand").unwrap();
    for n in 1..=9 {
        let row = format!("cmdMenu.cmdSubTop.cmdSub1Set.cmdSub1_{n}");
        let name = movie.text(&format!("{row}.cmdText1.txCmdName.txCmdNameCol.coCmdName")).unwrap_or_default();
        if !name.is_empty() {
            let by = movie.sprite(&format!("{row}.cmdSubBase1")).and_then(|s| s.stretch).map_or(0.0, |s| s.by);
            let number = movie.text(&format!("{row}.cmdText1.cmdNumber.cmdNumCol.txCmdNumber.cmdNumberTxt")).unwrap_or_default();
            println!("row {n}: {name:?} {number:?} wider by {by}");
        }
    }
    for size in [(1920, 1080), (2560, 1080), (1440, 1080), (1280, 800)] {
        r.shoot("1_summon_cursor_first", size);
    }
    for k in 1..4 {
        r.press(PadButton::DPadDown);
        r.frames(30, &[]);
        r.shoot(&format!("{}_summon_cursor_row{}", k + 1, k + 1), (1920, 1080));
    }
    // mid slide: the cursor moving up onto the longest-but-one
    r.frames(1, &[PadButton::DPadUp]);
    r.frames(3, &[]);
    r.shoot("5_summon_cursor_mid_slide", (1920, 1080));
    r.frames(30, &[]);
    // (Items after Summon on the same row clips: kh-ui test ash_rows, picture rows_5_items_after_summon_authored; this
    // Sora has no items, his Items row is grey)
}
