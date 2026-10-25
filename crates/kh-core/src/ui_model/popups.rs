//! Popups (docs/sora-ui-spec.md sections 6 - 8): what is raised (`UiPopup`, an event of the
//! step) and what is on screen (`PopupBoard`, with the spec's lifetimes). The lifetimes are clip
//! lengths in frames of the 60 fps stage (`sora-ui-params` rows `popup.*`).
//!
//! Order within one EXP award (UI spec section 8): EXP point, level-up popup, Next LV.

use crate::status::LevelUp;
use glam::Vec3;

/// `stage.fps`.
pub const STAGE_FPS: f32 = 60.0;
/// `popup.exp_point_lifetime` 3.0 s (the list entry; the clip itself animates 122 frames).
pub const EXP_POINT_LIFETIME: f32 = 3.0;
/// `popup.exp_point_frames` 122.
pub const EXP_POINT_FRAMES: f32 = 122.0;
/// `target.exp_points` 10: the pool of floating numbers.
pub const EXP_POINT_POOL: usize = 10;
/// `popup.level_up_frames` 272 (4.53 s).
pub const LEVEL_UP_FRAMES: f32 = 272.0;
/// `popup.next_lv_frames` 146 (2.43 s); the munny counter lasts the same.
pub const NEXT_LV_FRAMES: f32 = 146.0;
/// `popup.get_item_frames` 114 (1.9 s).
pub const GET_ITEM_FRAMES: f32 = 114.0;
/// `popup.munny_cap` 9999.
pub const MUNNY_CAP: i32 = 9999;

/// What a popup carries beyond its value.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum PopupDetail {
    #[default]
    None,
    /// The level-up panel's lines (`HudRightLevelUp.setParam`): the non-zero deltas are shown.
    LevelUp(LevelUp),
    /// (new game) The bonus panel's lines (`HudRightGetBonus.setParam`): up to two.
    Bonus([crate::status::BonusLine; 2]),
}

/// Something that pops up for a moment. (KH3's HUD has no damage numbers: the only floating
/// numbers are EXP points.)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiPopup {
    /// Which popup: the `UiPopup::KIND_*` constants.
    pub kind: u32,
    /// A name: the item's for `KIND_ITEM`, free text for the debug popup.
    pub text: &'static str,
    /// The number shown: EXP added, EXP to the next level, the new level, munny.
    pub value: f32,
    /// A world position it belongs to (an EXP point at a defeated enemy), cm; `None` = screen space.
    pub position: Option<Vec3>,
    pub detail: PopupDetail,
}

impl UiPopup {
    /// The test level's debug popup.
    pub const KIND_DEBUG: u32 = 0;
    /// A floating EXP number at a world position (`HudTarget.addExpPoint`, 0x141575840).
    pub const KIND_EXP_POINT: u32 = 1;
    /// The level-up panel (`HudRightLevelUp`, 0x141589330); `value` = the new level.
    pub const KIND_LEVEL_UP: u32 = 2;
    /// "Next LV" (`HudLeft.m_NextLv`, 0x141589570); `value` = EXP still needed.
    pub const KIND_NEXT_LV: u32 = 3;
    /// Munny picked up (0x141587530): a stub until prizes exist; `value` = the amount.
    pub const KIND_MUNNY: u32 = 4;
    /// Item obtained (0x141587510): a stub; `text` = the item.
    pub const KIND_ITEM: u32 = 5;
    /// (new game) The bonus panel (`HudRightGetBonus`, 0x1415874F0): `detail` = its lines.
    pub const KIND_BONUS: u32 = 6;

    pub const fn new(kind: u32, text: &'static str, value: f32, position: Option<Vec3>) -> UiPopup {
        UiPopup { kind, text, value, position, detail: PopupDetail::None }
    }

    /// A short word for logs: "debug", "exp-point", "level-up", "next-lv", "munny", "item".
    pub const fn kind_name(&self) -> &'static str {
        match self.kind {
            Self::KIND_EXP_POINT => "exp-point",
            Self::KIND_LEVEL_UP => "level-up",
            Self::KIND_NEXT_LV => "next-lv",
            Self::KIND_MUNNY => "munny",
            Self::KIND_ITEM => "item",
            Self::KIND_BONUS => "bonus",
            _ => "debug",
        }
    }
}

/// A floating EXP number on screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExpPoint {
    pub value: i32,
    pub position: Vec3,
    /// Seconds since it appeared (it is listed for 3.0 s; its clip runs 2.03 s).
    pub age: f32,
}

/// A screen-space popup on screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimedPopup {
    pub popup: UiPopup,
    pub age: f32,
    /// Seconds it stays.
    pub lifetime: f32,
}

/// What is on screen.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct PopupBoard {
    /// At most 10 (the pool): a further one is not shown.
    pub exp_points: Vec<ExpPoint>,
    /// One per character: a second level-up while it shows updates it in place (0x141659CD0).
    pub level_up: Option<TimedPopup>,
    pub next_lv: Option<TimedPopup>,
    /// The counter ACCUMULATES while visible and restarts otherwise; it shows at most 9999.
    pub munny: Option<TimedPopup>,
    /// One at a time; further items wait in `item_queue` and show as each closes.
    pub item: Option<TimedPopup>,
    pub item_queue: Vec<UiPopup>,
    /// The debug popup of the test level (2 s; not a game element).
    pub debug: Option<TimedPopup>,
    /// (new game) The bonus panel: one at a time (LOCAL: the game owns an array of five, whose
    /// stacking was not read); further ones wait in `bonus_queue`.
    pub bonus: Option<TimedPopup>,
    pub bonus_queue: Vec<UiPopup>,
}

impl PopupBoard {
    /// Takes a raised popup onto the board.
    pub fn raise(&mut self, popup: &UiPopup) {
        let timed = |frames: f32| Some(TimedPopup { popup: *popup, age: 0.0, lifetime: frames / STAGE_FPS });
        match popup.kind {
            UiPopup::KIND_EXP_POINT => {
                if let (Some(position), true) = (popup.position, self.exp_points.len() < EXP_POINT_POOL) {
                    self.exp_points.push(ExpPoint { value: popup.value as i32, position, age: 0.0 });
                }
            }
            UiPopup::KIND_LEVEL_UP => self.level_up = timed(LEVEL_UP_FRAMES),
            UiPopup::KIND_NEXT_LV => self.next_lv = timed(NEXT_LV_FRAMES),
            UiPopup::KIND_MUNNY => {
                let shown = self.munny.map_or(0.0, |m| m.popup.value);
                self.munny = Some(TimedPopup { popup: UiPopup { value: (shown + popup.value).min(MUNNY_CAP as f32), ..*popup }, age: 0.0, lifetime: NEXT_LV_FRAMES / STAGE_FPS });
            }
            UiPopup::KIND_ITEM => {
                if self.item.is_some() {
                    self.item_queue.push(*popup);
                } else {
                    self.item = timed(GET_ITEM_FRAMES);
                }
            }
            UiPopup::KIND_BONUS => {
                if self.bonus.is_some() {
                    self.bonus_queue.push(*popup);
                } else {
                    self.bonus = timed(LEVEL_UP_FRAMES);
                }
            }
            _ => self.debug = timed(120.0),
        }
    }

    /// Ages everything by `dt` and drops what has run out.
    pub fn step(&mut self, dt: f32) {
        for point in &mut self.exp_points {
            point.age += dt;
        }
        self.exp_points.retain(|point| point.age < EXP_POINT_LIFETIME - 1.0e-4);
        let expired = |slot: &mut Option<TimedPopup>| {
            if let Some(timed) = slot {
                timed.age += dt;
                if timed.age >= timed.lifetime - 1.0e-4 {
                    *slot = None;
                    return true;
                }
            }
            false
        };
        expired(&mut self.level_up);
        expired(&mut self.next_lv);
        expired(&mut self.munny);
        expired(&mut self.debug);
        if expired(&mut self.bonus) && !self.bonus_queue.is_empty() {
            let next = self.bonus_queue.remove(0);
            self.bonus = Some(TimedPopup { popup: next, age: 0.0, lifetime: LEVEL_UP_FRAMES / STAGE_FPS });
        }
        if expired(&mut self.item) && !self.item_queue.is_empty() {
            let next = self.item_queue.remove(0);
            self.item = Some(TimedPopup { popup: next, age: 0.0, lifetime: GET_ITEM_FRAMES / STAGE_FPS });
        }
    }

    /// True when nothing is on screen.
    pub fn is_empty(&self) -> bool {
        self.exp_points.is_empty() && self.level_up.is_none() && self.next_lv.is_none() && self.munny.is_none() && self.item.is_none() && self.debug.is_none() && self.bonus.is_none()
    }
}
