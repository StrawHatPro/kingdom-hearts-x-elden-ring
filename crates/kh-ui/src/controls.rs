//! Ports of the shared AS3 helper classes of the HUD movies (pseudo-AS3 in
//! `build/sora_pack/ui/as3/*.as3.txt`): `tres.utils.FollowGauge`, `tres.controls.TextFieldAdjust`,
//! `tres.controls.BitmapNumber`. Class and method are named at each function.

use crate::def::Align;
use crate::player::{GfxRect, Kind, NumberProps, Sprite};
use crate::render::Fonts;

/// `tres.utils.FollowGauge`: the red lag gauge. `DELAY_TIME` 800 ms, `GAUGE_TIMER_CALL` 33.3 ms,
/// `GAUGE_SPEED_BASE` 10. The AS3 object owns a `flash.utils.Timer` of 33.3 ms; here the owner
/// feeds elapsed time to [`FollowGauge::advance`] and applies the frames it returns to the gauge
/// clip (`m_gauge.gotoAndStop`).
#[derive(Clone, Debug, PartialEq)]
pub struct FollowGauge {
    /// `m_timer.running`.
    pub running: bool,
    /// Milliseconds since the timer's last tick.
    timer_ms: f64,
    pub delay: f64,
    pub current: f64,
    pub target: i32,
    pub add: f64,
    pub scale: f64,
    /// `m_isBegunFollow`.
    pub begun: bool,
}

impl Default for FollowGauge {
    fn default() -> Self {
        FollowGauge { running: false, timer_ms: 0.0, delay: 0.0, current: 0.0, target: 0, add: 0.0, scale: 1.0, begun: false }
    }
}

impl FollowGauge {
    pub const DELAY_TIME: f64 = 800.0;
    pub const GAUGE_TIMER_CALL: f64 = 33.3;
    pub const GAUGE_SPEED_BASE: f64 = 10.0;

    pub fn with_scale(scale: f64) -> FollowGauge {
        FollowGauge { scale, ..FollowGauge::default() }
    }

    /// `calcSpeed(a1)`: `a1 * m_scale / 10`, never 0 (at least 0.1 in the direction of `a1`).
    pub fn calc_speed(&self, a1: i32) -> f64 {
        let l2 = a1 as f64 * self.scale / Self::GAUGE_SPEED_BASE;
        if l2 != 0.0 {
            l2
        } else if a1 < 0 {
            -0.1
        } else {
            0.1
        }
    }

    /// `setGauge(a1, a2)`: `a1` = where the red gauge starts, `a2` = where it follows to.
    /// Returns the frame to show at once (`m_gauge.gotoAndStop(a1)`), if any.
    pub fn set_gauge(&mut self, mut a1: i32, a2: i32) -> Option<u32> {
        let mut l3 = true;
        if self.running {
            let l4 = self.current as i32;
            if self.add > 0.0 {
                if a2 > l4 {
                    self.target = a2;
                    self.add = self.calc_speed(a2 - l4);
                    l3 = false;
                } else {
                    a1 = a2;
                    self.running = false;
                }
            } else if a2 < l4 {
                self.target = a2;
                self.add = self.calc_speed(a2 - l4);
                l3 = false;
            } else {
                a1 = a2;
                self.running = false;
            }
        }
        if !l3 {
            return None;
        }
        if a1 != a2 {
            self.delay = 0.0;
            self.begun = false;
            self.target = a2;
            self.current = a1 as f64;
            self.add = self.calc_speed(a2 - a1);
            self.running = true;
            self.timer_ms = 0.0;
        }
        Some(a1.max(1) as u32)
    }

    /// `onTimer`: one 33.3 ms tick. Returns the frame for the gauge clip when it moves.
    fn on_timer(&mut self) -> Option<u32> {
        self.delay += Self::GAUGE_TIMER_CALL;
        if self.delay < Self::DELAY_TIME {
            return None;
        }
        self.current += self.add;
        let mut l2 = self.current as i32;
        if self.add < 0.0 {
            if l2 < self.target {
                l2 = self.target;
            }
        } else if l2 > self.target {
            l2 = self.target;
        }
        if l2 == self.target {
            self.running = false;
        }
        self.begun = true;
        Some(l2.max(1) as u32)
    }

    /// Lets `dt_ms` of wall-clock time pass. Returns the last frame the timer sent to the gauge
    /// clip in that time (`None`: it did not move).
    pub fn advance(&mut self, dt_ms: f64) -> Option<u32> {
        if !self.running {
            return None;
        }
        self.timer_ms += dt_ms;
        let mut out = None;
        // 1.0e-6: a 60 fps host adds 16.666.. ms twice per 33.3 ms tick.
        while self.running && self.timer_ms + 1.0e-6 >= Self::GAUGE_TIMER_CALL {
            self.timer_ms -= Self::GAUGE_TIMER_CALL;
            if let Some(frame) = self.on_timer() {
                out = Some(frame);
            }
        }
        out
    }

    /// `reset()`: the gauge clip goes to frame 1, the timer stops.
    pub fn reset(&mut self) {
        self.running = false;
    }
}

/// `tres.core.System.getDisplayString`: `^...{text}` shows `text` with `<br>` as line breaks;
/// anything else is shown as it is.
pub fn display_string(a1: &str) -> String {
    if a1.starts_with('^') {
        if let (Some(l4), Some(l5)) = (a1.find('{'), a1.rfind('}')) {
            if l4 > 0 && l4 < l5 {
                return a1[l4 + 1..l5].replace("<br>", "\n");
            }
        }
    }
    a1.to_string()
}

/// `TextFieldAdjust.<init>`: remembers the text field's authored text and width.
pub fn text_adjust_init(s: &mut Sprite) {
    if let Some(Kind::Text(t)) = s.child("textField").map(|n| &n.kind) {
        let b = &t.def.bounds;
        s.text_adjust = Some((t.shown().to_string(), b.x1 - b.x0));
    }
}

/// `TextFieldAdjust.set text` + `updateText`: the text goes to the child `textField`; a text
/// wider than the field (`textWidth + 5 > m_originalTextFieldW`) is squeezed with
/// `scaleX = width / (textWidth + 5)`, but not below 0.8 (then it is left unscaled), and the
/// field is shifted by its alignment (centre -2.5, right -5).
pub fn text_adjust_set(s: &mut Sprite, text: &str, fonts: &Fonts) -> bool {
    let Some((current, original_w)) = s.text_adjust.clone() else { return false };
    if current == text {
        return true;
    }
    let shown = display_string(text);
    s.text_adjust = Some((shown.clone(), original_w));
    let Some(Kind::Text(t)) = s.child_mut("textField").map(|n| &mut n.kind) else { return false };
    let width = if crate::render::icon_name(&shown).is_some() {
        0.0
    } else {
        fonts.get(t.def.font_class.as_deref()).map_or(0.0, |font| shown.split('\n').map(|line| crate::render::markup_width(fonts, font, line, t.def.size)).fold(0.0, f32::max))
    };
    t.text = Some(shown);
    let l1 = width + 5.0;
    t.adjust = Some(if l1 > original_w {
        let mut scale_x = original_w / l1;
        if scale_x < 0.8 {
            scale_x = 1.0;
        }
        let x = match t.def.align {
            Align::Center => -2.5,
            Align::Right => -5.0,
            Align::Left => 0.0,
        };
        (scale_x, x, l1 + 5.0)
    } else {
        (1.0, 0.0, original_w)
    });
    true
}

/// `BitmapNumber.<init>`: `stop()`; the authoring placeholder inside the canvas (`num.anim.canvas`,
/// or `num` itself without an `anim` clip) is removed.
pub fn number_init(s: &mut Sprite) {
    let Some(num) = s.child_mut("num").and_then(|n| n.sprite_mut()) else { return };
    match num.child_mut("anim").and_then(|n| n.sprite_mut()) {
        Some(anim) => {
            if let Some(canvas) = anim.child_mut("canvas").and_then(|n| n.sprite_mut()) {
                canvas.remove_children();
            }
        }
        None => num.remove_children(),
    }
}

/// The component parameters a parent timeline's `__setProp_*` method assigns, then `number`.
pub fn number_configure(s: &mut Sprite, props: NumberProps) {
    if s.number.as_deref() == Some(&props) {
        return;
    }
    s.number = Some(Box::new(props));
    number_update(s);
}

/// `BitmapNumber.setNumber(a1, false)` (the `number` setter).
pub fn number_set(s: &mut Sprite, value: i32) {
    let Some(props) = &mut s.number else { return };
    let value = value.max(0);
    if props.number == value && !s_is_blank(s) {
        return;
    }
    s.number.as_mut().unwrap().number = value;
    number_update(s);
}

fn s_is_blank(s: &Sprite) -> bool {
    let Some(num) = s.child("num").and_then(|n| n.sprite()) else { return true };
    match num.child("anim").and_then(|n| n.sprite()) {
        Some(anim) => anim.child("canvas").and_then(|n| n.sprite()).is_none_or(|c| c.gfx.is_empty()),
        None => num.gfx.is_empty(),
    }
}

/// `BitmapNumber.updateDisplay`, the branch for `wholeAnim` = true without the extra-zero bitmap
/// (`bmpZeroStartX / Y` = -1) and `reverseDraw` = false: what every `BitmapNumber` of the HUD
/// movies is set to. The digits are drawn left to right into one canvas, most significant
/// first; `originAnim` places the canvas' origin, `align` shifts the whole number.
/// NOT PORTED: per-digit animation instances (`wholeAnim` false), `reverseDraw`, the extra-zero
/// bitmap (`allExtraZero`).
pub fn number_update(s: &mut Sprite) {
    let Some(p) = s.number.as_deref().cloned() else { return };
    let Some(fill) = s.movie.bitmaps.get(&p.bmp_name).cloned() else { return };
    let number = p.number.max(0);
    let display_num_digit = if number > 0 { (number as f64).log10().floor() as i32 + 1 } else { 1 };
    let num_digit = p.num_digit.max(1);
    // m_displayNums[i] = the digit of 10^i (the top one takes what does not fit).
    let mut digits = vec![0i32; num_digit as usize];
    let mut l2 = number;
    for l3 in (0..num_digit).rev() {
        let l11 = 10i32.pow(l3 as u32);
        digits[l3 as usize] = l2 / l11;
        l2 -= digits[l3 as usize] * l11;
    }
    let l4 = display_num_digit.min(num_digit);
    let l5 = p.bmp_width + p.kerning;
    let l6 = if p.zero_padding { num_digit } else { l4 };
    let l7 = (l6 - 1) * l5;
    let l8 = l7;
    let (w, h) = (p.bmp_width as f32, p.bmp_height as f32);
    let Some(num) = s.child_mut("num") else { return };
    // align
    num.matrix.tx = match p.align {
        "center" => -(l7 as f32) * 0.5,
        "right" => -(l7 as f32),
        _ => 0.0,
    };
    num.script_matrix = true;
    let Some(num) = num.sprite_mut() else { return };
    let has_anim = num.child("anim").is_some();
    let (mut l9, mut l10) = (0i32, 0i32);
    if has_anim {
        let half_w = (-(w + l8 as f32) * 0.5) as i32;
        let full_w = -(p.bmp_width + l8);
        let half_h = (-h * 0.5) as i32;
        (l9, l10) = match p.origin_anim {
            "center" => (half_w, half_h),
            "left-up" => (0, 0),
            "up" => (half_w, 0),
            "right-up" => (full_w, 0),
            "left" => (0, half_h),
            "right" => (full_w, half_h),
            "left-down" => (0, -p.bmp_height),
            "down" => (half_w, -p.bmp_height),
            "right-down" => (full_w, -p.bmp_height),
            _ => (0, 0),
        };
    }
    let mut rects = Vec::new();
    let mut draw_num = |digit: i32, a3: i32, a4: i32| {
        // drawNum: the bitmap is translated so that the digit's cell lands on the rectangle.
        let mut f = fill.clone();
        f.matrix = crate::geom::Matrix::translate((-p.bmp_start_x - p.bmp_width * digit + a3) as f32, (-p.bmp_start_y + a4) as f32).mul(&fill.matrix);
        f.repeat = true;
        f.smooth = true;
        rects.push(GfxRect { x: a3 as f32, y: a4 as f32, w, h, fill: f });
    };
    if p.whole_anim {
        let mut x = 0;
        if p.zero_padding {
            for _ in l4..num_digit {
                draw_num(0, x + l9, l10);
                x += l5;
            }
        }
        for l3 in (0..l4).rev() {
            draw_num(digits[l3 as usize], x + l9, l10);
            x += l5;
        }
    } else {
        // One animation instance per digit: digit i (of 10^i) stands at -l9 + l8 - i * l5 with
        // its cell drawn at (l9, l10); here all of them go into the one canvas, which stays at
        // the origin (the per-digit instances and their own animations are not made).
        let (c9, c10) = match p.origin_anim {
            "center" => ((-w * 0.5) as i32, (-h * 0.5) as i32),
            "up" => ((-w * 0.5) as i32, 0),
            "right-up" => (-p.bmp_width, 0),
            "left" => (0, (-h * 0.5) as i32),
            "right" => (-p.bmp_width, (-h * 0.5) as i32),
            "left-down" => (0, -p.bmp_height),
            "down" => ((-w * 0.5) as i32, -p.bmp_height),
            "right-down" => (-p.bmp_width, -p.bmp_height),
            _ => (0, 0),
        };
        let _ = (c9, c10);
        (l9, l10) = (0, 0);
        let mut x = l8;
        for l3 in 0..l4 {
            draw_num(digits[l3 as usize], x, 0);
            x -= l5;
        }
        if p.zero_padding {
            for _ in l4..num_digit {
                draw_num(0, x, 0);
                x -= l5;
            }
        }
    }
    let canvas = if has_anim {
        let Some(anim) = num.child_mut("anim") else { return };
        anim.matrix.tx = -l9 as f32;
        anim.matrix.ty = -l10 as f32;
        anim.script_matrix = true;
        anim.sprite_mut().and_then(|a| a.child_mut("canvas")).and_then(|c| c.sprite_mut())
    } else {
        Some(num)
    };
    if let Some(canvas) = canvas {
        canvas.gfx = rects;
    }
}

/// (loot) `tres.controls.Icon` (the item picture of the "obtained" popup and of the menus): what its constructor keeps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IconProps {
    /// `m_defaultSize` = (width / scaleX, height / scaleY): the placeholder's size in the clip's own units.
    pub size: (f32, f32),
    /// `m_clipRect`: the child `rect`'s (x, y, width, height), when the clip has one.
    pub clip: Option<(f32, f32, f32, f32)>,
}

/// (loot) `Icon.<init>()` [the movie's script, method 320]: `m_defaultSize` from the clip's own bounds, the clip
/// rectangle from a child named `rect`, then `removeChildren()` (the authoring placeholder goes).
pub fn icon_init(s: &mut Sprite) {
    let bounds = crate::list::local_bounds(s);
    let clip = s.child("rect").map(|node| {
        let b = match &node.kind {
            crate::player::Kind::Sprite(child) => crate::list::local_bounds(child),
            crate::player::Kind::Shape(shape) => shape.bounds.rect(),
            _ => crate::geom::Rect::EMPTY,
        }
        .transform(&node.matrix);
        (b.x0, b.y0, b.x1 - b.x0, b.y1 - b.y0)
    });
    s.icon = Some(IconProps { size: ((bounds.x1 - bounds.x0).max(0.0), (bounds.y1 - bounds.y0).max(0.0)), clip });
    s.remove_children();
}

/// (loot) `Icon.load(path)` once the picture is there [`onLoaderLoadComplete`, method 324]:
/// `graphics.clear(); beginBitmapFill(bitmap, scale(defaultSize / bitmap size), false, true); drawRect(clip rect, or
/// 0, 0, defaultSize)`. `None` = `unload()` (nothing is drawn). False: the sprite is no `Icon`.
pub fn icon_load(s: &mut Sprite, texture: Option<std::sync::Arc<crate::def::Texture>>) -> bool {
    let Some(props) = s.icon else { return false };
    s.gfx.clear();
    let Some(texture) = texture else { return true };
    if texture.width == 0 || texture.height == 0 || props.size.0 <= 0.0 || props.size.1 <= 0.0 {
        return true;
    }
    let matrix = crate::geom::Matrix::scale(props.size.0 / texture.width as f32, props.size.1 / texture.height as f32);
    let (x, y, w, h) = props.clip.unwrap_or((0.0, 0.0, props.size.0, props.size.1));
    s.gfx.push(GfxRect { x, y, w, h, fill: crate::def::BitmapFill { texture, matrix, scale: (1.0, 1.0), repeat: false, smooth: true } });
    true
}

/// (IMPL_ER23B) `tres.controls.ImageIcon`: the component parameters (`bmpName`, `bmpWidth`, `bmpHeight`, `row`,
/// `column`) and `index`, then `draw()`: `graphics.beginBitmapFill(bitmap, translate(-(index % column) * w,
/// -(index / column) * h), true, true); drawRect(0, 0, w, h)`. `row` only gates the drawing (0 = nothing).
/// False: the sprite is no `ImageIcon` or its movie has no bitmap of that name (nothing is drawn).
pub fn image_icon_set(s: &mut Sprite, bmp_name: &str, bmp_width: f32, bmp_height: f32, row: i32, column: i32, index: i32) -> bool {
    if s.def.control != crate::def::Control::ImageIcon {
        return false;
    }
    s.gfx.clear();
    if row == 0 || column == 0 {
        return true;
    }
    let Some(fill) = s.movie.bitmaps.get(bmp_name).cloned() else { return false };
    let (l1, l2) = (index % column, index / column);
    let mut f = fill.clone();
    f.matrix = crate::geom::Matrix::translate(-(l1 as f32) * bmp_width, -(l2 as f32) * bmp_height).mul(&fill.matrix);
    f.repeat = true;
    f.smooth = true;
    s.gfx.push(GfxRect { x: 0.0, y: 0.0, w: bmp_width, h: bmp_height, fill: f });
    true
}
