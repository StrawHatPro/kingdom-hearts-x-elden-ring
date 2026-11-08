//! (BUTTON-ICONS, THE USER 2026-11-05: remapped buttons and keys must show in the UI and HUD, in Xbox, PlayStation and
//! keyboard + mouse glyphs, chosen in the settings menu; "xbox and playstation ... use the same mappings as each other")
//! THE REAL PACK (build/sora_pack/ui + the staged keyboard glyphs build/sora_tmp/weapons/BUTTON-ICONS/stage/ui; skipped
//! without the pack):
//! - every button icon the UI draws (the code's names and every `<icon(...)>` of the string tables the UI shows) resolves to
//!   a glyph of the pack in every set, with the default and a swapped table;
//! - a swapped table moves every icon of the role (Confirm on B: every Confirm icon is B / Circle / the new key);
//! - one controller table drawn as Xbox and as PlayStation: the same positions, the two sets' pictures;
//! - pictures: the HUD (ATTACK!, the Y event button, a situation command) and the settings screen (Display with Button
//!   Icons, Button Calibration, Key Configuration) in the three sets, with and without a swap:
//!   build/sora_tmp/weapons/BUTTON-ICONS/shots/.

use kh_core::{PadButton, PadButtons, Sora, StateId, TargetId, Vec3};
use kh_host::button_icons::glyph_state;
use kh_host::er_options::{Controller, MockGame};
use kh_host::hud::{Frame, Hud, HudTargets, TargetView};
use kh_host::hud_layers::stage_root;
use kh_host::remap::{Device, Table, MOUSE_LEFT};
use kh_ui::glyphs::{Family, GlyphState};

struct NoTargets;
impl HudTargets for NoTargets {
    fn target(&self, _id: TargetId) -> Option<TargetView> {
        None
    }
}

fn same(p: Vec3) -> Vec3 {
    p
}

fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn out_dir() -> std::path::PathBuf {
    let d = root().join("build/sora_tmp/weapons/BUTTON-ICONS/shots");
    std::fs::create_dir_all(&d).ok();
    d
}

/// The staged keyboard glyphs searched before the pack (kh-ui def.rs `KH_UI_OVERLAY`).
fn with_stage() {
    std::env::set_var("KH_UI_OVERLAY", root().join("build/sora_tmp/weapons/BUTTON-ICONS/stage/ui"));
}

/// THE BUTTON ICONS THE UI DRAWS FROM CODE (the inventory, STATE.md 1): the HUD (button notice, command menu, shortcuts,
/// situation / event rows, the forms' operation guide table, the Arcane stars), the menus' guides and tabs, the parry page.
const CODE_ICONS: &[&str] = &[
    "Cross2", "Circle2", "R22", "Circle", "Triangle", "Square", "Cross", "L1", "R1", "L2", "R2", "Pad", "R2Hold", "Decide", "Cancel", "Triangle2", "Square2", "KeyUp", "KeyLeft", "KeyDown", "KeyRight", "KeyUp2", "KeyLeft2", "KeyDown2",
    "KeyRight2", "Pad2", "L22", "Option", "Lstick", "Rstick", "LstickUpGre", "LstickDownGre", "LstickLeft", "LstickRightGre", "L3", "Decide2", "Cancel2", "KeyUpDown", "KeyLeftyRight", "KeyLeftRight", "LstickUp", "LstickDown", "LstickRight",
    "LstickLeftRight", "LstickUpDown", "RstickUp", "RstickDown", "RstickLeft", "RstickRight", "Sd_Option", "Sd_Pad", "Key", "R3",
];

/// Every button icon name in the string tables of the pack (help pages, guides, details, hints).
fn string_icons() -> Vec<String> {
    let mut out = std::collections::BTreeSet::new();
    let dir = root().join("build/sora_pack/ui/strings/en");
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let text = std::fs::read_to_string(entry.path()).unwrap_or_default();
        for piece in text.split("<icon(").skip(1) {
            if let Some(name) = piece.split(")>").next().filter(|n| n.len() < 40 && kh_ui::glyphs::is_button(n)) {
                out.insert(name.to_string());
            }
        }
    }
    out.into_iter().collect()
}

/// The six cases: each set, with the default tables and with Confirm swapped (pad: onto B; keyboard: onto Space).
fn cases() -> Vec<(&'static str, GlyphState, Table, Table)> {
    let (pad, kbm) = (Table::defaults(Device::Pad), Table::defaults(Device::Kbm));
    let mut pad_s = pad.clone();
    pad_s.assign(PadButton::FaceBottom as usize, PadButton::FaceRight as u16);
    let mut kbm_s = kbm.clone();
    kbm_s.assign(0, 0x20);
    let mut out = Vec::new();
    for (name, family, pad_family) in [("xbox", Family::Xbox, Family::Xbox), ("playstation", Family::PlayStation, Family::PlayStation), ("kbm", Family::Kbm, Family::Xbox)] {
        out.push((name, glyph_state((family, pad_family), &pad, &kbm), pad.clone(), kbm.clone()));
        let tag: &'static str = match name {
            "xbox" => "xbox_swapped",
            "playstation" => "playstation_swapped",
            _ => "kbm_swapped",
        };
        out.push((tag, glyph_state((family, pad_family), &pad_s, &kbm_s), pad_s.clone(), kbm_s.clone()));
    }
    out
}

fn icon_set() -> Option<kh_ui::render::IconSet> {
    with_stage();
    let mut pack = kh_ui::Pack::open_default().ok()?;
    kh_ui::render::IconSet::load(&mut pack)
}

#[test]
fn bi_every_ui_button_icon_resolves_to_a_glyph_in_every_set() {
    let Some(set) = icon_set() else {
        eprintln!("no UI pack: skipped");
        return;
    };
    assert!(!set.km.is_empty(), "the keyboard glyphs (stage/ui/data/icon_text_km.json)");
    let strings = string_icons();
    assert!(strings.len() > 20, "the tables' button icons: {strings:?}");
    let names: Vec<String> = CODE_ICONS.iter().map(|s| s.to_string()).chain(strings).collect();
    let tex_of = |family: Family| -> Vec<usize> {
        match family {
            Family::Kbm => set.km.values().map(|r| r.0).collect(),
            f => set.pad_sets[&f].values().map(|r| r.0).collect(),
        }
    };
    for (case, state, ..) in cases() {
        for name in &names {
            let g = set.glyphs_strict(&state, name).unwrap_or_else(|| panic!("{case}: <icon({name})> has no glyph"));
            // the role icons draw from the case's own set (the sticks of the keyboard set: its keys and its mouse)
            let fam = if state.family == Family::Kbm { Family::Kbm } else { state.family };
            assert!(g.iter().all(|g| tex_of(fam).contains(&g.tex)), "{case}: <icon({name})> drawn from another set");
        }
        eprintln!("{case}: {} button icons, every one a glyph", names.len());
    }
}

#[test]
fn bi_a_swap_moves_every_icon_of_the_role_and_one_pad_table_draws_in_both_arts() {
    let Some(set) = icon_set() else { return };
    for (case, state, pad, kbm) in cases() {
        let swapped = case.ends_with("swapped");
        let confirm = set.glyphs_strict(&state, "Decide").unwrap();
        let cross = set.glyphs_strict(&state, "Cross").unwrap();
        assert_eq!(confirm, cross, "{case}: Decide is Cross's role");
        match state.family {
            Family::Kbm => {
                let key = |k: &str| set.glyphs_strict(&state, &format!("KmKey_{k}")).unwrap();
                assert_eq!(confirm, if swapped { key("SpaceBar") } else { key("LeftMouseButton") }, "{case}");
                assert_eq!(set.glyphs_strict(&state, "Cancel").unwrap(), if swapped { key("LeftMouseButton") } else { key("SpaceBar") }, "{case}");
                assert_eq!(kbm.assign[0], if swapped { 0x20 } else { MOUSE_LEFT });
                // a key KH3 has no glyph for: KH3 PC's Blank
                let tab = set.glyphs_strict(&state, "KmKey_Tab").unwrap();
                assert_eq!(tab, vec![kh_ui::render::Glyph { tex: set.km["blank"].0, src: (set.km["blank"].1, set.km["blank"].2, set.km["blank"].3, set.km["blank"].4), w: 30.0, h: 30.0 }]);
            }
            _ => {
                let phys = |n: &str| set.glyphs_strict(&state, &format!("Phys_{n}")).unwrap();
                assert_eq!(confirm, if swapped { phys("Circle") } else { phys("Cross") }, "{case}: every Confirm icon is the button that holds Confirm");
                assert_eq!(set.glyphs_strict(&state, "Cancel2").unwrap(), if swapped { phys("Cross2") } else { phys("Circle2") }, "{case}: the notice's DODGE! too");
                assert_eq!(pad.assign[0], if swapped { 1 } else { 0 });
            }
        }
    }
    // ONE TABLE, TWO ARTS: the same rows (positions), the Xbox picture and the PlayStation picture
    let mut pad = Table::defaults(Device::Pad);
    pad.assign(PadButton::FaceBottom as usize, PadButton::FaceRight as u16);
    pad.assign(PadButton::FaceTop as usize, PadButton::RightShoulder as u16);
    let kbm = Table::defaults(Device::Kbm);
    let xb = glyph_state((Family::Xbox, Family::Xbox), &pad, &kbm);
    let ps = glyph_state((Family::PlayStation, Family::PlayStation), &pad, &kbm);
    for name in CODE_ICONS {
        let (a, b) = (set.glyphs_strict(&xb, name).unwrap(), set.glyphs_strict(&ps, name).unwrap());
        assert_eq!(a.len(), b.len(), "{name}");
        for (a, b) in a.iter().zip(&b) {
            assert_eq!((a.src, a.w, a.h), (b.src, b.w, b.h), "{name}: the same position (the sets share their layout)");
            assert_ne!(a.tex, b.tex, "{name}: another picture");
        }
    }
    assert_eq!(set.glyphs_strict(&xb, "Triangle").unwrap(), set.glyphs_strict(&xb, "Phys_R1").unwrap(), "Y's role on RB");
}

// ------------------------------------------------------------------------------------------------ pictures

fn sora() -> Sora {
    Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None)))
}

fn snapshot(sora: &Sora) -> kh_core::UiSnapshot {
    let mut snapshot = sora.ui_snapshot();
    for (row, (kind, name)) in snapshot.command.main.iter_mut().zip([(1u8, "Attack"), (28, "Magic"), (57, "Items"), (0, "Link")]) {
        (row.kind, row.name, row.enabled) = (kind, name, kind != 0);
    }
    snapshot
}

fn step(hud: &mut Hud, sora: &Sora, snap: &mut kh_core::UiSnapshot, n: usize, words: &[&str]) {
    for k in 0..n {
        let w: Vec<String> = if k == 0 { words.iter().map(|s| s.to_string()).collect() } else { Vec::new() };
        snap.step += 1;
        hud.frame(&mut Frame { snapshot: snap, sora, events: &[], state: StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 0, targets: &NoTargets, to_view: same, view: None, words: &w, dt: 1.0 / 60.0, ticks: 1, pressed: PadButtons::NONE, nav: Vec::new() });
    }
}

fn shot(hud: &Hud, fonts: &kh_ui::Fonts, movies: &[&str]) -> Vec<u8> {
    let (root_m, _) = stage_root(1920.0, 1080.0);
    let mut canvas = kh_ui::Canvas::new(1920, 1080);
    let mut renderer = kh_ui::Renderer::new(fonts.clone());
    for movie in movies {
        if let Some(m) = hud.movie(movie) {
            renderer.draw(&mut canvas, m, &root_m);
        }
    }
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|x, y| { let t = y as f32 / 1080.0; [0.27 + 0.2 * t, 0.19 + 0.08 * t, 0.16 + 0.02 * ((x / 64) % 2) as f32] }, &mut rgba);
    rgba
}

fn crop(src: &[u8], (x0, y0, w, h): (usize, usize, usize, usize)) -> Vec<u8> {
    let mut out = vec![0u8; w * h * 4];
    for y in 0..h {
        let s = ((y0 + y) * 1920 + x0) * 4;
        out[y * w * 4..(y + 1) * w * 4].copy_from_slice(&src[s..s + w * 4]);
    }
    out
}

fn save(name: &str, w: usize, h: usize, rgba: &[u8]) {
    kh_ui::def::write_png(&out_dir().join(name), w as u32, h as u32, rgba).unwrap();
}

/// The pictures of one case: the HUD (ATTACK!, the Save event button, a situation command), the settings screen's Display
/// tab (Button Icons), Button Calibration and Key Configuration, each cropped to what holds the icons.
fn case_pictures(case: &str, state: &GlyphState, pad: &Table, kbm: &Table) -> Vec<(String, usize, usize, Vec<u8>)> {
    let (mut hud, fonts) = Hud::load(&root().join("build/sora_pack/ui")).expect("the UI pack");
    kh_ui::glyphs::set_state(state.clone());
    let mut game = MockGame::default();
    game.ini.insert("remap.pad".into(), pad.format());
    game.ini.insert("remap.kbm".into(), kbm.format());
    game.ini.insert(kh_host::button_icons::SETTING_KEY.into(), match state.family {
        Family::Xbox => "1",
        Family::PlayStation => "2",
        Family::Kbm => "3",
    }.into());
    hud.options_host = Some(Box::new(Controller::new(game)));
    let sora = sora();
    let mut snap = snapshot(&sora);
    step(&mut hud, &sora, &mut snap, 90, &[]);
    // ---- the HUD: the reaction command's "ATTACK!" (Decide2), the Save event button (Triangle), a situation command
    hud.set_host_command(Some(Hud::SAVE_COMMAND));
    snap.command.situations = vec![kh_core::command::SituationRow { serial: 7, kind: 0, name: "Second Form", icon_type: 1, total: 20.0, remaining: 5.0, enabled: true, refreshed: 0 }];
    snap.prompt = Some(kh_core::ui_model::ButtonPrompt { kind: kh_core::ui_model::PromptKind::Reaction(2), target: TargetId::NONE });
    step(&mut hud, &sora, &mut snap, 40, &[]);
    let full = shot(&hud, &fonts, &["HudCommand", "HudTarget"]);
    save(&format!("{case}_hud.png"), 1920, 1080, &full);
    let mut out = vec![(format!("{case}: HUD command menu"), 640, 360, crop(&full, (0, 700, 640, 360))), (format!("{case}: ATTACK! notice"), 640, 360, crop(&full, (640, 600, 640, 360)))];
    // DODGE! (Cancel2)
    snap.prompt = Some(kh_core::ui_model::ButtonPrompt { kind: kh_core::ui_model::PromptKind::RiskDodge, target: TargetId::NONE });
    step(&mut hud, &sora, &mut snap, 30, &[]);
    let dodge = shot(&hud, &fonts, &["HudTarget"]);
    out.push((format!("{case}: DODGE! notice"), 640, 360, crop(&dodge, (640, 600, 640, 360))));
    snap.prompt = None;
    hud.set_host_command(None);
    snap.command.situations.clear();
    step(&mut hud, &sora, &mut snap, 30, &[]);
    // ---- the settings screen: Display (Button Icons selected), Button Calibration, Key Configuration
    assert!(hud.open_options(), "the options screen opens");
    step(&mut hud, &sora, &mut snap, 90, &[]);
    step(&mut hud, &sora, &mut snap, 30, &["menu r1"]);
    step(&mut hud, &sora, &mut snap, 30, &["menu r1"]);
    for _ in 0..7 {
        step(&mut hud, &sora, &mut snap, 6, &["menu down"]);
    }
    step(&mut hud, &sora, &mut snap, 120, &[]);
    let display = shot(&hud, &fonts, &["CampMenu", "Dialog"]);
    save(&format!("{case}_settings_display.png"), 1920, 1080, &display);
    out.push((format!("{case}: settings, Display"), 960, 540, crop(&display, (0, 0, 1920, 1080)).chunks(1920 * 4).step_by(2).flat_map(|r| r.chunks(4).step_by(2).flatten().copied().collect::<Vec<u8>>()).collect()));
    step(&mut hud, &sora, &mut snap, 30, &["menu r1"]);
    step(&mut hud, &sora, &mut snap, 30, &["menu r1"]);
    step(&mut hud, &sora, &mut snap, 30, &["menu decide"]);
    let pad_page = shot(&hud, &fonts, &["CampMenu", "Dialog"]);
    save(&format!("{case}_settings_button_calibration.png"), 1920, 1080, &pad_page);
    out.push((format!("{case}: Button Calibration"), 960, 540, pad_page.chunks(1920 * 4).step_by(2).flat_map(|r| r.chunks(4).step_by(2).flatten().copied().collect::<Vec<u8>>()).collect()));
    step(&mut hud, &sora, &mut snap, 20, &["menu cancel"]);
    step(&mut hud, &sora, &mut snap, 30, &["menu r1"]);
    step(&mut hud, &sora, &mut snap, 30, &["menu decide"]);
    let kbm_page = shot(&hud, &fonts, &["CampMenu", "Dialog"]);
    save(&format!("{case}_settings_key_configuration.png"), 1920, 1080, &kbm_page);
    out.push((format!("{case}: Key Configuration"), 960, 540, kbm_page.chunks(1920 * 4).step_by(2).flat_map(|r| r.chunks(4).step_by(2).flatten().copied().collect::<Vec<u8>>()).collect()));
    out
}

#[test]
fn bi_pictures_of_the_hud_and_the_settings_in_every_set_with_and_without_a_swap() {
    with_stage();
    if kh_ui::Pack::open_default().is_err() {
        return;
    }
    let mut sheet_rows = Vec::new();
    for (case, state, pad, kbm) in cases() {
        let pics = case_pictures(case, &state, &pad, &kbm);
        eprintln!("{case}: {} pictures", pics.len());
        sheet_rows.push(pics);
    }
    // contact sheet of the HUD crops (3 per case: command menu, ATTACK!, DODGE!), one case a row
    let (cw, ch) = (640usize, 360usize);
    let (sw, sh) = (cw * 3, ch * sheet_rows.len());
    let mut sheet = vec![0u8; sw * sh * 4];
    for (r, pics) in sheet_rows.iter().enumerate() {
        for (c, (_, w, h, rgba)) in pics.iter().take(3).enumerate() {
            for y in 0..*h.min(&ch) {
                let d = ((r * ch + y) * sw + c * cw) * 4;
                sheet[d..d + w.min(&cw) * 4].copy_from_slice(&rgba[y * w * 4..y * w * 4 + w.min(&cw) * 4]);
            }
        }
    }
    save("sheet_hud.png", sw, sh, &sheet);
    kh_ui::glyphs::set_state(GlyphState::default());
}
