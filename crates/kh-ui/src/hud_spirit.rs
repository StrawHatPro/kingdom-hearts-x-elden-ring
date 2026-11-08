//! (SUMMON) THE SUMMONED SPIRIT'S GAUGE (THE USER 2026-11-02: "Give spirit ashes a timer ... For Spirit Ashes, give them
//! their own KH-styled HUD element instead of using Elden Ring's HUD for them."). The element shows the spirit's NAME, its
//! HP and its TIME LEFT as a draining gauge, appears when the spirit is called, follows HP and time live, and goes when
//! the spirit leaves.
//!
//! WHICH KH3 PART: KH3's own GUEST PARTY-MEMBER GAUGE of the status HUD, slot 3 (`sttsSnpc1`, `HudStatus_fla.McSttsSnpc_14`:
//! the round gauge KH3 gives a world's guest member beside Donald and Goofy, at stage (1857, 833), above the friends' slots).
//! KH3 has no gauge for a Link summon itself (a Link takes the whole HUD over; the status HUD only plays its `link` looks),
//! so the guest member's gauge is KH3's piece for "an extra ally with an HP gauge". Driven through the movie's own AS3
//! interface (`setStatus(10 / 11, 3)` in / out, `setHP(3, ..)` with KH3's red follow, `setMP(3, ..)`, the `damage` label),
//! exactly as the native driver drives a friend slot:
//! - HP: the slot's HP arc (`npcHp`, 181 frames: KH3's friend rule `Fi(hp, maxHP, 180)`), the red part following a loss
//!   with KH3's `FollowGauge`; the slot's `damage` label (the bounce) on a loss; the base plate's looks (`snpcDodai`:
//!   `nomal`, `battle`, `dying` at KH3's pinch rule, `death`).
//! - TIME: the slot's second arc (`npcMp`, the friends' MP arc: 361 frames, the whole arc = the whole timer; see
//!   `hud_status::FRIEND_MP_FRAMES`) at `left / full`, so it drains as the timer runs. PORT DESIGN: the arc is recoloured
//!   with the spirit look (COMPOSED: a pale spirit blue, Elden Ring's spirit glow) so it does not read as MP.
//! - THE LOW-TIME WARNING (COMPOSED after KH3's own timer alert, `HudTimer` `counterAnm` `alert` / `counter` `red1`: the
//!   counter turns red and blinks in its last seconds): under [`SpiritLook::alert_below`] seconds the time arc and the
//!   time text pulse to red.
//! - THE PICTURE (THE USER 2026-11-03: "For the spirit ashes HUD picture, use the Elden Ring picture of the spirit ash."): in
//!   the face holder, the ash's own Elden Ring item icon (`EquipParamGoods.iconId`) by THE SHARED ELDEN RING ICON ROUTE
//!   (`item_art::er_icon_key` -> `ItemArt::icon`: the pack's `er_icons`, Elden Ring's `MENU_ItemIcon_<n>`, 160 x 160, scaled
//!   into the holder as `tres.controls.Icon` scales a picture into its clip). The Mimic Tear (Mimic Sora) shows the Mimic
//!   Tear ash's icon too, not a face. No icon number, or none in the pack: KH3's Link icon (IconText `Link`, the Summon
//!   command's icon) as the fallback.
//! - NAME and TIME TEXT (COMPOSED: KH3's party gauges carry no names): the game's name of the ash and `m:ss` left, in KH3's
//!   menu font with the HUD labels' white face and dark edge (`hud_goofy::peril_word_texture`), to the gauge's left.
//!
//! PLACEMENT: the guest slot is KH3's own place for a fourth member: above Donald's / Goofy's slots, right of Sora's /
//! the leader's main slot (and its GUARD bar, Resolve pips and Resonance badges, which sit over the main slot), far from
//! the command menu (bottom left). The port never seats anyone in slots 3 / 4 (three characters at most), so the slot is
//! free; it is driven here, outside `StatusDriver`, so the main slot's solo / party layout does not change with a spirit.
//!
//! The host gives [`SpiritValues`] every frame (`None`: no spirit out).

use std::sync::Arc;

use crate::def::{BitmapFill, Texture};
use crate::frames::{gauge_frame, gauge_frame_f, is_pinch};
use crate::geom::{Cx, Matrix};
use crate::hud_goofy::{peril_word_texture, PerilWord};
use crate::hud_resonance::{ensure_art_of, icon_rect, set_art, tex_rect};
use crate::item_art::{er_icon_key, ItemArt};
use crate::hud_status::{HudStatus, FRIEND_MP_FRAMES};
use crate::player::GfxRect;
use crate::render::Fonts;

/// The status HUD slot (KH3's first guest slot).
pub const SLOT: usize = 3;
/// The slot's clips.
pub const OUTER: &str = "sttsSnpc1";
pub const INNER: &str = "sttsSnpc1.sttsSnpc1";
const DODAI: &str = "sttsSnpc1.sttsSnpc1.snpcDodai";
const TIME_ARC: &str = "sttsSnpc1.sttsSnpc1.npcMp";
#[cfg(test)]
const HP_ARC: &str = "sttsSnpc1.sttsSnpc1.npcHp";
/// The art nodes this part adds to the slot (they move, bounce and fade with it).
pub const NAME_ART: &str = "sttsSnpc1.sttsSnpc1.spiritName";
pub const TIME_ART: &str = "sttsSnpc1.sttsSnpc1.spiritTime";
/// (THE USER 2026-11-03: "for the spirit ashes HUD pictures, have them appear below the health and timer bars layers") THE
/// PICTURE'S LAYER: a script child of the slot's BASE PLATE `snpcDodai` (timeline depth 1 of the slot), not of the slot: a
/// clip's script children draw right after its own timeline children, so the picture draws with the plate, BEFORE the time
/// arc `npcMp` (depth 5) and the HP arc `npcHp` (depth 11), which overlap it. (KH3's face holder `poNpcFace`, depth 19,
/// and the slot's own script children draw over the arcs: not used for it.)
pub const ICON_ART: &str = "sttsSnpc1.sttsSnpc1.snpcDodai.spiritIcon";
/// The class the art nodes are made from (a one-shape clip of the movie, emptied), as the Resolve pips'.
const ART_CLASS: &str = "CoPcMpLogo";
/// KH3's friend HP rule: `Fi(hp, maxHP, 180)` (0x1416608B0).
const HP_FRAMES: i32 = 180;

/// The spirit as the HUD draws it (from kh-core `SpiritOut`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct SpiritValues {
    pub name: String,
    pub hp: i32,
    pub hp_max: i32,
    /// Seconds left of its timer, and the timer's full length.
    pub left: f32,
    pub full: f32,
    /// The Mimic Tear (Mimic Sora).
    pub mimic: bool,
    /// The ash's Elden Ring icon number (`EquipParamGoods.iconId`; the Mimic Tear's for Mimic Sora); `None`: KH3's Link icon.
    pub icon: Option<u32>,
    /// In battle (the battle look of the base plate).
    pub in_battle: bool,
}

/// The look (COMPOSED, see the module text): status HUD units inside the slot (its origin is the gauge's centre), colours
/// as straight RGBA 0..1.
#[derive(Clone, Debug, PartialEq)]
pub struct SpiritLook {
    /// The time arc's colour, and the warning's.
    pub time: [f32; 4],
    pub alert: [f32; 4],
    /// The warning runs under this many seconds left (THE COORDINATOR: "a low-time warning ... under 10 s").
    pub alert_below: f32,
    pub blink_hz: f32,
    /// The name: its right end and its height centre, its height.
    pub name_at: (f32, f32),
    pub name_h: f32,
    /// The time text: its right end, height centre, height.
    pub time_at: (f32, f32),
    pub time_h: f32,
    /// The Link icon's height (in the face holder's place), and the Elden Ring picture's side (it fills the holder).
    pub icon_size: f32,
    pub picture_size: f32,
    /// (THE USER 2026-11-03: "for the spirit ashes picture, the corners should not be visible") the picture's round mask: its
    /// radius and soft edge, as shares of the picture's side (COMPOSED: the round portrait shape of KH3's party gauges).
    pub picture_radius: f32,
    pub picture_feather: f32,
    /// The texts' colours: the face, and the time text's (its own and under the warning).
    pub name_color: [f32; 4],
    pub time_color: [f32; 4],
    pub outline: [f32; 4],
}

impl Default for SpiritLook {
    fn default() -> SpiritLook {
        SpiritLook {
            time: [0.62, 0.88, 1.0, 1.0],
            alert: [1.0, 0.24, 0.18, 1.0],
            alert_below: 10.0,
            blink_hz: 2.5,
            name_at: (-44.0, -11.0),
            name_h: 20.0,
            time_at: (-44.0, 11.0),
            time_h: 18.0,
            icon_size: 30.0,
            // the picture's tablet is about 70 % of Elden Ring's 160 x 160 icon: at this size its edges run under the HP / time
            // arcs, which overlap it (THE USER 2026-11-03: the pictures "below the health and timer bars layers")
            picture_size: 78.0,
            // a disc inside the tablet (whose stone face is about 70 % of the icon): no square edge or corner remains
            picture_radius: 0.33,
            picture_feather: 0.05,
            name_color: [1.0, 1.0, 1.0, 1.0],
            time_color: [0.78, 0.93, 1.0, 1.0],
            outline: [0.04, 0.05, 0.08, 0.92],
        }
    }
}

/// What was last applied (tests and logs).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct SpiritShown {
    /// The HP arc's fill frame (0 .. 180) and the time arc's (0 .. 360), as sent (`frame + 1` to the clips).
    pub hp_frame: i32,
    pub time_frame: i32,
    pub alert: bool,
    pub name: String,
    pub time_text: String,
    /// The base plate's look label.
    pub look: &'static str,
    /// The Elden Ring icon number drawn in the holder (`None`: KH3's Link icon).
    pub picture: Option<u32>,
}

/// `m:ss` of the seconds left (rounded up: "0:01" until it is out).
pub fn time_text(left: f32) -> String {
    let s = left.max(0.0).ceil() as i32;
    format!("{}:{:02}", s / 60, s % 60)
}

/// The base plate's look (KH3's face-look rule of the native driver, 0x1416608B0): death under 1 HP, dying at a pinch,
/// battle, else normal; the labels of `HudStatus_fla.McSnpcDodai_16`.
pub fn look_label(v: &SpiritValues) -> &'static str {
    if v.hp < 1 {
        "death"
    } else if is_pinch(v.hp, v.hp_max) {
        "dying"
    } else if v.in_battle {
        "battle"
    } else {
        "nomal"
    }
}

fn flat(c: [f32; 4]) -> Cx {
    Cx { mul: [0.0, 0.0, 0.0, c[3]], add: [c[0] * 255.0, c[1] * 255.0, c[2] * 255.0, 0.0] }
}

fn mix(a: [f32; 4], b: [f32; 4], k: f32) -> [f32; 4] {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * k)
}

/// A text texture drawn right-aligned at `at` (right end, height centre), `h` tall.
fn text_rect(tex: &Arc<Texture>, at: (f32, f32), h: f32) -> GfxRect {
    let w = h * tex.width as f32 / tex.height.max(1) as f32;
    let (x, y) = (at.0 - w, at.1 - h / 2.0);
    let fill = BitmapFill { matrix: Matrix::new(w / tex.width as f32, 0.0, 0.0, h / tex.height as f32, x, y), texture: tex.clone(), scale: (1.0, 1.0), repeat: false, smooth: true };
    GfxRect { x, y, w, h, fill }
}

/// (THE USER 2026-11-03: "for the spirit ashes picture, the corners should not be visible") A ROUND CUT of a picture: its alpha
/// times a disc centred on it, `radius` and the soft edge `feather` as shares of its shorter side (1 inside, 0 past the edge,
/// a smooth step between), so the square tile's corners and edges are gone and only the round middle of the art shows.
pub fn round_mask(t: &Texture, radius: f32, feather: f32) -> Texture {
    let (w, h) = (t.width as usize, t.height as usize);
    let side = w.min(h) as f32;
    let (r, f) = (radius * side, (feather * side).max(1.0));
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let mut rgba = t.rgba.clone();
    for y in 0..h {
        for x in 0..w {
            let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
            let k = ((r + f / 2.0 - d) / f).clamp(0.0, 1.0);
            let k = k * k * (3.0 - 2.0 * k);
            if let Some(a) = rgba.get_mut((y * w + x) * 4 + 3) {
                *a = (*a as f32 * k + 0.5) as u8;
            }
        }
    }
    Texture { file: format!("{} (round cut)", t.file), width: t.width, height: t.height, rgba }
}

/// What the gauge draws with: KH3's fonts (the texts, the Link icon), the pack's item pictures (the shared Elden Ring icon
/// route) and the look.
#[derive(Clone, Copy)]
pub struct SpiritArt<'a> {
    pub fonts: &'a Fonts,
    pub art: &'a ItemArt,
    pub look: &'a SpiritLook,
}

/// The gauge's driver (one per HUD).
#[derive(Clone, Debug, Default)]
pub struct SpiritGauge {
    /// The gauge is up (from its `in` until the spirit leaves).
    pub shown: bool,
    hp: i32,
    hp_frame_sent: i32,
    time_frame_sent: i32,
    look_sent: Option<&'static str>,
    blink: f32,
    name_tex: Option<(String, Arc<Texture>)>,
    time_tex: Option<(String, bool, Arc<Texture>)>,
    /// The round cut of the picture shown, by icon number (made once per ash).
    picture_tex: Option<(u32, Arc<Texture>)>,
    /// (THE USER 2026-11-03: "have the spirit ashes HUD element fade away after they either die or time out") the slot's
    /// `out` runs on the spirit's last values.
    pub fading: bool,
    pub last: SpiritShown,
}

/// KH3's slot animations roll the gauge round (`McSttsSnpc_14` `out`: a full turn over frames 130 .. 165): the name and the
/// time beside it stay upright. Their matrix undoes the slot's turn and scale, so they only slide and fade with it.
fn upright(hud: &mut HudStatus) {
    let Some(m) = hud.movie.node(INNER).map(|n| n.matrix) else { return };
    let det = m.a * m.d - m.b * m.c;
    if det.abs() < 1.0e-6 {
        return;
    }
    let inverse = Matrix::new(m.d / det, -m.b / det, -m.c / det, m.a / det, 0.0, 0.0);
    for path in [NAME_ART, TIME_ART] {
        if let Some(n) = hud.movie.node_mut(path) {
            n.matrix = inverse;
            n.script_matrix = true;
        }
    }
}

impl SpiritGauge {
    /// One update of the status HUD (after its stage frames). `v` None = no spirit out; `leaving` = the spirit that just
    /// left, on its last values (kh-core `final_values`: HP 0 when it died, time 0 when it timed out), while the host keeps
    /// it; `art` = the pack's item pictures (the shared Elden Ring icon route).
    ///
    /// THE FADE (THE USER 2026-11-03: "have the spirit ashes HUD element fade away after they either die or time out"): the
    /// gauge shows the last values (no damage bounce), then plays KH3's own guest-slot `out` (setStatus(11, slot);
    /// `McSttsSnpc_14` frames 105 .. 172: 25 frames held, then 36 frames (0.6 s) in which the gauge rolls a full turn to the
    /// right (+160) as its alpha falls from 1 to 0; `off` at frame 173: 68 frames, 1.13 s in all at 60 fps). A new summon
    /// during it replaces it (its `in` again). No `leaving` (a load, the mode's end): the slot is off at once.
    pub fn update(&mut self, hud: &mut HudStatus, with: &SpiritArt, v: Option<&SpiritValues>, leaving: Option<&SpiritValues>, dt: f32) {
        let Some(v) = v else {
            match leaving {
                Some(last) if self.shown && !self.fading && hud.movie.exists(NAME_ART) => {
                    self.apply(hud, with, last, dt, false);
                    hud.set_status(11, SLOT as i32);
                    self.fading = true;
                }
                None if self.shown && !self.fading => {
                    hud.movie.goto_and_play(OUTER, "off");
                    self.shown = false;
                }
                _ => {}
            }
            if self.fading {
                upright(hud);
                if hud.movie.current_label(OUTER) == Some("off") {
                    self.fading = false;
                    self.shown = false;
                }
            }
            return;
        };
        if !hud.movie.exists(INNER) {
            return;
        }
        if self.fading {
            // a new summon during the fade: it comes in as a new spirit
            self.fading = false;
            self.shown = false;
        }
        if self.shown && !(hud.movie.exists(NAME_ART) && hud.movie.exists(ICON_ART)) {
            // the movie was made again (a new session): everything comes in again
            self.shown = false;
        }
        if !self.shown {
            // a new spirit: the slot's `in` (setStatus(10, slot)); everything is sent again
            hud.set_status(10, SLOT as i32);
            self.shown = true;
            self.hp = v.hp;
            self.hp_frame_sent = -1;
            self.time_frame_sent = -1;
            self.look_sent = None;
            self.blink = 0.0;
            for (path, parent, name) in [(NAME_ART, INNER, "spiritName"), (TIME_ART, INNER, "spiritTime"), (ICON_ART, DODAI, "spiritIcon")] {
                if !hud.movie.exists(path) && !ensure_art_of(&mut hud.movie, ART_CLASS, parent, name) {
                    return;
                }
            }
        }
        self.apply(hud, with, v, dt, true);
        upright(hud);
    }

    /// The values to the slot (`bounce`: a loss plays the slot's damage bounce; not on the last values of a fade).
    fn apply(&mut self, hud: &mut HudStatus, with: &SpiritArt, v: &SpiritValues, dt: f32, bounce: bool) {
        let SpiritArt { fonts, art, look } = *with;
        // HP: KH3's friend rule; the red part starts where the fill was and follows it down (setHP(slot, frame, red))
        let hp_frame = gauge_frame(v.hp.min(v.hp_max), v.hp_max, HP_FRAMES);
        if hp_frame != self.hp_frame_sent {
            let red = if self.hp_frame_sent < hp_frame { hp_frame } else { self.hp_frame_sent };
            hud.set_hp(SLOT, hp_frame + 1, red + 1);
            self.hp_frame_sent = hp_frame;
        }
        // a loss: the slot's damage bounce (KH3's `damage` label of the slot, as the friends' face-state damage plays it)
        if bounce && v.hp < self.hp && matches!(hud.movie.current_label(OUTER), Some("loop" | "damage")) {
            hud.movie.goto_and_play(OUTER, "damage");
        }
        self.hp = v.hp;
        let label = look_label(v);
        if self.look_sent != Some(label) {
            hud.movie.goto_and_play(DODAI, label);
            let id = match label {
                "death" => crate::hud_status::FACE_LOOK_DEATH,
                "dying" => crate::hud_status::FACE_LOOK_DYING,
                "battle" => crate::hud_status::FACE_LOOK_BATTLE,
                _ => crate::hud_status::FACE_LOOK_NORMAL,
            };
            hud.set_face_look(SLOT, id);
            self.look_sent = Some(label);
        }
        // TIME: the second arc at left / full of the whole arc (it drains with the timer)
        let time_frame = gauge_frame_f(v.left.max(0.0), v.full, FRIEND_MP_FRAMES);
        if time_frame != self.time_frame_sent {
            hud.set_mp(SLOT, time_frame + 1);
            self.time_frame_sent = time_frame;
        }
        let alert = v.left > 0.0 && v.left < look.alert_below;
        self.blink = if alert { (self.blink + dt * look.blink_hz.max(0.0)).fract() } else { 0.0 };
        let pulse = if alert { 0.5 - 0.5 * (self.blink * std::f32::consts::TAU).cos() } else { 0.0 };
        let arc = if alert { mix(look.time, look.alert, 0.45 + 0.55 * pulse) } else { look.time };
        if let Some(n) = hud.movie.node_mut(TIME_ARC) {
            n.cx = flat(arc);
        }
        // the texts (made again only when they change: the name once, the time once a second and on the warning's edge)
        if self.name_tex.as_ref().is_none_or(|(name, _)| *name != v.name) {
            self.name_tex = peril_word_texture(fonts, &PerilWord { text: v.name.clone(), color: look.name_color }, look.outline).map(|t| (v.name.clone(), Arc::new(t)));
        }
        let text = time_text(v.left);
        if self.time_tex.as_ref().is_none_or(|(t, a, _)| *t != text || *a != alert) {
            let color = if alert { look.alert } else { look.time_color };
            self.time_tex = peril_word_texture(fonts, &PerilWord { text: text.clone(), color }, look.outline).map(|t| (text.clone(), alert, Arc::new(t)));
        }
        let m = &mut hud.movie;
        match &self.name_tex {
            Some((_, t)) => set_art(m, NAME_ART, vec![text_rect(t, look.name_at, look.name_h)], Cx::IDENTITY, true),
            None => set_art(m, NAME_ART, Vec::new(), Cx::IDENTITY, false),
        }
        // under the warning the time text pulses with the arc
        let fade = if alert { Cx { mul: [1.0, 1.0, 1.0, 0.55 + 0.45 * pulse], add: [0.0; 4] } } else { Cx::IDENTITY };
        match &self.time_tex {
            Some((_, _, t)) => set_art(m, TIME_ART, vec![text_rect(t, look.time_at, look.time_h)], fade, true),
            None => set_art(m, TIME_ART, Vec::new(), Cx::IDENTITY, false),
        }
        // the face holder: the ash's Elden Ring picture (the shared route; kept loaded by `ItemArt`), centred and scaled to
        // fill it; else KH3's Link icon. Set every frame, so another spirit without a gap shows its own at once.
        if let Some(n) = v.icon.filter(|n| self.picture_tex.as_ref().is_none_or(|(k, _)| k != n)) {
            self.picture_tex = art.icon(&er_icon_key(n)).map(|t| (n, Arc::new(round_mask(&t, look.picture_radius, look.picture_feather))));
        }
        let picture = v.icon.and_then(|n| self.picture_tex.as_ref().filter(|(k, _)| *k == n).cloned());
        let (gfx, drawn) = match &picture {
            Some((n, t)) => {
                let s = look.picture_size;
                (vec![tex_rect(t, (0.0, 0.0, t.width as f32, t.height as f32), (-s / 2.0, -s / 2.0, s, s))], Some(*n))
            }
            None => (fonts.icons.as_ref().and_then(|set| icon_rect(set, "link", 0.0, 0.0, look.icon_size)).into_iter().collect(), None),
        };
        let shown = !gfx.is_empty();
        set_art(m, ICON_ART, gfx, Cx::IDENTITY, shown);
        self.last = SpiritShown { hp_frame, time_frame, alert, name: v.name.clone(), time_text: text, look: label, picture: drawn };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Pack;

    const DT: f32 = 1.0 / 60.0;

    fn spirit(hp: i32, left: f32) -> SpiritValues {
        SpiritValues { name: "Lone Wolf Ashes +3".to_string(), hp, hp_max: 600, left, full: 69.0, mimic: false, icon: None, in_battle: true }
    }

    /// The time text, the look rule (no pack needed).
    #[test]
    fn summon_time_text_and_look_rule() {
        assert_eq!(time_text(69.0), "1:09");
        assert_eq!(time_text(9.2), "0:10");
        assert_eq!(time_text(0.4), "0:01");
        assert_eq!(time_text(-1.0), "0:00");
        assert_eq!(look_label(&spirit(600, 30.0)), "battle");
        assert_eq!(look_label(&spirit(150, 30.0)), "dying", "KH3's pinch: 25 %");
        assert_eq!(look_label(&spirit(0, 30.0)), "death");
        assert_eq!(look_label(&SpiritValues { in_battle: false, ..spirit(600, 30.0) }), "nomal");
    }

    /// (THE USER 2026-11-02: "For Spirit Ashes, give them their own KH-styled HUD element") The gauge comes in on a summon,
    /// its time arc drains with `left / full`, its HP arc follows HP (the red part following a loss), the warning runs under
    /// 10 s, and it goes out when the spirit leaves.
    #[test]
    fn summon_gauge_appears_drains_follows_hp_and_goes() {
        let Ok(mut pack) = Pack::open_default() else {
            eprintln!("SKIPPED: no build/sora_pack/ui");
            return;
        };
        let fonts = Fonts::load(&mut pack).unwrap();
        let mut hud = HudStatus::new(pack.movie("HudStatus").unwrap());
        hud.set_status(0, 0);
        let mut g = SpiritGauge::default();
        let look = SpiritLook::default();
        let art = ItemArt::default();
        let run = |hud: &mut HudStatus, g: &mut SpiritGauge, v: Option<&SpiritValues>, n: usize| {
            for _ in 0..n {
                hud.tick(DT);
                g.update(hud, &SpiritArt { fonts: &fonts, art: &art, look: &look }, v, None, DT);
            }
        };
        // nothing out: the slot stays off
        run(&mut hud, &mut g, None, 10);
        assert!(!g.shown && !hud.calls.iter().any(|c| c.starts_with("setStatus(10, 3)")));
        // summoned: in, full time, full HP
        let full = spirit(600, 69.0);
        run(&mut hud, &mut g, Some(&full), 150);
        assert!(g.shown && hud.calls.iter().any(|c| c == "setStatus(10, 3)"), "{:?}", hud.calls);
        assert_eq!((g.last.hp_frame, g.last.time_frame), (180, 360));
        assert_eq!(hud.movie.current_frame(TIME_ARC), Some(361));
        assert_eq!(hud.hp_frame(SLOT), 181);
        assert!(hud.movie.node(OUTER).is_some_and(|n| n.is_visible()), "the slot is up");
        assert_eq!(g.last.name, "Lone Wolf Ashes +3");
        assert_eq!(g.last.time_text, "1:09");
        assert!(hud.movie.node(NAME_ART).is_some_and(|n| n.is_visible()) && hud.movie.node(ICON_ART).is_some_and(|n| n.is_visible()));
        // half the time gone and a hit: the arcs follow; the red HP part starts at the old fill and follows down
        let half = spirit(300, 34.5);
        run(&mut hud, &mut g, Some(&half), 1);
        assert_eq!((g.last.hp_frame, g.last.time_frame), (90, 180));
        assert_eq!(hud.movie.current_frame(TIME_ARC), Some(181));
        assert_eq!(hud.hp_frame(SLOT), 91);
        assert!(hud.hp_red_frame(SLOT) > 91, "the lost part is red first: {}", hud.hp_red_frame(SLOT));
        assert_eq!(hud.movie.current_label(OUTER), Some("damage"), "the slot's damage bounce");
        run(&mut hud, &mut g, Some(&half), 180);
        assert_eq!(hud.hp_red_frame(SLOT), 91, "the red part caught up");
        assert!(!g.last.alert);
        // under 10 s: the warning
        let low = spirit(300, 8.0);
        run(&mut hud, &mut g, Some(&low), 10);
        assert!(g.last.alert && g.last.time_text == "0:08");
        let red = hud.movie.node(TIME_ARC).unwrap().cx.add;
        assert!(red[0] > red[2], "the arc turns red: {red:?}");
        // the spirit went with nothing to show (a load, the mode's end: no `leaving`): the slot is off at once
        run(&mut hud, &mut g, None, 1);
        assert!(!g.shown && !g.fading && !hud.calls.iter().any(|c| c == "setStatus(11, 3)"));
        assert_eq!(hud.movie.current_label(OUTER), Some("off"), "off at once");
        // a new spirit later comes in again with everything sent anew
        run(&mut hud, &mut g, Some(&full), 30);
        assert_eq!(hud.calls.iter().filter(|c| *c == "setStatus(10, 3)").count(), 2);
        assert_eq!((g.last.hp_frame, g.last.time_frame), (180, 360));
        assert_eq!(g.last.picture, None, "no icon number: KH3's Link icon");
        assert!(drawn_file(&hud).is_some_and(|f| !f.contains("er_icons")), "the Link icon: {:?}", drawn_file(&hud));
    }

    /// (THE USER 2026-11-03: "for the spirit ashes picture, the corners should not be visible") The round cut: the corners and
    /// the square's edges are clear, the middle is the art as it was, the edge is soft.
    #[test]
    fn summon_picture_round_cut_hides_the_corners() {
        let t = Texture { file: "t".to_string(), width: 160, height: 160, rgba: vec![255; 160 * 160 * 4] };
        let m = round_mask(&t, 0.33, 0.05);
        let a = |x: usize, y: usize| m.rgba[(y * 160 + x) * 4 + 3];
        for (x, y) in [(0, 0), (159, 0), (0, 159), (159, 159), (80, 2), (2, 80), (80, 157), (157, 80), (25, 25)] {
            assert_eq!(a(x, y), 0, "({x}, {y}) is clear");
        }
        assert_eq!(a(80, 80), 255, "the middle is kept");
        assert_eq!(a(80, 40), 255, "inside the disc (radius 52.8 px)");
        assert!(a(80, 28) > 0 && a(80, 28) < 255, "a soft edge: {}", a(80, 28));
        assert_eq!(&m.rgba[(80 * 160 + 80) * 4..][..3], &[255, 255, 255], "the colours are untouched");
    }

    /// (THE USER 2026-11-03: "have the spirit ashes HUD element fade away after they either die or time out") A death fades on
    /// empty HP, a time-out on empty time, by KH3's own slot `out` (the roll and the alpha fall); the slot is off after it; the
    /// texts stay upright through the roll; a new summon during a fade comes in again.
    #[test]
    fn summon_gauge_fades_out_on_death_and_time_out() {
        let Ok(mut pack) = Pack::open_default() else {
            eprintln!("SKIPPED: no build/sora_pack/ui");
            return;
        };
        let fonts = Fonts::load(&mut pack).unwrap();
        let (art, look) = (ItemArt::default(), SpiritLook::default());
        let with = SpiritArt { fonts: &fonts, art: &art, look: &look };
        let mut hud = HudStatus::new(pack.movie("HudStatus").unwrap());
        hud.set_status(0, 0);
        let mut g = SpiritGauge::default();
        let step = |hud: &mut HudStatus, g: &mut SpiritGauge, v: Option<&SpiritValues>, leaving: Option<&SpiritValues>, n: usize| {
            for _ in 0..n {
                hud.tick(DT);
                g.update(hud, &with, v, leaving, DT);
            }
        };
        let alive = spirit(400, 30.0);
        for (case, last) in [("died", SpiritValues { hp: 0, ..alive.clone() }), ("timed out", SpiritValues { left: 0.0, ..alive.clone() })] {
            step(&mut hud, &mut g, Some(&alive), None, 150);
            assert!(g.shown && !g.fading, "{case}: up");
            let outs = hud.calls.iter().filter(|c| *c == "setStatus(11, 3)").count();
            step(&mut hud, &mut g, None, Some(&last), 1);
            assert!(g.fading, "{case}: fading");
            assert_eq!(hud.calls.iter().filter(|c| *c == "setStatus(11, 3)").count(), outs + 1, "{case}: KH3's slot `out`");
            if case == "died" {
                assert_eq!((g.last.hp_frame, g.last.look), (0, "death"), "{case}: the HP arc empty, the death plate");
                assert_eq!(hud.hp_frame(SLOT), 1);
            } else {
                assert_eq!(g.last.time_frame, 0, "{case}: the time arc empty");
                assert_eq!(hud.movie.current_frame(TIME_ARC), Some(1));
            }
            assert_ne!(hud.movie.current_label(OUTER), Some("damage"), "{case}: no bounce on the last values");
            // mid-fade: the slot rolls and fades; the name stays upright (its matrix undoes the slot's turn)
            step(&mut hud, &mut g, None, Some(&last), 45);
            let inner = hud.movie.node(INNER).unwrap();
            let (m, alpha) = (inner.matrix, inner.cx.mul[3]);
            assert!(m.b.abs() > 0.1 && alpha < 0.95 && alpha > 0.05, "{case}: rolling and fading: {m:?} alpha {alpha}");
            let n = hud.movie.node(NAME_ART).unwrap().matrix;
            let (a, b) = (m.a * n.a + m.c * n.b, m.b * n.a + m.d * n.b);
            assert!((a - 1.0).abs() < 1e-3 && b.abs() < 1e-3, "{case}: the name upright: {a} {b}");
            // the end: the slot is off, the gauge is done (the host still holds `leaving` until 1.5 s: nothing replays)
            step(&mut hud, &mut g, None, Some(&last), 30);
            assert_eq!(hud.movie.current_label(OUTER), Some("off"), "{case}: off after the fade");
            assert!(!g.shown && !g.fading);
            step(&mut hud, &mut g, None, Some(&last), 10);
            assert_eq!(hud.calls.iter().filter(|c| *c == "setStatus(11, 3)").count(), outs + 1, "{case}: one fade");
            step(&mut hud, &mut g, None, None, 1);
        }
        // a new summon during a fade replaces it: `in` again, its values
        step(&mut hud, &mut g, Some(&alive), None, 150);
        step(&mut hud, &mut g, None, Some(&SpiritValues { hp: 0, ..alive.clone() }), 20);
        let ins = hud.calls.iter().filter(|c| *c == "setStatus(10, 3)").count();
        let next = SpiritValues { name: "Mimic Tear +10".to_string(), mimic: true, ..spirit(600, 90.0) };
        step(&mut hud, &mut g, Some(&next), None, 150);
        assert!(g.shown && !g.fading && hud.calls.iter().filter(|c| *c == "setStatus(10, 3)").count() == ins + 1);
        assert_eq!((g.last.name.as_str(), g.last.hp_frame, g.last.look), ("Mimic Tear +10", 180, "battle"));
        assert_eq!(hud.movie.current_label(OUTER), Some("loop"));
    }

    /// The texture file the holder's art draws (tests).
    fn drawn_file(hud: &HudStatus) -> Option<String> {
        hud.movie.sprite(ICON_ART).and_then(|s| s.gfx.first()).map(|r| r.fill.texture.file.clone())
    }

    /// (THE USER 2026-11-03: "For the spirit ashes HUD picture, use the Elden Ring picture of the spirit ash.") The ash's own
    /// Elden Ring icon in the holder by the shared route (`er_icon_key` -> `ItemArt::icon`), the Mimic Tear's for Mimic Sora;
    /// another spirit straight after shows its own; no number (or none in the pack): KH3's Link icon.
    #[test]
    fn summon_gauge_shows_the_ashs_elden_ring_icon_with_the_link_fallback() {
        let Ok(mut pack) = Pack::open_default() else {
            eprintln!("SKIPPED: no build/sora_pack/ui");
            return;
        };
        let fonts = Fonts::load(&mut pack).unwrap();
        let art = ItemArt::load(&mut pack);
        // Elden Ring 2.7.2.0 EquipParamGoods.iconId: Black Knife Tiche (row 200000) 113, the Mimic Tear (row 207000) 117
        if art.host_icon(113).is_none() || art.host_icon(117).is_none() {
            eprintln!("SKIPPED: the pack's er_icons lack the ash icons (tools/er_icons.js)");
            return;
        }
        let mut hud = HudStatus::new(pack.movie("HudStatus").unwrap());
        hud.set_status(0, 0);
        let mut g = SpiritGauge::default();
        let look = SpiritLook::default();
        let tiche = SpiritValues { name: "Black Knife Tiche +3".to_string(), icon: Some(113), ..spirit(600, 60.0) };
        let mimic = SpiritValues { name: "Mimic Tear +10".to_string(), mimic: true, icon: Some(117), ..spirit(600, 60.0) };
        let unknown = SpiritValues { icon: None, ..tiche.clone() };
        let missing = SpiritValues { icon: Some(999_999), ..tiche.clone() };
        for (v, want, file) in [(&tiche, Some(113), Some("er_icons/00113.png")), (&mimic, Some(117), Some("er_icons/00117.png")), (&unknown, None, None), (&missing, None, None)] {
            for _ in 0..5 {
                hud.tick(DT);
                g.update(&mut hud, &SpiritArt { fonts: &fonts, art: &art, look: &look }, Some(v), None, DT);
            }
            assert_eq!(g.last.picture, want, "{}", v.name);
            let drawn = drawn_file(&hud).unwrap_or_default();
            match file {
                Some(f) => assert_eq!(drawn, format!("{f} (round cut)"), "{}: its own picture, cut round", v.name),
                None => assert!(!drawn.is_empty() && !drawn.contains("er_icons"), "{:?}: the Link icon, drew {drawn}", v.icon),
            }
            assert!(hud.movie.node(ICON_ART).is_some_and(|n| n.is_visible()));
        }
        assert_eq!(hud.calls.iter().filter(|c| *c == "setStatus(10, 3)").count(), 1, "the slot stayed up through the swaps");

        // (THE USER 2026-11-03: "for the spirit ashes HUD pictures, have them appear below the health and timer bars layers")
        // THE LAYERS, drawn: where the arcs and the picture overlap, the picture changes nothing (the arcs are on top), and
        // there is such an overlap. (After the slot's `in`.)
        for _ in 0..150 {
            hud.tick(DT);
            g.update(&mut hud, &SpiritArt { fonts: &fonts, art: &art, look: &look }, Some(&tiche), None, DT);
        }
        let (x0, y0, w, h) = (1857 - 70, 833 - 70, 140usize, 140usize);
        let draw = |hud: &HudStatus, icon: bool, arcs: bool| {
            let mut m = hud.movie.clone();
            m.set_visible(ICON_ART, icon);
            m.set_visible(TIME_ARC, arcs);
            m.set_visible(HP_ARC, arcs);
            let mut canvas = crate::Canvas::region(x0, y0, w, h);
            crate::Renderer::new(fonts.clone()).draw(&mut canvas, &m, &Matrix::IDENTITY);
            let mut out = Vec::new();
            canvas.to_rgba8(&mut out);
            out
        };
        let (all, no_icon, bare, icon_only) = (draw(&hud, true, true), draw(&hud, false, true), draw(&hud, false, false), draw(&hud, true, false));
        let differs = |a: &[u8], b: &[u8], i: usize| (0..4).any(|k| a[i * 4 + k].abs_diff(b[i * 4 + k]) > 24);
        // covered: the pixel is the arcs' own (unchanged by the picture); on top: it is the picture's (the arcs hidden by it)
        let (mut overlap, mut covered, mut on_top) = (0, 0, 0);
        for i in 0..w * h {
            let arc = differs(&no_icon, &bare, i);
            let picture = differs(&icon_only, &bare, i);
            if arc && picture {
                overlap += 1;
                covered += (!differs(&all, &no_icon, i)) as usize;
                on_top += (!differs(&all, &icon_only, i) && differs(&all, &no_icon, i)) as usize;
            }
        }
        let count = |a: &[u8], b: &[u8]| (0..w * h).filter(|i| differs(a, b, *i)).count();
        let opaque = all.chunks(4).filter(|p| p[3] > 0).count();
        assert!(overlap > 200, "the arcs overlap the picture: {overlap} px (arcs {}, picture {}, drawn {opaque})", count(&no_icon, &bare), count(&icon_only, &bare));
        // (the arcs' anti-aliased / translucent edges let a little of the picture through: not all of the overlap is unchanged)
        assert!(covered * 100 >= overlap * 70, "the arcs are on top of the picture: {covered} / {overlap} px unchanged by it");
        assert!(on_top * 100 <= overlap * 10, "the picture does not cover the arcs: {on_top} / {overlap} px");
    }
}
