//! `HudCommand` (bottom left: the command menu), the AS3 document class ported method by
//! method from `build/sora_pack/ui/as3/HudCommand.as3.txt` (class `HudCommand`, methods 842 -
//! 901). Constant names are the class's. The native side (`UTresUIP_HudCommand`) calls these
//! methods and also sets a few clips directly through its `m_pGFx...` handles: those are the
//! `native_*` functions at the end (docs/sora-ui-spec.md 5.1).
//!
//! NOT PORTED (they need the form-logo movies `act_logo/*`, which the pack does not hold): the
//! `Loader` of the form logo (`m_actLogoLoader`, `onActLogoLoadComplete`); `m_mcCmdActLogo` is
//! therefore always null, as it is in the game while no logo is loaded.

use std::sync::Arc;

use crate::controls;
use crate::def::MovieDef;
use crate::player::{AnimEnd, Movie, NumberProps};
use crate::render::Fonts;

pub const MAIN_COMMAND_LIST_ATTACK: i32 = 0;
pub const MAIN_COMMAND_LIST_MAGIC: i32 = 1;
pub const MAIN_COMMAND_LIST_ITEM: i32 = 2;
pub const MAIN_COMMAND_LIST_LINK: i32 = 3;
pub const MAIN_COMMAND_LIST_MAX: i32 = 4;
pub const SUB_COMMAND_INDEX: usize = 4;
pub const SUB_COMMAND_MAX: usize = 9;
pub const SUB_COMMAND2_MAX: usize = 6;
pub const ACTION_COMMAND_MAX: usize = 7;
pub const EX_COMMAND_MAX: usize = 4;
pub const SHORTCUT_COMMAND_MAX: usize = 4;

pub const STATE_MODE_IN: i32 = 0;
pub const STATE_MODE_OUT: i32 = 1;
pub const STATE_MODE_STYLE_IN: i32 = 2;
pub const STATE_MODE_VEHICLE_IN: i32 = 3;

const COMMAND_LEVEL_SUBCOMMAND: usize = 1;
const COMMAND_LEVEL_SUBTARGET: usize = 2;

pub const MAIN_COMMAND_STYLE_IN: i32 = 0;
pub const MAIN_COMMAND_STYLE_OUT: i32 = 1;
pub const MAIN_COMMAND_LOGO_IN: i32 = 2;
pub const MAIN_COMMAND_LOGO_OUT: i32 = 3;
pub const MAIN_COMMAND_MOVE_NEXT: i32 = 4;
pub const MAIN_COMMAND_BACK_ENABLE: i32 = 5;
pub const MAIN_COMMAND_BACK_DISABLE: i32 = 6;
pub const MAIN_COMMAND_FIRST_IN: i32 = 7;
pub const MAIN_COMMAND_FIRST_STYLE_IN: i32 = 8;
pub const MAIN_COMMAND_FIRST_VEHICLE_IN: i32 = 9;
pub const MAIN_COMMAND_ATTACK_TEXT_CHANGE: i32 = 10;
pub const MAIN_COMMAND_ATTACK_TEXT_CANCEL: i32 = 11;

pub const SUB_COMMAND_IN: i32 = 0;
pub const SUB_COMMAND_OUT: i32 = 1;
pub const SUB_COMMAND_CANCEL_OUT: i32 = 2;
pub const SUB_COMMAND_DECIDE_OUT: i32 = 3;
pub const SUB_COMMAND_FINISH_IN: i32 = 4;
pub const SUB_COMMAND_FINISH_OUT: i32 = 5;

pub const ACTION_COMMAND_TYPE_STYLE_POINT_GAUGE: i32 = 100;

pub const ACTION_COMMAND_IN: i32 = 0;
pub const ACTION_COMMAND_OUT: i32 = 1;
pub const ACTION_COMMAND_PUSH_UP: i32 = 2;
pub const ACTION_COMMAND_PUSH_DOWN: i32 = 3;
pub const ACTION_COMMAND_DISABLE: i32 = 4;
pub const ACTION_COMMAND_ENABLE: i32 = 5;
pub const ACTION_COMMAND_RESTRICT_ON: i32 = 6;
pub const ACTION_COMMAND_RESTRICT_OFF: i32 = 7;
pub const ACTION_COMMAND_FINISH_DISABLE: i32 = 8;
pub const ACTION_COMMAND_FINISH_ENABLE: i32 = 9;
pub const ACTION_COMMAND_FINISH_IN: i32 = 10;
pub const ACTION_COMMAND_FINISH_OUT: i32 = 11;
pub const ACTION_COMMAND_DISABLE_IN: i32 = 12;

const ACTION_COMMAND_BUTTON_IN: i32 = 0;
const ACTION_COMMAND_BUTTON_OUT: i32 = 1;
const ACTION_COMMAND_BUTTON_DECIDE_OUT: i32 = 2;
const ACTION_COMMAND_BUTTON_PUSH_UP: i32 = 3;
const ACTION_COMMAND_BUTTON_PUSH_DOWN: i32 = 4;
const ACTION_COMMAND_BUTTON_ADD_TRI: i32 = 5;
const ACTION_COMMAND_BUTTON_DEL_TRI: i32 = 6;
const ACTION_COMMAND_BUTTON_EFFECT: i32 = 7;
const ACTION_COMMAND_BUTTON_BREAK: i32 = 8;

pub const SHORTCUT_COMMAND_IN: i32 = 0;
pub const SHORTCUT_COMMAND_OUT: i32 = 1;
pub const SHORTCUT_COMMAND_OUT_EXEC: i32 = 2;
pub const SHORTCUT_COMMAND_PAGE_DOWN: i32 = 3;
pub const SHORTCUT_COMMAND_PAGE_UP: i32 = 4;
const SHORTCUT_COMMAND_MOVE_UP: i32 = 5;
const SHORTCUT_COMMAND_MOVE_DOWN: i32 = 6;
const SHORTCUT_COMMAND_IN_END: i32 = 7;
const SHORTCUT_COMMAND_OUT_END: i32 = 8;

pub const SHOOT_FLOW_COMMAND_IN: i32 = 0;
pub const SHOOT_FLOW_COMMAND_OUT: i32 = 1;
pub const SHOOT_FLOW_COMMAND_CHANGE: i32 = 2;
pub const SHOOT_FLOW_COMMAND_LOGO: i32 = 3;

pub const TITLE_TOP_COMMAND_BLUE: i32 = 0;
pub const TITLE_TOP_COMMAND_YELLOW: i32 = 1;
pub const TITLE_TOP_COMMAND_RED: i32 = 2;
pub const TITLE_TOP_SHORTCUT: i32 = 3;
pub const TITLE_TOP_BUTTON: i32 = 4;
pub const TITLE_TOP_SHOOTLOCK: i32 = 5;
const TITLE_TOP_MAX: i32 = 6;

const MODE_COMMAND_IN: i32 = 0;
const MODE_COMMAND_OUT: i32 = 1;
const MODE_COMMAND_IN_END: i32 = 2;
const MODE_COMMAND_OUT_END: i32 = 3;

pub const STYLE_POINT_PHASE0: i32 = 0;
pub const STYLE_POINT_PHASE1: i32 = 1;
pub const STYLE_POINT_PHASE2: i32 = 2;
pub const STYLE_POINT_PHASE3: i32 = 3;

pub const CALLBACK_ID_ATTACK_TEXT_RESET: i32 = 10;
pub const CALLBACK_ID_ANIM_ACTLOGO_OUT: i32 = 20;

const ANIM_MCACTLOGOANM_IN: i32 = 0;
const ANIM_MCACTLOGOANM_OUT: i32 = 1;
const ANIM_MCMAIN_WEAPON: i32 = 2;
const ANIM_MCMAIN_ATTACK: i32 = 3;
const ANIM_MCMAIN_FINISH: i32 = 4;

pub const BUTTON_TEXT_RIGHT: &str = "<icon(Circle)>";
pub const BUTTON_TEXT_UP: &str = "<icon(Triangle)>";
pub const BUTTON_TEXT_LEFT: &str = "<icon(Square)>";
pub const BUTTON_TEXT_DOWN: &str = "<icon(Cross)>";
pub const BUTTON_TEXT_L1: &str = "<icon(L1)>";

const LABELS_SUBCOMMAND_COLOR: [&str; 5] = ["", "mahou", "item", "link", "party"];
const LABELS_ACTION_COMMAND_POS: [&str; 7] = ["start", "start2", "start3", "start4", "start5", "start6", "start7"];
const LABELS_ACTION_COMMAND_TYPE: [&str; 6] = ["", "wep", "atf", "magic", "friend", "link"];

const CMD_SUB_TOP: &str = "cmdMenu.cmdSubTop";
const CMD_ACT_TOP: &str = "cmdMenu.cmdActTop";
const CMD_ACT_ALL: &str = "cmdMenu.cmdActTop.cmdActAll";
const CMD_ACT_LOGO_GG: &str = "cmdMenu.cmdActLogoGg";
const CMD_ACT_LOGO_GG_SHORT: &str = "cmdMenu.cmdActLogoGg.cmdActLogoGgShort";
const CMD_ACT_EV_TOP: &str = "cmdMenu.cmdActEvTop";
const CMD_ACT_EV_SHORT: &str = "cmdMenu.cmdActEvTop.cmdActEvShort";
const CMD_ACT_EV: &str = "cmdMenu.cmdActEvTop.cmdActEvShort.cmdActEv";
const CMD_CENTER: &str = "cmdMenu.cmdCenter";
const CMD_EX_TOP: &str = "cmdMenu.cmdExTop";
const CMD_SCUT_TOP: &str = "cmdMenu.cmdScutTop";
const CMD_SCUT_SHORT: &str = "cmdMenu.cmdScutTop.cmdShort";
const CMD_ACT_LOGO: &str = "cmdMenu.cmdActLogo";
const CMD_ACT_LOGO_SHORT: &str = "cmdMenu.cmdActLogo.cmdActLogoShort";
const CMD_TITLE_TOP: &str = "cmdMenu.cmdTitleTop";
const CMD_TITLE: &str = "cmdMenu.cmdTitleTop.cmdTitle";
const CMD_MAIN_TOP: &str = "cmdMenu.cmdMainTop";
const CMD_MAIN: &str = "cmdMenu.cmdMainTop.cmdMain";
const CMD_BACK: &str = "cmdMenu.cmdBack";
const CMD_SUB1_NEXT: &str = "cmdMenu.cmdSubTop.cmdSub1Set.cmdSub1Next";
/// `...cmdText1.txCmdName.txCmdNameCol.coCmdName` below a text clip.
const NAME: &str = "txCmdName.txCmdNameCol.coCmdName";
/// `...cmdNumber.cmdNumCol.txCmdNumber.cmdNumberTxt` below a text clip.
const NUMBER_TXT: &str = "cmdNumber.cmdNumCol.txCmdNumber.cmdNumberTxt";

pub struct HudCommand {
    pub movie: Movie,
    pub fonts: Fonts,
    /// (DON-UI) The sub lists' colour labels by main slot (KH3: Magic "mahou", Items "item", Link "link"); Donald's menu
    /// (Attack / Magic / Attunement / Items) gives Attunement the magic colour and Items in the fourth slot the item
    /// colour. None = KH3's.
    pub sub_colors: Option<[&'static str; 4]>,
    /// `m_mcSubBtnAry2[list][row]`: the row clip (index into `cmdSub1_N` / `cmdSub2_N`, 0-based).
    sub_btn2: [[usize; SUB_COMMAND_MAX]; 5],
    /// `m_nActionSelectBtnIdxAry[position]` = button, `m_nActionSelectBtnPosAry[button]` = position.
    action_idx: [i32; ACTION_COMMAND_MAX],
    action_pos: [i32; ACTION_COMMAND_MAX],
    /// `data.oCmdAct1` of each button is set (null for the style-point gauge button).
    action_has_act1: [bool; ACTION_COMMAND_MAX],
    main_btn_index: i32,
    sub_btn_index: [[i32; 3]; 4],
    sub_btn_index_old: [[i32; 4]; 4],
    sub_btn_max: [[i32; 4]; 4],
    mode: i32,
    style: i32,
    /// `m_mcModeCommand` (the mode panel `cmdExTop.cmdEx`) is set.
    mode_command: bool,
    act_logo_mode: i32,
    act_logo: bool,
    command_ex_text_index: i32,
    level: usize,
    action_btn_cnt: i32,
    shortcut_select: i32,
    shortcut: bool,
    visible_situation_finish_button: bool,
    style_point_gauge_button_index: i32,
    style_point_gauge_phase: i32,
    atk_cmd_active_static: bool,
    mode_command_count: i32,
    battle_mode: i32,
    simple_hud: bool,
    command_title: bool,
    cmd_main_change: bool,
    cmd_main_label: String,
    link_command_none: bool,
    legacy_mode: bool,
    /// `ExternalInterface.call("OnCallback", id, param)` since the last `take_callbacks`.
    callbacks: Vec<(i32, i32)>,
    /// Every interface call, newest last (tests and the debug page); capped.
    pub calls: Vec<String>,
}

fn main_btn(i: i32) -> String {
    format!("{CMD_MAIN}.cmdMain{}", i + 1)
}

/// (DON-UI) An AS situation button's clip (`cmdActPos{slot + 1}`), for art drawn on it.
pub fn action_button_path(slot: usize) -> String {
    act_btn(slot)
}

fn act_btn(i: usize) -> String {
    format!("{CMD_ACT_ALL}.cmdActState.cmdActPos{}", i + 1)
}

fn ex_set(panel: &str, row: usize) -> String {
    format!("{panel}.cmdEx.cmdExTextSet{}", row + 1)
}

impl HudCommand {
    /// The constructor of the document class and `onAddedToStage`.
    pub fn new(def: Arc<MovieDef>, fonts: Fonts) -> HudCommand {
        let mut hud = HudCommand {
            movie: Movie::new(def),
            fonts,
            sub_colors: None, // (DON-UI)
            sub_btn2: [[0; SUB_COMMAND_MAX]; 5],
            action_idx: [-1; ACTION_COMMAND_MAX],
            action_pos: [-1; ACTION_COMMAND_MAX],
            action_has_act1: [true; ACTION_COMMAND_MAX],
            main_btn_index: -1,
            sub_btn_index: [[-1; 3]; 4],
            sub_btn_index_old: [[0; 4]; 4],
            sub_btn_max: [[0; 4]; 4],
            mode: -1,
            style: -1,
            mode_command: false,
            act_logo_mode: -1,
            act_logo: true,
            command_ex_text_index: -1,
            level: 0,
            action_btn_cnt: 0,
            shortcut_select: 0,
            shortcut: false,
            visible_situation_finish_button: false,
            style_point_gauge_button_index: -1,
            style_point_gauge_phase: -1,
            atk_cmd_active_static: false,
            mode_command_count: 0,
            battle_mode: 0,
            simple_hud: true,
            command_title: true,
            cmd_main_change: false,
            cmd_main_label: String::new(),
            link_command_none: false,
            legacy_mode: false,
            callbacks: Vec::new(),
            calls: Vec::new(),
        };
        // <init>: the BitmapNumbers of the seven situation buttons get their component
        // parameters (McActCount_145.__setProp_*), updateStyle(1).
        for i in 0..ACTION_COMMAND_MAX {
            hud.configure_counter(i);
        }
        hud.update_style(1);
        // onAddedToStage
        hud.movie.goto_and_play("", "loop");
        hud.movie.goto_and_play(CMD_ACT_TOP, "loop");
        hud.movie.goto_and_play(CMD_ACT_ALL, "loop");
        hud.movie.goto_and_play(CMD_ACT_LOGO_GG, "loop");
        hud.movie.goto_and_play(CMD_ACT_LOGO_GG_SHORT, "off");
        hud.movie.goto_and_play(CMD_ACT_EV_TOP, "loop");
        hud.movie.goto_and_play(CMD_ACT_LOGO, "loop");
        hud.movie.goto_and_play(CMD_TITLE, "loop");
        hud.act_logo = true;
        hud.calls.clear();
        hud
    }

    /// `McActCount_145.__setProp_actCountS_*` / `__setProp_actCountSs_*`: bitmap `ActCount`,
    /// seconds 28 x 20 from x 0 (2 digits, kerning -6, aligned left on frames 1 - 8 and 19 - 28,
    /// right on 9 - 18), tenths 23 x 17 from x 280 (1 digit, left).
    fn configure_counter(&mut self, button: usize) {
        let count = format!("{}.cmdAct.cmdAct1.actCount", act_btn(button));
        let frame = self.movie.current_frame(&count).unwrap_or(1);
        let align = if (9..=18).contains(&frame) { "right" } else { "left" };
        let (s_number, ss_number) = (self.counter_number(&count, "actCountS"), self.counter_number(&count, "actCountSs"));
        if let Some(s) = self.movie.sprite_mut(&format!("{count}.actCountS")) {
            controls::number_configure(s, NumberProps { bmp_name: "ActCount".into(), bmp_width: 28, bmp_height: 20, bmp_start_x: 0, bmp_start_y: 0, num_digit: 2, zero_padding: false, kerning: -6, align, origin_anim: "center", whole_anim: true, number: s_number });
        }
        if let Some(s) = self.movie.sprite_mut(&format!("{count}.actCountSs")) {
            controls::number_configure(s, NumberProps { bmp_name: "ActCount".into(), bmp_width: 23, bmp_height: 17, bmp_start_x: 280, bmp_start_y: 0, num_digit: 1, zero_padding: false, kerning: 0, align: "left", origin_anim: "center", whole_anim: true, number: ss_number });
        }
    }

    fn counter_number(&self, count: &str, name: &str) -> i32 {
        self.movie.sprite(&format!("{count}.{name}")).and_then(|s| s.number.as_ref().map(|n| n.number)).unwrap_or(0)
    }

    fn log(&mut self, call: String) {
        if self.calls.len() >= 512 {
            self.calls.remove(0);
        }
        self.calls.push(call);
    }

    fn text(&mut self, path: &str, text: &str) {
        self.movie.set_text(path, text, &self.fonts);
    }

    /// (RC) A pad-button icon field (`padBtn.txPadBtn`): KH3 turns `<icon(Triangle)>` into an inline image on the text
    /// line (`<img src='img://icontext:%s' vspace='%d'>`, 0x141578970; IconTextData: 30 x 30, VSpace -7), whose middle
    /// lands on the bar's middle (situation bar 0..38, padBtn at y 25, $sougei 23: icon 18.8; ex rows 0..30: 15.8).
    fn pad_text(&mut self, path: &str, text: &str) {
        self.movie.set_text_icon_native(path, true);
        self.movie.set_text(path, text, &self.fonts);
    }

    fn label_is(&self, path: &str, label: &str) -> bool {
        self.movie.current_label(path) == Some(label)
    }

    /// One frame of the stage: timelines advance, then the `AnimEvent.END` listeners run.
    pub fn tick(&mut self) {
        self.movie.tick();
        for event in self.movie.take_events() {
            self.on_anim_end(&event);
        }
    }

    /// The `OnCallback(id, param)` calls the movie made (10 = attack text reset, 20 = form logo out).
    pub fn take_callbacks(&mut self) -> Vec<(i32, i32)> {
        std::mem::take(&mut self.callbacks)
    }

    /// `onAnimEnd(a1)`.
    fn on_anim_end(&mut self, a1: &AnimEnd) {
        match a1.value {
            ANIM_MCACTLOGOANM_IN | ANIM_MCACTLOGOANM_OUT => {
                if self.act_logo_mode >= 0 && self.mode_command {
                    if a1.value == ANIM_MCACTLOGOANM_IN {
                        self.set_mode_command(MODE_COMMAND_IN_END, &[], &[], false);
                    } else {
                        self.set_mode_command(MODE_COMMAND_OUT_END, &[], &[], false);
                        self.callbacks.push((CALLBACK_ID_ANIM_ACTLOGO_OUT, 0));
                    }
                }
                // (m_nActLogoMode == -1 && OUT: the logo loader is unloaded; nothing is loaded here.)
            }
            ANIM_MCMAIN_WEAPON => self.cmd_main_change = true,
            ANIM_MCMAIN_ATTACK => {
                self.callbacks.push((CALLBACK_ID_ATTACK_TEXT_RESET, 0));
                self.cmd_main_change = false;
            }
            ANIM_MCMAIN_FINISH => {
                if !self.cmd_main_label.is_empty() {
                    let label = self.cmd_main_label.clone();
                    self.movie.goto_and_play(&main_btn(MAIN_COMMAND_LIST_ATTACK), &label);
                }
            }
            _ => {}
        }
    }

    /// `unloadLogoLoader()`.
    pub fn unload_logo_loader(&mut self) {
        self.log("unloadLogoLoader()".into());
    }

    /// `setLegacyMode(a1)`.
    pub fn set_legacy_mode(&mut self, a1: bool) {
        self.log(format!("setLegacyMode({a1})"));
        self.legacy_mode = a1;
    }

    /// `setState(a1, a2)`.
    pub fn set_state(&mut self, a1: i32, a2: bool) {
        self.log(format!("setState({a1}, {a2})"));
        self.simple_hud = a2;
        match a1 {
            STATE_MODE_IN => {
                self.set_main_command(MAIN_COMMAND_FIRST_IN);
                self.select_main_btn_active(0);
            }
            STATE_MODE_STYLE_IN => {
                self.set_main_command(MAIN_COMMAND_FIRST_STYLE_IN);
                self.select_main_btn_active(0);
            }
            STATE_MODE_VEHICLE_IN => {
                self.set_main_command(MAIN_COMMAND_FIRST_VEHICLE_IN);
                self.select_main_btn_active(0);
            }
            _ => {}
        }
    }

    /// `setModeIn(a1, a2, a3, a4, a5, ...rest)`: style `a1`, `a2` rows of (text, button) in
    /// `rows`, icon type `a3`, `a4` = has a time gauge, `a5` = logo movie ("" = none).
    pub fn set_mode_in(&mut self, a1: i32, a2: i32, a3: i32, a4: bool, a5: &str, rows: &[(&str, &str)]) {
        self.log(format!("setModeIn({a1}, {a2}, {a3}, {a4}, {a5:?}, {rows:?})"));
        if a1 < 0 {
            return;
        }
        if a1 == 0 {
            self.set_main_command(MAIN_COMMAND_STYLE_IN);
        } else {
            self.act_logo_mode = -1;
            if self.label_is(CMD_ACT_LOGO_SHORT, "off") || self.label_is(CMD_ACT_LOGO_SHORT, "out") {
                self.movie.goto_and_play(CMD_ACT_LOGO_SHORT, "in");
            }
            if a4 {
                self.movie.goto_and_play(CMD_ACT_LOGO_GG_SHORT, "in");
                let l9 = format!("{CMD_ACT_LOGO_GG_SHORT}.cmdActLogoGgAnm");
                self.movie.goto_frame(&l9, 1);
                self.movie.goto_and_play(&format!("{l9}.actLogoGgWaku"), "loop");
                if let Some(label) = LABELS_ACTION_COMMAND_TYPE.get(a3 as usize) {
                    self.movie.goto_and_play(&format!("{l9}.actLogoGgCol"), label);
                }
            } else {
                self.movie.goto_and_play(CMD_ACT_LOGO_GG_SHORT, "off");
            }
            if !a5.is_empty() {
                self.set_action_select(ACTION_COMMAND_PUSH_UP, 0, 0);
                self.command_title = false;
            } else {
                self.movie.goto_and_play(&format!("{CMD_TITLE}.cmdTitleName"), "cmdBtn");
                self.movie.goto_and_play(&format!("{CMD_TITLE}.cmdTitleName2"), "cmdBtn");
                self.movie.goto_and_play(CMD_TITLE, "logoIn");
                self.movie.goto_and_play(CMD_TITLE_TOP, "loop");
                self.command_title = true;
            }
            self.mode_command = false;
            if !a5.is_empty() && a3 != 0 {
                // m_actLogoLoader.load(a5): the logo movie is not in the pack.
                self.act_logo_mode = a1;
            }
            if !self.act_logo {
                self.set_title_top(self.battle_mode, false, false);
                self.show_act_logo(false);
            }
            self.movie.goto_and_play(CMD_CENTER, "in2");
            self.movie.goto_and_play(CMD_BACK, "in2");
            if a2 > 0 {
                let n = (a2 as usize).min(rows.len());
                let texts: Vec<&str> = rows[..n].iter().map(|r| r.0).collect();
                let buttons: Vec<&str> = rows[..n].iter().map(|r| r.1).collect();
                self.mode_command = true;
                self.movie.goto_and_play(&format!("{CMD_EX_TOP}.cmdEx.cmdExBaseSet"), &format!("line{a2}"));
                self.movie.goto_and_play(CMD_EX_TOP, "in");
                self.set_mode_command(MODE_COMMAND_IN, &texts, &buttons, false);
            }
        }
        self.mode = a1;
    }

    /// `setModeOut(a1)`.
    pub fn set_mode_out(&mut self, a1: i32) {
        self.log(format!("setModeOut({a1})"));
        if a1 < 0 {
            return;
        }
        if a1 == 0 {
            self.set_main_command(MAIN_COMMAND_STYLE_OUT);
            return;
        }
        if self.label_is(CMD_ACT_LOGO_SHORT, "loop") || self.label_is(CMD_ACT_LOGO_SHORT, "in") {
            self.movie.goto_and_play(CMD_ACT_LOGO_SHORT, "out");
        }
        if ["loop", "in", "break"].iter().any(|l| self.label_is(CMD_ACT_LOGO_GG_SHORT, l)) {
            self.movie.goto_and_play(CMD_ACT_LOGO_GG_SHORT, "out");
        }
        self.movie.goto_and_play(CMD_CENTER, "out2");
        self.movie.goto_and_play(CMD_BACK, "out2");
        self.movie.goto_and_play(CMD_EX_TOP, "out");
        if self.act_logo_mode >= 0 {
            self.set_mode_command(MODE_COMMAND_OUT, &[], &[], false);
        }
    }

    /// `setModeCommand(a1, a2, a3, a4)`: the rows of the mode panel.
    pub fn set_mode_command(&mut self, a1: i32, a2: &[&str], a3: &[&str], a4: bool) {
        if !self.mode_command {
            return;
        }
        let l5 = a2.len();
        match a1 {
            MODE_COMMAND_IN => {
                self.mode_command_count = l5 as i32;
                for l7 in 0..EX_COMMAND_MAX {
                    let l6 = ex_set(CMD_EX_TOP, l7);
                    if !self.movie.exists(&l6) {
                        break;
                    }
                    if l7 < l5 {
                        self.movie.goto_and_play(&l6, "loop");
                        let l9 = format!("{CMD_EX_TOP}.cmdEx.cmdExBaseSet.cmdExBase1_{}", l7 + 1);
                        if a3[l7] != BUTTON_TEXT_L1 || a4 {
                            self.movie.goto_and_play(&format!("{l6}.cmdExText1"), if !a4 { "disabled" } else { "on" });
                            self.movie.goto_and_play(&l9, "on");
                        } else {
                            self.movie.goto_and_play(&format!("{l6}.cmdExText1"), "disabled");
                            self.movie.goto_and_play(&l9, "cancel");
                        }
                        if self.act_logo_mode < 0 {
                            self.movie.goto_and_play(&format!("{l6}.cmdExText1"), "on");
                        }
                        self.text(&format!("{l6}.cmdExText1.{NAME}"), a2[l7]);
                        self.pad_text(&format!("{l6}.padBtn.txPadBtn"), a3[l7]); // (RC)
                    } else {
                        self.movie.goto_and_play(&l6, "off");
                    }
                }
            }
            MODE_COMMAND_OUT => {
                for l7 in 0..self.mode_command_count.max(0) as usize {
                    let l6 = ex_set(CMD_EX_TOP, l7);
                    if !self.movie.exists(&l6) {
                        break;
                    }
                    self.movie.goto_and_play(&format!("{l6}.cmdExText1"), "disabled");
                }
                self.mode_command_count = 0;
            }
            MODE_COMMAND_IN_END => {
                for l7 in 0..self.mode_command_count.max(0) as usize {
                    let l6 = ex_set(CMD_EX_TOP, l7);
                    if !self.movie.exists(&l6) {
                        break;
                    }
                    let text = format!("{l6}.cmdExText1");
                    if self.label_is(&text, "disabled") && self.command_ex_text_index != l7 as i32 {
                        self.movie.goto_and_play(&text, "on");
                    }
                }
                self.command_ex_text_index = -1;
            }
            MODE_COMMAND_OUT_END => {
                self.movie.goto_and_play(CMD_EX_TOP, "off");
                self.command_ex_text_index = -1;
            }
            _ => {}
        }
    }

    /// `setModeCommandText(a1, a2, a3)`.
    pub fn set_mode_command_text(&mut self, a1: i32, a2: &str, a3: bool) {
        self.log(format!("setModeCommandText({a1}, {a2:?}, {a3})"));
        if !self.mode_command || a1 < 0 || a1 >= EX_COMMAND_MAX as i32 {
            return;
        }
        let l4 = ex_set(CMD_EX_TOP, a1 as usize);
        if self.movie.exists(&l4) {
            self.text(&format!("{l4}.cmdExText1.{NAME}"), a2);
            self.set_mode_command_effect(a1, a3);
        }
    }

    /// `setModeCommandEffect(a1, a2)`.
    pub fn set_mode_command_effect(&mut self, a1: i32, a2: bool) {
        if !self.mode_command {
            return;
        }
        let l3 = ex_set(CMD_EX_TOP, a1.max(0) as usize);
        self.movie.goto_and_play(&format!("{l3}.cmdEffChange"), if a2 { "in" } else { "off" });
    }

    /// `setEnableCommandExText(a1, a2, a3)`.
    pub fn set_enable_command_ex_text(&mut self, a1: i32, a2: bool, a3: bool) {
        self.log(format!("setEnableCommandExText({a1}, {a2}, {a3})"));
        if !self.mode_command || a1 < 0 || a1 >= EX_COMMAND_MAX as i32 {
            return;
        }
        let l4 = ex_set(CMD_EX_TOP, a1 as usize);
        if self.movie.exists(&l4) {
            self.command_ex_text_index = if a3 { a1 } else { -1 };
            let a2 = a2 && !a3;
            self.movie.goto_and_play(&format!("{l4}.cmdExText1"), if a2 { "on" } else { "disabled" });
        }
    }

    /// `setMainCommandSize()`.
    pub fn set_main_command_size(&mut self) {
        let label = if self.link_command_none { "com3" } else { "com4" };
        self.movie.goto_and_play(&format!("{CMD_MAIN}.cmdUnder"), label);
        self.movie.goto_and_play(&format!("{CMD_MAIN}.cmdLeft"), label);
    }

    /// `SetLinkCommandNone(a1)`.
    pub fn set_link_command_none(&mut self, a1: bool) {
        self.log(format!("SetLinkCommandNone({a1})"));
        self.link_command_none = a1;
        let link = main_btn(MAIN_COMMAND_LIST_LINK);
        if a1 {
            if !self.label_is(&link, "off") {
                self.movie.goto_and_play(&link, "out");
            }
        } else if !self.label_is(&link, "loop") {
            self.movie.goto_and_play(&link, "in");
        }
        self.set_main_command_size();
    }

    /// `initMainCommand(a1, a2, a3, a4)`.
    pub fn init_main_command(&mut self, a1: &str, a2: &str, a3: &str, a4: &str) {
        self.log(format!("initMainCommand({a1:?}, {a2:?}, {a3:?}, {a4:?})"));
        for (i, text) in [a1, a2, a3, a4].into_iter().enumerate() {
            self.text(&format!("{}.cmdText1.{NAME}", main_btn(i as i32)), text);
        }
        for l6 in 0..MAIN_COMMAND_LIST_MAX {
            self.set_main_command_right_icon(l6, false);
        }
        self.cmd_main_change = false;
    }

    /// `setMainCommandText(a1, a2, a3)`: returns the text it replaced.
    pub fn set_main_command_text(&mut self, a1: i32, a2: &str, a3: bool) -> String {
        self.log(format!("setMainCommandText({a1}, {a2:?}, {a3})"));
        let l4 = main_btn(a1);
        let name = format!("{l4}.cmdText1.{NAME}");
        let l6 = self.movie.text(&name).unwrap_or_default();
        self.text(&name, a2);
        self.movie.goto_and_play(&format!("{l4}.cmdEffChange"), if a3 { "in" } else { "off" });
        let l9 = ["cmdBlue", "cmdYellow", "cmdRed"][self.battle_mode.clamp(0, 2) as usize];
        self.movie.goto_and_play(&format!("{l4}.cmdEffChange.cmdEffChangeAnm"), l9);
        l6
    }

    /// `setMainCommandTextColor(a1, a2, a3)`.
    pub fn set_main_command_text_color(&mut self, a1: i32, a2: &str, a3: i32) -> String {
        self.log(format!("setMainCommandTextColor({a1}, {a2:?}, {a3})"));
        let l4 = main_btn(a1);
        let name = format!("{l4}.cmdText1.{NAME}");
        let l6 = self.movie.text(&name).unwrap_or_default();
        self.text(&name, a2);
        if let Some(label) = ["col1", "col2", "col3"].get(a3 as usize) {
            self.movie.goto_and_play(&format!("{l4}.cmdText1.txCmdName"), label);
        }
        l6
    }

    /// `setMainCommandTextBadStatus(a1, a2, a3)`.
    pub fn set_main_command_text_bad_status(&mut self, a1: i32, a2: &str, a3: i32) -> String {
        self.log(format!("setMainCommandTextBadStatus({a1}, {a2:?}, {a3})"));
        let l4 = main_btn(a1);
        let name = format!("{l4}.cmdText1.{NAME}");
        let l6 = self.movie.text(&name).unwrap_or_default();
        self.text(&name, a2);
        self.movie.goto_and_play(&format!("{l4}.cmdEffChange"), if a3 != 0 { "in2" } else { "out" });
        l6
    }

    /// `setMainCommandRightIcon(a1, a2)`.
    pub fn set_main_command_right_icon(&mut self, a1: i32, a2: bool) {
        if self.link_command_none && a1 == MAIN_COMMAND_LIST_LINK {
            return;
        }
        self.movie.goto_and_stop(&format!("{}.cmdRightIcon", main_btn(a1)), if a2 { "on" } else { "off" });
    }

    /// `setMainCommand(a1)`.
    pub fn set_main_command(&mut self, a1: i32) {
        self.log(format!("setMainCommand({a1})"));
        let attack = main_btn(MAIN_COMMAND_LIST_ATTACK);
        match a1 {
            MAIN_COMMAND_STYLE_IN => {
                self.movie.goto_and_play(CMD_TITLE, "logoIn");
                self.movie.goto_and_play(CMD_MAIN, "in2");
                self.movie.goto_and_play(CMD_CENTER, "in2");
                self.movie.goto_and_play(CMD_BACK, "in2");
                self.set_style(self.style, -1, 0, 0, false, None, 0);
            }
            MAIN_COMMAND_STYLE_OUT => {
                if !self.command_title {
                    self.movie.goto_and_play(CMD_TITLE, "logoOut");
                }
                for clip in [CMD_MAIN, CMD_CENTER, CMD_BACK] {
                    let label = if self.label_is(clip, "loop") { "out2" } else { "off" };
                    self.movie.goto_and_play(clip, label);
                }
                self.set_style(-1, self.style, 0, 0, false, None, 0);
            }
            MAIN_COMMAND_LOGO_IN => {
                self.movie.goto_and_play(CMD_MAIN, "in2");
                self.show_act_logo(false);
            }
            MAIN_COMMAND_LOGO_OUT => {
                self.movie.goto_and_play(CMD_MAIN, "out2");
                if self.cmd_main_change {
                    let label = self.cmd_main_label.clone();
                    self.movie.goto_and_play(&attack, &label);
                    self.cmd_main_change = false;
                }
                self.hide_act_logo();
            }
            MAIN_COMMAND_MOVE_NEXT | MAIN_COMMAND_BACK_ENABLE | MAIN_COMMAND_BACK_DISABLE => {
                for l2 in 0..MAIN_COMMAND_LIST_MAX {
                    if self.link_command_none && l2 == MAIN_COMMAND_LIST_LINK {
                        continue;
                    }
                    let btn = main_btn(l2);
                    if l2 != self.main_btn_index {
                        self.movie.goto_and_play(&btn, if a1 == MAIN_COMMAND_MOVE_NEXT { "out" } else { "in" });
                        continue;
                    }
                    let n = self.main_btn_index + 1;
                    match a1 {
                        MAIN_COMMAND_MOVE_NEXT => drop(self.movie.goto_and_play(&btn, &format!("next_{n}"))),
                        MAIN_COMMAND_BACK_ENABLE => {
                            self.movie.goto_and_play(&btn, &format!("turn2_{n}"));
                            self.movie.set_visible(&format!("{btn}.cmdEffChange"), true);
                        }
                        _ => {
                            self.movie.goto_and_play(&btn, &format!("turn1_{n}"));
                            self.movie.set_visible(&format!("{btn}.cmdEffChange"), true);
                            let l8 = format!("{btn}.cmdText1");
                            if !self.label_is(&l8, "disabled") {
                                self.movie.goto_and_play(&l8, "off");
                            }
                        }
                    }
                }
            }
            MAIN_COMMAND_FIRST_IN => {
                self.movie.goto_and_play(CMD_TITLE_TOP, "in");
                self.movie.goto_and_play(CMD_MAIN, "in");
                self.movie.goto_and_play(CMD_CENTER, "in");
                self.movie.goto_and_play(CMD_BACK, "in");
                self.set_style(0, -1, 0, 0, false, None, 0);
            }
            MAIN_COMMAND_FIRST_STYLE_IN => {
                self.movie.goto_and_play(CMD_TITLE_TOP, "loop");
                self.movie.goto_and_play(CMD_TITLE, "off");
                self.movie.goto_and_play(CMD_MAIN, "in");
                self.movie.goto_and_play(CMD_CENTER, "in");
                self.movie.goto_and_play(CMD_BACK, "in");
            }
            MAIN_COMMAND_FIRST_VEHICLE_IN => {
                self.movie.goto_and_play(CMD_TITLE_TOP, "off");
                self.movie.goto_and_play(CMD_MAIN, "off");
                self.movie.goto_and_play(CMD_CENTER, "off");
                self.movie.goto_and_play(CMD_BACK, "off");
                self.set_style(0, -1, 0, 0, false, None, 0);
            }
            MAIN_COMMAND_ATTACK_TEXT_CHANGE => {
                let label = if self.cmd_main_label != "up2" { "weapon" } else { "weapon2" };
                self.movie.goto_and_play(&attack, label);
            }
            MAIN_COMMAND_ATTACK_TEXT_CANCEL => {
                let label = self.cmd_main_label.clone();
                self.movie.goto_and_play(&attack, &label);
            }
            _ => {}
        }
    }

    /// `setStyle(a1, a2, a3, a4, a5, a6, a7)`: style in `a1`, style out `a2`.
    #[allow(clippy::too_many_arguments)]
    pub fn set_style(&mut self, a1: i32, a2: i32, a3: i32, a4: i32, a5: bool, a6: Option<&str>, a7: i32) {
        self.log(format!("setStyle({a1}, {a2}, {a3}, {a4}, {a5}, {a6:?}, {a7})"));
        if a2 >= 0 {
            if a2 == 0 {
                self.set_style_normal(1);
            } else {
                if a7 == 0 {
                    self.movie.goto_and_play(CMD_ACT_LOGO_SHORT, "out");
                    let label = if self.label_is(CMD_ACT_LOGO_GG_SHORT, "loop") || self.label_is(CMD_ACT_LOGO_GG_SHORT, "break") { "out" } else { "off" };
                    self.movie.goto_and_play(CMD_ACT_LOGO_GG_SHORT, label);
                }
                self.set_action_select(ACTION_COMMAND_PUSH_DOWN, 0, 0);
            }
        }
        if a1 >= 0 {
            if a1 == 0 {
                if a3 == 1 || a3 == 2 {
                    self.set_style_normal(0);
                }
            } else {
                self.movie.goto_and_play(CMD_ACT_LOGO_SHORT, "in");
                if a5 {
                    self.movie.goto_and_play(CMD_ACT_LOGO_GG_SHORT, "in");
                    let l9 = format!("{CMD_ACT_LOGO_GG_SHORT}.cmdActLogoGgAnm");
                    self.movie.goto_frame(&l9, 1);
                    self.movie.goto_and_play(&format!("{l9}.actLogoGgWaku"), "loop");
                    if a4 >= 0 {
                        if let Some(label) = LABELS_ACTION_COMMAND_TYPE.get(a4 as usize) {
                            self.movie.goto_and_play(&format!("{l9}.actLogoGgCol"), label);
                        }
                    }
                } else {
                    self.movie.goto_and_play(CMD_ACT_LOGO_GG_SHORT, "off");
                }
                self.mode_command = false;
                if a6.is_some() {
                    self.movie.goto_and_play(CMD_TITLE, "off");
                    self.act_logo_mode = -1;
                    self.command_title = false;
                } else {
                    self.command_title = true;
                }
                self.set_action_select(ACTION_COMMAND_PUSH_UP, 0, 0);
            }
            self.style = a1;
        }
    }

    /// `showActLogo(a1)`.
    pub fn show_act_logo(&mut self, a1: bool) {
        let label = if a1 { "inShort" } else { "loop" };
        self.movie.goto_and_play(CMD_ACT_LOGO_GG, label);
        self.movie.goto_and_play(CMD_ACT_EV_TOP, label);
        self.movie.goto_and_play(CMD_ACT_LOGO, label);
        self.act_logo = true;
    }

    /// `hideActLogo()`.
    pub fn hide_act_logo(&mut self) {
        self.movie.goto_and_play(CMD_ACT_LOGO_GG, "outShort");
        self.movie.goto_and_play(CMD_ACT_EV_TOP, "outShort");
        self.movie.goto_and_play(CMD_ACT_LOGO, "outShort");
        self.act_logo = false;
    }

    /// `setStyleNormal(a1)`.
    pub fn set_style_normal(&mut self, a1: i32) {
        match a1 {
            0 => {
                let label = if self.shortcut {
                    if !self.simple_hud {
                        "change"
                    } else {
                        "scutIn"
                    }
                } else {
                    "logoIn"
                };
                self.movie.goto_and_play(CMD_TITLE, label);
                self.command_title = true;
            }
            1 => {
                let name = format!("{CMD_TITLE}.cmdTitleName");
                if ["cmdBlue", "cmdYellow", "cmdRed"].iter().any(|l| self.label_is(&name, l)) {
                    let label = if self.label_is(CMD_TITLE, "loop") { "logoOut" } else { "off" };
                    self.movie.goto_and_play(CMD_TITLE, label);
                }
                self.command_title = false;
            }
            _ => {}
        }
    }

    /// `updateStyle(a1)`: the form gauge's frame.
    pub fn update_style(&mut self, a1: i32) {
        if a1 < 0 {
            return;
        }
        self.movie.goto_frame(&format!("{CMD_ACT_LOGO_GG_SHORT}.cmdActLogoGgAnm"), a1.max(1) as u32);
    }

    /// `showLogoGauge(a1)`.
    pub fn show_logo_gauge(&mut self, a1: bool) {
        self.log(format!("showLogoGauge({a1})"));
        self.movie.goto_and_play(CMD_ACT_LOGO_GG_SHORT, if a1 { "in" } else { "loop" });
    }

    /// `hideLogoGauge(a1)`.
    pub fn hide_logo_gauge(&mut self, a1: bool) {
        self.log(format!("hideLogoGauge({a1})"));
        self.movie.goto_and_play(CMD_ACT_LOGO_GG_SHORT, if a1 { "out" } else { "off" });
    }

    /// `selectMainBtnActive(a1)`.
    pub fn select_main_btn_active(&mut self, a1: i32) {
        if a1 == self.main_btn_index {
            return;
        }
        if a1 >= 0 {
            let l2 = main_btn(a1);
            self.movie.goto_and_play(&l2, "over");
            let l3 = format!("{l2}.cmdText1");
            if !self.label_is(&l3, "disabled") {
                self.movie.goto_and_play(&l3, "on");
            }
            self.movie.goto_and_play(&format!("{l2}.cmdMainBaseSel"), "over");
            if a1 == MAIN_COMMAND_LIST_ATTACK {
                self.cmd_main_label = "down".into();
            }
        }
        if self.main_btn_index >= 0 {
            let l2 = main_btn(self.main_btn_index);
            self.movie.goto_and_play(&l2, "up");
            let l3 = format!("{l2}.cmdText1");
            if !self.label_is(&l3, "disabled") {
                self.movie.goto_and_play(&l3, "off");
            }
        }
        self.main_btn_index = a1;
    }

    /// `selectMainBtnActiveStatic(a1, a2)`.
    pub fn select_main_btn_active_static(&mut self, a1: i32, a2: bool) {
        let l3 = main_btn(a1);
        let (l4, l5) = (format!("{l3}.cmdText1"), format!("{l3}.cmdMainBaseSel"));
        if a2 {
            if !self.label_is(&l3, "up2") {
                self.movie.goto_and_play(&l3, "up2");
                self.movie.goto_and_play(&l4, "on");
                self.movie.goto_and_play(&l5, "up");
            }
            if a1 == MAIN_COMMAND_LIST_ATTACK {
                self.cmd_main_label = "up2".into();
            }
        } else {
            self.movie.goto_and_play(&l3, "up");
            self.movie.goto_and_play(&l4, "off");
            self.movie.goto_and_play(&l5, "over");
        }
    }

    /// `selectMainCommand(a1, a2, a3)`.
    pub fn select_main_command(&mut self, a1: i32, a2: i32, a3: bool) {
        self.log(format!("selectMainCommand({a1}, {a2}, {a3})"));
        self.select_main_btn_active(a1);
        let l4 = a2 >= 0 || a3;
        if l4 {
            if !self.atk_cmd_active_static {
                self.select_main_btn_active_static(0, true);
                self.atk_cmd_active_static = true;
            }
        } else if a1 != 0 && self.atk_cmd_active_static {
            self.select_main_btn_active_static(0, false);
            self.atk_cmd_active_static = false;
        }
        if a1 > MAIN_COMMAND_LIST_ATTACK && self.cmd_main_change {
            self.cmd_main_change = false;
        }
    }

    /// `selectSubCommand(a1)`.
    pub fn select_sub_command(&mut self, a1: i32) {
        self.log(format!("selectSubCommand({a1})"));
        self.select_sub_btn_active(a1);
    }

    /// `initSubCommand(a1, a2)`: `a2` rows of list `a1` (1 Magic, 2 Items, 3 Link), bottom
    /// aligned: the first row uses `cmdSub1_6` with fewer than 4 rows, else `cmdSub1_(10 - a2)`.
    pub fn init_sub_command(&mut self, a1: i32, a2: i32) {
        self.log(format!("initSubCommand({a1}, {a2})"));
        if !(MAIN_COMMAND_LIST_MAGIC..MAIN_COMMAND_LIST_MAX).contains(&a1) {
            return;
        }
        self.sub_btn_max[a1 as usize][COMMAND_LEVEL_SUBCOMMAND] = a2;
        let mut l3: usize = 5;
        if a2 == 0 {
            l3 = 0;
        } else if a2 >= 4 {
            l3 = SUB_COMMAND_MAX.saturating_sub(a2 as usize);
        }
        for l5 in 0..SUB_COMMAND_MAX {
            self.sub_btn2[a1 as usize][l5] = l3;
            l3 += 1;
            if l3 >= SUB_COMMAND_MAX {
                l3 = 0;
            }
        }
    }

    /// `initSubCommand2(a1, a2)`: the party target list (`a1` must be 4).
    pub fn init_sub_command2(&mut self, a1: i32, a2: i32) {
        self.log(format!("initSubCommand2({a1}, {a2})"));
        if a1 != SUB_COMMAND_INDEX as i32 {
            return;
        }
        for list in 1..=3 {
            self.sub_btn_max[list][COMMAND_LEVEL_SUBTARGET] = a2;
        }
        let mut l3: usize = 3;
        if a2 == 0 {
            l3 = 0;
        } else if a2 >= 3 {
            l3 = SUB_COMMAND2_MAX.saturating_sub(a2 as usize);
        }
        for l5 in 0..SUB_COMMAND2_MAX {
            self.sub_btn2[SUB_COMMAND_INDEX][l5] = l3;
            l3 += 1;
            if l3 >= SUB_COMMAND2_MAX {
                l3 = 0;
            }
        }
    }

    /// The row clip `m_mcSubBtnAry2[list][row]`.
    fn sub_btn(&self, list: usize, row: usize) -> String {
        if list == SUB_COMMAND_INDEX {
            format!("{CMD_SUB_TOP}.cmdSub2Set.cmdSub2_{}", self.sub_btn2[list][row % SUB_COMMAND2_MAX] + 1)
        } else {
            format!("{CMD_SUB_TOP}.cmdSub1Set.cmdSub1_{}", self.sub_btn2[list.min(4)][row % SUB_COMMAND_MAX] + 1)
        }
    }

    /// The clip name of a first-level row (for tests): `cmdSub1_N`.
    pub fn sub_row_clip(&self, list: i32, row: usize) -> String {
        self.sub_btn(list as usize, row)
    }

    fn main_index(&self) -> usize {
        self.main_btn_index.clamp(0, 3) as usize
    }

    /// `setSubCommandText(...rest)`: the texts of the open list's rows.
    pub fn set_sub_command_text(&mut self, l1: &[&str]) {
        self.log(format!("setSubCommandText({l1:?})"));
        let main = self.main_index();
        for l4 in 0..SUB_COMMAND_MAX {
            let l3 = self.sub_btn(main, l4);
            self.text(&format!("{l3}.cmdText1.{NAME}"), l1.get(l4).copied().unwrap_or(""));
        }
    }

    /// `setSubCommand2Text(a1, ...rest)`: the carried row's text and the party names.
    pub fn set_sub_command2_text(&mut self, a1: &str, l2: &[&str]) {
        self.log(format!("setSubCommand2Text({a1:?}, {l2:?})"));
        self.text(&format!("{CMD_SUB1_NEXT}.cmdText1.{NAME}"), a1);
        for l5 in 0..SUB_COMMAND2_MAX {
            let l4 = self.sub_btn(SUB_COMMAND_INDEX, l5);
            self.text(&format!("{l4}.cmdText1.{NAME}"), l2.get(l5).copied().unwrap_or(""));
        }
    }

    fn sub_parent(level: usize) -> &'static str {
        if level == COMMAND_LEVEL_SUBTARGET {
            "cmdMenu.cmdSubTop.cmdSub2Set"
        } else {
            "cmdMenu.cmdSubTop.cmdSub1Set"
        }
    }

    /// `setSubCommand(a1, a2)`: 0 in, 1 out, 2 cancel out, 3 decide out, 4 finish in, 5 finish out.
    pub fn set_sub_command(&mut self, a1: i32, a2: i32) {
        self.log(format!("setSubCommand({a1}, {a2})"));
        match a1 {
            SUB_COMMAND_IN => {
                if self.main_btn_index == MAIN_COMMAND_LIST_ATTACK {
                    return;
                }
                let main = self.main_index();
                self.level = (self.level + 1).min(COMMAND_LEVEL_SUBTARGET);
                if self.level == COMMAND_LEVEL_SUBCOMMAND {
                    let colour = self.sub_colors.map_or(LABELS_SUBCOMMAND_COLOR[main], |c| c[main.min(3)]); // (DON-UI)
                    self.movie.goto_and_play(CMD_SUB_TOP, colour);
                }
                let l3 = if self.level != COMMAND_LEVEL_SUBTARGET { main } else { SUB_COMMAND_INDEX };
                let mut l4 = self.sub_btn_max[main][self.level];
                self.movie.goto_and_play(Self::sub_parent(self.level), "in");
                let l6 = if l3 != SUB_COMMAND_INDEX { SUB_COMMAND_MAX } else { SUB_COMMAND2_MAX };
                for l5 in 0..l6 {
                    let l7 = self.sub_btn(l3, l5);
                    if (l5 as i32) < l4 {
                        self.movie.goto_and_play(&l7, "up");
                        self.movie.goto_and_play(&format!("{l7}.cmdText1"), "off");
                        if l3 != SUB_COMMAND_INDEX {
                            self.text(&format!("{l7}.cmdText1.{NUMBER_TXT}"), "");
                        }
                    } else {
                        self.movie.goto_and_play(&l7, "off");
                    }
                }
                if a2 >= 0 {
                    self.sub_btn_index_old[main][self.level] = a2;
                }
                if self.level == COMMAND_LEVEL_SUBCOMMAND {
                    self.sub_btn_index[main][self.level] = -1;
                    self.set_main_command(MAIN_COMMAND_MOVE_NEXT);
                } else if self.level == COMMAND_LEVEL_SUBTARGET {
                    l4 = self.sub_btn_max[main][self.level - 1];
                    for l5 in 0..l4.max(0) as usize {
                        let row = self.sub_btn(main, l5);
                        if self.sub_btn_index[main][self.level - 1] == l5 as i32 {
                            self.movie.goto_and_play(&row, "nextOut");
                            self.movie.goto_and_play(CMD_SUB1_NEXT, "nextIn");
                            if main as i32 != MAIN_COMMAND_LIST_ITEM {
                                self.text(&format!("{CMD_SUB1_NEXT}.cmdText1.{NUMBER_TXT}"), "");
                            }
                        } else {
                            self.movie.goto_and_play(&row, "out");
                        }
                    }
                }
                self.select_sub_btn_active(self.sub_btn_index_old[main][self.level]);
            }
            SUB_COMMAND_OUT | SUB_COMMAND_CANCEL_OUT => {
                if self.level == 0 {
                    return;
                }
                let main = self.main_index();
                self.sub_btn_index[main][self.level] = -1;
                self.movie.goto_and_play(Self::sub_parent(self.level), "out");
                if self.level == COMMAND_LEVEL_SUBCOMMAND {
                    if a1 == SUB_COMMAND_OUT {
                        self.set_main_command(MAIN_COMMAND_BACK_ENABLE);
                    } else {
                        self.set_main_command(MAIN_COMMAND_BACK_DISABLE);
                        self.main_btn_index = -1;
                        self.select_main_btn_active(0);
                    }
                } else if self.level == COMMAND_LEVEL_SUBTARGET {
                    let l4 = self.sub_btn_max[main][self.level - 1];
                    for l5 in 0..l4.max(0) as usize {
                        let row = self.sub_btn(main, l5);
                        if self.sub_btn_index[main][self.level - 1] == l5 as i32 {
                            self.movie.goto_and_play(&row, "turnIn");
                            self.movie.goto_and_play(CMD_SUB1_NEXT, "turnOut");
                        } else {
                            self.movie.goto_and_play(&row, "in");
                        }
                    }
                }
                self.level -= 1;
            }
            SUB_COMMAND_DECIDE_OUT => {
                if self.level == 0 {
                    return;
                }
                let main = self.main_index();
                self.sub_btn_index_old[main][self.level] = self.sub_btn_index[main][self.level];
                self.sub_btn_index[main][self.level] = -1;
                self.movie.goto_and_play(Self::sub_parent(self.level), "out");
                self.set_main_command(MAIN_COMMAND_BACK_DISABLE);
                self.main_btn_index = -1;
                self.select_main_btn_active(0);
                if self.level == COMMAND_LEVEL_SUBTARGET {
                    // (The AS3 indexes with m_nMainBtnIndex, which is 0 by now.)
                    let main = self.main_index();
                    self.movie.goto_and_play(Self::sub_parent(self.level - 1), "out");
                    self.sub_btn_index_old[main][self.level - 1] = self.sub_btn_index[main][self.level - 1];
                    self.sub_btn_index[main][self.level - 1] = -1;
                    self.movie.goto_and_play(CMD_SUB1_NEXT, "out");
                    self.level = COMMAND_LEVEL_SUBCOMMAND;
                }
                self.level -= 1;
            }
            SUB_COMMAND_FINISH_IN => {
                let main = self.main_index();
                self.select_sub_btn_active(self.sub_btn_index_old[main][self.level.min(3)]);
            }
            SUB_COMMAND_FINISH_OUT => self.level = self.level.saturating_sub(1),
            _ => {}
        }
    }

    /// `setSubCommandNumText(a1, a2, a3, ...rest)`: the quantity texts of list `a1`'s first
    /// `a2` rows ("" = no number); `a3` = in battle (label `battle`, else `nomal`).
    pub fn set_sub_command_num_text(&mut self, a1: i32, a2: i32, a3: bool, l4: &[&str]) {
        self.log(format!("setSubCommandNumText({a1}, {a2}, {a3}, {l4:?})"));
        if !(1..=4).contains(&a1) {
            return;
        }
        for l6 in 0..a2.max(0) as usize {
            let l8 = format!("{}.cmdText1.cmdNumber", self.sub_btn(a1 as usize, l6));
            self.number_text(&l8, l4.get(l6).copied().unwrap_or(""), a3);
        }
    }

    fn number_text(&mut self, number: &str, text: &str, battle: bool) {
        if !text.is_empty() {
            self.movie.goto_and_play(number, "loop");
            self.text(&format!("{number}.cmdNumCol.txCmdNumber.cmdNumberTxt"), text);
            self.movie.goto_and_play(&format!("{number}.cmdNumCol"), if battle { "battle" } else { "nomal" });
        } else {
            self.movie.goto_and_play(number, "off");
        }
    }

    /// `setSubCommandNextNumText(a1, a2)`.
    pub fn set_sub_command_next_num_text(&mut self, a1: bool, a2: &str) {
        self.log(format!("setSubCommandNextNumText({a1}, {a2:?})"));
        self.number_text(&format!("{CMD_SUB1_NEXT}.cmdText1.cmdNumber"), a2, a1);
    }

    /// `selectSubBtnActive(a1)`.
    pub fn select_sub_btn_active(&mut self, a1: i32) {
        let main = self.main_index();
        let level = self.level.min(2);
        if a1 == self.sub_btn_index[main][level] {
            return;
        }
        let l2 = if level != COMMAND_LEVEL_SUBTARGET { main } else { SUB_COMMAND_INDEX };
        if a1 >= 0 {
            let l3 = self.sub_btn(l2, a1 as usize);
            self.movie.goto_and_play(&l3, "over");
            let l4 = format!("{l3}.cmdText1");
            if !self.label_is(&l4, "disabled") {
                self.movie.goto_and_play(&l4, "on");
            }
            self.movie.goto_and_play(&format!("{l3}.cmdSubBaseSel"), "over");
        }
        let current = self.sub_btn_index[main][level];
        if current >= 0 {
            let l3 = self.sub_btn(l2, current as usize);
            self.movie.goto_and_play(&l3, "up");
            let l4 = format!("{l3}.cmdText1");
            if !self.label_is(&l4, "disabled") {
                self.movie.goto_and_play(&l4, "off");
            }
            self.movie.goto_and_play(&format!("{l3}.cmdSubBaseSel"), "up");
        }
        self.sub_btn_index_old[main][level] = current;
        self.sub_btn_index[main][level] = a1;
    }

    /// `addActionSelectButton(a1, a2, a3, a4, a5)`: position, name, icon type, in-effect,
    /// has a timer. Returns the button index (-1: none free).
    pub fn add_action_select_button(&mut self, a1: i32, a2: &str, a3: i32, a4: i32, a5: bool) -> i32 {
        self.log(format!("addActionSelectButton({a1}, {a2:?}, {a3}, {a4}, {a5})"));
        let Some(l7) = (0..ACTION_COMMAND_MAX).find(|l6| self.action_pos[*l6] < 0) else { return -1 };
        if !(0..ACTION_COMMAND_MAX as i32).contains(&a1) {
            return -1;
        }
        for l6 in 0..ACTION_COMMAND_MAX {
            if self.action_pos[l6] >= 0 && self.action_pos[l6] >= a1 {
                self.action_pos[l6] += 1;
                if let Some(slot) = self.action_idx.get_mut(self.action_pos[l6] as usize) {
                    *slot = l6 as i32;
                }
            }
        }
        self.set_action_select_button(ACTION_COMMAND_BUTTON_IN, l7 as i32, a2, a3, a1, a4, a5);
        self.action_pos[l7] = a1;
        self.action_idx[a1 as usize] = l7 as i32;
        self.action_btn_cnt += 1;
        l7 as i32
    }

    /// `addActionSelectEventButton(a1, a2, a3)`.
    pub fn add_action_select_event_button(&mut self, a1: &str, a2: bool, a3: i32) -> i32 {
        self.log(format!("addActionSelectEventButton({a1:?}, {a2}, {a3})"));
        self.visible_situation_finish_button = true;
        self.movie.goto_and_play(CMD_ACT_EV_SHORT, "in");
        if let Some(label) = ["in", "in2", "in3", "in4"].get(a3 as usize) {
            self.movie.goto_and_play(&format!("{CMD_ACT_EV_SHORT}.cmdActInEff"), label);
        }
        self.movie.goto_and_play(CMD_ACT_EV, "up2");
        let l7 = format!("{CMD_ACT_EV}.cmdActEvText");
        self.movie.goto_and_play(&format!("{l7}.cmdText1"), "off");
        self.text(&format!("{l7}.cmdText1.{NAME}"), a1);
        self.movie.goto_and_stop(&l7, if a2 { "gage" } else { "normal" });
        self.movie.goto_and_play(CMD_ACT_LOGO_GG_SHORT, if a2 { "in" } else { "off" });
        -1
    }

    /// `deleteActionSelectButton(a1, a2, a3)`: position, used (decide), is a form change.
    pub fn delete_action_select_button(&mut self, a1: i32, a2: bool, a3: bool) {
        self.log(format!("deleteActionSelectButton({a1}, {a2}, {a3})"));
        let Some(l5) = (0..ACTION_COMMAND_MAX).find(|l4| self.action_pos[*l4] == a1 && a1 >= 0) else { return };
        self.set_action_select_button(if a2 { ACTION_COMMAND_BUTTON_DECIDE_OUT } else { ACTION_COMMAND_BUTTON_OUT }, l5 as i32, "", 0, 0, 0, false);
        self.action_pos[l5] = -1;
        self.action_idx[a1 as usize] = -1;
        self.action_btn_cnt -= 1;
        for l4 in 0..ACTION_COMMAND_MAX {
            if self.action_pos[l4] >= 0 && self.action_pos[l4] > a1 {
                self.set_action_select_button(ACTION_COMMAND_BUTTON_PUSH_DOWN, l4 as i32, "", 0, 0, 0, false);
                self.action_pos[l4] -= 1;
                self.action_idx[self.action_pos[l4] as usize] = l4 as i32;
            }
        }
        // (a2 && a3 && m_mcCmdActLogo != null: the form logo plays "finishIn"; no logo here.)
        let _ = a3;
    }

    /// `deleteActionSelectEventButton(a1, a2, a3)`.
    pub fn delete_action_select_event_button(&mut self, a1: bool, a2: bool, a3: bool) {
        self.log(format!("deleteActionSelectEventButton({a1}, {a2}, {a3})"));
        let _ = a2;
        self.visible_situation_finish_button = false;
        let l4 = CMD_ACT_LOGO_GG_SHORT;
        if a1 {
            self.movie.goto_and_play(CMD_ACT_EV_SHORT, "down");
            if self.label_is(l4, "loop") || self.label_is(l4, "break") {
                self.movie.goto_and_play(l4, "out");
            } else if self.label_is(l4, "off") && a3 {
                self.movie.goto_and_play(l4, "loop");
            }
        } else {
            self.movie.goto_and_play(CMD_ACT_EV_SHORT, "out");
            if a3 {
                self.movie.goto_and_play(l4, "loop");
            } else if ["in", "loop", "break"].iter().any(|l| self.label_is(l4, l)) {
                self.movie.goto_and_play(l4, "out");
            }
        }
    }

    /// `setActionCommandMark(a1, a2, a3, a4)`: the active entry's list index (-1 = the event
    /// button), selected, restricted, show the triangle.
    pub fn set_action_command_mark(&mut self, a1: i32, a2: bool, a3: bool, a4: bool) {
        self.log(format!("setActionCommandMark({a1}, {a2}, {a3}, {a4})"));
        for l9 in 0..self.action_btn_cnt.clamp(0, ACTION_COMMAND_MAX as i32) {
            let l8 = self.action_idx[l9 as usize];
            if l8 < 0 || !self.action_has_act1[l8 as usize] {
                continue;
            }
            let l5 = format!("{}.cmdAct.cmdAct1", act_btn(l8 as usize));
            self.mark(&l5, &format!("{l5}.cmdText1"), l9 == a1 && !a3, a2, a4, "off");
        }
        if self.visible_situation_finish_button {
            let disabled = if !a3 { "off" } else { "disabled" };
            self.mark(CMD_ACT_EV, &format!("{CMD_ACT_EV}.cmdActEvText.cmdText1"), a1 == -1 && !a3, a2, a4, disabled);
        }
    }

    fn mark(&mut self, l5: &str, l7: &str, active: bool, a2: bool, a4: bool, other_text: &str) {
        let l6 = format!("{l5}.padBtn");
        if active {
            if self.legacy_mode && a2 {
                if !self.label_is(l5, "over") {
                    self.movie.goto_and_play(l5, "over");
                }
            } else {
                self.movie.goto_and_play(l5, "up2");
            }
            if a4 {
                self.pad_text(&format!("{l6}.txPadBtn"), BUTTON_TEXT_UP); // (RC)
                self.movie.set_visible(&l6, true);
            } else {
                self.movie.set_visible(&l6, false);
            }
            self.movie.goto_and_play(l7, "on");
        } else {
            self.movie.goto_and_play(l5, "up");
            // (RC) the clip's own timeline hides padBtn at "up" (frame 7: placed hidden at frame 1, shown at "up2" frame 11);
            // our player keeps the visibility a script set, so a row that lost the mark kept its Y icon. Hidden here.
            self.movie.set_visible(&l6, false);
            self.movie.goto_and_play(l7, other_text);
        }
    }

    /// `setStylePointPhase(a1)`.
    pub fn set_style_point_phase(&mut self, a1: i32) {
        self.log(format!("setStylePointPhase({a1})"));
        if self.style_point_gauge_button_index >= 0 {
            let l2 = format!("{}.cmdAct.cmdActStylePntGg", act_btn(self.style_point_gauge_button_index as usize));
            let label = match a1 {
                STYLE_POINT_PHASE0 => Some("off"),
                STYLE_POINT_PHASE1 => Some(if self.style_point_gauge_phase > STYLE_POINT_PHASE1 { "st1" } else { "in" }),
                STYLE_POINT_PHASE2 => Some(if self.style_point_gauge_phase > STYLE_POINT_PHASE2 { "st2" } else { "in2" }),
                STYLE_POINT_PHASE3 => Some("in3"),
                _ => None,
            };
            if let Some(label) = label {
                self.movie.goto_and_play(&l2, label);
            }
        }
        self.style_point_gauge_phase = a1;
    }

    /// `playActionCommandEffect(a1)`.
    pub fn play_action_command_effect(&mut self, a1: i32) {
        self.log(format!("playActionCommandEffect({a1})"));
        if let Some(button) = self.action_idx.get(a1.max(0) as usize).copied().filter(|b| *b >= 0) {
            self.set_action_select_button(ACTION_COMMAND_BUTTON_EFFECT, button, "", 0, 0, 0, false);
        }
    }

    /// `breakActionSelectButton(a1)`.
    pub fn break_action_select_button(&mut self, a1: i32) {
        self.log(format!("breakActionSelectButton({a1})"));
        if let Some(button) = self.action_idx.get(a1.max(0) as usize).copied().filter(|b| *b >= 0) {
            self.set_action_select_button(ACTION_COMMAND_BUTTON_BREAK, button, "", 0, 0, 0, false);
        }
    }

    /// `breakActionSelectEventButton(a1, a2)`.
    pub fn break_action_select_event_button(&mut self, a1: bool, a2: bool) {
        self.log(format!("breakActionSelectEventButton({a1}, {a2})"));
        if a2 {
            self.movie.goto_and_play(CMD_ACT_EV_SHORT, "break");
        }
        if a1 {
            self.movie.goto_and_play(CMD_ACT_LOGO_GG_SHORT, "break");
        }
    }

    /// `setActionSelectButton(a1, a2, a3, a4, a5, a6, a7)`.
    #[allow(clippy::too_many_arguments)]
    fn set_action_select_button(&mut self, a1: i32, a2: i32, a3: &str, a4: i32, a5: i32, a6: i32, a7: bool) {
        let Some(button) = (0..ACTION_COMMAND_MAX as i32).contains(&a2).then_some(a2 as usize) else { return };
        let l8 = act_btn(button);
        let act = format!("{l8}.cmdAct");
        let l9 = format!("{act}.cmdAct1");
        let bring_to_front = |movie: &mut Movie| {
            if let Some(parent) = movie.sprite_mut(&format!("{CMD_ACT_ALL}.cmdActState")) {
                parent.bring_to_front(&format!("cmdActPos{}", button + 1));
            }
        };
        match a1 {
            ACTION_COMMAND_BUTTON_IN => {
                bring_to_front(&mut self.movie);
                self.movie.goto_and_play(&l8, LABELS_ACTION_COMMAND_POS[a5.clamp(0, 6) as usize]);
                self.movie.goto_and_play(&act, "in");
                self.style_point_gauge_button_index = -1;
                self.style_point_gauge_phase = -1;
                if a4 == ACTION_COMMAND_TYPE_STYLE_POINT_GAUGE {
                    self.style_point_gauge_button_index = a2;
                    self.movie.goto_and_play(&l9, "off");
                    self.action_has_act1[button] = false;
                } else {
                    let ty = LABELS_ACTION_COMMAND_TYPE.get(a4 as usize).copied().unwrap_or("");
                    if let Some(label) = ["in", "in2", "in3", "in4"].get(a6 as usize) {
                        self.movie.goto_and_play(&format!("{act}.cmdActInEff"), label);
                    }
                    self.movie.goto_and_play(&l9, "up2");
                    if a7 {
                        self.movie.goto_and_stop(&format!("{l9}.actCount"), "loop");
                        self.movie.goto_and_stop(&format!("{l9}.actGgAnm"), "loop");
                        self.movie.goto_and_play(&format!("{l9}.actGgAnm.actGg"), ty);
                    } else {
                        self.movie.goto_and_stop(&format!("{l9}.actCount"), "off");
                        self.movie.goto_and_stop(&format!("{l9}.actGgAnm"), "off");
                    }
                    self.configure_counter(button);
                    self.movie.goto_and_play(&l9, "up");
                    self.movie.goto_and_play(&format!("{l9}.cmdText1"), "off");
                    self.text(&format!("{l9}.cmdText1.{NAME}"), a3);
                    // (RC) type 0 (none, e.g. the host's stacked Y interactions) has no label: the clip's own "off" (frame 1),
                    // else a reused row kept the icon of the command before it
                    self.movie.goto_and_play(&format!("{l9}.cmdActIco"), if ty.is_empty() { "off" } else { ty });
                    self.action_has_act1[button] = true;
                }
                self.movie.goto_and_play(&format!("{act}.cmdActStylePntGg"), "off");
            }
            ACTION_COMMAND_BUTTON_OUT => drop(self.movie.goto_and_play(&act, "out")),
            ACTION_COMMAND_BUTTON_DECIDE_OUT => {
                bring_to_front(&mut self.movie);
                self.movie.goto_and_play(&act, "down");
                if self.action_has_act1[button] {
                    self.movie.goto_and_play(&l9, "down");
                }
            }
            ACTION_COMMAND_BUTTON_PUSH_UP => {}
            ACTION_COMMAND_BUTTON_PUSH_DOWN => {
                let l10 = self.action_pos[button];
                self.movie.goto_and_play(&l8, &format!("sita{l10}"));
            }
            ACTION_COMMAND_BUTTON_ADD_TRI => {
                if self.action_has_act1[button] {
                    self.movie.goto_and_play(&l9, "up2");
                }
            }
            ACTION_COMMAND_BUTTON_DEL_TRI => {
                if self.action_has_act1[button] {
                    self.movie.goto_and_play(&l9, "up");
                }
            }
            ACTION_COMMAND_BUTTON_EFFECT => drop(self.movie.goto_and_play(&format!("{act}.cmdActInEff"), "in")),
            ACTION_COMMAND_BUTTON_BREAK => drop(self.movie.goto_and_play(&act, "break")),
            _ => {}
        }
    }

    /// `setActionSelect(a1, a2, a3)`.
    pub fn set_action_select(&mut self, a1: i32, a2: i32, a3: i32) {
        self.log(format!("setActionSelect({a1}, {a2}, {a3})"));
        let top_is = |hud: &HudCommand, labels: &[&str]| labels.iter().any(|l| hud.label_is(CMD_ACT_TOP, l));
        match a1 {
            ACTION_COMMAND_IN => {
                self.movie.goto_and_play(CMD_ACT_TOP, "loop");
                if a3 != 0 {
                    self.show_act_logo(false);
                }
            }
            ACTION_COMMAND_OUT => drop(self.movie.goto_and_play(CMD_ACT_TOP, "off")),
            ACTION_COMMAND_PUSH_UP => {
                if !self.label_is(CMD_ACT_ALL, "up") {
                    self.movie.goto_and_play(CMD_ACT_ALL, "up");
                }
            }
            ACTION_COMMAND_PUSH_DOWN => {
                if !self.label_is(CMD_ACT_ALL, "loop") {
                    self.movie.goto_and_play(CMD_ACT_ALL, "down");
                }
            }
            ACTION_COMMAND_DISABLE => {
                if top_is(self, &["loop", "back"]) {
                    self.movie.goto_and_play(CMD_ACT_TOP, "dark");
                    if self.visible_situation_finish_button {
                        self.movie.goto_and_play(CMD_ACT_EV, "up");
                    }
                }
            }
            ACTION_COMMAND_ENABLE => {
                if top_is(self, &["dark"]) {
                    self.movie.goto_and_play(CMD_ACT_TOP, "back");
                    if self.visible_situation_finish_button {
                        self.movie.goto_and_play(CMD_ACT_EV, "over");
                    }
                }
            }
            ACTION_COMMAND_RESTRICT_ON => {
                if top_is(self, &["loop", "back"]) {
                    self.movie.goto_and_play(CMD_ACT_TOP, "dark");
                }
            }
            ACTION_COMMAND_RESTRICT_OFF => {
                if top_is(self, &["dark"]) {
                    self.movie.goto_and_play(CMD_ACT_TOP, "back");
                }
            }
            ACTION_COMMAND_FINISH_DISABLE => drop(self.movie.goto_and_play(CMD_ACT_EV, if a2 == 0 { "up" } else { "up2" })),
            ACTION_COMMAND_FINISH_ENABLE => drop(self.movie.goto_and_play(CMD_ACT_EV, "over")),
            ACTION_COMMAND_FINISH_IN => drop(self.movie.goto_and_play(CMD_ACT_EV_TOP, "loop")),
            ACTION_COMMAND_FINISH_OUT => drop(self.movie.goto_and_play(CMD_ACT_EV_TOP, "off")),
            ACTION_COMMAND_DISABLE_IN => {
                self.movie.goto_and_play(CMD_ACT_TOP, "dark");
                if a3 != 0 {
                    self.show_act_logo(false);
                }
            }
            _ => {}
        }
    }

    /// `setShortcutText(a1, a2, a3, a4)`.
    pub fn set_shortcut_text(&mut self, a1: &str, a2: &str, a3: &str, a4: &str) {
        self.log(format!("setShortcutText({a1:?}, {a2:?}, {a3:?}, {a4:?})"));
        for (i, text) in [a1, a2, a3, a4].into_iter().enumerate() {
            self.text(&format!("{}.cmdExText1.{NAME}", ex_set(CMD_SCUT_SHORT, i)), text);
        }
        self.shortcut_text_status([0; 4]);
    }

    /// `setShortcutTextStatus(a1, a2, a3, a4)`: non-zero flashes the row.
    pub fn set_shortcut_text_status(&mut self, a1: i32, a2: i32, a3: i32, a4: i32) {
        self.log(format!("setShortcutTextStatus({a1}, {a2}, {a3}, {a4})"));
        self.shortcut_text_status([a1, a2, a3, a4]);
    }

    fn shortcut_text_status(&mut self, status: [i32; 4]) {
        for (i, s) in status.into_iter().enumerate() {
            self.movie.goto_and_play(&format!("{}.cmdEffChange", ex_set(CMD_SCUT_SHORT, i)), if s != 0 { "in" } else { "off" });
        }
    }

    /// `setShortcutTextColor(a1, a2, a3, a4)`.
    pub fn set_shortcut_text_color(&mut self, a1: i32, a2: i32, a3: &str, a4: bool) {
        self.log(format!("setShortcutTextColor({a1}, {a2}, {a3:?}, {a4})"));
        let l7 = format!("{}.cmdExText1.txCmdName", ex_set(CMD_SCUT_SHORT, a2.max(0) as usize));
        self.text(&format!("{l7}.txCmdNameCol.coCmdName"), a3);
        self.movie.goto_and_play(&l7, if a4 { "col1" } else { "col2" });
    }

    /// `setShortcutTextBadStatus(a1, a2, a3, a4)`.
    pub fn set_shortcut_text_bad_status(&mut self, a1: i32, a2: i32, a3: &str, a4: i32) {
        self.log(format!("setShortcutTextBadStatus({a1}, {a2}, {a3:?}, {a4})"));
        let l7 = ex_set(CMD_SCUT_SHORT, a2.max(0) as usize);
        self.text(&format!("{l7}.cmdExText1.{NAME}"), a3);
        self.movie.goto_and_play(&format!("{l7}.cmdEffChange"), if a4 != 0 { "in" } else { "off" });
    }

    /// `setModeShortcut(a1, a2)`: 0 in, 1 out, 2 out after an execution, 3 page down, 4 page up.
    pub fn set_mode_shortcut(&mut self, a1: i32, a2: i32) {
        self.log(format!("setModeShortcut({a1}, {a2})"));
        match a1 {
            SHORTCUT_COMMAND_IN => {
                self.shortcut_select = a2;
                self.set_shortcut_button(SHORTCUT_COMMAND_IN, a2);
                self.set_main_command(MAIN_COMMAND_LOGO_OUT);
            }
            SHORTCUT_COMMAND_OUT => {
                self.set_shortcut_button(SHORTCUT_COMMAND_OUT, a2);
                self.set_main_command(MAIN_COMMAND_LOGO_IN);
            }
            SHORTCUT_COMMAND_OUT_EXEC => self.set_shortcut_button(SHORTCUT_COMMAND_OUT_EXEC, a2),
            SHORTCUT_COMMAND_PAGE_DOWN => self.set_shortcut_button(SHORTCUT_COMMAND_MOVE_UP, a2),
            SHORTCUT_COMMAND_PAGE_UP => self.set_shortcut_button(SHORTCUT_COMMAND_MOVE_DOWN, a2),
            _ => {}
        }
    }

    /// `setShortcutButton(a1, a2)`.
    fn set_shortcut_button(&mut self, a1: i32, a2: i32) {
        let l7 = CMD_SCUT_SHORT;
        let deck = format!("{l7}.cmdScutDeck");
        let logo_shown = self.label_is(CMD_ACT_LOGO_SHORT, "in") || self.label_is(CMD_ACT_LOGO_SHORT, "loop");
        match a1 {
            SHORTCUT_COMMAND_IN => {
                self.movie.goto_and_play(&format!("{l7}.cmdEx.cmdExBaseSet"), "scut");
                self.movie.goto_and_play(CMD_SCUT_TOP, "in");
                for page in 0..3 {
                    self.movie.goto_and_play(&format!("{deck}.cmdScutPage{}", page + 1), if a2 == page { "loop" } else { "disabled" });
                }
                let l9 = [BUTTON_TEXT_RIGHT, BUTTON_TEXT_UP, BUTTON_TEXT_LEFT, BUTTON_TEXT_DOWN];
                for l3 in 0..SHORTCUT_COMMAND_MAX {
                    let l5 = ex_set(l7, l3);
                    self.movie.goto_and_play(&l5, "loop");
                    self.movie.goto_and_play(&format!("{l5}.cmdExText1"), "on");
                    self.pad_text(&format!("{l5}.padBtn.txPadBtn"), l9[l3]); // (RC)
                }
                self.movie.goto_and_play(CMD_TITLE, "scutIn");
                self.set_title_top(TITLE_TOP_SHORTCUT, false, false);
                self.shortcut = true;
            }
            SHORTCUT_COMMAND_OUT => {
                self.movie.goto_and_play(CMD_SCUT_TOP, "out");
                if logo_shown {
                    self.movie.goto_and_play(CMD_TITLE, "off");
                } else {
                    self.movie.goto_and_play(CMD_TITLE_TOP, "loop");
                    if self.simple_hud {
                        self.movie.goto_and_play(CMD_TITLE, "scutOut");
                    }
                }
                self.set_title_top(self.battle_mode, false, false);
                self.shortcut = false;
            }
            SHORTCUT_COMMAND_OUT_EXEC => {
                self.movie.goto_and_play(CMD_SCUT_TOP, "out");
                if logo_shown {
                    self.movie.goto_and_play(CMD_TITLE, "off");
                } else {
                    self.movie.goto_and_play(CMD_TITLE_TOP, "loop");
                    self.movie.goto_and_play(CMD_TITLE, "logoOut");
                }
                self.shortcut = false;
            }
            SHORTCUT_COMMAND_IN_END => {}
            SHORTCUT_COMMAND_OUT_END => self.shortcut = false,
            SHORTCUT_COMMAND_MOVE_UP | SHORTCUT_COMMAND_MOVE_DOWN => {
                let down = a1 == SHORTCUT_COMMAND_MOVE_UP;
                self.movie.goto_and_play(&format!("{l7}.cmdEx.cmdExBaseSet"), "scutChange");
                self.movie.goto_and_play(&format!("{deck}.{}", if down { "cmdScutArrowD" } else { "cmdScutArrowU" }), "in");
                if (0..3).contains(&self.shortcut_select) {
                    let from = self.shortcut_select + 1;
                    // Page down: 1 -> 2 -> 3 -> 1; page up: 1 -> 3 -> 2 -> 1.
                    let to = if down { from % 3 + 1 } else { (from + 1) % 3 + 1 };
                    self.movie.goto_and_play(&format!("{deck}.cmdScutPage{from}"), "out");
                    self.movie.goto_and_play(&format!("{deck}.cmdScutPage{to}"), "in");
                }
                self.shortcut_select = a2;
                for l3 in 0..SHORTCUT_COMMAND_MAX {
                    let l5 = ex_set(l7, l3);
                    self.movie.goto_and_play(&l5, "scutChange");
                    self.movie.goto_and_play(&format!("{l5}.cmdExText1"), "on");
                }
            }
            _ => {}
        }
    }

    /// `setShortcutCommandNumText(a1, ...rest)`.
    pub fn set_shortcut_command_num_text(&mut self, a1: bool, l2: &[&str]) {
        self.log(format!("setShortcutCommandNumText({a1}, {l2:?})"));
        for l4 in 0..SHORTCUT_COMMAND_MAX {
            let l6 = format!("{}.cmdExText1.cmdNumber", ex_set(CMD_SCUT_SHORT, l4));
            self.number_text(&l6, l2.get(l4).copied().unwrap_or(""), a1);
        }
    }

    /// `setShootFlow(a1, a2, a3, a4, ...rest)`: 0 in (`a2` rows of (text, button)), 1 out,
    /// 2 change (`a3` = enabled bits of rows 1 and 2), 3 logo. `a4` = entered from the shortcut
    /// panel (the main slots are not moved).
    pub fn set_shoot_flow(&mut self, a1: i32, a2: i32, a3: i32, a4: bool, rows: &[(&str, &str)]) {
        self.log(format!("setShootFlow({a1}, {a2}, {a3}, {a4}, {rows:?})"));
        match a1 {
            SHOOT_FLOW_COMMAND_IN => {
                if a2 > 0 && !rows.is_empty() {
                    let n = (a2 as usize).min(rows.len());
                    let texts: Vec<&str> = rows[..n].iter().map(|r| r.0).collect();
                    let buttons: Vec<&str> = rows[..n].iter().map(|r| r.1).collect();
                    self.mode_command = true;
                    self.movie.goto_and_play(&format!("{CMD_EX_TOP}.cmdEx.cmdExBaseSet"), &format!("line{a2}"));
                    self.set_mode_command(MODE_COMMAND_IN, &texts, &buttons, true);
                    self.movie.goto_and_play(CMD_EX_TOP, "in");
                    self.set_title_top(TITLE_TOP_SHOOTLOCK, false, false);
                    if self.style > 0 {
                        self.movie.goto_and_play(CMD_TITLE, "loop");
                    }
                }
                if !a4 {
                    self.set_main_command(MAIN_COMMAND_LOGO_OUT);
                }
            }
            SHOOT_FLOW_COMMAND_OUT => {
                self.movie.goto_and_play(CMD_EX_TOP, "out");
                self.set_title_top(self.battle_mode, false, false);
                if self.style > 0 {
                    self.movie.goto_and_play(CMD_TITLE, "off");
                }
                if !a4 {
                    self.set_main_command(MAIN_COMMAND_LOGO_IN);
                }
            }
            SHOOT_FLOW_COMMAND_CHANGE => {
                if self.mode_command {
                    self.movie.goto_and_play(&format!("{}.cmdExText1", ex_set(CMD_EX_TOP, 0)), if a3 & 1 == 1 { "on" } else { "disabled" });
                    self.movie.goto_and_play(&format!("{}.cmdExText1", ex_set(CMD_EX_TOP, 1)), if a3 & 2 == 2 { "on" } else { "disabled" });
                }
            }
            SHOOT_FLOW_COMMAND_LOGO => self.show_act_logo(false),
            _ => {}
        }
    }

    /// `setBattleMode(a1, a2)`: 0 field (blue), 1 yellow, 2 battle (red); `a2` = keep the title.
    pub fn set_battle_mode(&mut self, a1: i32, a2: bool) {
        self.log(format!("setBattleMode({a1}, {a2})"));
        if !(TITLE_TOP_COMMAND_BLUE..=TITLE_TOP_COMMAND_RED).contains(&a1) {
            return;
        }
        self.battle_mode = a1;
        self.movie.goto_and_play(CMD_MAIN_TOP, ["cmdBlue", "cmdYellow", "cmdRed"][a1 as usize]);
        if !a2 {
            self.set_title_top(self.battle_mode, false, false);
        }
    }

    /// `setTitleTop(a1, a2, a3)`.
    pub fn set_title_top(&mut self, a1: i32, a2: bool, a3: bool) {
        if !(TITLE_TOP_COMMAND_BLUE..TITLE_TOP_MAX).contains(&a1) {
            return;
        }
        let l5 = ["cmdBlue", "cmdYellow", "cmdRed", "cmdScut", "cmdBtn", "cmdSlock"][a1 as usize];
        self.movie.goto_and_play(&format!("{CMD_TITLE}.cmdTitleName"), l5);
        if self.movie.exists(&format!("{CMD_TITLE}.cmdTitleName2")) {
            self.movie.goto_and_play(&format!("{CMD_TITLE}.cmdTitleName2"), l5);
        }
        if a2 {
            self.movie.goto_and_play(CMD_TITLE_TOP, "loop");
        }
        if !self.simple_hud && a3 && self.command_title {
            self.movie.goto_and_play(CMD_TITLE, "change");
        }
    }

    // ---- the native side's direct handles (spec 5.1: m_pGFx...; gotoAndPlay(label) 0x1438F9470,
    // gotoAndStop(frame) 0x1438F9810, the bitmap-number setter 0x1415499C0)

    /// `m_pGFxMainCmdList[slot]` (the slot's text clip `cmdText1`): `on` / `off` / `disabled`.
    pub fn native_main_text(&mut self, slot: i32, label: &str) {
        self.movie.goto_and_play(&format!("{}.cmdText1", main_btn(slot)), label);
    }

    /// `m_pGFxSubCmdText[row]` of the open first-level list: `on` / `off` / `disabled`.
    pub fn native_sub_text(&mut self, list: i32, row: usize, label: &str) {
        let clip = format!("{}.cmdText1", self.sub_btn(list.clamp(1, 3) as usize, row));
        self.movie.goto_and_play(&clip, label);
    }

    /// `m_pGFxSubCmdName[row]`: `col1`, or `col2` when the command costs more MP than he has.
    pub fn native_sub_name(&mut self, list: i32, row: usize, label: &str) {
        let clip = format!("{}.cmdText1.txCmdName", self.sub_btn(list.clamp(1, 3) as usize, row));
        self.movie.goto_and_play(&clip, label);
    }

    /// `m_pGFxShortcut[row]` (`cmdExText1`): `on` / `disabled`.
    pub fn native_shortcut_text(&mut self, row: usize, label: &str) {
        self.movie.goto_and_play(&format!("{}.cmdExText1", ex_set(CMD_SCUT_SHORT, row)), label);
    }

    /// `m_pGFxShortcutName[row]`: `col1` / `col2`.
    pub fn native_shortcut_name(&mut self, row: usize, label: &str) {
        self.movie.goto_and_play(&format!("{}.cmdExText1.txCmdName", ex_set(CMD_SCUT_SHORT, row)), label);
    }

    /// `m_pGFxActionCmdText[button]`: `on` / `off` / `disabled`.
    pub fn native_action_text(&mut self, button: i32, label: &str) {
        self.movie.goto_and_play(&format!("{}.cmdAct.cmdAct1.cmdText1", act_btn(button.max(0) as usize)), label);
    }

    /// The per-frame display of a timed situation command (`0x141630ED0`): seconds and tenths
    /// into `m_pGFxActionCmdTime / Time2`, the counter's colour label (`loop`, `red` below
    /// 10 s) when it changes, the gauge clip's frame.
    pub fn native_action_timer(&mut self, button: i32, seconds: i32, tenths: i32, count_label: &str, gauge_frame: i32) {
        let l9 = format!("{}.cmdAct.cmdAct1", act_btn(button.max(0) as usize));
        let count = format!("{l9}.actCount");
        if !self.label_is(&count, count_label) {
            self.movie.goto_and_play(&count, count_label);
            self.configure_counter(button.max(0) as usize);
        }
        if let Some(s) = self.movie.sprite_mut(&format!("{count}.actCountS")) {
            controls::number_set(s, seconds);
        }
        if let Some(s) = self.movie.sprite_mut(&format!("{count}.actCountSs")) {
            controls::number_set(s, tenths);
        }
        self.movie.goto_frame(&format!("{l9}.actGgAnm"), gauge_frame.max(1) as u32);
    }

    /// `m_pGFxActLogoGg[0]` (the form gauge clip's frame) and `[1]` (`loop` / `alert`).
    pub fn native_form_gauge(&mut self, frame: i32, label: &str) {
        self.update_style(frame);
        let clip = format!("{CMD_ACT_LOGO_GG_SHORT}.cmdActLogoGgAnm.actLogoGgWaku");
        if !self.label_is(&clip, label) {
            self.movie.goto_and_play(&clip, label);
        }
    }

    /// Root label: `off` (hidden) / `loop` (shown), as the cockpit's hide / show does.
    pub fn native_root(&mut self, label: &str) {
        self.movie.goto_and_play("", label);
    }

    pub fn main_cursor(&self) -> i32 {
        self.main_btn_index
    }

    pub fn level(&self) -> usize {
        self.level
    }
}
