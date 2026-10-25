//! (BN) THE REWARD PANELS THROUGH THE REAL HUD PATH, OFFLINE: kh-host `Hud` fed as the hosts feed it (Sora's popup board,
//! `Hud::party_bonus`, `Hud::party_level_up`), HudRight drawn at 1920 x 1080 and cut to the right-hand stack, each picture
//! also written under KH3's own reference (party/bn/ref) for comparison.
//!   cargo run -p kh-host --release --example bn_bonus_hud -- [OUT_DIR] [FRAMES]
use kh_core::status::BonusLine;
use kh_core::ui_model::{PopupDetail, TimedPopup, UiPopup};
use kh_core::{PadButtons, Sora, TargetId, Vec3};
use kh_host::hud::{Frame, Hud, HudTargets, TargetView};
use kh_host::party::{PartyChara, PartyHudSlot};

struct NoTargets;
impl HudTargets for NoTargets {
    fn target(&self, _id: TargetId) -> Option<TargetView> {
        None
    }
}

fn same(p: Vec3) -> Vec3 {
    p
}

fn shot(movie: &kh_ui::Movie, fonts: &kh_ui::Fonts, path: &std::path::Path) {
    let (w, h) = (1920usize, 1080usize);
    let mut canvas = kh_ui::Canvas::new(w, h);
    kh_ui::Renderer::new(fonts.clone()).draw(&mut canvas, movie, &kh_ui::Matrix::IDENTITY);
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|x, y| { let t = (x as f32 / 1920.0) * 0.2 + (y as f32 / 1080.0) * 0.1; [0.52 - t, 0.40 - t * 0.8, 0.30 - t * 0.6] }, &mut rgba);
    let reference = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/party/bn/ref/kh3_bonus_panels_reference.png");
    let Ok((rw, rh, rref)) = kh_ui::def::read_png(&reference) else {
        panic!("no reference picture at {}", reference.display());
    };
    // KH3's picture is 1:1 with a 1080p stage: its first panel's left edge at x 8, top at y 2; ours at stage (1360, 72)
    let (rw, rh) = (rw as usize, rh as usize);
    let (x0, y0) = (1352usize, 70usize);
    let ours_h = 330usize;
    let gap = 8usize;
    let (ow, oh) = (rw, rh + gap + ours_h);
    let mut out = vec![255u8; ow * oh * 4];
    for y in 0..rh {
        out[y * ow * 4..(y + 1) * ow * 4].copy_from_slice(&rref[y * rw * 4..(y + 1) * rw * 4]);
    }
    for y in 0..ours_h {
        for x in 0..rw {
            let (sx, sy) = (x0 + x, y0 + y);
            out[((rh + gap + y) * ow + x) * 4..][..4].copy_from_slice(&rgba[(sy * w + sx) * 4..][..4]);
        }
    }
    kh_ui::def::write_png(path, ow as u32, oh as u32, &out).unwrap();
    println!("written {}", path.display());
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = args.first().map(std::path::PathBuf::from).unwrap_or_else(|| root.join("build/sora_tmp/party/bn/shots"));
    let frames: usize = args.get(1).and_then(|a| a.parse().ok()).unwrap_or(120);
    std::fs::create_dir_all(&out).unwrap();
    let flash = kh_core::enums::ETresAbilityKind::from_game_name("FLASH_STEP").expect("FLASH_STEP");
    let fire = kh_core::magic::spells::Element::Fire;
    let thunder = kh_core::magic::spells::Element::Thunder;
    type Scene = (&'static str, Option<[BonusLine; 2]>, &'static [PartyChara]);
    let scenes: [Scene; 6] = [
        ("hud_sora_ability", Some([BonusLine::Ability(flash), BonusLine::None]), &[]),
        ("hud_goofy_max_hp", None, &[PartyChara::Goofy]),
        ("hud_donald_spell", Some([BonusLine::Magic(thunder.command(1)), BonusLine::None]), &[PartyChara::Donald, PartyChara::Goofy]),
        ("hud_sora_magic_tier", Some([BonusLine::Magic(fire.command(2)), BonusLine::None]), &[]),
        ("hud_reference_pair", Some([BonusLine::Ability(flash), BonusLine::None]), &[PartyChara::Goofy]),
        ("hud_goofy_level_up", None, &[PartyChara::Goofy]),
    ];
    for (name, sora_lines, party) in scenes {
        let (mut hud, fonts) = Hud::load(&root.join("build/sora_pack/ui")).expect("the UI pack");
        let sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 });
        let mut snapshot = sora.ui_snapshot();
        let slot = |c: PartyChara| PartyHudSlot { chara: c, hp: 160, hp_max: 220, mp: 60, mp_max: 100, mp_charging: false, mp_charge_value: 0.0, in_battle: false, focus: None };
        hud.set_party(&party.iter().map(|c| slot(*c)).collect::<Vec<_>>());
        if let Some(lines) = sora_lines {
            snapshot.board.bonus = Some(TimedPopup { popup: UiPopup { detail: PopupDetail::Bonus(lines), ..UiPopup::new(UiPopup::KIND_BONUS, "", 0.0, None) }, age: 0.0, lifetime: 4.53 });
        }
        if name == "hud_goofy_max_hp" || name == "hud_reference_pair" {
            let line = hud.bonus_line_text(BonusLine::MaxHp(10));
            hud.party_bonus(PartyChara::Goofy, &[line]);
        }
        if name == "hud_goofy_level_up" {
            let mut lines = hud.level_up_lines(0, 0, 2, 0, None);
            lines.push(kh_ui::hud_side::icon_line(kh_ui::hud_side::LineIcon::Ability, "Iron Wall"));
            hud.party_level_up(PartyChara::Goofy, 12, &lines);
        }
        for _ in 0..frames {
            hud.frame(&mut Frame { snapshot: &snapshot, sora: &sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 0, targets: &NoTargets, to_view: same, view: None, words: &[], dt: 1.0 / 60.0, ticks: 1, pressed: PadButtons::NONE, nav: Vec::new() });
        }
        let movie = hud.movie("HudRight").expect("HudRight");
        let line = |p: &str| movie.text(p).unwrap_or_default();
        println!("{name}: bonus window 1 '{}' / '{}' colour {:?}; window 2 '{}' colour {:?}; level-up window '{}' colour {:?}", line("hudRight.getBonus.getBonusSet.txGetBonusCapacity1.textField"), line("hudRight.getBonus.getBonusSet.txGetBonusCapacity2.textField"), movie.current_label("hudRight.getBonus.getBonusSet.lvupBonusWin1.lvupBonusWin1Col"), line("hudRight.getBonus2.getBonusSet.txGetBonusCapacity1.textField"), movie.current_label("hudRight.getBonus2.getBonusSet.lvupBonusWin1.lvupBonusWin1Col"), line("hudRight.levelUp.levelUpSet.txLevelUpCapacity1.textField"), movie.current_label("hudRight.levelUp.levelUpSet.lvupBonusWin1.lvupBonusWin1Col"));
        shot(movie, &fonts, &out.join(format!("{name}_vs_ref.png")));
    }
}
