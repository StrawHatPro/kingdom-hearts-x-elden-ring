//! The frame formulas of the native HUD drivers (docs/sora-ui-spec.md sections 4 - 7).

/// `F(v, max, frames)` 0x141598CA0 [I]: 0 if `v < 1` or `max < 1`; `frames` if `v == max`; else
/// `trunc(v * frames / max + 0.5)` (float) clamped to `[1, frames - 1]`. THE CLIP RECEIVES
/// `F + 1` (frame 1 = empty).
pub fn gauge_frame(v: i32, max: i32, frames: i32) -> i32 {
    if v < 1 || max < 1 {
        return 0;
    }
    if v == max {
        return frames;
    }
    let r = (v as f32 * frames as f32 / max as f32 + 0.5) as i32;
    r.clamp(1, (frames - 1).max(1))
}

/// `Ff(v, max, frames)` 0x141598C50 [I]: 0 when `max <= 0`, `frames` when `v >= max`, else
/// `trunc(v * frames / max + 0.5)`; no clamp.
pub fn gauge_frame_f(v: f32, max: f32, frames: i32) -> i32 {
    if max <= 0.0 {
        return 0;
    }
    if v >= max {
        return frames;
    }
    (v * frames as f32 / max + 0.5) as i32
}

/// `hpMaxFrames = min(668, trunc(maxHP + maxHP))`.
pub fn hp_max_frames(max_hp: i32) -> i32 {
    ((max_hp.max(0) as f32 * 2.0) as i32).min(668)
}

/// `mpMaxFrames = min(500, trunc(maxMP * 1.8))`.
pub fn mp_max_frames(max_mp: i32) -> i32 {
    ((max_mp.max(0) as f32 * 1.8) as i32).min(500)
}

/// Pinch 0x1405AD600: `hp <= maxHP * 25 / 100`, integer arithmetic.
pub fn is_pinch(hp: i32, max_hp: i32) -> bool {
    hp <= max_hp * 25 / 100
}

/// Situation-command time gauge (`actGgAnm`, 219 frames): `219 - Ff(t / total, 1, 214)`.
pub fn situation_gauge_frame(remaining: f32, total: f32) -> i32 {
    219 - gauge_frame_f(if total > 0.0 { remaining / total } else { 0.0 }, 1.0, 214)
}

/// Form time gauge (201 frames): `201 - Ff(t / total, 1, 200)`.
pub fn form_gauge_frame(remaining: f32, total: f32) -> i32 {
    201 - gauge_frame_f(if total > 0.0 { remaining / total } else { 0.0 }, 1.0, 200)
}

/// Situation-command counter digits: seconds and tenths of `min(t, 99.9)`.
pub fn situation_digits(remaining: f32) -> (i32, i32) {
    let t = remaining.clamp(0.0, 99.9);
    let tenths = (t * 10.0) as i32;
    (tenths / 10, tenths % 10)
}

/// Shotlock scope ring (`fSGage`, 317 frames): `Ff(1 - remaining / aimTime, 1, 316) + 1`.
pub fn flow_scope_frame(remaining: f32, aim_time: f32) -> i32 {
    gauge_frame_f(if aim_time > 0.0 { 1.0 - remaining / aim_time } else { 1.0 }, 1.0, 316) + 1
}

/// Enemy gauge (section 6): one bar = `unit` HP (200; 2000 for large targets), shown HP capped
/// at `unit * 120`. Returns (stock = full bars under the top bar, top bar clip frame).
pub fn enemy_gauge(hp: i32, unit: i32) -> (i32, i32) {
    let hp = hp.clamp(0, unit * 120);
    let (mut stock, mut rest) = (hp / unit, hp % unit);
    if rest == 0 && stock > 0 {
        // A whole number of bars: the top bar is a full one.
        stock -= 1;
        rest = unit;
    }
    (stock, gauge_frame(rest, unit, 500) + 1)
}

/// The Scaleform default viewport 0x1438F8E90 (section 11a): the centred 16:9 rectangle
/// `(left, top, w, h)` of a `width` x `height` window the 1920 x 1080 stage is stretched over
/// (ExactFit). `fit` = the two "Display Area" factors (1.0 by default).
pub fn stage_viewport(width: f32, height: f32, fit: (f32, f32)) -> (f32, f32, f32, f32) {
    let mut left = ((width - width * fit.0) / 2.0).trunc();
    let mut top = ((height - height * fit.1) / 2.0).trunc();
    let mut w = width - 2.0 * left;
    let mut h = height - 2.0 * top;
    let aspect = width / height;
    if aspect > 1.7777 {
        let e = w - height * 1.7777;
        left += ((e + 1.0) / 2.0).trunc();
        w = (w - (e - 0.5)).trunc();
    }
    if aspect < 1.7777 {
        let e = h - width * 0.562_524_6;
        top += ((e + 1.0) / 2.0).trunc();
        h = (h - (e - 0.5)).trunc();
    }
    (left, top, w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_11b_tier1_gauge_frames() {
        // Max HP 250 / max MP 179 at level 99.
        let hp = hp_max_frames(250);
        assert_eq!(hp + 1, 501);
        assert_eq!(gauge_frame(250, 250, hp) + 1, 501);
        assert_eq!(gauge_frame(125, 250, hp) + 1, 251);
        assert_eq!(gauge_frame(1, 250, hp) + 1, 3);
        assert_eq!(gauge_frame(0, 250, hp) + 1, 1);
        assert_eq!(gauge_frame(180, 250, hp) + 1, 361);
        let mp = mp_max_frames(179);
        assert_eq!(mp + 1, 323);
        assert_eq!(gauge_frame(179, 179, mp) + 1, 323);
        assert_eq!(gauge_frame(60, 100, 100) + 1, 61);
        assert_eq!(gauge_frame(100, 100, 100) + 1, 101);
        assert_eq!(hp_max_frames(280) + 1, 561);
        assert_eq!(hp_max_frames(139) + 1, 279);
        assert_eq!(hp_max_frames(400) + 1, 669);
    }

    #[test]
    fn spec_11b_pinch() {
        assert!(is_pinch(62, 250) && !is_pinch(63, 250));
        assert_eq!(gauge_frame(62, 250, 500) + 1, 125);
        assert!(is_pinch(34, 139) && !is_pinch(35, 139));
    }

    #[test]
    fn spec_11b_situation_command() {
        // 12.34 s of 20 s: digits 12 and 3, gauge frame 219 - trunc(0.617 * 214 + 0.5) = 87.
        assert_eq!(situation_digits(12.34), (12, 3));
        assert_eq!(situation_gauge_frame(12.34, 20.0), 87);
        assert_eq!(situation_gauge_frame(20.0, 20.0), 5);
        assert_eq!(situation_gauge_frame(0.0, 20.0), 219);
        assert_eq!(situation_digits(250.0), (99, 9));
    }

    #[test]
    fn spec_11b_enemy_gauge_and_scope() {
        // Max HP 3000: 14 stock squares at full (15 bars); HP 1234: 6 squares, top bar frame 86.
        assert_eq!(enemy_gauge(3000, 200).0, 14);
        assert_eq!(enemy_gauge(1234, 200), (6, 86));
        // Shotlock aim of 6 s: after 3 s updateFlowScope(159).
        assert_eq!(flow_scope_frame(3.0, 6.0), 159);
    }

    #[test]
    fn spec_11b_viewport() {
        let (l, t, w, h) = stage_viewport(2560.0, 1080.0, (1.0, 1.0));
        assert_eq!((l, t, l + w, h), (320.0, 0.0, 2240.0, 1080.0));
        let (l, t, w, h) = stage_viewport(1920.0, 1200.0, (1.0, 1.0));
        assert_eq!((l, t, w, t + h), (0.0, 60.0, 1920.0, 1140.0));
        assert_eq!(stage_viewport(1280.0, 720.0, (1.0, 1.0)), (0.0, 0.0, 1280.0, 720.0));
    }
}
