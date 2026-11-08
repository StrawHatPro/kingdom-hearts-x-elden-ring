//! (CAMP-OPTIONS) THE TABBED CONFIG SCREEN, OFFLINE: the camp menu's Config with Elden Ring's options (kh-host er_options.rs
//! over its mock game: the game's own defaults, English texts), drawn by the CPU compositor as the Elden Ring host's layers
//! are; one picture per tab, the Advanced Settings and HDR sub pages, the "Keep current settings?" question, a changed row,
//! then a contact sheet of them all.
//!
//!   cargo run -p kh-host --release --example co_options_sheet -- OUT_DIR

use kh_core::{PadButtons, Sora, TargetId, Vec3};
use kh_host::er_options::{Controller, MockGame};
use kh_host::hud::{Frame, Hud, HudTargets, TargetView};
use kh_host::hud_layers::stage_root;

struct NoTargets;
impl HudTargets for NoTargets {
    fn target(&self, _id: TargetId) -> Option<TargetView> {
        None
    }
}

fn same(p: Vec3) -> Vec3 {
    p
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = args.first().map(std::path::PathBuf::from).unwrap_or_else(|| root.join("build/sora_tmp/weapons/CAMP-OPTIONS/shots"));
    std::fs::create_dir_all(&out).unwrap();
    let (mut hud, fonts) = Hud::load(&root.join("build/sora_pack/ui")).expect("the UI pack");
    // (the first button capture answers B: the swap question of "Confirm")
    let game = MockGame { hdr: args.iter().any(|a| a == "--hdr"), capture_next: Some(Some(kh_core::PadButton::FaceRight as u16)), ..MockGame::default() };
    hud.options_host = Some(Box::new(Controller::new(game)));
    let sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None)));
    let snapshot = sora.ui_snapshot();
    let step = |hud: &mut Hud, words: &[&str], n: usize| {
        for k in 0..n {
            let w: Vec<String> = if k == 0 { words.iter().map(|s| s.to_string()).collect() } else { Vec::new() };
            hud.frame(&mut Frame { snapshot: &snapshot, sora: &sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 0, targets: &NoTargets, to_view: same, view: None, words: &w, dt: 1.0 / 60.0, ticks: 1, pressed: PadButtons::NONE, nav: Vec::new() });
        }
    };
    let (w, h) = (1920usize, 1080usize);
    let mut shots: Vec<(String, Vec<u8>)> = Vec::new();
    let shot = |hud: &Hud, name: &str, shots: &mut Vec<(String, Vec<u8>)>| {
        let (root_m, _) = stage_root(w as f32, h as f32);
        let mut canvas = kh_ui::Canvas::new(w, h);
        let mut renderer = kh_ui::Renderer::new(fonts.clone());
        for movie in ["CampMenu", "Dialog"] {
            if let Some(m) = hud.movie(movie) {
                renderer.draw(&mut canvas, m, &root_m);
            }
        }
        let mut rgba = Vec::new();
        canvas.to_rgba8_over(|x, y| { let g = 0.10 + 0.05 * ((x / 64 + y / 64) % 2) as f32; [g, g, g + 0.02] }, &mut rgba);
        let _ = kh_ui::def::write_png(&out.join(format!("{name}.png")), w as u32, h as u32, &rgba);
        println!("{name}");
        shots.push((name.to_string(), rgba));
    };
    step(&mut hud, &[], 20);
    assert!(hud.open_options(), "the options screen opens");
    step(&mut hud, &[], 120);
    let names = ["01_game", "02_camera", "03_display", "04_sound", "05_controller", "06_keyboard", "08_graphics", "09_quit", "10_kh"];
    for (k, name) in names.iter().enumerate() {
        if k > 0 {
            step(&mut hud, &["menu r1"], 30);
        }
        shot(&hud, name, &mut shots);
        // (CAMP-OPTIONS, BUTTON-ICONS's report) the disabled Adjust Image Quality row (HDR off): the cursor on it, then past
        // it for 120 frames: its highlight is gone
        if *name == "03_display" {
            for _ in 0..5 {
                step(&mut hud, &["menu down"], 8);
            }
            step(&mut hud, &[], 20);
            shot(&hud, "03b_display_on_disabled_row", &mut shots);
            step(&mut hud, &["menu down"], 120);
            shot(&hud, "03c_display_cursor_left_disabled_row", &mut shots);
            for _ in 0..6 {
                step(&mut hud, &["menu up"], 8);
            }
        }
    }
    // Graphics again (LB from the KH page twice), Advanced Settings (row 7)
    step(&mut hud, &["menu l1"], 20);
    step(&mut hud, &["menu l1"], 30);
    for _ in 0..6 {
        step(&mut hud, &["menu down"], 6);
    }
    step(&mut hud, &["menu decide"], 30);
    shot(&hud, "11_advanced", &mut shots);
    step(&mut hud, &["menu cancel"], 20);
    // Style: Elden Ring <-> Kingdom Hearts 3 (row 1 of Graphics)
    for _ in 0..6 {
        step(&mut hud, &["menu up"], 6);
    }
    step(&mut hud, &["menu left"], 20);
    shot(&hud, "12_style_elden_ring", &mut shots);
    step(&mut hud, &["menu right"], 20);
    shot(&hud, "13_style_kh3", &mut shots);
    // Screen Mode -> the game's question
    step(&mut hud, &["menu down"], 6);
    step(&mut hud, &["menu left"], 40);
    shot(&hud, "14_keep_question", &mut shots);
    // No (B): the previous screen mode back
    step(&mut hud, &["menu cancel"], 30);
    // Sound: Master Volume moved
    for _ in 0..3 {
        step(&mut hud, &["menu l1"], 12);
    }
    step(&mut hud, &["menu left"], 4);
    step(&mut hud, &["menu left"], 20);
    shot(&hud, "15_sound_changed", &mut shots);
    // (the button tables) Controller Settings > Button Calibration, then Keyboard/Mouse Settings > Key Configuration
    step(&mut hud, &["menu r1"], 20);
    step(&mut hud, &["menu decide"], 30);
    shot(&hud, "16_pad_remap", &mut shots);
    // A on Confirm: the capture (the mock answers B at once: the swap question), Yes: swapped
    step(&mut hud, &["menu decide"], 40);
    shot(&hud, "16b_swap_question", &mut shots);
    step(&mut hud, &["menu decide"], 30);
    shot(&hud, "16c_swapped", &mut shots);
    step(&mut hud, &["menu cancel"], 20);
    step(&mut hud, &["menu r1"], 20);
    step(&mut hud, &["menu decide"], 30);
    shot(&hud, "17_kbm_remap", &mut shots);
    step(&mut hud, &["menu r2"], 20);
    shot(&hud, "18_kbm_remap_page2", &mut shots);
    // the contact sheet: 4 columns at a quarter size
    let (cw, ch, cols) = (w / 4, h / 4, 4usize);
    let rows = shots.len().div_ceil(cols);
    let (sw, sh) = (cw * cols, ch * rows);
    let mut sheet = vec![0u8; sw * sh * 4];
    for (i, (_, rgba)) in shots.iter().enumerate() {
        let (ox, oy) = ((i % cols) * cw, (i / cols) * ch);
        for y in 0..ch {
            for x in 0..cw {
                let s = ((y * 4) * w + x * 4) * 4;
                let d = ((oy + y) * sw + ox + x) * 4;
                sheet[d..d + 4].copy_from_slice(&rgba[s..s + 4]);
            }
        }
    }
    let _ = kh_ui::def::write_png(&out.join("sheet.png"), sw as u32, sh as u32, &sheet);
    println!("sheet: {} pictures", shots.len());
}
