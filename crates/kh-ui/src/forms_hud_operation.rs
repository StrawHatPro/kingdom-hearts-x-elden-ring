//! (F-HUD) The operation guide `HudOperation` (KH3's `UTresUIP_HudOperation`, rows of `MainCommandData`): the bottom-right
//! button guide KH3 shows while Sora is in ShooterMode (the gun forms' RT-held aim mode: "<RT hold> Auto-Shot"), and for
//! swimming, under water, the Gummi chart and the director skip.
//!
//! - [`HudOperation`]: the movie's AS3 (`build/sora_pack/ui/as3/HudOperation.as3.txt`). Its document class `HudOperation`
//!   (methods 172 - 177) and the clips `McOperation_1` (86 - 90), `McOperationGuide_3` (76 - 84), `McPushMark_8` (92 - 95)
//!   hold nothing but frame scripts (`stop()` / `gotoAndPlay("loop" | "off")`), which the player runs from the timeline;
//!   `CoOperationTxt` / `tres.controls::TextFieldAdjust` (97 - 106, 169 - 170) is never reached: the native code writes the
//!   inner `textField` directly. `tres.controls::Icon`, `FocusManager`, `IconManager` are in the ABC but no clip of the movie
//!   uses them.
//! - [`OperationGuide`]: the native part, traced from the decompiled C (addresses below), plus the per-frame driver the host
//!   calls.
//!
//! TRACE (KH3 decomp):
//! - ShooterMode's camera on / off (`UTresPlayerState_ShooterMode` slot 114, `0x1413BED50`) calls `0x14140BE40(ctrl, on)`:
//!   for the forms 0x80 / 0x83 / 0x8E / 0x8F (`ctrl+0xB38 - 0x80`, mask 0xC009) it reaches the cockpit's `0x141589700(hud)`,
//!   which does `hud+0x90->vt+0x250(0)` = `Open` (`0x1416425E0`) and `0x141647D30(hud+0x90, 4)` = `SetOperationGuide(SHOOTERMODE)`.
//!   The vtable of the part is `0x146AC69C8` (its constructor `0x141636C70`): +0x238 Initialize `0x14163FA60`, +0x248 Dispose
//!   `0x14163B0A0`, +0x250 Open `0x1416425E0`, +0x258 Close `0x141639FA0`, +0x260 IsOpen `0x1415C3DE0` (`+0xA1`), +0x280
//!   refresh rows `0x141643D40` [read from `asm/data/rdata.bin`].
//! - THE CLOSE PATH: `0x141589700` ignores the on / off byte (asm: `xor edx,edx; call [rax+0x250]` then `mov edx,4; jmp
//!   0x141647D30`), so the camera OFF (and the form end, `UTresPlayerState_StyleChange 0x1413CE760` and the controller's
//!   `0x14141DD00` / `0x14141E2B0`, which call `0x14140BE40(ctrl, 0)` for the same forms) OPENS the guide again (a no-op:
//!   it is open). ShooterMode's End (`0x1413ABE40`) only closes the scope (`0x14140BF50` -> HudTarget). No code in
//!   0x1413xxxxx - 0x1415xxxxx calls `Close` on `hud+0x90` except the Gummi ship's end (`0x141577B00`). KH3 therefore never
//!   closes the ShooterMode guide on its own; the port closes it (the part's own `Close`, `0x141639FA0`: `operation` plays
//!   `out`) when the caller stops asking for it ([`OperationGuide::update`] with `None`), so it does not stay on screen.
//! - `Open` `0x1416425E0`: when not open: open flag (+0xA1) on, `+0x3A8 = 1`, `operation` plays `in`, the root plays `loop`,
//!   the six rows' state (+0x360) = -1, buttons (+0x378, +0x390) = 0.
//! - `Close` `0x141639FA0`: when open: the kind (+0x3AC) = 0, `operation` plays `out`, the open flag off.
//! - `SetOperationGuide(kind)` `0x141647D30`: when open (and the MainCommandData table, `uiManager+0x3A0`, exists): the kind
//!   is kept (+0x3AC); its row name (1 `CA_SWIMMING`, 2 `CA_UNDERWATER`, 3 `CA_NAUTICALCHART`, 4 `SHOOTERMODE`, 5
//!   `DIRECTORSKIP`) is looked up (`0x14155DA60`); n = min(rows of `m_eBtnTypeAry`, 6); the first 6 - n rows go `off`
//!   (state -1, buttons 0); row 6 - n + j gets the j-th localised text (`0x1415807F0`) on its `textField` (`0x1439093E0`,
//!   SetText), state 0, button 1 = `m_eBtnTypeAry[j]`, button 2 = 0, and `SetGuideRow` (`0x141648010`).
//! - Row i is `operationGuide{6 - i}` (Initialize `0x14163FA60` binds `operationGuide%d` from 6 down to 1): the last row,
//!   `operationGuide1`, is the bottom one (y 753 of the stage), so a one-row guide sits at the bottom.
//! - `SetGuideRow(pad, i)` `0x141648010`: the four pad texts and the row's label, see [`OperationGuide::set_guide_row`].
//!   `pad` = `uiManager+0x6BB1`: the pad is the input device (written by `0x1415B0ED0` from the controller's vt+0xCE8; a
//!   change refreshes the rows, vt+0x280 `0x141643D40`). The port is controller first: the host passes `true`.
//! - Pad texts: `0x141597760(type, 0)` = the `<icon(...)>` of an `ETresUIPadButtonType` ([`pad_button_text`]); the
//!   ShooterMode row is `m_eBtnTypeAry = [TUPBT_R2HOLD]` (10) = `<icon(R2Hold)>` (the Xbox set draws RT with the hold mark),
//!   text `kh3_system.command.og_shootermood` = "Auto-Shot". Button 10 with the pad also blinks the row's `pushMark` (`loop`).
//!
//! THE MOVIE ON THE STAGE: 1920 x 1080 (layout `build/sora_pack/ui/layout/HudOperation.json`); the rows' text fields end at
//! x 1834 (right aligned, 208 wide), the pad icon is centred at x 1856; `operationGuide1` is at y 741 - 777, each row above
//! it 40 px higher (guide6 at y 541). Drawn like the other cockpit movies with the stage matrix (`Renderer::draw`).

use std::sync::Arc;

use crate::def::{MovieDef, Pack};
use crate::player::Movie;
use crate::render::Fonts;

/// The rows of the guide (`m_pGFxOperationGuide[6]`).
pub const OPERATION_GUIDE_MAX: usize = 6;

/// `ETresUIPadButtonType` (sheets/reflect/enum-values.json, values 0 - 37 in this order).
pub const TUPBT_NAMES: [&str; 38] = [
    "TUPBT_NONE", "TUPBT_RIGHT", "TUPBT_UP", "TUPBT_LEFT", "TUPBT_DOWN", "TUPBT_R1", "TUPBT_L1", "TUPBT_R2", "TUPBT_L2", "TUPBT_TOUCH", "TUPBT_R2HOLD", "TUPBT_DECIDE", "TUPBT_CANCEL", "TUPBT_RIGHT_L", "TUPBT_UP_L", "TUPBT_LEFT_L", "TUPBT_DOWN_L", "TUPBT_KEYUP", "TUPBT_KEYLEFT", "TUPBT_KEYDOWN", "TUPBT_KEYRIGHT", "TUPBT_KEYUP_L", "TUPBT_KEYLEFT_L", "TUPBT_KEYDOWN_L", "TUPBT_KEYRIGHT_L", "TUPBT_TOUCH_L", "TUPBT_R2_L", "TUPBT_L2_L", "TUPBT_OPTION", "TUPBT_LSTICK", "TUPBT_RSTICK", "TUPBT_LSTICK_UP", "TUPBT_LSTICK_DOWN", "TUPBT_LSTICK_LEFT", "TUPBT_LSTICK_RIGHT", "TUPBT_L3", "TUPBT_DECIDE_L", "TUPBT_CANCEL_L",
];
pub const TUPBT_R2HOLD: i32 = 10;
const TUPBT_KEYUP: i32 = 17;
const TUPBT_KEYLEFT: i32 = 18;
const TUPBT_KEYDOWN: i32 = 19;
const TUPBT_KEYRIGHT: i32 = 20;
const TUPBT_LSTICK: i32 = 29;

/// `0x141597760`'s table (`&DAT_1486E0640 + type * 0x10`, filled once): the text of a pad button type; 0 = "".
const PAD_BUTTON_TEXT: [&str; 38] = [
    "", "<icon(Circle)>", "<icon(Triangle)>", "<icon(Square)>", "<icon(Cross)>", "<icon(R1)>", "<icon(L1)>", "<icon(R2)>", "<icon(L2)>", "<icon(Pad)>", "<icon(R2Hold)>", "<icon(Decide)>", "<icon(Cancel)>", "<icon(Circle2)>", "<icon(Triangle2)>", "<icon(Square2)>", "<icon(Cross2)>", "<icon(KeyUp)>", "<icon(KeyLeft)>", "<icon(KeyDown)>", "<icon(KeyRight)>", "<icon(KeyUp2)>", "<icon(KeyLeft2)>", "<icon(KeyDown2)>", "<icon(KeyRight2)>", "<icon(Pad2)>", "<icon(R22)>", "<icon(L22)>", "<icon(Option)>", "<icon(Lstick)>", "<icon(Rstick)>", "<icon(LstickUpGre)>", "<icon(LstickDownGre)>", "<icon(LstickLeft)>", "<icon(LstickRightGre)>", "<icon(L3)>", "<icon(Decide2)>", "<icon(Cancel2)>",
];

/// The value of a `TUPBT_*` name.
pub fn tupbt(name: &str) -> Option<i32> {
    TUPBT_NAMES.iter().position(|n| *n == name).map(|i| i as i32)
}

/// `0x141597760(type, second)`: the `<icon(...)>` text of a pad button type; `second` picks the "2" set (1 -> 13, 2 -> 14,
/// 3 -> 15, 4 -> 16, 7 -> 26, 8 -> 27, 9 -> 25, 11 -> 36, 12 -> 37, 17 -> 21, 18 -> 22, 19 -> 23, 20 -> 24).
pub fn pad_button_text(ty: i32, second: bool) -> &'static str {
    let ty = if second {
        match ty {
            1 => 0xD,
            2 => 0xE,
            3 => 0xF,
            4 => 0x10,
            7 => 0x1A,
            8 => 0x1B,
            9 => 0x19,
            0xB => 0x24,
            0xC => 0x25,
            0x11 => 0x15,
            0x12 => 0x16,
            0x13 => 0x17,
            0x14 => 0x18,
            other => other,
        }
    } else {
        ty
    };
    usize::try_from(ty).ok().and_then(|i| PAD_BUTTON_TEXT.get(i)).copied().unwrap_or("")
}

/// The guide types of `SetOperationGuide` (`+0x3AC`; 0 = none).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OperationKind {
    Swimming = 1,
    Underwater = 2,
    NauticalChart = 3,
    ShooterMode = 4,
    DirectorSkip = 5,
}

impl OperationKind {
    /// The MainCommandData row name `0x141647D30` looks up.
    pub fn row_name(self) -> &'static str {
        match self {
            OperationKind::Swimming => "CA_SWIMMING",
            OperationKind::Underwater => "CA_UNDERWATER",
            OperationKind::NauticalChart => "CA_NAUTICALCHART",
            OperationKind::ShooterMode => "SHOOTERMODE",
            OperationKind::DirectorSkip => "DIRECTORSKIP",
        }
    }
}

/// One MainCommandData row of the guide: the localised texts (`m_ButtonText`) and the pad button types (`m_eBtnTypeAry`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationRow {
    pub id: String,
    pub texts: Vec<String>,
    pub buttons: Vec<i32>,
}

/// The rows of `data/main_command_styles.json` that carry an `m_eBtnTypeAry` (the MainCommandData map `uiManager+0x3A0`).
#[derive(Clone, Debug, Default)]
pub struct OperationTable {
    pub rows: Vec<OperationRow>,
}

impl OperationTable {
    pub fn load(pack: &mut Pack) -> OperationTable {
        let mut out = OperationTable::default();
        let Ok(data) = pack.data("main_command_styles") else { return out };
        for row in data["rows"].as_array().into_iter().flatten() {
            let Some(types) = row["style_props"].as_array().into_iter().flatten().find(|p| p[0].as_str() == Some("m_eBtnTypeAry")) else { continue };
            let buttons: Vec<i32> = types[1]["items"].as_array().into_iter().flatten().map(|t| t.as_str().and_then(tupbt).unwrap_or(0)).collect();
            let keys = row["buttons"].as_array().cloned().unwrap_or_default();
            let english = row["buttons_en"].as_array().cloned().unwrap_or_default();
            let texts = (0..keys.len().max(english.len()))
                .map(|i| keys.get(i).and_then(|k| k.as_str()).and_then(|k| pack.string(k)).or_else(|| english.get(i).and_then(|t| t.as_str()).map(str::to_string)).unwrap_or_default())
                .collect();
            out.rows.push(OperationRow { id: row["id"].as_str().unwrap_or("").to_string(), texts, buttons });
        }
        out
    }

    /// `0x14155DA60`: the row of a name.
    pub fn row(&self, name: &str) -> Option<&OperationRow> {
        self.rows.iter().find(|r| r.id == name)
    }
}

const OPERATION: &str = "operation";

/// The clip of row `i` (`m_pGFxOperationGuide[i]` = `operationGuide{6 - i}`).
pub fn guide_path(i: usize) -> String {
    format!("operation.operationSet.operationGuide{}", OPERATION_GUIDE_MAX - i)
}

/// The movie's AS3: the document class and its clips (frame scripts only, run by the player's timeline).
pub struct HudOperation {
    pub movie: Movie,
    pub fonts: Fonts,
    pub calls: Vec<String>,
}

impl HudOperation {
    pub fn new(def: Arc<MovieDef>, fonts: Fonts) -> HudOperation {
        let mut hud = HudOperation { movie: Movie::new(def), fonts, calls: Vec::new() };
        // The pad fields' `<icon(...)>` are KH3's inline images (`<img src='img://icontext:%s'>`, 0x141578970) at the
        // IconText table's own size, as the command menu's `padBtn.txPadBtn` (hud_command::pad_text).
        for i in 0..OPERATION_GUIDE_MAX {
            let guide = guide_path(i);
            for button in ["padBtn", "padBtn2", "padBtn3", "padBtn4"] {
                hud.movie.set_text_icon_native(&format!("{guide}.{button}.txPadBtn"), true);
            }
        }
        hud
    }

    pub fn log(&mut self, call: String) {
        if self.calls.len() >= 256 {
            self.calls.remove(0);
        }
        self.calls.push(call);
    }

    /// One frame of the stage (the frame scripts run in the timeline; the movie dispatches no events).
    pub fn tick(&mut self) {
        self.movie.tick();
        self.movie.take_events();
    }

    /// `FUN_1438F9470`: `clip.gotoAndPlay(label)`, logged.
    fn play(&mut self, path: &str, label: &str) {
        self.log(format!("{path}.gotoAndPlay({label:?})"));
        self.movie.goto_and_play(path, label);
    }

    /// `FUN_1439093E0`: `textField` SetText, logged.
    fn text(&mut self, path: &str, text: &str) {
        self.log(format!("{path}.text = {text:?}"));
        self.movie.set_text(path, text, &self.fonts);
    }
}

/// The native part `UTresUIP_HudOperation` and the driver the host calls every frame.
pub struct OperationGuide {
    pub hud: HudOperation,
    pub table: OperationTable,
    /// `+0xA1`: open (vt+0x260).
    open: bool,
    /// `+0x3AC`: the guide type set last (0 = none).
    kind: i32,
    /// `+0x360`: per row -1 = empty, 0 = shown, > 0 = disabled.
    state: [i32; OPERATION_GUIDE_MAX],
    /// `+0x378` / `+0x390`: per row the pad button types.
    button: [i32; OPERATION_GUIDE_MAX],
    button2: [i32; OPERATION_GUIDE_MAX],
    /// `uiManager+0x6BB1` as last given.
    pad: bool,
    /// What the caller asked for last.
    wanted: Option<OperationKind>,
}

impl OperationGuide {
    /// The movie `HudOperation` and the MainCommandData rows. None: the pack has no such movie.
    pub fn load(pack: &mut Pack, fonts: &Fonts) -> Option<OperationGuide> {
        let def = pack.movie("HudOperation").ok()?;
        let table = OperationTable::load(pack);
        Some(OperationGuide::new(def, fonts.clone(), table))
    }

    pub fn new(def: Arc<MovieDef>, fonts: Fonts, table: OperationTable) -> OperationGuide {
        OperationGuide { hud: HudOperation::new(def, fonts), table, open: false, kind: 0, state: [-1; OPERATION_GUIDE_MAX], button: [0; OPERATION_GUIDE_MAX], button2: [0; OPERATION_GUIDE_MAX], pad: true, wanted: None }
    }

    pub fn movie(&self) -> &Movie {
        &self.hud.movie
    }

    /// vt+0x260 `0x1415C3DE0`.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// `+0x3AC`: the guide type set last (0 = none).
    pub fn kind(&self) -> i32 {
        self.kind
    }

    /// The row states (`+0x360`) and button types (`+0x378`, `+0x390`).
    pub fn rows(&self) -> [(i32, i32, i32); OPERATION_GUIDE_MAX] {
        std::array::from_fn(|i| (self.state[i], self.button[i], self.button2[i]))
    }

    /// Every frame: `wanted` = `Some(ShooterMode)` while ShooterMode's camera is on (any guide type works the same way),
    /// `pad` = the pad is the input device (controller first: true). A new kind opens the guide and sets its rows (the
    /// cockpit's `0x141589700` sequence: `Open`, then `SetOperationGuide`); `None` closes it (see the module's close path);
    /// a pad change refreshes the rows (vt+0x280). The stage frames are [`OperationGuide::tick`].
    pub fn update(&mut self, wanted: Option<OperationKind>, pad: bool) {
        if wanted != self.wanted {
            self.wanted = wanted;
            match wanted {
                Some(kind) => {
                    self.pad = pad;
                    self.open_guide();
                    self.set_operation_guide(kind);
                }
                None => self.close(),
            }
        }
        if pad != self.pad {
            self.pad = pad;
            self.refresh(pad);
        }
    }

    /// One stage frame (1/60 s): the host calls it in its `for _ in 0..f.ticks` loop.
    pub fn tick(&mut self) {
        self.hud.tick();
    }

    /// vt+0x250 `Open` `0x1416425E0` (its argument is unused).
    pub fn open_guide(&mut self) {
        if self.open {
            return;
        }
        self.hud.log("Open".into());
        self.open = true;
        self.hud.play(OPERATION, "in");
        self.hud.play("", "loop");
        self.state = [-1; OPERATION_GUIDE_MAX];
        self.button = [0; OPERATION_GUIDE_MAX];
        self.button2 = [0; OPERATION_GUIDE_MAX];
    }

    /// vt+0x258 `Close` `0x141639FA0`.
    pub fn close(&mut self) {
        if !self.open {
            return;
        }
        self.hud.log("Close".into());
        self.kind = 0;
        self.hud.play(OPERATION, "out");
        self.open = false;
    }

    /// `SetOperationGuide(kind)` `0x141647D30`.
    pub fn set_operation_guide(&mut self, kind: OperationKind) {
        if !self.open {
            return;
        }
        self.kind = kind as i32;
        self.hud.log(format!("SetOperationGuide({})", kind as i32));
        let Some(row) = self.table.row(kind.row_name()).cloned() else { return };
        let n = row.buttons.len().min(OPERATION_GUIDE_MAX);
        let mut j = 0;
        for i in 0..OPERATION_GUIDE_MAX {
            if i < OPERATION_GUIDE_MAX - n {
                self.hud.play(&guide_path(i), "off");
                self.state[i] = -1;
                self.button[i] = 0;
                self.button2[i] = 0;
            } else {
                let text = row.texts.get(j).cloned().unwrap_or_default();
                self.hud.text(&format!("{}.txOperation.coOperationTxt.textField", guide_path(i)), &text);
                self.state[i] = 0;
                self.button[i] = row.buttons[j];
                self.button2[i] = 0;
                self.set_guide_row(self.pad, i);
                j += 1;
            }
        }
    }

    /// vt+0x280 `0x141643D40`: every row again with a new `pad`.
    pub fn refresh(&mut self, pad: bool) {
        for i in 0..OPERATION_GUIDE_MAX {
            self.set_guide_row(pad, i);
        }
    }

    /// `SetGuideRow(pad, i)` `0x141648010`: the pad texts of row `i` (`padBtn` .. `padBtn4`), its label (`off` empty; shown
    /// `loop` / `loop2` / `loop4` for 1 / 2 / 4 buttons; disabled `disable` / `disable2` / `disable4`) and its push marks.
    /// Without the pad, KEYUP + KEYDOWN become `<icon(KeyUpDown)>` and KEYLEFT + KEYRIGHT `<icon(KeyLeftyRight)>`; with it,
    /// LSTICK alone becomes its four directions. (`disable4` is not a label of `McOperationGuide_3`, whose fourth disabled
    /// label is `disable3`: in KH3 that goto does nothing; it stays so.)
    pub fn set_guide_row(&mut self, pad: bool, i: usize) {
        let (b1, b2) = (self.button[i], self.button2[i]);
        let mut s: [&str; 4] = [""; 4];
        if b1 == 0 {
            if b2 != 0 {
                s[0] = pad_button_text(b2, false);
            }
        } else if !pad && b1 == TUPBT_KEYUP && b2 == TUPBT_KEYDOWN {
            s[0] = "<icon(KeyUpDown)>";
        } else if !pad && b1 == TUPBT_KEYLEFT && b2 == TUPBT_KEYRIGHT {
            s[0] = "<icon(KeyLeftyRight)>";
        } else if b2 == 0 {
            if pad && b1 == TUPBT_LSTICK {
                s = ["<icon(LstickUp)>", "<icon(LstickLeft)>", "<icon(LstickRight)>", "<icon(LstickDown)>"];
            } else {
                s[0] = pad_button_text(b1, false);
            }
        } else {
            s[0] = pad_button_text(b1, false);
            s[1] = pad_button_text(b2, false);
        }
        let four = !s[2].is_empty() && !s[3].is_empty();
        let label = match self.state[i] {
            st if st < 0 => "off",
            0 if four => "loop4",
            0 if s[1].is_empty() => "loop",
            0 => "loop2",
            _ if four => "disable4",
            _ if s[1].is_empty() => "disable",
            _ => "disable2",
        };
        let guide = guide_path(i);
        self.hud.play(&guide, label);
        for (button, text) in ["padBtn", "padBtn2", "padBtn3", "padBtn4"].into_iter().zip(s) {
            self.hud.text(&format!("{guide}.{button}.txPadBtn"), text);
        }
        let mark = |b: i32| if pad && b == TUPBT_R2HOLD { "loop" } else { "off" };
        self.hud.play(&format!("{guide}.padBtn.pushMark"), mark(b1));
        self.hud.play(&format!("{guide}.padBtn2.pushMark"), mark(b2));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pad_button_table_follows_the_enum() {
        assert_eq!(tupbt("TUPBT_R2HOLD"), Some(TUPBT_R2HOLD));
        assert_eq!(pad_button_text(TUPBT_R2HOLD, false), "<icon(R2Hold)>");
        assert_eq!(pad_button_text(tupbt("TUPBT_L2").unwrap(), false), "<icon(L2)>");
        assert_eq!(pad_button_text(tupbt("TUPBT_CANCEL").unwrap(), false), "<icon(Cancel)>");
        assert_eq!(pad_button_text(tupbt("TUPBT_LEFT").unwrap(), false), "<icon(Square)>");
        assert_eq!(pad_button_text(1, true), "<icon(Circle2)>");
        assert_eq!(pad_button_text(0, false), "");
        assert_eq!(pad_button_text(99, false), "");
        assert_eq!(guide_path(5), "operation.operationSet.operationGuide1");
        assert_eq!(guide_path(0), "operation.operationSet.operationGuide6");
    }
}
