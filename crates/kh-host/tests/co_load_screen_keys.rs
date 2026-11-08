//! (CAMP-OPTIONS, the coordinator 2026-11-05: "every KH screen (load, save, Workshop, shops, camp, dialogs) takes pad and
//! keyboard input through the remapped table plus the protected defaults") KH3'S LOAD SCREEN BY THE KEYBOARD, as kh-er's
//! title feeds it (hud.rs `title_load_frame`: the keyboard through the table, `apply_kbm_protected` on a KH screen, into
//! MenuInput, into the HUD's frame): the cursor moves and a slot loads with KH3's default keys and with a remapped table.
//! Needs the UI pack (skipped without it).

use kh_core::{Input, PadButtons, PadState, Sora, TargetId, Vec3};
use kh_host::hud::{Frame, Hud, HudTargets, TargetView};
use kh_host::menu::MenuInput;
use kh_host::remap::{Device, Table};

/// The arrow Down and Enter (virtual-key codes).
const VK_DOWN: u16 = 0x28;
const VK_RETURN: u16 = 0x0D;
use kh_ui::saveload::{SaveLoadResult, SlotState, SlotSummary};

struct NoTargets;
impl HudTargets for NoTargets {
    fn target(&self, _id: TargetId) -> Option<TargetView> {
        None
    }
}

fn same(p: Vec3) -> Vec3 {
    p
}

struct Screen {
    hud: Hud,
    sora: Sora,
    input: MenuInput,
    held_before: PadButtons,
}

impl Screen {
    fn open() -> Option<Screen> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/ui");
        let (mut hud, _) = Hud::load(&root).ok()?;
        if !hud.saveload_available() {
            return None;
        }
        let slots: Vec<SlotSummary> = (0..10).map(|i| SlotSummary { state: SlotState::Used, level: 1, is_current: i == 0, ..SlotSummary::empty(i) }).collect();
        hud.saveload_background = true;
        hud.set_saveload_options(false, false, false);
        assert!(hud.open_load(&slots), "the load screen opens");
        let sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 });
        let mut s = Screen { hud, sora, input: MenuInput::new(), held_before: PadButtons::NONE };
        s.frames(&Table::defaults(Device::Kbm), &[], 40);
        Some(s)
    }

    /// `n` frames with `keys` held (virtual-key codes), the keyboard read as kh-er reads it on a KH screen.
    fn frames(&mut self, table: &Table, keys: &[u16], n: u32) {
        let snapshot = self.sora.ui_snapshot();
        for _ in 0..n {
            let (stick, held) = table.apply_kbm_protected(&|code| keys.contains(&code));
            let pad = PadState::from_held(held, self.held_before);
            self.held_before = held;
            self.input.step(&Input { move_stick: stick.clamp_length_max(1.0), ..Input::default() }.with_pad(pad), 1.0 / 60.0);
            let (pressed, nav) = self.input.take();
            self.hud.frame(&mut Frame { snapshot: &snapshot, sora: &self.sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 0, targets: &NoTargets, to_view: same, view: None, words: &[], dt: 1.0 / 60.0, ticks: 1, pressed, nav });
        }
    }

    fn press(&mut self, table: &Table, key: u16) {
        self.frames(table, &[key], 3);
        self.frames(table, &[], 12);
    }

    fn cursor(&self) -> usize {
        self.hud.saveload_state().map(|(_, _, c, _)| c).expect("the screen is up")
    }
}

#[test]
fn co_the_load_screen_moves_and_loads_by_the_keyboard_default_and_remapped() {
    // KH3's defaults: Down moves the cursor, Enter loads
    let Some(mut s) = Screen::open() else {
        println!("no UI pack: skipped");
        return;
    };
    let defaults = Table::defaults(Device::Kbm);
    let start = s.cursor();
    s.press(&defaults, VK_DOWN);
    assert_eq!(s.cursor(), start + 1, "Down moves the cursor (KH3's default)");
    s.press(&defaults, VK_RETURN);
    assert_eq!(s.hud.take_saveload_result(), Some(SaveLoadResult::Load(start + 1)), "Enter loads the slot under the cursor");

    // a remapped table: Confirm (Enter's role) on G, Move cursor: Down on S (swapped with Move character: Backward)
    let Some(mut s) = Screen::open() else { return };
    let mut table = Table::defaults(Device::Kbm);
    let role = |name: &str| kh_host::remap::KBM_ROLES.iter().position(|r| r.name == name).expect(name);
    table.assign(role("Confirm2"), b'G' as u16);
    table.assign(role("CursorDown"), b'S' as u16);
    let start = s.cursor();
    s.press(&table, b'S' as u16);
    assert_eq!(s.cursor(), start + 1, "S moves the cursor down (its new role)");
    // the arrow now plays Backward (the stick down): menus read the stick too, so it still moves the cursor
    s.press(&table, VK_DOWN);
    assert_eq!(s.cursor(), start + 2, "the arrow, now Backward, still moves down (the stick)");
    // Enter is free (no role holds it): KH3's default key acts on a KH screen (the lock-out protection)
    s.press(&table, VK_RETURN);
    assert_eq!(s.hud.take_saveload_result(), Some(SaveLoadResult::Load(start + 2)), "Enter still loads (the protected default)");
    let Some(mut s) = Screen::open() else { return };
    let start = s.cursor();
    s.press(&table, b'G' as u16);
    assert_eq!(s.hud.take_saveload_result(), Some(SaveLoadResult::Load(start)), "G loads (Confirm's new key)");
}
