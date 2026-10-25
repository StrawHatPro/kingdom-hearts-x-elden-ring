//! (BN) THE REWARD PANELS THROUGH kh-host's `Hud`: which icon a reward line gets (KH3's 0x141596150: the sparkle for an
//! ability, KH3's magic icon for a spell or tier, none for a stat or slot line), which panel a reward goes to (LEVEL UP!
//! for what levelling gives, BONUS! for everything else), and each character's window from the roster's data (colour,
//! MenuFace portrait). The `Hud` parts need the UI pack (skipped without it).

use kh_core::status::BonusLine;
use kh_core::ui_model::{PopupDetail, TimedPopup, UiPopup};
use kh_core::{PadButtons, Sora, TargetId, Vec3};
use kh_host::hud::{bonus_line_icon, Frame, Hud, HudTargets, TargetView};
use kh_host::party::{PartyChara, PartyHudSlot};
use kh_ui::hud_side::LineIcon;

struct NoTargets;
impl HudTargets for NoTargets {
    fn target(&self, _id: TargetId) -> Option<TargetView> {
        None
    }
}

fn same(p: Vec3) -> Vec3 {
    p
}

#[test]
fn the_icon_of_each_reward_kind_is_kh3s() {
    let flash = kh_core::enums::ETresAbilityKind::from_game_name("FLASH_STEP").unwrap();
    let fira = kh_core::magic::spells::Element::Fire.command(2);
    let thunder = kh_core::magic::spells::Element::Thunder.command(1);
    assert_eq!(bonus_line_icon(BonusLine::Ability(flash)), LineIcon::Ability, "an ability: the sparkle");
    assert_eq!(bonus_line_icon(BonusLine::Magic(thunder)), LineIcon::Magic, "a new spell: KH3's magic icon");
    assert_eq!(bonus_line_icon(BonusLine::Magic(fira)), LineIcon::Magic, "a magic tier: KH3's magic icon");
    for stat in [BonusLine::MaxHp(10), BonusLine::MaxMp(5), BonusLine::MaxAp(1), BonusLine::ArmorSlot, BonusLine::AccessorySlot, BonusLine::ItemSlot, BonusLine::Text("Strength +1"), BonusLine::None] {
        assert_eq!(bonus_line_icon(stat), LineIcon::None, "{stat:?}: a stat / slot line has no icon (the reference's Goofy line)");
    }
}

fn frame(hud: &mut Hud, snapshot: &kh_core::ui_model::UiSnapshot, sora: &Sora, n: usize) {
    for _ in 0..n {
        hud.frame(&mut Frame { snapshot, sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 0, targets: &NoTargets, to_view: same, view: None, words: &[], dt: 1.0 / 60.0, ticks: 1, pressed: PadButtons::NONE, nav: Vec::new() });
    }
}

fn slot(c: PartyChara) -> PartyHudSlot {
    PartyHudSlot { chara: c, hp: 160, hp_max: 220, mp: 60, mp_max: 100, mp_charging: false, mp_charge_value: 0.0, in_battle: false, focus: None }
}

#[test]
fn level_up_rewards_go_to_level_up_and_the_rest_to_bonus() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/ui");
    let Ok((mut hud, _)) = Hud::load(&root) else {
        eprintln!("SKIPPED: no UI pack");
        return;
    };
    // the texts as KH3 writes them
    let fira = kh_core::magic::spells::Element::Fire.command(2);
    assert_eq!(hud.bonus_line_text(BonusLine::MaxHp(10)), "Maximum HP increased!");
    assert_eq!(hud.bonus_line_text(BonusLine::Magic(fira)), "<icon(Magic)>Fira");
    assert_eq!(hud.level_up_lines(0, 0, 0, 0, Some(kh_core::enums::ETresAbilityKind::from_game_name("FLASH_STEP").unwrap())), vec!["<icon(Ability)>Flash Step".to_string()]);
    // every roster row: its colour and its portrait (data, not names)
    for row in kh_host::formation::ROSTER {
        let p = hud.panel_for(row.chara);
        assert_eq!(kh_ui::hud_side::PANEL_COLOURS[p.colour], row.panel);
        assert!(p.portrait.is_some(), "{}: the save-load pack's face", row.key);
    }
    let sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 });
    let mut snapshot = sora.ui_snapshot();
    hud.set_party(&[slot(PartyChara::Donald), slot(PartyChara::Goofy)]);
    // Sora learns Fira (outside levelling: BONUS!); Goofy levels up (LEVEL UP!); Goofy gets a boss's max HP (BONUS!)
    snapshot.board.bonus = Some(TimedPopup { popup: UiPopup { detail: PopupDetail::Bonus([BonusLine::Magic(fira), BonusLine::None]), ..UiPopup::new(UiPopup::KIND_BONUS, "", 0.0, None) }, age: 0.0, lifetime: 4.53 });
    let lines = hud.level_up_lines(0, 0, 2, 0, None);
    hud.party_level_up(PartyChara::Goofy, 12, &lines);
    let hp = hud.bonus_line_text(BonusLine::MaxHp(10));
    hud.party_bonus(PartyChara::Goofy, &[hp]);
    frame(&mut hud, &snapshot, &sora, 30);
    let m = hud.movie("HudRight").unwrap();
    let text = |p: &str| m.text(p).unwrap_or_default();
    let colour = |p: &str| m.current_label(p).unwrap_or("").to_string();
    // LEVEL UP!: Goofy's level-up window, its lettering, his colour and portrait
    assert_eq!(text("hudRight.levelUp.levelUpSet.levelUpTitle.coLevelUpTitle"), "LEVEL UP!");
    assert_eq!(text("hudRight.levelUp.levelUpSet.txLevelUpCapacity1.textField"), "Defense increased!");
    assert_eq!(colour("hudRight.levelUp.levelUpSet.lvupBonusWin1.lvupBonusWin1Col"), "goofy");
    assert!(m.sprite("hudRight.levelUp.levelUpSet.charaImage.coImage").is_some_and(|s| !s.gfx.is_empty()), "Goofy's portrait");
    // BONUS!: Sora's Fira (his window), Donald's Fira (he casts the shared book: roster row casts_magic), Goofy's max HP
    let windows: Vec<(String, String, String)> = ["hudRight.getBonus", "hudRight.getBonus2", "hudRight.getBonus3", "hudRight.getBonus4"]
        .iter()
        .map(|w| (text(&format!("{w}.getBonusSet.getBonusTitle.coGetBonusTitle")), colour(&format!("{w}.getBonusSet.lvupBonusWin1.lvupBonusWin1Col")), text(&format!("{w}.getBonusSet.txGetBonusCapacity1.textField"))))
        .collect();
    assert_eq!(windows[0], ("BONUS!".into(), "sora".into(), "<icon(Magic)>Fira".into()));
    assert!(windows.contains(&("BONUS!".into(), "donald".into(), "<icon(Magic)>Fira".into())), "{windows:?}");
    assert!(windows.contains(&("BONUS!".into(), "goofy".into(), "Maximum HP increased!".into())), "{windows:?}");
    assert!(!windows.iter().any(|w| w.1 == "goofy" && w.2.contains("Fira")), "Goofy does not cast the shared magic (roster row): {windows:?}");
}
