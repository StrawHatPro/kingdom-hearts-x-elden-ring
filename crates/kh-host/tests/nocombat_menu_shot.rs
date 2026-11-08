//! (NOCOMBAT) THE GREYED COMMAND MENU, OFFLINE, through the REAL `Hud` (every layer drawn by the kh_ui software renderer):
//! Sora's menu outside the zone (lit) and where Elden Ring forbids attacks (everything but Items greyed, KH3's `disabled`
//! look), his Items list still open-able, and each friend's menu as leader (Donald's own, Goofy's). The rule is driven by
//! kh_core::nocombat's thread scope (the game-state reader's answer, mocked). Pictures: build/sora_tmp/weapons/NOCOMBAT/shots/.

use kh_core::{PadButton, PadButtons, PadState, Sora, TargetId, Vec3};
use kh_host::hud::{Frame, Hud, HudTargets, TargetView};
use kh_host::hud_layers::{stage_root, LAYERS};

struct NoTargets;
impl HudTargets for NoTargets {
    fn target(&self, _id: TargetId) -> Option<TargetView> {
        None
    }
}

fn same(p: Vec3) -> Vec3 {
    p
}

const W: usize = 1920;
const H: usize = 1080;

fn frames(hud: &mut Hud, sora: &Sora, snapshot: &kh_core::ui_model::UiSnapshot, n: u32) {
    for _ in 0..n {
        hud.frame(&mut Frame { snapshot, sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 3725, targets: &NoTargets, to_view: same, view: None, words: &[], dt: 1.0 / 60.0, ticks: 1, pressed: PadButtons::NONE, nav: Vec::new() });
    }
}

/// The command menu's corner (bottom left) at full size.
fn shot(hud: &Hud, fonts: &kh_ui::Fonts, out: &std::path::Path, name: &str) {
    let (root, _) = stage_root(W as f32, H as f32);
    let mut canvas = kh_ui::Canvas::new(W, H);
    let mut renderer = kh_ui::Renderer::new(fonts.clone());
    for layer in LAYERS {
        if let Some(movie) = hud.movie(layer) {
            renderer.draw(&mut canvas, movie, &root);
        }
    }
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|_, y| [0.30 + 0.1 * (y as f32 / 1080.0), 0.38, 0.46], &mut rgba);
    let (x0, y0, cw, ch) = (0usize, 560usize, 760usize, 520usize);
    let mut crop = vec![0u8; cw * ch * 4];
    for y in 0..ch {
        let s = ((y0 + y) * W + x0) * 4;
        crop[y * cw * 4..(y + 1) * cw * 4].copy_from_slice(&rgba[s..s + cw * 4]);
    }
    kh_ui::def::write_png(&out.join(format!("{name}.png")), cw as u32, ch as u32, &crop).unwrap();
}

/// The level-99 test loadout (sheets/port/sora-loadout-99.json: spells, shortcuts, a pouch of items).
fn level99() -> kh_core::Status {
    use kh_core::status::LoadoutValue;
    let cell = |json: &serde_json::Value| -> Option<LoadoutValue> {
        match json {
            serde_json::Value::Number(n) => n.as_f64().map(LoadoutValue::Number),
            serde_json::Value::String(s) => Some(LoadoutValue::Text(s.clone())),
            serde_json::Value::Array(items) => items.iter().map(|i| i.as_str().map(str::to_string)).collect::<Option<Vec<_>>>().map(LoadoutValue::List),
            serde_json::Value::Object(map) => map.iter().map(|(k, v)| v.as_f64().map(|n| (k.clone(), n))).collect::<Option<Vec<_>>>().map(LoadoutValue::Map),
            _ => None,
        }
    };
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap();
    let mut loadout = kh_core::Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let key = row[0].as_str().unwrap();
        let value = if key == "abilities.equipped" { LoadoutValue::List(Vec::new()) } else { cell(&row[1]).unwrap() };
        loadout.set(key, &value).unwrap_or_else(|e| panic!("{e}"));
    }
    loadout.resolve().unwrap();
    kh_core::Status::from_loadout(&loadout)
}

/// Sora stepped `n` frames with nothing pressed (the snapshot the HUD reads), then one press each of `presses`.
fn stepped(sora: &mut Sora, presses: &[PadButton]) {
    let world = kh_core::test_world::TestWorld::floor();
    for _ in 0..10 {
        sora.step(1.0 / 60.0, &kh_core::Input::default(), &world);
    }
    for b in presses {
        let down = PadState::from_held(PadButtons::NONE.with(*b), PadButtons::NONE);
        sora.step(1.0 / 60.0, &kh_core::Input::default().with_pad(down), &world);
        sora.step(1.0 / 60.0, &kh_core::Input::default().with_pad(PadState::from_held(PadButtons::NONE, PadButtons::NONE.with(*b))), &world);
    }
}

#[test]
fn nocombat_the_greyed_command_menu_offline() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let Ok((mut hud, fonts)) = Hud::load(&root.join("build/sora_pack/ui")) else {
        eprintln!("no UI pack: skipped");
        return;
    };
    let out = root.join("build/sora_tmp/weapons/NOCOMBAT/shots");
    let _ = std::fs::create_dir_all(&out);
    let mut sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(level99());
    // 1. outside the zone: lit
    {
        let _rule = kh_core::nocombat::scope(false);
        stepped(&mut sora, &[]);
        let snap = sora.ui_snapshot();
        frames(&mut hud, &sora, &snap, 120);
        shot(&hud, &fonts, &out, "01_sora_outside_lit");
    }
    let _rule = kh_core::nocombat::scope(true);
    // 2. in the zone: greyed but Items
    stepped(&mut sora, &[]);
    let snap = sora.ui_snapshot();
    assert!(!snap.command.main[0].enabled && !snap.command.main[1].enabled, "Attack and Magic greyed");
    frames(&mut hud, &sora, &snap, 60);
    shot(&hud, &fonts, &out, "02_sora_zone_greyed");
    // 3. the cursor on Items, its list opened (A): it opens
    stepped(&mut sora, &[PadButton::DPadDown, PadButton::DPadDown, PadButton::ATTACK]);
    let snap = sora.ui_snapshot();
    frames(&mut hud, &sora, &snap, 40);
    shot(&hud, &fonts, &out, "03_sora_zone_items_open");
    eprintln!("Sora's Items list state: {:?}, rows {:?}", snap.command.state, snap.command.lists[2].iter().map(|r| (r.name, r.enabled)).collect::<Vec<_>>());
    // 4, 5. each friend's own menu as leader (the friend's snapshot in the command slot, as the host does while he leads)
    let mut party = kh_party::Party::new(kh_party::PartyConfig::test_level99());
    for (k, chara) in kh_party::Chara::ALL.into_iter().enumerate() {
        let id = party.spawn(chara, Vec3::ZERO, 0.0);
        party.set_pad(id, Some(kh_core::Tuning::USER));
        let mut snap = sora.ui_snapshot();
        snap.command = party.pad_menu(id).expect("his menu");
        assert!(!snap.command.main[0].enabled, "{chara:?}: Attack greyed");
        frames(&mut hud, &sora, &snap, 60);
        shot(&hud, &fonts, &out, &format!("{:02}_{}_zone_greyed", 4 + k, chara.name().to_lowercase()));
    }
    eprintln!("pictures: {}", out.display());
}
