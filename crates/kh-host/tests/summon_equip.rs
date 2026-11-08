//! (Summon, THE USER 2026-11-02: "In the equipment menu for Sora, make a new area for him to equip spirit ashes (4) and
//! these equipped spirit ashes are what appears in the Summon command menu. Summon is just greyed out if nothing is
//! equipped.") THE SPIRIT-ASH AREA AND THE SUMMON LIST THROUGH THE REAL HUD, driven as a pad drives it (the HUD's menu words
//! are the pad's keys; the command menu by Sora's own pad steps): Sora's Equipment screen has the four slots after the
//! pouch, a slot's stock lists only his ashes (none that sits in another slot), Decide equips (`spirit_ashes` = the ash's
//! level-0 id), Square removes; the Summon list then shows the equipped ashes' names with their MP cost in KH3's number
//! field. Needs the UI pack (skipped without it). Pictures: build/sora_tmp/party/SUMMON/pics/*.png.
//! The ashes here are test data (names and costs as a host would send them), not read from Elden Ring.

use kh_core::command::summon::{AshSlot, SummonView};
use kh_core::host::NullHost;
use kh_core::test_world::TestWorld;
use kh_core::{Input, PadButton, PadButtons, PadState, Pose, Sora, TargetId, Vec3, SORA_PARAMS};
use kh_host::hud::{equip_spirit, spirit_in_slot, summon_row_text, Frame, Hud, HudTargets, TargetView};
use kh_host::hud_layers::stage_root;
use kh_ui::camp::CampScreen;
use kh_ui::camp_equip::{spirit_id, EquipLayer, KIND_SPIRIT};

const DT: f32 = 1.0 / 60.0;
/// The Equipment screen's item panes: the slot list's and the stock list's.
const A1: &str = "equip.equipDetailAnm.equipDetail.itemStatusA1";
const A2: &str = "equip.equipReplaceAnm.equipReplace.itemStatusA2";

struct NoTargets;
impl HudTargets for NoTargets {
    fn target(&self, _id: TargetId) -> Option<TargetView> {
        None
    }
}

fn same(p: Vec3) -> Vec3 {
    p
}

fn ash(goods: u32, name: &str, mp: i32, renowned: bool) -> AshSlot {
    AshSlot { goods, level: kh_core::command::summon::level_of(goods), name: name.into(), mp, renowned, mimic: goods / 1000 == 207, usable: true, icon: icon_of(goods) }
}

/// (THE USER 2026-11-03: "use the Elden Ring picture of the spirit ash") The ashes' `EquipParamGoods.iconId`, read offline
/// from the 2.7.2.0 regulation (build/sora_tmp/party/SUMMON/reg.js EquipParamGoods): what kh-er `ericon::goods_icon_id`
/// gives at run time. 113 is Black Knife Tiche's picture, 117 the Mimic Tear's (er_icons/00113.png, 00117.png).
fn icon_of(goods: u32) -> Option<u32> {
    match goods - goods % 100 {
        200000 => Some(113),
        207000 => Some(117),
        213000 => Some(103),
        241000 => Some(107),
        _ => None,
    }
}

/// What a host with spirits says: the bell, the four ashes he owns, the equipped slots resolved from `spirit_ashes`.
fn view(sora: &Sora) -> SummonView {
    let owned = vec![ash(200005, "Black Knife Tiche +5", 66, true), ash(207002, "Mimic Tear Ashes +2", 99, false), ash(241000, "Jellyfish Ashes", 6, false), ash(213003, "Lone Wolf Ashes +3", 9, false)];
    let mut v = SummonView { allowed: true, bell: true, owned, ..SummonView::default() };
    let bases = sora.command().spirit_ashes;
    for (slot, base) in bases.iter().enumerate() {
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
    fn new() -> Option<Rig> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let Ok((hud, fonts)) = Hud::load(&root.join("build/sora_pack/ui")) else {
            eprintln!("no UI pack: skipped");
            return None;
        };
        let out = root.join("build/sora_tmp/party/SUMMON/pics");
        std::fs::create_dir_all(&out).ok();
        let origin = Vec3::Z * Sora::origin_height_on_floor(&SORA_PARAMS);
        let mut sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None)));
        kh_core::command::customize::ensure(&mut sora);
        sora.command_mut().spirit_ashes[0] = 200000; // Black Knife Tiche, equipped before (at level 0: the slot keeps the ash)
        Some(Rig { hud, fonts, sora, world: TestWorld::floor(), held: PadButtons::NONE, out })
    }

    /// `n` frames as the host runs them: the summon view renewed, Sora stepped with `pad` held on the first frame, the HUD's
    /// frame with `word` on the first, the menus' actions applied.
    fn frames(&mut self, n: u32, word: Option<&str>, pad: &[PadButton]) {
        for k in 0..n {
            self.sora.command_mut().summon = view(&self.sora);
            let now = if k == 0 { pad.iter().fold(PadButtons::NONE, |all, b| all.with(*b)) } else { PadButtons::NONE };
            let input = Input::default().with_pad(PadState::from_held(now, self.held));
            self.held = now;
            let mut null = NullHost::default();
            let mut host = null.host(&self.world);
            let state = if self.hud.paused() { kh_core::StateId::Idle } else { self.sora.step_hosted(DT, &input, &mut host).state };
            let snapshot = self.sora.ui_snapshot();
            let w: Vec<String> = if k == 0 { word.map(str::to_string).into_iter().collect() } else { Vec::new() };
            self.hud.frame(&mut Frame { snapshot: &snapshot, sora: &self.sora, events: &[], state, step_dt: DT, play_seconds: 100, targets: &NoTargets, to_view: same, view: None, words: &w, dt: DT, ticks: 1, pressed: PadButtons::NONE, nav: Vec::new() });
            self.hud.apply_actions(&mut self.sora);
            let _ = self.hud.take_sounds();
        }
    }

    fn keys(&mut self, words: &[&str]) {
        for w in words {
            self.frames(14, Some(w), &[]);
        }
    }

    fn press(&mut self, b: PadButton) {
        self.frames(1, None, &[b]);
        self.frames(12, None, &[]);
    }

    /// (THE USER 2026-11-03) The picture an item pane shows (`itemImage.imageAnm.coImage`, a `tres.controls.Icon`): its
    /// texture file, `None` when it shows none.
    fn pane_picture(&self, pane: &str) -> Option<String> {
        let movie = self.hud.movie("CampMenu")?;
        if !movie.node(&format!("{pane}.itemImage")).is_some_and(|n| n.is_visible()) {
            return None;
        }
        movie.sprite(&format!("{pane}.itemImage.imageAnm.coImage"))?.gfx.first().map(|g| g.fill.texture.file.clone())
    }

    fn equipment(&self) -> &kh_ui::camp_equip::EquipScreen {
        &self.hud.camp_menu().expect("the camp menu").equipment
    }

    fn draw(&self, movies: &[&str]) -> (Vec<u8>, usize, usize) {
        let (w, h) = (1920usize, 1080usize);
        let (root, _) = stage_root(w as f32, h as f32);
        let mut canvas = kh_ui::Canvas::new(w, h);
        let mut r = kh_ui::Renderer::new(self.fonts.clone());
        for m in movies {
            r.draw(&mut canvas, self.hud.movie(m).unwrap_or_else(|| panic!("movie {m}")), &root);
        }
        let mut rgba = Vec::new();
        canvas.to_rgba8_over(|_, y| [0.30 + 0.08 * (y as f32 / 1080.0), 0.27, 0.24], &mut rgba);
        (rgba, w, h)
    }

    fn shoot(&self, name: &str, movies: &[&str]) {
        let (rgba, w, h) = self.draw(movies);
        kh_ui::def::write_png(&self.out.join(format!("{name}.png")), w as u32, h as u32, &rgba).unwrap();
    }

    /// The command menu's corner, twice the size (KH3's bottom left).
    fn shoot_command(&self, name: &str) {
        let (rgba, w, _) = self.draw(&["HudCommand", "HudStatus"]);
        let (x0, y0, cw, ch, z) = (0usize, 800usize, 700usize, 270usize, 2usize);
        let mut out = vec![0u8; cw * ch * z * z * 4];
        for y in 0..ch * z {
            for x in 0..cw * z {
                let i = ((y0 + y / z) * w + x0 + x / z) * 4;
                let o = (y * cw * z + x) * 4;
                out[o..o + 4].copy_from_slice(&rgba[i..i + 4]);
            }
        }
        kh_ui::def::write_png(&self.out.join(format!("{name}.png")), (cw * z) as u32, (ch * z) as u32, &out).unwrap();
        self.shoot(&format!("{name}_full"), &["HudCommand", "HudStatus"]);
    }
}

#[test]
fn summon_soras_equipment_has_the_spirit_area_and_the_summon_list_shows_its_ashes() {
    let Some(mut r) = Rig::new() else { return };
    r.frames(60, None, &[]);
    // ---- the camp menu: Equipment (Sora alone: straight to his page)
    r.frames(2, Some("menu open"), &[]);
    r.frames(80, None, &[]);
    r.keys(&["menu decide"]);
    if r.hud.camp_screen() != Some(CampScreen::Equipment) {
        r.keys(&["menu decide"]);
    }
    r.frames(30, None, &[]);
    assert_eq!(r.hud.camp_screen(), Some(CampScreen::Equipment));
    let slots = &r.equipment().data.slots;
    let spirits: Vec<(usize, Option<String>)> = slots.iter().filter(|s| s.kind == KIND_SPIRIT).map(|s| (s.slot, s.piece.as_ref().map(|p| p.name.clone()))).collect();
    assert_eq!(spirits, vec![(0, Some("Black Knife Tiche +5".into())), (1, None), (2, None), (3, None)], "THE USER: four slots; slot 0 holds the ash at the level he owns");
    assert_eq!(slots.last().map(|s| s.kind), Some(KIND_SPIRIT), "after the pouch");
    // the cursor to the first spirit slot by the pad's Down
    for _ in 0..40 {
        if r.equipment().slot().is_some_and(|s| s.kind == KIND_SPIRIT) {
            break;
        }
        r.keys(&["menu down"]);
    }
    assert_eq!(r.equipment().slot().map(|s| (s.kind, s.slot)), Some((KIND_SPIRIT, 0)));
    r.frames(20, None, &[]);
    r.shoot("1_equipment_spirit_area", &["CampMenu"]);
    // (THE USER 2026-11-03: "use the Elden Ring picture of the spirit ash") the slot's ash: Elden Ring's own picture in
    // the item pane, through the shared ERICON route
    assert_eq!(r.equipment().highlighted().map(|p| p.art_id()), Some("ERICON_113".to_string()));
    assert_eq!(r.pane_picture(A1).as_deref(), Some("er_icons/00113.png"), "Black Knife Tiche's picture");
    r.shoot("1_equipment_spirit_area_icon", &["CampMenu"]);
    // slot 1 (empty): Decide opens its stock, the ashes only, Black Knife Tiche left out (it is in slot 0)
    r.keys(&["menu down", "menu decide"]);
    r.frames(30, None, &[]);
    assert_eq!(r.equipment().layer, EquipLayer::Stock);
    let stock: Vec<(String, String)> = r.equipment().stock_rows.iter().map(|s| (s.piece.name.clone(), s.piece.cost_text())).collect();
    // (THE USER 2026-11-03: "mimic tear should make Sora instantly go into MP recharge") the Mimic Tear's cost reads "All"
    assert_eq!(stock, vec![("Mimic Tear Ashes +2".to_string(), "All".to_string()), ("Jellyfish Ashes".to_string(), "6".to_string()), ("Lone Wolf Ashes +3".to_string(), "9".to_string())]);
    assert!(r.equipment().highlighted().is_some_and(|p| p.help.contains("Spends all of Sora's MP.")), "the item pane: the all-MP line");
    r.keys(&["menu down"]);
    assert!(r.equipment().highlighted().is_some_and(|p| p.help.contains("Costs 6 MP.")), "an ordinary ash: its cost");
    r.keys(&["menu up"]);
    r.frames(10, None, &[]);
    r.shoot("2_spirit_stock_list", &["CampMenu"]);
    assert_eq!(r.pane_picture(A2).as_deref(), Some("er_icons/00117.png"), "the Mimic Tear's picture in the stock's pane");
    r.shoot("2_spirit_stock_list_icon", &["CampMenu"]);
    // the next stock row: its own picture
    r.keys(&["menu down"]);
    r.frames(20, None, &[]);
    assert_eq!(r.pane_picture(A2).as_deref(), Some("er_icons/00107.png"));
    r.keys(&["menu up"]);
    r.frames(10, None, &[]);
    // Decide on the Mimic Tear: equipped as its level-0 id
    r.keys(&["menu decide"]);
    r.frames(20, None, &[]);
    assert_eq!(r.sora.command().spirit_ashes, [200000, 207000, 0, 0]);
    assert_eq!(r.equipment().layer, EquipLayer::Slots);
    assert_eq!(r.equipment().slot().and_then(|s| s.piece.as_ref()).map(|p| p.name.as_str()), Some("Mimic Tear Ashes +2"), "the row shows it at once");
    // Square: KH3's Remove empties it; Decide + Decide puts the Jellyfish there
    r.keys(&["menu square"]);
    assert_eq!(r.sora.command().spirit_ashes, [200000, 0, 0, 0]);
    r.keys(&["menu decide", "menu down", "menu decide"]);
    r.frames(20, None, &[]);
    assert_eq!(r.sora.command().spirit_ashes, [200000, 241000, 0, 0]);
    // the Mimic Tear into slot 2 (its stock: the Mimic Tear and the Lone Wolf, the two equipped ones left out)
    r.keys(&["menu down", "menu decide", "menu decide"]);
    r.frames(20, None, &[]);
    assert_eq!(r.sora.command().spirit_ashes, [200000, 241000, 207000, 0]);
    r.frames(20, None, &[]);
    r.shoot("3_equipment_two_ashes", &["CampMenu"]);
    // close the menu
    r.keys(&["menu cancel"]);
    r.frames(2, Some("menu open"), &[]);
    r.frames(70, None, &[]);
    assert!(!r.hud.paused(), "the menu closed");
    // ---- the command menu: the fourth row "Summon", its list = the equipped ashes with their MP cost
    r.frames(30, None, &[]);
    let c = r.sora.ui_snapshot().command;
    assert_eq!((c.link_none, c.main[3].name, c.main[3].enabled), (false, "Summon", true));
    for _ in 0..3 {
        r.press(PadButton::DPadDown);
    }
    r.press(PadButton::ATTACK);
    r.frames(30, None, &[]);
    let c = r.sora.ui_snapshot().command;
    assert_eq!(c.list, 3, "the Summon list is open");
    let rows: Vec<(String, String)> = c.lists[3].iter().filter_map(|row| summon_row_text(row.kind, &r.sora)).collect();
    assert_eq!(rows, vec![("Black Knife Tiche +5".to_string(), "66".to_string()), ("Jellyfish Ashes".to_string(), "6".to_string()), ("Mimic Tear Ashes +2".to_string(), "All".to_string())], "the names and costs the HUD shows (the Mimic Tear: all his MP)");
    // what the HUD movie shows: the long name cut before its number, the level kept; the cost in KH3's number field
    let movie = r.hud.movie("HudCommand").unwrap();
    let shown: Vec<(String, String)> = (1..=12)
        .filter_map(|n| {
            let row = format!("cmdMenu.cmdSubTop.cmdSub1Set.cmdSub1_{n}.cmdText1");
            let name = movie.text(&format!("{row}.txCmdName.txCmdNameCol.coCmdName"))?;
            let number = movie.text(&format!("{row}.cmdNumber.cmdNumCol.txCmdNumber.cmdNumberTxt")).unwrap_or_default();
            (!name.is_empty()).then_some((name, number))
        })
        .collect();
    // (ASH-ROWS, THE USER 2026-11-05: "make the bubble dynamically extend to show the whole name (only for spirit ashes)")
    // the whole names, the plates widened (the host flags the rows as spirit ashes)
    assert!(shown.contains(&("Black Knife Tiche +5".to_string(), "66".to_string())), "{shown:?}");
    assert!(shown.contains(&("Jellyfish Ashes".to_string(), "6".to_string())), "{shown:?}");
    assert!(shown.contains(&("Mimic Tear Ashes +2".to_string(), "All".to_string())), "{shown:?}");
    let widths: Vec<f32> = (1..=9).filter_map(|n| movie.sprite(&format!("cmdMenu.cmdSubTop.cmdSub1Set.cmdSub1_{n}.cmdSubBase1")).and_then(|s| s.stretch).map(|s| s.by)).collect();
    assert!(widths.len() == 3 && widths.iter().all(|w| *w > 0.0 && *w == widths[0]), "the Summon list's three plates one width, wider than authored: {widths:?}");
    r.shoot_command("4_summon_command_list");
    // greyed with nothing equipped (THE USER: "Summon is just greyed out if nothing is equipped")
    r.press(PadButton::JUMP);
    r.frames(20, None, &[]);
    for slot in 0..4 {
        equip_spirit(&mut r.sora, slot, None).unwrap();
    }
    r.frames(30, None, &[]);
    let c = r.sora.ui_snapshot().command;
    assert_eq!((c.main[3].name, c.main[3].enabled), ("Summon", false));
    r.shoot_command("5_summon_greyed_nothing_equipped");
}

#[test]
fn summon_the_hosts_equip_action_writes_the_level_0_id_and_keeps_an_ash_in_one_slot() {
    let origin = Vec3::Z * Sora::origin_height_on_floor(&SORA_PARAMS);
    let mut sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 });
    sora.command_mut().summon = view(&sora);
    assert_eq!(equip_spirit(&mut sora, 2, Some(&spirit_id(200005))), Ok(()));
    assert_eq!(sora.command().spirit_ashes, [0, 0, 200000, 0], "the level-0 id: an upgrade keeps the slot");
    assert_eq!(equip_spirit(&mut sora, 0, Some(&spirit_id(200005))), Err("already in another slot"), "the duplicate rule");
    assert_eq!(equip_spirit(&mut sora, 2, Some(&spirit_id(200005))), Ok(()), "into its own slot again: no change");
    assert_eq!(equip_spirit(&mut sora, 1, Some(&spirit_id(999000))), Err("not owned"));
    assert_eq!(equip_spirit(&mut sora, 1, Some("Potion")), Err("not a spirit ash"));
    assert_eq!(equip_spirit(&mut sora, 4, None), Err("no such slot"));
    assert_eq!(equip_spirit(&mut sora, 2, None), Ok(()));
    assert_eq!(sora.command().spirit_ashes, [0; 4]);
    // the row texts: only the summon kinds, the equipped slot's ash
    sora.command_mut().spirit_ashes[1] = 207000;
    sora.command_mut().summon = view(&sora);
    assert_eq!(summon_row_text(249, &sora), Some(("Mimic Tear Ashes +2".to_string(), "All".to_string())), "all his MP");
    assert_eq!(summon_row_text(248, &sora), None, "an empty slot");
    assert_eq!(summon_row_text(30, &sora), None, "not a summon row");
}
