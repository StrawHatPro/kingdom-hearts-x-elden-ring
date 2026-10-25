//! `HudTarget`: the world-anchored layer (docs/sora-ui-spec.md section 7): target cursors, the
//! shotlock scope and lock counter, floating EXP points, the style-gauge-full notice.
//!
//! - [`HudTarget`]: the AS3 document class (`build/sora_pack/ui/as3/HudTarget.as3.txt`, class
//!   `HudTarget`, methods 759 - 779), ported method by method;
//! - [`TargetDriver`]: the native part `UTresUIP_HudTarget` (cursor add `0x141668C60`, delete
//!   `0x14166C730`, projection `0x141677E90` / `0x14167E1D0`, update `0x14167CEF0`, scope
//!   `0x14167A8F0`, lock count `0x14167B630`, EXP points `0x141575840`).

use std::sync::Arc;

use glam::{Mat4, Vec3};

use crate::controls;
use crate::def::MovieDef;
use crate::frames::flow_scope_frame;
use crate::player::{Movie, NumberProps};
use crate::render::Fonts;
use crate::UiSound;

pub const LOCKON_TYPE_AUTO: usize = 0;
pub const LOCKON_TYPE_NORMAL: usize = 1;
pub const LOCKON_TYPE_FLOWSHOTATTACK: usize = 2;
pub const LOCKON_TYPE_FLOWSHOTFOCUSCHARGE: usize = 3;
pub const LOCKON_TYPE_FLOWSHOTMOVE: usize = 4;
pub const LOCKON_TYPE_ACTION: usize = 5;
pub const LOCKON_TYPE_ATHLETIC: usize = 6;
pub const LOCKON_TYPE_ATTRACTION: usize = 7;
pub const LOCKON_TYPE_MAX: usize = 8;
/// `m_nLockonCursorTypeMax` (type 3 shares type 2's clips).
pub const LOCKON_POOL: [usize; LOCKON_TYPE_MAX] = [10, 2, 32, 32, 10, 2, 2, 6];
const LOCKON_CLASS: [&str; LOCKON_TYPE_MAX] = ["ALockonMark", "MLockonMark", "FlowMarkAtk", "FlowMarkAtk", "FlowMarkMov", "ActLockonMark", "AthleticMark", "AttractionMark"];
pub const EXP_POINT_MAX: usize = 10;
const MAX_DIGIT_EXP: usize = 5;
const LABELS_EXP_POINT: [&str; 6] = ["keta1", "keta1", "keta2", "keta3", "keta4", "keta5"];

pub const SFSS_IN: i32 = 0;
pub const SFSS_OUT: i32 = 1;
pub const SFSS_OUT_SHOOT_FLOW0: i32 = 2;
pub const SFSS_OUT_ATHLETIC_FLOW0: i32 = 4;
pub const SFSS_OUT_ATHLETIC_FLOW1: i32 = 5;
pub const SFSS_OFF: i32 = 6;
pub const SFSS_GAUGE_EMPTY: i32 = 11;
pub const SHOOT_FLOW_GAUGE_ANIM_FRAME: u32 = 317;

pub const SCOPE_IN: i32 = 0;
pub const SCOPE_OUT: i32 = 1;
pub const SCOPE_OFF: i32 = 2;
pub const SCOPE_FULL_CHARGE: i32 = 3;
pub const SCOPE_CHARGING: i32 = 4;

const PLANE: &str = "planeParent.lockonMarkPlane";
const EXP_PLANE: &str = "planeParent.poExpPntPlane";

/// (IMPL_ER23B) THE UNIT AFTER AN EXP POPUP'S NUMBER. The movie's field (`expPntTxt.textField`, authoring text "t") is
/// localised by its link `^kh3_system.cockpit.pop_0010`: "p" in the English and Japanese tables, "Pt." in the German
/// one: the game's popup reads "8 P" (points) [read from the assets]. `None` = that. A host may put another text
/// there (the user asked for "EXP": test app KH_UI_EXP_UNIT=EXP, Elden Ring `hud.exp_unit = EXP`).
static EXP_UNIT: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

pub fn set_exp_unit(text: Option<&str>) {
    if let Ok(mut guard) = EXP_UNIT.lock() {
        *guard = text.map(str::to_string);
    }
}

fn exp_unit() -> Option<String> {
    EXP_UNIT.lock().ok().and_then(|g| g.clone()).or_else(|| std::env::var("KH_UI_EXP_UNIT").ok()).filter(|s| !s.is_empty())
}

pub struct HudTarget {
    pub movie: Movie,
    pub fonts: Fonts,
    exp_index: usize,
    /// `m_mcCurrentFlowScope`: `flowScope` or `flowScopeC`.
    current_flow_scope: Option<&'static str>,
    add_input_gauge_result_now: i32,
    is_full_charge: bool,
    is_open_shooter_scope: bool,
    is_open_lockon_count: bool,
    pub calls: Vec<String>,
    callbacks: Vec<(i32, i32)>,
}

fn cursor_name(ty: usize, index: usize) -> String {
    // Types 2 and 3 share their clips.
    format!("cursor{}_{index}", if ty == LOCKON_TYPE_FLOWSHOTFOCUSCHARGE { LOCKON_TYPE_FLOWSHOTATTACK } else { ty })
}

/// (DON-UI) The clip path of a target cursor (Donald's status icons follow the lock-on cursor: kh_ui::hud_arcane).
pub fn cursor_path(ty: usize, index: usize) -> String {
    format!("{PLANE}.{}", cursor_name(ty, index))
}

impl HudTarget {
    /// The constructor of the document class.
    pub fn new(def: Arc<MovieDef>, fonts: Fonts) -> HudTarget {
        let mut hud = HudTarget {
            movie: Movie::new(def),
            fonts,
            exp_index: 0,
            current_flow_scope: None,
            add_input_gauge_result_now: 0,
            is_full_charge: false,
            is_open_shooter_scope: false,
            is_open_lockon_count: false,
            calls: Vec::new(),
            callbacks: Vec::new(),
        };
        // m_lockonNumber = flowScope.flowLock.sFLockCount.coFSCount with the component parameters
        // of McFSCount_22.__setProp_coFSCount_*: bitmap ShootLockCount 64 x 48, 2 digits,
        // kerning -12, aligned right, origin left-up, one animation per digit.
        if let Some(s) = hud.movie.sprite_mut("flowScope.flowLock.sFLockCount.coFSCount") {
            controls::number_configure(s, NumberProps { bmp_name: "ShootLockCount".into(), bmp_width: 64, bmp_height: 48, bmp_start_x: 0, bmp_start_y: 0, num_digit: 2, zero_padding: false, kerning: -12, align: "right", origin_anim: "left-up", whole_anim: false, number: 28 });
        }
        hud.movie.stop("flowScope.fSGage");
        hud.movie.stop("flowScopeC.fSGage");
        // The ten EXP point clips are on their plane from the start.
        for l1 in 0..EXP_POINT_MAX {
            hud.movie.add_instance(EXP_PLANE, "ExpPnt", &format!("exp{l1}"));
        }
        hud
    }

    fn log(&mut self, call: String) {
        if self.calls.len() >= 256 {
            self.calls.remove(0);
        }
        self.calls.push(call);
    }

    /// One frame of the stage, then the `AnimEvent.END` listener (`onAnimEnd`).
    pub fn tick(&mut self) {
        self.movie.tick();
        for event in self.movie.take_events() {
            if event.name == "ANIM_END_ATTRACTION_MARKER_OUT" {
                // Callback(CALLBACK_ID_ANIM_END_ATTRACTION_MARKER_OUT, target.tabIndex)
                let index = event.path.rsplit('_').next().and_then(|i| i.parse().ok()).unwrap_or(0);
                self.callbacks.push((1, index));
            }
        }
    }

    pub fn take_callbacks(&mut self) -> Vec<(i32, i32)> {
        std::mem::take(&mut self.callbacks)
    }

    /// `setFlowScope(a1, a2, a3)`: state, focus-syphon scope, no aim time.
    pub fn set_flow_scope(&mut self, a1: i32, a2: bool, a3: bool) {
        self.log(format!("setFlowScope({a1}, {a2}, {a3})"));
        match a1 {
            SFSS_IN => {
                let scope = if a2 { "flowScopeC" } else { "flowScope" };
                self.current_flow_scope = Some(scope);
                self.movie.goto_and_play(scope, "in");
                if !a2 && !a3 {
                    for clip in ["fSFilterCol", "fSFilterInCol", "fSFilLineCol"] {
                        self.movie.goto_and_play(&format!("{scope}.{clip}"), "nomal");
                    }
                }
                if a3 {
                    self.movie.goto_frame(&format!("{scope}.fSGage"), SHOOT_FLOW_GAUGE_ANIM_FRAME);
                    self.movie.goto_and_play(&format!("{scope}.fSBg1"), "loop3");
                    for clip in ["fSFilterCol", "fSFilterInCol", "fSFilLineCol"] {
                        self.movie.goto_and_play(&format!("{scope}.{clip}"), "empty");
                    }
                } else {
                    self.movie.goto_frame(&format!("{scope}.fSGage"), 1);
                }
            }
            SFSS_OUT | SFSS_OUT_SHOOT_FLOW0 | SFSS_OUT_ATHLETIC_FLOW1 | SFSS_OFF => {
                if let Some(scope) = self.current_flow_scope.take() {
                    let label = match a1 {
                        SFSS_OUT => "out",
                        SFSS_OUT_SHOOT_FLOW0 => "select",
                        SFSS_OUT_ATHLETIC_FLOW1 => "out2",
                        _ => "off",
                    };
                    self.movie.goto_and_play(scope, label);
                }
            }
            SFSS_OUT_ATHLETIC_FLOW0 => {
                // The scope stays the current one.
                if let Some(scope) = self.current_flow_scope {
                    self.movie.goto_and_play(scope, "select2");
                }
            }
            SFSS_GAUGE_EMPTY => {
                if let (false, Some(scope)) = (a2, self.current_flow_scope) {
                    self.movie.goto_and_play(&format!("{scope}.fSFilLineCol"), "empty");
                }
            }
            _ => {}
        }
    }

    /// `openButtonNotice(a1, a2)`.
    pub fn open_button_notice(&mut self, a1: &str, a2: &str) {
        self.log(format!("openButtonNotice({a1:?}, {a2:?})"));
        let notice = "buttonNoticeSet.buttonNotice";
        self.movie.set_text(&format!("{notice}.txPadBtn.txPadBtn"), a1, &self.fonts);
        self.movie.set_text(&format!("{notice}.attack.txButtonNotice.coButtonNoticeTx"), a2, &self.fonts);
        self.movie.set_text(&format!("{notice}.attack.txButtonNotice2.coButtonNoticeTx"), a2, &self.fonts);
    }

    /// `selectButtonNotice(a1)`: empty in the movie.
    pub fn select_button_notice(&mut self, _a1: bool) {}

    /// `closeButtonNotice()`: empty in the movie.
    pub fn close_button_notice(&mut self) {}

    /// `updateFlowScope(a1)`: the ring gauge's frame.
    pub fn update_flow_scope(&mut self, a1: i32) {
        if let Some(scope) = self.current_flow_scope {
            self.movie.goto_frame(&format!("{scope}.fSGage"), a1.max(1) as u32);
        }
    }

    /// `setShooterModeScope(a1, a2)` (the gummi / shooter scopes; not Sora's shotlock).
    pub fn set_shooter_mode_scope(&mut self, a1: i32, a2: i32) {
        self.log(format!("setShooterModeScope({a1}, {a2})"));
        let mut l3 = "shootScpB";
        if a2 == 0 {
            if a1 >= SCOPE_FULL_CHARGE {
                return;
            }
            l3 = "shootScpW";
        }
        match a1 {
            SCOPE_IN => {
                if !self.is_open_shooter_scope {
                    self.movie.goto_and_play(l3, "in");
                    self.is_full_charge = false;
                    self.is_open_shooter_scope = true;
                }
            }
            SCOPE_OUT => {
                if self.is_open_shooter_scope {
                    self.movie.goto_and_play(l3, if self.is_full_charge { "out_full" } else { "out" });
                    self.is_full_charge = false;
                    self.is_open_shooter_scope = false;
                }
            }
            SCOPE_OFF => {
                if self.is_open_shooter_scope {
                    self.movie.goto_and_play(l3, "off");
                    self.is_full_charge = false;
                    self.is_open_shooter_scope = false;
                }
            }
            SCOPE_FULL_CHARGE => {
                if self.is_open_shooter_scope {
                    self.movie.goto_and_play(l3, "change_full");
                    self.is_full_charge = true;
                }
            }
            SCOPE_CHARGING => {
                if self.is_open_shooter_scope {
                    self.movie.goto_and_play(l3, "change_nomal");
                    self.is_full_charge = false;
                }
            }
            _ => {}
        }
    }

    /// `setLockonCount(a1, a2)`: total locks, maximum.
    pub fn set_lockon_count(&mut self, a1: i32, a2: i32) {
        self.log(format!("setLockonCount({a1}, {a2})"));
        if !self.is_open_lockon_count {
            self.movie.goto_and_play("flowScope.flowLock", "in");
            self.is_open_lockon_count = true;
        }
        if let Some(s) = self.movie.sprite_mut("flowScope.flowLock.sFLockCount.coFSCount") {
            controls::number_set(s, a1);
        }
        if a1 == a2 {
            self.movie.goto_and_play("flowScope.flowLockMax", "max");
        }
    }

    /// `closeLockonCount()`.
    pub fn close_lockon_count(&mut self) {
        self.log("closeLockonCount()".into());
        self.movie.goto_and_play("flowScope.flowLock", "off");
        self.movie.goto_and_play("flowScope.flowLockMax", "off");
        self.is_open_lockon_count = false;
    }

    /// `addTargetCursor(a1, a2, a3, a4)`: pool index, type, x, y. False when refused.
    pub fn add_target_cursor(&mut self, a1: i32, a2: i32, a3: i32, a4: i32) -> bool {
        self.log(format!("addTargetCursor({a1}, {a2}, {a3}, {a4})"));
        if a1 < 0 || a2 < 0 || a2 as usize >= LOCKON_TYPE_MAX || a1 as usize >= LOCKON_POOL[a2 as usize] {
            return false;
        }
        let (index, ty) = (a1 as usize, a2 as usize);
        let name = cursor_name(ty, index);
        // lockonMarkPlane.addChild(l5): on top of the plane's children.
        if !self.movie.add_instance(PLANE, LOCKON_CLASS[ty], &name) {
            return false;
        }
        let path = format!("{PLANE}.{name}");
        if ty == LOCKON_TYPE_ATTRACTION {
            self.movie.stop(&format!("{path}.attractionMarkGg"));
        }
        if ty == LOCKON_TYPE_FLOWSHOTATTACK {
            self.movie.goto_and_play(&format!("{path}.flowMarkAtkSet"), "shoot");
        } else if ty == LOCKON_TYPE_FLOWSHOTFOCUSCHARGE {
            self.movie.goto_and_play(&format!("{path}.flowMarkAtkSet"), "aspil");
        }
        self.movie.goto_and_play(&path, "in");
        self.movie.set_position(&path, a3 as f32, a4 as f32, 0.0);
        true
    }

    /// `deleteTargetCursor(a1, a2)`: `removeChild`, no out animation.
    pub fn delete_target_cursor(&mut self, a1: i32, a2: i32) {
        self.log(format!("deleteTargetCursor({a1}, {a2})"));
        if a1 < 0 || a2 < 0 || a2 as usize >= LOCKON_TYPE_MAX {
            return;
        }
        self.movie.remove_instance(PLANE, &cursor_name(a2 as usize, a1 as usize));
    }

    /// `updateTargetCursor(a1, a2, a3, a4, a5, a6)`: index, type, x, y, rotation, visible.
    pub fn update_target_cursor(&mut self, a1: i32, a2: i32, a3: i32, a4: i32, a5: i32, a6: bool) {
        if a1 < 0 || a2 < 0 || a2 as usize >= LOCKON_TYPE_MAX {
            return;
        }
        let path = format!("{PLANE}.{}", cursor_name(a2 as usize, a1 as usize));
        if self.movie.exists(&path) {
            self.movie.set_position(&path, a3 as f32, a4 as f32, a5 as f32);
            self.movie.set_visible(&path, a6);
        }
    }

    /// `addExpPoint(a1)`: returns the clip index.
    pub fn add_exp_point(&mut self, a1: i32) -> usize {
        self.log(format!("addExpPoint({a1})"));
        self.exp_index += 1;
        let l2 = self.exp_index;
        if self.exp_index >= EXP_POINT_MAX {
            self.exp_index = 0;
        }
        // (The AS3 reads m_mcExpPointAry[l2] with l2 up to 10: the tenth call would index past
        // the vector; the clip index is taken modulo the pool here.)
        let l2 = l2 % EXP_POINT_MAX;
        let l3 = format!("{EXP_PLANE}.exp{l2}");
        let l4 = a1.to_string();
        self.movie.set_text(&format!("{l3}.expPntCount.expPntSuji.textField"), &l4, &self.fonts);
        self.movie.set_text(&format!("{l3}.expPntCount2.expPntSuji.textField"), &l4, &self.fonts);
        if let Some(unit) = exp_unit() {
            for count in ["expPntCount", "expPntCount2"] {
                let field = format!("{l3}.{count}.expPntTxt.textField");
                self.movie.set_text(&field, &unit, &self.fonts);
                // (the field is as wide as one letter and a half: a longer unit gets a field of four times the width)
                if let Some(node) = self.movie.node_mut(&field) {
                    let tx = node.matrix.tx;
                    if let crate::player::Kind::Text(t) = &mut node.kind {
                        let b = t.def.bounds;
                        t.adjust = Some((1.0, tx + b.x0, (b.x1 - b.x0) * 4.0));
                    }
                }
            }
        }
        let mut l5 = l4.len();
        if l5 > MAX_DIGIT_EXP {
            l5 = 0;
        }
        self.movie.goto_and_stop(&format!("{l3}.expPntCount"), LABELS_EXP_POINT[l5]);
        self.movie.goto_and_stop(&format!("{l3}.expPntCount2"), LABELS_EXP_POINT[l5]);
        self.movie.goto_and_play(&l3, "in");
        l2
    }

    /// `updateExpPoint(a1, a2, a3, a4)`: index, x, y, visible.
    pub fn update_exp_point(&mut self, a1: usize, a2: i32, a3: i32, a4: i32) {
        let path = format!("{EXP_PLANE}.exp{}", a1 % EXP_POINT_MAX);
        self.movie.set_position(&path, a2 as f32, a3 as f32, 0.0);
        self.movie.set_visible(&path, a4 != 0);
    }

    /// `setExtraInputIn(a1)` (shotlock extra input; 0 = Aqua's colours, else Riku's).
    pub fn set_extra_input_in(&mut self, a1: i32) {
        self.log(format!("setExtraInputIn({a1})"));
        self.movie.goto_and_play("addInput", "in");
        self.add_input_gauge_result_now = -1;
        self.set_extra_input_gauge(1, 0);
        self.set_extra_input_colour(if a1 != 0 { "riku" } else { "aqua" });
    }

    /// `setExtraInputSelect(a1)`: 3 = excellent.
    pub fn set_extra_input_select(&mut self, a1: i32) {
        self.log(format!("setExtraInputSelect({a1})"));
        if a1 == 3 {
            self.movie.goto_and_play("addInput.addInputLine", "excellent");
        }
        self.movie.goto_and_play("addInput", "select");
    }

    /// `setExtraInputGauge(a1, a2)`: gauge frame, result (2 good, 3 excellent).
    pub fn set_extra_input_gauge(&mut self, a1: i32, a2: i32) {
        self.movie.goto_frame("addInput.addInputLine.addInputGgAnm", a1.max(1) as u32);
        if self.add_input_gauge_result_now != a2 {
            self.add_input_gauge_result_now = a2;
            self.movie.goto_and_play("addInput.addInputLine", if a2 == 2 || a2 == 3 { "good" } else { "nomal" });
        }
    }

    /// `setStylePoinMax()`.
    pub fn set_style_poin_max(&mut self) {
        self.log("setStylePoinMax()".into());
        self.movie.goto_and_play("styleMaxInfo", "in");
    }

    /// `setExtraInputAqua()` / `setExtraInputRiku()`.
    pub fn set_extra_input_colour(&mut self, label: &str) {
        let line = "addInput.addInputLine";
        self.movie.goto_and_play(&format!("{line}.addInputGgAnm.addInputGgLCol"), label);
        self.movie.goto_and_play(&format!("{line}.addinputGg.addInputGgLoopCol"), label);
        self.movie.goto_and_play(&format!("{line}.effPrism"), label);
        self.movie.goto_and_play(&format!("{line}.addInputGg_M"), label);
    }

    /// `setupPushButtonMark(a1, a2, a3, a4)`: button text, label (None = no label), green,
    /// continuous.
    pub fn setup_push_button_mark(&mut self, a1: &str, a2: Option<&str>, a3: bool, a4: bool) {
        self.log(format!("setupPushButtonMark({a1:?}, {a2:?}, {a3}, {a4})"));
        let l5 = "pushBtnMark.pushBtnMarkSet";
        self.movie.set_text(&format!("{l5}.pushBtnAnm.txPadBtn.txPadBtn"), a1, &self.fonts);
        let text = a2.unwrap_or("");
        self.movie.set_text(&format!("{l5}.pushBtnTextSet.pushBtnTextCol.coPushBtnText"), text, &self.fonts);
        self.movie.set_text(&format!("{l5}.pushBtnTextSet.pushBtnTextCol2.coPushBtnText"), text, &self.fonts);
        let (colour, base) = if a3 { ("green", "green") } else { ("other", "yellow") };
        self.movie.goto_and_play(&format!("{l5}.pushBtnTextSet"), colour);
        self.movie.goto_and_play(&format!("{l5}.pushBtnEff1"), colour);
        self.movie.goto_and_play(&format!("{l5}.pushBtnBase"), base);
        if a2.is_none() {
            self.movie.goto_and_play(&format!("{l5}.pushBtnTextSet"), "off");
        }
        let anim = if a4 { "continue" } else { "loop" };
        self.movie.goto_and_play(&format!("{l5}.pushBtnAnm"), anim);
        self.movie.goto_and_play(&format!("{l5}.pushBtnEff2"), anim);
    }

    /// (integration pass) The native side of the push-button prompt, `0x1416793E0(part, mode,
    /// key, label, flag)` [decompiled C]: mode 0 calls `setupPushButtonMark(icon, label)` (two
    /// arguments: yellow, `loop`), makes the clip visible and plays it in; 1 hides it; 2 = used.
    /// The frame labels the native code plays are FNames that were not resolved: `in` / `out` /
    /// `select` of `pushBtnMarkSet` (its only labels besides `off` and `loop`) are taken.
    pub fn push_button_mark(&mut self, mode: i32, icon: &str, label: &str) {
        self.log(format!("pushButtonMark({mode}, {icon:?}, {label:?})"));
        let set = "pushBtnMark.pushBtnMarkSet";
        match mode {
            0 => {
                self.setup_push_button_mark(icon, Some(label), false, false);
                self.movie.set_visible("pushBtnMark", true);
                self.movie.goto_and_play(set, "in");
            }
            1 => drop(self.movie.goto_and_play(set, "out")),
            _ => drop(self.movie.goto_and_play(set, "select")),
        }
    }

    /// Root label, as the cockpit's hide / show sets the target plane.
    pub fn native_root(&mut self, label: &str) {
        self.movie.goto_and_play("", label);
    }
}

/// The view of the frame (`0x141677E90`): the view-projection matrix (column vectors: clip =
/// matrix * point, the same product as the game's row-vector `point * ViewProjection`) and the
/// view rectangle in pixels.
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub clip_from_world: Mat4,
    /// (x0, y0, x1, y1).
    pub rect: (f32, f32, f32, f32),
}

/// A stage position of a world point (`0x14167E1D0`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Projected {
    pub x: i32,
    pub y: i32,
    /// On screen (with the 100 px margin) and in front of the camera.
    pub on_screen: bool,
    pub depth: f32,
}

fn round_away(v: f32) -> i32 {
    // Round half away from zero.
    if v >= 0.0 {
        (v + 0.5) as i32
    } else {
        (v - 0.5) as i32
    }
}

impl View {
    /// The projection of 7.2: `offset` = the multi-mark offset in pixels (scaled with depth).
    pub fn project(&self, p: Vec3, offset: (f32, f32)) -> Projected {
        let (x0, y0, x1, y1) = self.rect;
        let c = self.clip_from_world * p.extend(1.0);
        let w = if c.w.abs() <= 1.0e-8 { 1.0e-6 } else { c.w };
        let sx = (c.x / w + 1.0) * (x1 - x0) / 2.0 + x0;
        let sy = (1.0 - c.y / w) * (y1 - y0) / 2.0 + y0;
        let depth = if c.w > 0.0 { c.z / w } else { 0.0 };
        let k = (depth * 50.0 + 0.25).min(2.0);
        let (mut x, mut y) = (sx - x0 + offset.0 * k, sy - y0 + offset.1 * k);
        let on_screen = -100.0 <= x && x < (x1 - x0) + 100.0 && -100.0 <= y && y < (y1 - y0) + 100.0 && depth > 0.0;
        if on_screen {
            x *= 1920.0 / (x1 - x0);
            y *= 1080.0 / (y1 - y0);
        }
        Projected { x: round_away(x), y: round_away(y), on_screen, depth }
    }
}

/// A target cursor the HUD shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cursor {
    /// The host's id of the thing it marks (a target component).
    pub key: u64,
    pub ty: usize,
    /// Pool index of its clip.
    pub index: usize,
    /// Multi-mark offset (pixels) and rotation (degrees).
    pub offset: (f32, f32),
    pub rotation: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ExpEntry {
    clip: usize,
    position: Vec3,
    life: f32,
}

/// Static SE of the target HUD (spec section 9).
pub const SE_LOCKON_ON: u32 = 7;
pub const SE_LOCKON_OFF: u32 = 8;
pub const SE_MARK: u32 = 19;
pub const SE_LOCK_MAX: u32 = 20;
pub const SE_SCOPE: u32 = 18;
pub const SE_SCOPE_EMPTY: u32 = 25;
pub const SE_STYLE_MAX: u32 = 26;
/// Seconds an EXP point entry lives (`0x141575840`; its clip animates 122 frames).
pub const EXP_POINT_LIFETIME: f32 = 3.0;

/// The native target HUD.
#[derive(Clone, Debug, Default)]
pub struct TargetDriver {
    pub cursors: Vec<Cursor>,
    exp: Vec<ExpEntry>,
    /// The shotlock aim: (aim time, remaining, syphon); `None` when closed.
    scope: Option<(f32, f32, bool)>,
    scope_empty_sent: bool,
    style_max_flag: bool,
    serial: u32,
    /// (integration pass) The push-button prompt on screen: the cursor key of the thing it is
    /// about (its clip is put at that thing's screen position), if any.
    push_mark: Option<Option<u64>>,
}

/// LOCAL: where the push-button prompt sits when the thing it is about is off screen or unknown
/// (stage pixels), and how far above the thing's lock-on point it is drawn. The native placement
/// of `pushBtnMark` was not read (the movie authors it at the stage origin, hidden).
pub const PUSH_MARK_FALLBACK: (f32, f32) = (960.0, 640.0);
pub const PUSH_MARK_LIFT: f32 = 110.0;

impl TargetDriver {
    /// `0x1415759C0` -> `0x141668C60`: adds a cursor of `ty` for the thing `key`. Refused (None)
    /// when the pool is full, or `key` already has a cursor of that type and `allow_multi` is
    /// false. Lock-on plays static SE 7, an airstep mark SE 19; the auto cursor is silent.
    pub fn add_cursor(&mut self, hud: &mut HudTarget, key: u64, ty: usize, allow_multi: bool, sounds: &mut Vec<UiSound>) -> Option<usize> {
        if ty >= LOCKON_TYPE_MAX {
            return None;
        }
        let pool = if ty == LOCKON_TYPE_FLOWSHOTFOCUSCHARGE { LOCKON_TYPE_FLOWSHOTATTACK } else { ty };
        let same = self.cursors.iter().filter(|c| c.key == key && c.ty == ty).count();
        if same > 0 && !allow_multi {
            return None;
        }
        let used = |i: usize| self.cursors.iter().any(|c| c.index == i && (if c.ty == LOCKON_TYPE_FLOWSHOTFOCUSCHARGE { LOCKON_TYPE_FLOWSHOTATTACK } else { c.ty }) == pool);
        let index = (0..LOCKON_POOL[ty]).find(|i| !used(*i))?;
        // Several marks on one target (types 2, 3): the n-th extra mark is offset by a radius
        // rand * 25 + 10 + 2 n at the angle ((serial & 3) + n) * 1.7 rad, rotated by -angle.
        let (mut offset, mut rotation) = ((0.0, 0.0), 0.0);
        if same > 0 {
            self.serial = self.serial.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let rand = ((self.serial >> 16) & 0x7FFF) as f32 / 32_768.0;
            let radius = rand * 25.0 + 10.0 + 2.0 * same as f32;
            let angle = ((self.serial & 3) as f32 + same as f32) * 1.7;
            offset = (radius * angle.cos(), radius * angle.sin());
            rotation = -angle.to_degrees();
        }
        if !hud.add_target_cursor(index as i32, ty as i32, 0, 0) {
            return None;
        }
        self.cursors.push(Cursor { key, ty, index, offset, rotation });
        match ty {
            LOCKON_TYPE_NORMAL => sounds.push(UiSound::Play(SE_LOCKON_ON)),
            LOCKON_TYPE_FLOWSHOTMOVE => sounds.push(UiSound::Play(SE_MARK)),
            _ => {}
        }
        Some(index)
    }

    /// `0x14157A7A0` -> `0x14166C730`: removes the cursors of `ty` on `key` (no out animation).
    /// Removing a lock-on cursor plays static SE 8.
    pub fn remove_cursor(&mut self, hud: &mut HudTarget, key: u64, ty: usize, sounds: &mut Vec<UiSound>) {
        let mut removed = false;
        self.cursors.retain(|c| {
            if c.key == key && c.ty == ty {
                hud.delete_target_cursor(c.index as i32, c.ty as i32);
                removed = true;
                false
            } else {
                true
            }
        });
        if removed && ty == LOCKON_TYPE_NORMAL {
            sounds.push(UiSound::Play(SE_LOCKON_OFF));
        }
    }

    pub fn has_cursor(&self, key: u64, ty: usize) -> bool {
        self.cursors.iter().any(|c| c.key == key && c.ty == ty)
    }

    /// Auto (0) or lock-on (1) cursors on screen: what keeps the enemy gauge shown.
    pub fn target_cursor_count(&self) -> usize {
        self.cursors.iter().filter(|c| c.ty <= LOCKON_TYPE_NORMAL).count()
    }

    /// `0x141575840`: a floating EXP number at a world position.
    pub fn add_exp_point(&mut self, hud: &mut HudTarget, position: Vec3, added: i32) {
        let clip = hud.add_exp_point(added);
        self.exp.retain(|e| e.clip != clip);
        self.exp.push(ExpEntry { clip, position, life: EXP_POINT_LIFETIME });
    }

    /// `0x141587480` -> `0x14167A8F0(target, 0, aimTime, syphon)`: the shotlock aim opens.
    /// Static SE 18 (25 for focus syphon or no aim time).
    pub fn open_scope(&mut self, hud: &mut HudTarget, aim_time: f32, syphon: bool, sounds: &mut Vec<UiSound>) {
        let empty = aim_time <= 0.0;
        hud.set_flow_scope(SFSS_IN, syphon, empty);
        sounds.push(UiSound::Play(if syphon || empty { SE_SCOPE_EMPTY } else { SE_SCOPE }));
        self.scope = Some((aim_time, aim_time, syphon));
        self.scope_empty_sent = empty;
    }

    /// `0x141577520(cockpit, state)`: released / cancelled (1 out, 2 select, 4 select2, 5 out2,
    /// 6 off). The lock counter leaves with the scope clip.
    pub fn close_scope(&mut self, hud: &mut HudTarget, state: i32) {
        if self.scope.take().is_some() || state == SFSS_OFF {
            hud.set_flow_scope(state, false, true);
            hud.is_open_lockon_count = false;
        }
    }

    pub fn scope_open(&self) -> bool {
        self.scope.is_some()
    }

    /// (integration pass) The controller's prompt call with mode 0 (`0x14158C260` /
    /// `0x14158B120` -> `0x1416793E0`): shows the push-button prompt with the pad icon markup
    /// `icon` and the label; `key` = the thing it is about. A prompt already up is replaced.
    pub fn show_push_mark(&mut self, hud: &mut HudTarget, icon: &str, label: &str, key: Option<u64>) {
        hud.push_button_mark(0, icon, label);
        hud.movie.set_position("pushBtnMark", PUSH_MARK_FALLBACK.0, PUSH_MARK_FALLBACK.1, 0.0);
        self.push_mark = Some(key);
    }

    /// The prompt call with mode 1 (hide) or 3 / 2 (used).
    pub fn hide_push_mark(&mut self, hud: &mut HudTarget, used: bool) {
        if self.push_mark.take().is_some() {
            hud.push_button_mark(if used { 2 } else { 1 }, "", "");
        }
    }

    pub fn push_mark_shown(&self) -> bool {
        self.push_mark.is_some()
    }

    /// `0x1415895F0` -> `0x14167B630`: the lock counter. Static SE 19 per lock, 20 on the lock
    /// that reaches the maximum.
    pub fn set_lock_count(&mut self, hud: &mut HudTarget, total: i32, max: i32, sounds: &mut Vec<UiSound>) {
        hud.set_lockon_count(total, max);
        sounds.push(UiSound::Play(if total == max { SE_LOCK_MAX } else { SE_MARK }));
    }

    /// `0x14158C410`: the form-change point ratio; at 1.0 the next update shows the notice.
    pub fn set_style_ratio(&mut self, ratio: f32) {
        if ratio >= 1.0 {
            self.style_max_flag = true;
        }
    }

    /// The update (`0x14167CEF0`, undilated frame time) and the projection pass (`0x141677E90`,
    /// with this frame's view). `position_of` gives a cursor key's lock-on point in the view's
    /// world space, or `None` when the thing is gone (its cursors are then removed).
    pub fn update(&mut self, hud: &mut HudTarget, dt: f32, view: Option<&View>, position_of: &dyn Fn(u64) -> Option<Vec3>, sounds: &mut Vec<UiSound>) {
        if std::mem::take(&mut self.style_max_flag) {
            sounds.push(UiSound::Play(SE_STYLE_MAX));
            hud.set_style_poin_max();
        }
        if let Some((aim_time, remaining, syphon)) = &mut self.scope {
            if *aim_time > 0.0 {
                *remaining = (*remaining - dt).max(0.0);
                hud.update_flow_scope(flow_scope_frame(*remaining, *aim_time));
                if *remaining <= 0.0 && !self.scope_empty_sent {
                    self.scope_empty_sent = true;
                    hud.set_flow_scope(SFSS_GAUGE_EMPTY, *syphon, false);
                }
            }
        }
        for e in &mut self.exp {
            e.life -= dt;
        }
        self.exp.retain(|e| e.life > 0.0);
        // A cursor whose component died is removed by the projection pass.
        let gone: Vec<Cursor> = self.cursors.iter().filter(|c| position_of(c.key).is_none()).copied().collect();
        for c in gone {
            hud.delete_target_cursor(c.index as i32, c.ty as i32);
            self.cursors.retain(|x| *x != c);
        }
        let Some(view) = view else { return };
        // The push-button prompt follows the thing it is about (LOCAL placement, see the constants).
        if let Some(Some(key)) = self.push_mark {
            let q = position_of(key).map(|p| view.project(p, (0.0, 0.0))).filter(|q| q.on_screen);
            let (x, y) = q.map_or(PUSH_MARK_FALLBACK, |q| ((q.x as f32).clamp(160.0, 1760.0), (q.y as f32 - PUSH_MARK_LIFT).clamp(120.0, 900.0)));
            hud.movie.set_position("pushBtnMark", x, y, 0.0);
        }
        for c in &self.cursors {
            let Some(p) = position_of(c.key) else { continue };
            let q = view.project(p, c.offset);
            hud.update_target_cursor(c.index as i32, c.ty as i32, q.x, q.y, round_away(c.rotation), q.on_screen);
        }
        for e in &self.exp {
            let q = view.project(e.position, (0.0, 0.0));
            hud.update_exp_point(e.clip, q.x, q.y, q.on_screen as i32);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_rule_of_7_2() {
        // A perspective view looking down -Z from the origin (a Bevy-style reversed infinite
        // projection: clip z / w = near / distance).
        let proj = Mat4::perspective_infinite_reverse_rh(60f32.to_radians(), 16.0 / 9.0, 0.1);
        let view = View { clip_from_world: proj, rect: (0.0, 0.0, 1280.0, 720.0) };
        // A target at the view centre gives (960, 540).
        let c = view.project(Vec3::new(0.0, 0.0, -5.0), (0.0, 0.0));
        assert_eq!((c.x, c.y, c.on_screen), (960, 540, true));
        // 150 px outside the view edge: hidden, no clamping (x keeps its view pixels).
        let half_width = (60f32.to_radians() / 2.0).tan() * 16.0 / 9.0 * 5.0;
        let out = view.project(Vec3::new(half_width * (640.0 + 150.0) / 640.0, 0.0, -5.0), (0.0, 0.0));
        assert!(!out.on_screen && out.x > 1280, "{out:?}");
        // 50 px outside: still shown (the 100 px margin), scaled to the stage.
        let near = view.project(Vec3::new(half_width * (640.0 + 50.0) / 640.0, 0.0, -5.0), (0.0, 0.0));
        assert!(near.on_screen && near.x == ((1280.0 + 50.0) * 1.5_f32).round() as i32, "{near:?}");
        // Behind the camera: hidden.
        assert!(!view.project(Vec3::new(0.0, 0.0, 5.0), (0.0, 0.0)).on_screen);
        // Multi-mark offset scale k = min(2, depth * 50 + 0.25): 2 closer than about 2.9 m.
        let close = view.project(Vec3::new(0.0, 0.0, -2.0), (10.0, 0.0));
        assert_eq!(close.x, 960 + 30, "k = 2: 20 view px = 30 stage px");
        let far = view.project(Vec3::new(0.0, 0.0, -100.0), (10.0, 0.0));
        assert_eq!(far.x, 960 + (10.0 * (0.001 * 50.0 + 0.25) * 1.5_f32).round() as i32);
    }
}
