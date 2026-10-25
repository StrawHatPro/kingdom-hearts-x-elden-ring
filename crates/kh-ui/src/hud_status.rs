//! `HudStatus` (bottom right: HP, MP, focus, portrait). Two halves, as in the game:
//!
//! - [`HudStatus`]: the movie with the 19 interface methods of its AS3 document class
//!   (`build/sora_pack/ui/as3/HudStatus.as3.txt`, class `HudStatus`) and the classes they use
//!   (`tres.controls.Face`, `tres.utils.FollowGauge`), ported method by method;
//! - [`StatusDriver`]: the native part `UTresUIP_HudStatus` (update 0x1416608B0, flush
//!   0x141662640; docs/sora-ui-spec.md section 4): values -> cached frames and dirty bits ->
//!   the AS3 calls, plus the low-HP alarm request (`0x14158A340` / `0x14158D800`).

use std::sync::Arc;

use crate::controls::FollowGauge;
use crate::def::MovieDef;
use crate::frames::{gauge_frame, hp_max_frames, is_pinch, mp_max_frames};
use crate::player::Movie;
use crate::render::Fonts;
use crate::UiSound;

/// `HudStatus.STATUS_INFO_MAX`.
pub const STATUS_INFO_MAX: usize = 5;
/// `HudStatus.FRIEND_MAX`.
pub const FRIEND_MAX: usize = 4;

/// (RC, the user 2026-10-20: "100% mp should mean a full mp bar for them") THE FRIEND SLOT'S MP LENGTH. KH3's update
/// (0x1416608B0) sends a friend's MP as `Fi(mp, maxMP, 0xB4)` = frames 1..181, but the slot's `npcMp` clip (HudStatus
/// sprite 43) is a 361-frame timeline: its mask turns 0.5 degrees a frame over the 180-degree MP arc (frame 1 empty, 361
/// full), so KH3's 181 fills only HALF the arc at 100 %. The fill is MP / maxMP of the whole arc here: 360 frames, the
/// clip's own length (test `friend_mp_arc_is_full_at_full_mp`). The friends' HP clip is 181 frames: its 180 stays.
pub const FRIEND_MP_FRAMES: i32 = 360;

/// `tres.constants.Types.FACE_STATE_ID_*` (= indices of `Face.LABELS_STATE`).
pub const FACE_STATE_USUALLY: i32 = 0;
pub const FACE_STATE_DAMAGE: i32 = 1;
pub const FACE_STATE_LINK: i32 = 2;
/// `Types.FACE_LOOK_ID_*` (= indices of `Face.LABELS_LOOK`).
pub const FACE_LOOK_NORMAL: i32 = 0;
pub const FACE_LOOK_BATTLE: i32 = 1;
pub const FACE_LOOK_DYING: i32 = 2;
pub const FACE_LOOK_DEATH: i32 = 3;

const LABELS_STATE: [&str; 3] = ["usually", "damage", "link"];
const LABELS_LOOK: [&str; 4] = ["normal", "battle", "dying", "death"];
const LABELS_LOOK_TRANSITION: [&str; 2] = ["b2n", "n2b"];

/// `tres.controls.Face`: one portrait holder.
#[derive(Clone, Debug, Default)]
struct Face {
    /// Instance path of the holder clip (`poPcFace` / `poNpcFace`).
    host: String,
    enable_form: bool,
    is_pc: bool,
    /// `m_face`: the loaded movie's first child (its instance name), when loaded.
    face: Option<String>,
    link_node: Option<String>,
    damage_nodes: [Option<String>; 2],
    damage_node_labels: [Option<String>; 2],
    look_id: i32,
    form_id: i32,
}

/// (party P) The content extent of the PC face movies (FacePcExSora01: -384 .. 384) and of the NPC face movies
/// (FaceNpcExDonald / Goofy / Sora: -209.7 .. 212.7), stage units [A: their shapes' bounds].
const PC_FACE_EXTENT: f32 = 768.0;
const NPC_FACE_EXTENT: f32 = 422.4;

impl Face {
    fn face_path(&self) -> Option<String> {
        self.face.as_ref().map(|f| format!("{}.{f}", self.host))
    }

    /// `Face.unload` + `Face.load` + `onLoadComplete` (the load completes at once here).
    fn load(&mut self, movie: &mut Movie, def: Option<&Arc<MovieDef>>) {
        // unload(): removeChild(m_face); m_faceLookId = 0.
        if self.face.take().is_some() {
            movie.detach(&self.host);
            self.look_id = 0;
        }
        let Some(def) = def else { return };
        // onLoadComplete: the content's first child at (0, 0); faceHyoj / faceEff by name.
        let Some(name) = movie.attach_first_child(&self.host, def) else { return };
        self.face = Some(name);
        // (party P) A FACE OF THE OTHER SIZE CLASS in this holder (the leader switch: Donald / Goofy have no PC face movie,
        // KH3 never puts them in the main slot): the movie's content is scaled to the holder's class, the PC faces'
        // content being PC_FACE_EXTENT and the NPC faces' NPC_FACE_EXTENT stage units across [A: the face movies' own
        // bounds, FacePcExSora01 vs FaceNpcExDonald / Goofy / Sora], so it fills the holder as the face it replaces did.
        let pc_movie = def.name.starts_with("FacePc");
        if pc_movie != self.is_pc {
            let s = if self.is_pc { PC_FACE_EXTENT / NPC_FACE_EXTENT } else { NPC_FACE_EXTENT / PC_FACE_EXTENT };
            if let Some(path) = self.face_path() {
                movie.scale_node(&path, s);
            }
        }
        self.set_look(movie, self.look_id);
        if self.enable_form {
            self.set_form(movie, self.form_id);
        }
    }

    /// `Face.setState(a1)`.
    fn set_state(&mut self, movie: &mut Movie, a1: i32) {
        if a1 == FACE_STATE_LINK && !self.is_pc {
            return;
        }
        let (Some(face), Some(label)) = (self.face_path(), LABELS_STATE.get(a1 as usize)) else { return };
        movie.goto_and_play(&face, label);
        if a1 == FACE_STATE_DAMAGE {
            for i in 0..2 {
                if let Some(node) = &self.damage_nodes[i] {
                    movie.goto_and_play(node, self.damage_node_labels[i].as_deref().unwrap_or(label));
                }
            }
        }
    }

    /// `Face.setLook(a1)`.
    fn set_look(&mut self, movie: &mut Movie, a1: i32) {
        if let (Some(face), Some(l2)) = (self.face_path(), LABELS_LOOK.get(a1 as usize)) {
            let look = format!("{face}.faceHyoj");
            if movie.exists(&look) {
                let mut l3 = *l2;
                if a1 == FACE_LOOK_BATTLE && self.look_id == FACE_LOOK_NORMAL {
                    l3 = LABELS_LOOK_TRANSITION[1];
                } else if a1 == FACE_LOOK_NORMAL && self.look_id == FACE_LOOK_BATTLE {
                    l3 = LABELS_LOOK_TRANSITION[0];
                }
                movie.goto_and_play(&look, l3);
                let effect = format!("{face}.faceEff");
                if movie.exists(&effect) {
                    movie.goto_and_play(&effect, l2);
                }
                if let Some(node) = &self.link_node {
                    movie.goto_and_play(node, l2);
                }
            }
        }
        self.look_id = a1;
    }

    /// `Face.setForm(a1)`: labels `f0` .. `f10` on the four form nodes.
    fn set_form(&mut self, movie: &mut Movie, a1: i32) {
        if !self.enable_form {
            return;
        }
        if let (Some(face), true) = (self.face_path(), (0..=10).contains(&a1)) {
            if movie.exists(&format!("{face}.faceHyoj")) {
                let label = format!("f{a1}");
                for node in ["faceHyoj.facePc001", "faceHyoj.facePc002", "facePc003", "faceHyoj.facePc004"] {
                    let path = format!("{face}.{node}");
                    if movie.exists(&path) {
                        movie.goto_and_play(&path, &label);
                    }
                }
            }
        }
        self.form_id = a1;
    }
}

/// The movie and its AS3 document class.
pub struct HudStatus {
    pub movie: Movie,
    /// `m_statusStateAry`: sttsPc, sttsNpc1, sttsNpc2, sttsSnpc1, sttsSnpc2.
    status_state: [&'static str; STATUS_INFO_MAX],
    /// `m_hpGAry` / `m_hpRAry` / `m_mpAry`.
    hp_g: [String; STATUS_INFO_MAX],
    hp_r: [String; STATUS_INFO_MAX],
    mp: [String; STATUS_INFO_MAX],
    hp_follow: [FollowGauge; STATUS_INFO_MAX],
    focus_follow: FollowGauge,
    faces: [Face; STATUS_INFO_MAX],
    is_solo: bool,
    is_focus_max: bool,
    is_focus_charge: bool,
    focus_gauge: i32,
    is_focus_empty: bool,
    is_focus_lock: bool,
    /// Every interface call, newest last (tests and the debug page); capped.
    pub calls: Vec<String>,
}

const PC: &str = "sttsPc";
const PC_HP_PACK: &str = "sttsPc.pcHp.pcHpPack";
const PC_HP_SET: &str = "sttsPc.pcHp.pcHpPack.pcHpGgSet";
const PC_MP: &str = "sttsPc.pcMp.pcMpPack";
const PC_FOCUS: &str = "sttsPc.pcEx.pcFocusGg";

impl HudStatus {
    /// The constructor of the document class and `onAddedToStage`.
    pub fn new(def: Arc<MovieDef>) -> HudStatus {
        let status_state = ["sttsPc", "sttsNpc1", "sttsNpc2", "sttsSnpc1", "sttsSnpc2"];
        // m_statusAry[1..] = sttsNpc1.sttsNpc, sttsNpc2.sttsNpc, sttsSnpc1.sttsSnpc1, sttsSnpc2.sttsSnpc1
        let status = ["sttsPc", "sttsNpc1.sttsNpc", "sttsNpc2.sttsNpc", "sttsSnpc1.sttsSnpc1", "sttsSnpc2.sttsSnpc1"];
        let mut faces: [Face; STATUS_INFO_MAX] = Default::default();
        faces[0] = Face { host: format!("{PC_HP_PACK}.poPcFace"), is_pc: true, link_node: Some(format!("{PC_HP_PACK}.pcFaceDodai.pcFaceDodai1")), damage_nodes: [Some(PC.to_string()), Some(format!("{PC_HP_PACK}.pcFaceDodai"))], ..Face::default() };
        let mut hp_g: [String; STATUS_INFO_MAX] = Default::default();
        let mut hp_r: [String; STATUS_INFO_MAX] = Default::default();
        let mut mp: [String; STATUS_INFO_MAX] = Default::default();
        hp_g[0] = format!("{PC_HP_SET}.pcHpGg");
        hp_r[0] = format!("{PC_HP_SET}.pcHpGgR");
        mp[0] = format!("{PC_MP}.pcMpGg");
        for i in 1..STATUS_INFO_MAX {
            let l5 = status[i];
            hp_g[i] = format!("{l5}.npcHp.npcHpGg");
            hp_r[i] = format!("{l5}.npcHp.npcHpGgR");
            mp[i] = format!("{l5}.npcMp");
            // setDamageNode(l5.parent): the slot's state clip.
            faces[i] = Face { host: format!("{l5}.poNpcFace"), link_node: Some(format!("{l5}.npcDodai")), damage_nodes: [Some(status_state[i].to_string()), None], ..Face::default() };
        }
        let mut hud = HudStatus { movie: Movie::new(def), status_state, hp_g, hp_r, mp, hp_follow: Default::default(), focus_follow: FollowGauge::default(), faces, is_solo: false, is_focus_max: false, is_focus_charge: false, focus_gauge: 0, is_focus_empty: false, is_focus_lock: false, calls: Vec::new() };
        // onAddedToStage
        hud.set_focus_gauge(1, 1, false, false);
        hud.movie.goto_frame(&format!("{PC_FOCUS}.pcExGgC"), 1);
        hud.set_hp_max(0, 1);
        hud.set_mp_max(0, 1);
        for l1 in 0..STATUS_INFO_MAX {
            hud.set_hp(l1, 1, 1);
            hud.set_mp(l1, 1);
        }
        hud.set_mp_charge(1);
        hud.calls.clear();
        hud
    }

    fn log(&mut self, call: String) {
        if self.calls.len() >= 256 {
            self.calls.remove(0);
        }
        self.calls.push(call);
    }

    /// One frame of the stage: the timelines advance and the `FollowGauge` timers run on
    /// `dt` seconds of real time.
    pub fn tick(&mut self, dt: f32) {
        self.movie.tick();
        let ms = dt as f64 * 1000.0;
        for i in 0..STATUS_INFO_MAX {
            if let Some(frame) = self.hp_follow[i].advance(ms) {
                self.movie.goto_frame(&self.hp_r[i], frame);
            }
        }
        if let Some(frame) = self.focus_follow.advance(ms) {
            self.movie.goto_frame(&format!("{PC_FOCUS}.pcExGgR"), frame);
        }
    }

    /// `setSolo(a1)`.
    pub fn set_solo(&mut self, a1: bool) {
        self.log(format!("setSolo({a1})"));
        self.is_solo = a1;
        self.faces[0].damage_node_labels = [a1.then(|| "damage2".to_string()), None];
    }

    /// `setStatus(a1, a2)`: -1 off, 0 loop, 1 inShort, 2 outShort on the root; 10 in / 11 out of
    /// slot `a2` (`in2` / `out2` for Sora alone); 12 change2, 13 change1 on Sora's slot.
    pub fn set_status(&mut self, a1: i32, a2: i32) {
        self.log(format!("setStatus({a1}, {a2})"));
        let slot = (0..STATUS_INFO_MAX as i32).contains(&a2).then_some(a2 as usize);
        match a1 {
            -1 => drop(self.movie.goto_and_play("", "off")),
            0 => drop(self.movie.goto_and_play("", "loop")),
            1 => drop(self.movie.goto_and_play("", "inShort")),
            2 => drop(self.movie.goto_and_play("", "outShort")),
            10 => {
                if let Some(slot) = slot {
                    let label = if slot == 0 && self.is_solo { "in2" } else { "in" };
                    self.movie.goto_and_play(self.status_state[slot], label);
                }
            }
            11 => {
                if let Some(slot) = slot {
                    let label = if slot == 0 && self.is_solo { "out2" } else { "out" };
                    self.movie.goto_and_play(self.status_state[slot], label);
                }
            }
            12 => drop(self.movie.goto_and_play(self.status_state[0], "change2")),
            13 => drop(self.movie.goto_and_play(self.status_state[0], "change1")),
            _ => {}
        }
    }

    /// `setFocusChargeMode(a1, a2)`.
    pub fn set_focus_charge_mode(&mut self, a1: bool, a2: bool) {
        self.log(format!("setFocusChargeMode({a1}, {a2})"));
        if a1 {
            self.movie.goto_frame(&format!("{PC_FOCUS}.pcExGg"), 1);
            if !self.is_focus_lock {
                self.movie.goto_and_play(PC_FOCUS, if a2 { "charge_loop" } else { "charge_in" });
            }
        } else {
            self.movie.goto_frame(&format!("{PC_FOCUS}.pcExGgC"), 1);
            if !self.is_focus_lock {
                self.movie.goto_and_play(PC_FOCUS, "loop");
            }
        }
        self.is_focus_charge = a1;
    }

    /// `setFocusGaugeLock(a1)`.
    pub fn set_focus_gauge_lock(&mut self, a1: bool) {
        self.log(format!("setFocusGaugeLock({a1})"));
        self.is_focus_lock = a1;
        let label = if a1 {
            "lock"
        } else if self.is_focus_charge {
            "charge_loop"
        } else if self.is_focus_max {
            "max_loop"
        } else {
            "loop"
        };
        self.movie.goto_and_play(PC_FOCUS, label);
    }

    /// `setFocusGauge(a1, a2, a3, a4)`: fill frame, red start frame, isMax, consume mode.
    pub fn set_focus_gauge(&mut self, a1: i32, a2: i32, a3: bool, a4: bool) {
        self.log(format!("setFocusGauge({a1}, {a2}, {a3}, {a4})"));
        let red = format!("{PC_FOCUS}.pcExGgR");
        if self.is_focus_charge {
            if !self.is_focus_lock && self.focus_gauge < a1 {
                self.movie.goto_and_play(PC_FOCUS, "charge_up");
            }
            self.movie.goto_frame(&format!("{PC_FOCUS}.pcExGgC"), a1.max(1) as u32);
            if let Some(frame) = self.focus_follow.set_gauge(a2, a1) {
                self.movie.goto_frame(&red, frame);
            }
        } else {
            self.movie.goto_frame(&format!("{PC_FOCUS}.pcExGg"), a1.max(1) as u32);
            if a4 {
                self.movie.goto_frame(&red, a2.max(1) as u32);
            } else if let Some(frame) = self.focus_follow.set_gauge(a2, a1) {
                self.movie.goto_frame(&red, frame);
            }
            if self.is_focus_max != a3 {
                self.is_focus_max = a3;
                if !self.is_focus_lock {
                    self.movie.goto_and_play(PC_FOCUS, if a3 { "max_in" } else { "max_out" });
                }
            }
        }
        self.focus_gauge = a1;
        if a4 && a2 <= 1 {
            let l5 = a1 <= 1;
            if self.is_focus_empty != l5 {
                self.is_focus_empty = l5;
                self.movie.goto_and_play(&format!("{PC_FOCUS}.pcExDodai.pcExDodaiAnm"), if l5 { "not" } else { "loop" });
            }
        }
    }

    /// `setHPMax(a1, a2)`: Sora's slot only.
    pub fn set_hp_max(&mut self, a1: usize, a2: i32) {
        self.log(format!("setHPMax({a1}, {a2})"));
        if a1 == 0 {
            self.movie.goto_frame(&format!("{PC_HP_SET}.pcHpDodai"), a2.max(1) as u32);
        }
    }

    /// `setHP(a1, a2, a3)`: fill frame `a2`; the red gauge starts at `a3` and follows to `a2`.
    pub fn set_hp(&mut self, a1: usize, a2: i32, a3: i32) {
        self.log(format!("setHP({a1}, {a2}, {a3})"));
        if a1 >= STATUS_INFO_MAX {
            return;
        }
        self.movie.goto_frame(&self.hp_g[a1], a2.max(1) as u32);
        if let Some(frame) = self.hp_follow[a1].set_gauge(a3, a2) {
            self.movie.goto_frame(&self.hp_r[a1], frame);
        }
    }

    /// `setMPMax(a1, a2)`.
    pub fn set_mp_max(&mut self, a1: usize, a2: i32) {
        self.log(format!("setMPMax({a1}, {a2})"));
        if a1 == 0 {
            self.movie.goto_frame(&format!("{PC_MP}.pcMpDodai"), a2.max(1) as u32);
            self.movie.goto_frame(&format!("{PC_MP}.pcMpDodaiC"), a2.max(1) as u32);
        }
    }

    /// `setMP(a1, a2)`.
    pub fn set_mp(&mut self, a1: usize, a2: i32) {
        self.log(format!("setMP({a1}, {a2})"));
        if a1 < STATUS_INFO_MAX {
            self.movie.goto_frame(&self.mp[a1], a2.max(1) as u32);
        }
    }

    /// `loadFace(a1, a2, a3)`: `face` = the loaded face movie (the game passes its path).
    pub fn load_face(&mut self, a1: usize, face: Option<&Arc<MovieDef>>, a3: bool) {
        self.log(format!("loadFace({a1}, {:?}, {a3})", face.map(|f| f.name.as_str())));
        let Some(slot) = self.faces.get_mut(a1) else { return };
        // The AS3 sets enableForm after load(); the load completes asynchronously there, so
        // onLoadComplete sees the new value: set it first.
        slot.enable_form = a3;
        slot.load(&mut self.movie, face);
    }

    /// `setFaceState(a1, a2)`: 0 usually, 1 damage, 2 link.
    pub fn set_face_state(&mut self, a1: usize, a2: i32) {
        self.log(format!("setFaceState({a1}, {a2})"));
        if let Some(face) = self.faces.get_mut(a1) {
            face.set_state(&mut self.movie, a2);
        }
    }

    /// `setFaceLook(a1, a2)`: 0 normal, 1 battle, 2 dying, 3 death.
    pub fn set_face_look(&mut self, a1: usize, a2: i32) {
        self.log(format!("setFaceLook({a1}, {a2})"));
        if let Some(face) = self.faces.get_mut(a1) {
            face.set_look(&mut self.movie, a2);
        }
    }

    /// `setFaceForm(a1, a2)`.
    pub fn set_face_form(&mut self, a1: usize, a2: i32) {
        self.log(format!("setFaceForm({a1}, {a2})"));
        if let Some(face) = self.faces.get_mut(a1) {
            face.set_form(&mut self.movie, a2);
        }
    }

    /// `setLinkEffect(a1, a2)`: `a1` = bit mask of friends, `a2` = face state.
    pub fn set_link_effect(&mut self, a1: u32, a2: i32) {
        self.log(format!("setLinkEffect({a1}, {a2})"));
        self.movie.goto_and_play("sttsPc.pcHpEff", "in");
        self.faces[0].set_state(&mut self.movie, a2);
        for l3 in 0..FRIEND_MAX {
            if a1 & (1 << l3) != 0 {
                self.movie.goto_and_play(&format!("sttsPc.pcHpEff.pcLinkEff.npcLinkEff.npcLinikEffBase{}", l3 + 1), "in");
                self.faces[l3 + 1].set_state(&mut self.movie, a2);
            }
        }
    }

    /// `setMPChargeMode(a1, a2)`.
    pub fn set_mp_charge_mode(&mut self, a1: bool, a2: bool) {
        self.log(format!("setMPChargeMode({a1}, {a2})"));
        let label = match (a1, a2) {
            (true, true) => "charge_loop",
            (true, false) => "charge_in",
            (false, true) => "loop",
            (false, false) => "charge_out",
        };
        self.movie.goto_and_play(PC_MP, label);
    }

    /// `setMPCharge(a1)`.
    pub fn set_mp_charge(&mut self, a1: i32) {
        self.log(format!("setMPCharge({a1})"));
        self.movie.goto_frame(&format!("{PC_MP}.pcMpGgC"), a1.max(1) as u32);
    }

    /// `setPcPartsVisible(a1, a2)`: 0 HP, 1 MP, 2 focus.
    pub fn set_pc_parts_visible(&mut self, a1: i32, a2: bool) {
        self.log(format!("setPcPartsVisible({a1}, {a2})"));
        let part = match a1 {
            0 => "sttsPc.pcHp",
            1 => "sttsPc.pcMp",
            2 => "sttsPc.pcEx",
            _ => return,
        };
        self.movie.goto_and_play(part, if a2 { "on" } else { "off" });
    }

    /// `setEnabledCuisineIcon(a1, a2)`.
    pub fn set_enabled_cuisine_icon(&mut self, a1: bool, a2: bool) {
        self.log(format!("setEnabledCuisineIcon({a1}, {a2})"));
        let label = if !a1 {
            "off"
        } else if a2 {
            "alert"
        } else {
            "loop"
        };
        self.movie.goto_and_play(&format!("{PC_HP_PACK}.stateIcon.cuisine"), label);
    }

    /// `setEnabledCodeMenuIcon(a1, a2)`.
    pub fn set_enabled_code_menu_icon(&mut self, a1: i32, a2: i32, fonts: &Fonts) {
        self.log(format!("setEnabledCodeMenuIcon({a1}, {a2})"));
        let icon = format!("{PC_HP_PACK}.stateIcon");
        self.movie.set_text(&format!("{icon}.blackCodeSet.txBlackCodeCount.textField"), &a2.to_string(), fonts);
        self.movie.set_text(&format!("{icon}.fastPathSet.txFastPathCount.textField"), &a1.to_string(), fonts);
        let label = match (a1 > 0, a2 > 0) {
            (true, true) => "doubleCuisine",
            (true, false) => "fastPathCuisine",
            (false, true) => "blackCodeCuisine",
            (false, false) => "cuisine",
        };
        self.movie.goto_and_play(&icon, label);
    }

    /// Where the red HP gauge of a slot stands (its clip's frame).
    pub fn hp_red_frame(&self, slot: usize) -> u32 {
        self.movie.current_frame(&self.hp_r[slot]).unwrap_or(0)
    }

    pub fn hp_frame(&self, slot: usize) -> u32 {
        self.movie.current_frame(&self.hp_g[slot]).unwrap_or(0)
    }
}

/// What the native driver reads from one pawn each frame (the pawn virtuals of section 4).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct SlotValues {
    /// The pawn exists and is shown.
    pub active: bool,
    pub hp: i32,
    pub hp_max: i32,
    pub mp: i32,
    pub mp_max: i32,
    /// vt+0x17E0.
    pub mp_charging: bool,
    /// vt+0x17F8.
    pub mp_charge_value: f32,
    /// vt+0x1750 (the pawn is in battle).
    pub in_battle: bool,
}

/// What the status HUD reads per frame.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct StatusValues {
    /// Slot 0 = Sora; 1 - 2 friends, 3 - 4 guests.
    pub slots: [SlotValues; STATUS_INFO_MAX],
    pub focus: i32,
    pub focus_max: i32,
    /// vt+0x19D0: focus charge (syphon) mode.
    pub focus_charge: bool,
    /// Shotlock aim open: the pending cost (focus shown = focus - cost, red = unspent). `None`
    /// outside the aim.
    pub focus_consume: Option<i32>,
    /// The game is in battle (`0x140EABA10`).
    pub game_in_battle: bool,
    /// Story flags (game flag byte +0x95B0 bit 2 / bit 4): MP and focus gauges exist.
    pub mp_gauge_unlocked: bool,
    pub focus_gauge_unlocked: bool,
    /// The form id of Sora's portrait (0 = base).
    pub form: i32,
}

#[derive(Clone, Copy, Debug, Default)]
struct Slot {
    active: bool,
    max_hp: i32,
    max_mp: i32,
    hp: i32,
    mp: i32,
    mp_charge: i32,
    hp_max_frame: i32,
    mp_max_frame: i32,
    hp_frame: i32,
    mp_frame: i32,
    hp_frame_sent: i32,
    mp_frame_sent: i32,
    mp_charge_frame: i32,
    mp_charge_sent: i32,
    look_sent: i32,
    look_pending: i32,
    /// +0x210: b0 MP charge, b1 status dirty, b3 HP max dirty, b4 MP max dirty, b5 face-state
    /// dirty, b6 MP-charge-mode dirty, b7 enableForm, b8 loadFace dirty.
    flags: u32,
}

/// The low-HP alarm of the cockpit (`0x14158A340` request, `0x14158A800` tick, `0x14158D800`
/// release): static SE 9 as a loop, started 0.8 s AND 15 ticks after the request.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HpAlarm {
    /// Request bits (+0x258); bit 0 = the status HUD's pinch.
    pub requests: u8,
    /// The component exists (+0x248).
    armed: bool,
    delay: f32,
    ticks: i32,
    pub sounding: bool,
}

impl HpAlarm {
    pub const STATIC_SE: u32 = 9;
    pub const DELAY: f32 = 0.8;
    pub const TICKS: i32 = 15;

    pub fn request(&mut self, bit: u8) {
        if self.requests & bit != 0 {
            return;
        }
        self.requests |= bit;
        if !self.armed {
            self.armed = true;
            self.delay = Self::DELAY;
            self.ticks = Self::TICKS;
        }
    }

    pub fn release(&mut self, bit: u8, sounds: &mut Vec<UiSound>) {
        if self.requests & bit == 0 {
            return;
        }
        self.requests &= !bit;
        if self.requests == 0 {
            if self.sounding {
                sounds.push(UiSound::LoopStop(Self::STATIC_SE));
            }
            *self = HpAlarm::default();
        }
    }

    pub fn tick(&mut self, dt: f32, sounds: &mut Vec<UiSound>) {
        if !self.armed || self.sounding {
            return;
        }
        self.delay -= dt;
        self.ticks -= 1;
        if self.delay <= 0.0 && self.ticks <= 0 {
            self.sounding = true;
            sounds.push(UiSound::LoopStart(Self::STATIC_SE));
        }
    }
}

/// `UTresUIP_HudStatus`: the native side.
#[derive(Clone, Debug)]
pub struct StatusDriver {
    slots: [Slot; STATUS_INFO_MAX],
    max_focus: i32,
    focus: i32,
    /// +0x3E4: the unspent focus while aiming.
    focus_raw: i32,
    focus_frame: i32,
    focus_frame_sent: i32,
    /// +0x3F0 / +0x3F4: the red frame of consume mode.
    consume_frame: i32,
    consume_frame_sent: i32,
    form: i32,
    /// +0x3FC: b0 consume, b1 focus charge, b2 lock, b3 charge dirty, b4 lock dirty, b5 form
    /// dirty, b9 MP part dirty, b10 MP shown, b11 focus part dirty, b12 focus shown.
    flags: u32,
    /// +0x408: b0 solo, b1 solo dirty.
    solo: u32,
    /// +0x40C: the first update has to send the parts.
    init: bool,
    pub alarm: HpAlarm,
    /// The face movie of each slot (the game loads it by the pawn's data asset path).
    pub faces: [Option<Arc<MovieDef>>; STATUS_INFO_MAX],
}

impl Default for StatusDriver {
    fn default() -> Self {
        let slot = Slot { look_sent: -1, look_pending: -1, hp_frame_sent: -1, mp_frame_sent: -1, mp_charge_sent: -1, max_hp: -1, max_mp: -1, hp: -1, mp: -1, mp_charge: -1, ..Slot::default() };
        StatusDriver { slots: [slot; STATUS_INFO_MAX], max_focus: -1, focus: -1, focus_raw: -1, focus_frame: 0, focus_frame_sent: -1, consume_frame: 0, consume_frame_sent: 0, form: 0, flags: 0, solo: 0, init: true, alarm: HpAlarm::default(), faces: Default::default() }
    }
}

impl StatusDriver {
    /// Request of the portrait's damage flash (`setFaceState(slot, 1)`, flag b5). The game sets
    /// the bit from its damage path, which the UI spec does not trace: the host calls this when
    /// the slot's pawn takes a hit.
    pub fn notify_damage(&mut self, slot: usize) {
        if let Some(s) = self.slots.get_mut(slot) {
            s.flags |= 0x20;
        }
    }

    /// (party P) The slot's face movie was replaced (the leader switch): the face is loaded again and its look and
    /// gauges sent again at the next update, as when the slot's pawn changes.
    pub fn reload_face(&mut self, slot: usize) {
        if let Some(s) = self.slots.get_mut(slot) {
            s.look_sent = -1;
            s.flags |= 2 | 8;
            s.max_hp = -1;
            s.max_mp = -1;
            s.hp = -1;
            s.mp = -1;
            if s.active {
                s.flags |= 0x100;
            }
        }
    }

    /// The cached frames (clip frames, `F + 1`): (HP base, HP, MP base, MP, focus).
    pub fn frames(&self) -> (i32, i32, i32, i32, i32) {
        let s = &self.slots[0];
        (s.hp_max_frame + 1, s.hp_frame + 1, s.mp_max_frame + 1, s.mp_frame + 1, self.focus_frame + 1)
    }

    /// `0x1416608B0` then `0x141662640`: one update of the part. `dt` runs the alarm.
    pub fn update(&mut self, hud: &mut HudStatus, v: &StatusValues, dt: f32, sounds: &mut Vec<UiSound>) {
        if !v.slots[0].active {
            // No player pawn: the alarm request is released and nothing else happens.
            self.alarm.release(1, sounds);
            self.alarm.tick(dt, sounds);
            return;
        }
        if self.init {
            // +0x40C: parts visibility from the story flags; b9 | b11 dirty (and b13: code icons).
            self.flags = (self.flags & !0x1400) | ((v.mp_gauge_unlocked as u32) << 10) | ((v.focus_gauge_unlocked as u32) << 12) | 0x0A00;
            self.init = false;
        } else {
            for (bit, shown) in [(10, v.mp_gauge_unlocked), (12, v.focus_gauge_unlocked)] {
                if (self.flags >> bit) & 1 != shown as u32 {
                    self.flags = (self.flags & !(1 << bit)) | ((shown as u32) << bit) | (1 << (bit - 1));
                }
            }
        }
        let mut friends = 0;
        for i in 0..STATUS_INFO_MAX {
            let (pawn, s) = (&v.slots[i], &mut self.slots[i]);
            if pawn.active != s.active {
                // The slot's pawn changed: status dirty; a new pawn's face is loaded.
                s.look_sent = -1;
                s.flags = (s.flags & !0x80) | 2;
                s.active = pawn.active;
                if pawn.active {
                    // b8 loadFace dirty, b7 enableForm (Sora's portrait has form frames).
                    s.flags |= 0x100 | if i == 0 { 0x80 } else { 0 };
                } else {
                    s.flags &= !0x100;
                    if i == 0 {
                        self.alarm.release(1, sounds);
                    }
                }
            }
            if !pawn.active {
                continue;
            }
            friends += (i != 0) as u32;
            if s.max_hp != pawn.hp_max {
                s.flags |= 8;
                s.max_hp = pawn.hp_max;
                s.hp_max_frame = gauge_frame(pawn.hp_max, pawn.hp_max, (pawn.hp_max as f32 + pawn.hp_max as f32) as i32).clamp(0, 668);
                debug_assert_eq!(s.hp_max_frame, hp_max_frames(pawn.hp_max).max(0));
                s.hp = -1;
            }
            let mut death = false;
            let mut pinch = false;
            if s.hp != pawn.hp {
                s.hp = pawn.hp;
                s.hp_frame = if i == 0 { gauge_frame(pawn.hp, pawn.hp_max, s.hp_max_frame).clamp(0, 668) } else { gauge_frame(pawn.hp, pawn.hp_max, 180) };
            }
            if pawn.hp < 1 && i != 0 {
                death = true;
            } else {
                pinch = is_pinch(pawn.hp, pawn.hp_max);
            }
            if i == 0 {
                if pinch {
                    self.alarm.request(1);
                } else {
                    self.alarm.release(1, sounds);
                }
            }
            if s.max_mp != pawn.mp_max {
                s.flags |= 0x10;
                s.max_mp = pawn.mp_max;
                s.mp_max_frame = gauge_frame(pawn.mp_max, pawn.mp_max, (pawn.mp_max as f32 * 1.8) as i32).clamp(0, 500);
                debug_assert_eq!(s.mp_max_frame, mp_max_frames(pawn.mp_max).max(0));
                s.mp = -1;
                s.mp_charge = -1;
            }
            if (s.flags & 1) != pawn.mp_charging as u32 {
                s.flags = (s.flags & !1) | pawn.mp_charging as u32 | 0x40;
            }
            if pawn.mp_charging {
                let charge = pawn.mp_charge_value as i32;
                if s.mp_charge != charge {
                    s.mp_charge = charge;
                    s.mp_charge_frame = if i == 0 { gauge_frame(charge, pawn.mp_max, s.mp_max_frame).clamp(0, 500) } else { gauge_frame(charge, pawn.mp_max, FRIEND_MP_FRAMES) /* (RC) */ };
                }
            }
            if s.mp != pawn.mp {
                s.mp = pawn.mp;
                s.mp_frame = if i == 0 { gauge_frame(pawn.mp, pawn.mp_max, s.mp_max_frame).clamp(0, 500) } else { gauge_frame(pawn.mp, pawn.mp_max, FRIEND_MP_FRAMES) /* (RC) */ };
            }
            let look = if death {
                FACE_LOOK_DEATH
            } else if pinch {
                FACE_LOOK_DYING
            } else if pawn.in_battle || v.game_in_battle {
                FACE_LOOK_BATTLE
            } else {
                FACE_LOOK_NORMAL
            };
            if s.look_sent != look {
                s.look_pending = look;
            }
        }
        // Sora's form (portrait frames).
        if self.form != v.form {
            self.form = v.form;
            self.flags |= 0x20;
        }
        // Focus.
        let mut focus = v.focus;
        let before = self.flags;
        if (self.flags >> 1) & 1 != v.focus_charge as u32 {
            self.flags = (self.flags & !2) | ((v.focus_charge as u32) << 1) | 8;
        }
        self.flags &= !1;
        if let Some(cost) = v.focus_consume.filter(|c| *c >= 0) {
            if v.focus != self.focus_raw {
                self.focus_raw = v.focus;
                self.consume_frame = gauge_frame(v.focus, self.max_focus.max(v.focus_max), 100);
            }
            self.flags |= 1;
            focus -= cost;
        } else if before & 1 != 0 {
            // Leaving consume mode: the red gauge starts from the unspent value.
            self.focus_frame_sent = gauge_frame(self.focus_raw, self.max_focus, 100);
        }
        if v.focus_max != self.max_focus || focus != self.focus {
            self.max_focus = v.focus_max;
            self.focus = focus;
            self.focus_frame = gauge_frame(focus, v.focus_max, 100);
        }
        // Solo: Sora without friends (+0x408).
        let solo = (friends == 0) as u32;
        if self.solo & 1 != solo {
            self.solo = solo | 2;
        }
        self.alarm.tick(dt, sounds);
        self.flush(hud);
    }

    /// `0x141662640`: what changed goes to the movie.
    fn flush(&mut self, hud: &mut HudStatus) {
        if self.solo & 2 != 0 {
            hud.set_solo(self.solo & 1 != 0);
            self.solo &= !2;
            if self.slots[0].active && self.slots[0].flags & 2 == 0 {
                // change2 = to the solo layout, change1 = to the party layout.
                hud.set_status(if self.solo & 1 != 0 { 12 } else { 13 }, 0);
            }
        }
        for i in 0..STATUS_INFO_MAX {
            let s = &mut self.slots[i];
            let status_dirty = s.flags & 2 != 0;
            if status_dirty {
                if !s.active {
                    hud.set_status(11, i as i32);
                } else {
                    if s.flags & 0x100 != 0 {
                        s.flags &= !0x120;
                        hud.load_face(i, self.faces[i].as_ref(), s.flags & 0x80 != 0);
                    }
                    hud.set_status(10, i as i32);
                }
                s.flags &= !2;
            }
            if !s.active {
                continue;
            }
            if s.flags & 8 != 0 {
                hud.set_hp_max(i, s.hp_max_frame + 1);
                s.flags &= !8;
            }
            if s.hp_frame_sent != s.hp_frame {
                if s.hp_max_frame < s.hp_frame_sent {
                    s.hp_frame_sent = s.hp_max_frame;
                }
                let mut red = s.hp_frame_sent;
                if red < s.hp_frame || status_dirty {
                    red = s.hp_frame;
                }
                hud.set_hp(i, s.hp_frame + 1, red + 1);
                s.hp_frame_sent = s.hp_frame;
            }
            if s.flags & 0x20 != 0 {
                hud.set_face_state(i, FACE_STATE_DAMAGE);
                s.flags &= !0x20;
            }
            if s.look_pending != -1 {
                hud.set_face_look(i, s.look_pending);
                s.look_sent = s.look_pending;
                s.look_pending = -1;
            }
            if s.flags & 0x10 != 0 {
                hud.set_mp_max(i, s.mp_max_frame + 1);
                s.flags &= !0x10;
            }
            if s.mp_frame_sent != s.mp_frame {
                hud.set_mp(i, s.mp_frame + 1);
                s.mp_frame_sent = s.mp_frame;
            }
            if i == 0 {
                if s.flags & 1 != 0 && s.mp_charge_sent != s.mp_charge_frame {
                    hud.set_mp_charge(s.mp_charge_frame + 1);
                    s.mp_charge_sent = s.mp_charge_frame;
                }
                if s.flags & 0x40 != 0 {
                    s.flags &= !0x40;
                    hud.set_mp_charge_mode(s.flags & 1 != 0, false);
                    if s.flags & 1 == 0 {
                        hud.set_mp_charge(0);
                        s.mp_charge_sent = -1;
                    }
                }
            }
        }
        if self.flags & 0x20 != 0 {
            self.flags &= !0x20;
            hud.set_face_form(0, self.form);
        }
        if self.flags & 8 != 0 {
            self.flags &= !8;
            hud.set_focus_charge_mode(self.flags & 2 != 0, false);
        }
        if self.focus_frame != self.focus_frame_sent || self.consume_frame != self.consume_frame_sent {
            if self.flags & 1 == 0 {
                let red = self.focus_frame.max(self.focus_frame_sent);
                hud.set_focus_gauge(self.focus_frame + 1, red + 1, self.focus == self.max_focus, false);
            } else {
                hud.set_focus_gauge(self.focus_frame + 1, self.consume_frame + 1, self.consume_frame == self.max_focus, true);
            }
            self.focus_frame_sent = self.focus_frame;
            self.consume_frame_sent = self.consume_frame;
        }
        if self.flags & 0x10 != 0 {
            self.flags &= !0x10;
            hud.set_focus_gauge_lock(self.flags & 4 != 0);
        }
        if self.flags & 0x200 != 0 {
            self.flags &= !0x200;
            hud.set_pc_parts_visible(1, self.flags & 0x400 != 0);
        }
        if self.flags & 0x800 != 0 {
            self.flags &= !0x800;
            hud.set_pc_parts_visible(2, self.flags & 0x1000 != 0);
        }
    }
}
