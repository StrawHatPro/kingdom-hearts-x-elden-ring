//! (P8 MOOGLE NPC, documentation/KH3_WEAPONS_PLAN.md 1.5; state note build/sora_tmp/weapons/P8/STATE.md) KH3'S SYNTHESIS
//! MOOGLE AT ROUNDTABLE HOLD. UNTESTED IN THE GAME.
//!
//! THE USER (2026-11-03): spot A is approved (m11_10_00_00, (-290.5, -22.61, -306.5), rot_y 150, the corridor wall across
//! from Hewg); "The moogle in the round table is only for synthesis": talking to him opens KH3's Workshop directly, no Buy /
//! Sell / M.O.G. Card.
//!
//! WHAT RUNS (sheet `sheets/port/kh-moogle.json`, cargo feature `party`, key `moogle.on` default 1):
//! - DRAWN BY OUR RENDERER, like Donald at the Finger Reader: (MOOGLE-HOVER, THE USER 2026-11-05: "The moogle should look and
//!   animate exactly like the one in Twilight Town") KH3's TWILIGHT TOWN Moogle: n_ex014 of the gimmick g_ex_mogli_01 (tt_01_npc
//!   g_tt_SynthesisShop87, TresShopActor TT_MOOGLE1; sheet rows moogle.model.tt / moogle.anim.idle.tt, pack PACK/party/n_ex014,
//!   draw id kh-er-render `party::MOOGLE_TT`; older builds read moogle.model: the field Moogle n_ex028, whose own materials
//!   carry KH3's CRT screen-RGB emissive, the honeycomb the user saw), his mesh `hover_cm` up (105 cm, the user's choice: his
//!   head centre level with Sora's; KH3 has him 85 cm above the street in Twilight Town), handed to the renderer's party path
//!   (his own translated shaders with the hybrid set, the game's light on him through his own probe, his shadow, his
//!   velocity), only while the player is in Roundtable Hold (the map of the sheet's spot) within `moogle.range.show`.
//!   (MOOGLE-HOVER) His swaying hover M_001 loops, as KH3's ATresShopActor plays its m_DefaultIdlingMotName (0x140F9A330,
//!   looped), and no other clip; his Bonamik (kh-anim, the KH3 asset: the antenna hair1..3, the pom-pom fusen1 / fusen2,
//!   both wings) runs on fixed 60 Hz steps, drawn between the two latest steps.
//!   No collision: nothing of the game is made for him.
//! - TALKED TO: the player in the hold, within `moogle.range.talk` (2.5 m, Elden Ring's own Talk radius) and `.height` of
//!   his spot and facing him (`.facing`, 70 degrees): KH3's green "Talk" reaction command (kh-er conv::offer_talk, Y on an
//!   Xbox pad), never while Elden Ring's own action prompt is up, a KH menu or a conversation is up, or in a fight. Y: his
//!   voice (KH3's shop voice rows: TalkWelcome, or TalkNewArrival while the Workshop has a new recipe), no clip (KH3's shop actor keeps its idle),
//!   and KH3's Workshop opened on its list (P6b; the world stands while it is open, as for the KH shop: hud.rs pause);
//!   closed: back to play. Not open within `moogle.prompt.open.open_within_s`: given up (logged).
//!
//! KEYS: `moogle.on` (1; 0 = no Moogle), `moogle.log` (0; 1 = his facts once a second). Logs: `moogle:` lines.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use glam::{Quat, Vec3};
use kh_anim::Trs;
use kh_core::{PadButton, PadButtons};

use crate::control::Control;
use crate::coords;
use crate::erlog;

/// The talk owner of the conversation system's party-talk offers (conv::offer_talk).
pub const OWNER: &str = "moogle";
/// The audio handle of his voice (the UI sounds' path; a range of ours).
const VOICE_HANDLE: u32 = 0x7E00_0300;
/// His pose steps (s): fixed, the frame draws between the two latest.
const STEP: f32 = 1.0 / 60.0;

// ------------------------------------------------------------------------------------------------------------- the rules

/// THE PURE RULES (sheet, geometry, the talk's phases, the voice): tested below, no game state.
pub mod rules {
    use glam::{Vec2, Vec3};
    use serde_json::Value;

    pub const SHEET_FILE: &str = "kh-moogle.json";

    /// The sheet's rows the host reads.
    #[derive(Clone, Debug, PartialEq)]
    pub struct Sheet {
        /// `mAA_BB_CC_DD` of the spot.
        pub map: String,
        /// The spot in the map's own coordinates (Elden Ring metres; y = the navmesh floor).
        pub pos: Vec3,
        /// His facing as a KH3 yaw (kh-er coords.rs `facing_er`: (sin, cos) in X / Z).
        pub rot_y: f32,
        pub approved: bool,
        /// PACK-relative folder of his pack and his draw id (kh-er-render party.rs MOOGLE).
        pub pack_dir: String,
        pub draw_id: u8,
        /// (sequence, length s) of the idle and of the talk row; the blend from the talk back to the idle.
        pub idle: (String, f32),
        /// (P8 fix) None: no talk row (KH3's floating Moogle keeps floating while he talks).
        pub talk: Option<(String, f32)>,
        pub talk_blend: f32,
        /// (P8 fix) His mesh above the spot's floor (cm). (MOOGLE-HOVER) 103: the user's choice (his head at Sora's head);
        /// KH3's TresGimmickMesh0 RelativeLocation z of g_ex_mogli_02 is 75 (the sheet's `hover_cm_kh3`).
        pub hover_cm: f32,
        pub welcome: Vec<String>,
        pub new_arrival: Vec<String>,
        pub show_m: f32,
        pub talk_m: f32,
        pub height_m: f32,
        pub facing_deg: f32,
        /// (part, distance m, prompt radius m) of the nearest Elden Ring talk parts.
        pub clear_of: Vec<(String, f32, f32)>,
        pub open_within_s: f32,
    }

    fn row<'a>(rows: &'a [Value], id: &str) -> Option<&'a Value> {
        rows.iter().find(|r| r.get(0).and_then(Value::as_str) == Some(id)).and_then(|r| r.get(1))
    }

    fn num(v: Option<&Value>) -> Option<f32> {
        v.and_then(Value::as_f64).map(|x| x as f32)
    }

    fn strings(v: Option<&Value>) -> Vec<String> {
        v.and_then(Value::as_array).into_iter().flatten().filter_map(|s| s.as_str().map(str::to_string)).collect()
    }

    impl Sheet {
        /// The sheet's text (`sheets/port/kh-moogle.json`). An error names the missing row.
        pub fn from_text(text: &str) -> Result<Sheet, String> {
            let json: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| e.to_string())?;
            let rows = json.get("rows").and_then(Value::as_array).ok_or("no rows")?;
            let need = |id: &str| row(rows, id).ok_or(format!("no row {id}"));
            let spot = need("moogle.spot")?;
            let p: Vec<f32> = spot.get("pos").and_then(Value::as_array).into_iter().flatten().filter_map(|x| x.as_f64().map(|x| x as f32)).collect();
            if p.len() != 3 {
                return Err("moogle.spot: pos is not three numbers".to_string());
            }
            // (MOOGLE-HOVER, THE USER 2026-11-05: "The moogle should look and animate exactly like the one in Twilight Town") the
            // Twilight Town Moogle's rows (n_ex014 of g_ex_mogli_01, his own materials, his swaying M_001) when the sheet has them;
            // the older rows stay for the builds that read them (n_ex028)
            let model = match row(rows, "moogle.model.tt") {
                Some(tt) => tt,
                None => need("moogle.model")?,
            };
            let idle = match row(rows, "moogle.anim.idle.tt") {
                Some(tt) => tt,
                None => need("moogle.anim.idle")?,
            };
            let talk = need("moogle.anim.talk")?;
            let seq = |v: &Value| (v.get("sequence").and_then(Value::as_str).unwrap_or("").to_string(), num(v.get("length_s")).unwrap_or(1.0));
            let talk_seq = talk.get("sequence").and_then(Value::as_str).map(|q| (q.to_string(), num(talk.get("length_s")).unwrap_or(1.0)));
            let clear_of = row(rows, "moogle.range.clear_of").and_then(Value::as_array).into_iter().flatten().filter_map(|c| Some((c.get("part")?.as_str()?.to_string(), num(c.get("distance_m"))?, num(c.get("prompt_m"))?))).collect();
            Ok(Sheet {
                map: spot.get("map").and_then(Value::as_str).ok_or("moogle.spot: no map")?.to_string(),
                pos: Vec3::new(p[0], p[1], p[2]),
                rot_y: num(spot.get("rot_y")).unwrap_or(0.0),
                approved: spot.get("approved").and_then(Value::as_bool) == Some(true),
                pack_dir: model.get("pack_dir").and_then(Value::as_str).unwrap_or("party/n_ex028").to_string(),
                draw_id: model.get("draw_id").and_then(Value::as_u64).unwrap_or(8) as u8,
                idle: seq(idle),
                talk: talk_seq,
                hover_cm: num(model.get("hover_cm")).unwrap_or(0.0),
                talk_blend: num(talk.get("blend_s")).unwrap_or(0.2),
                welcome: strings(row(rows, "moogle.sound.welcome")),
                new_arrival: strings(row(rows, "moogle.sound.new_arrival")),
                show_m: num(row(rows, "moogle.range.show")).unwrap_or(30.0),
                talk_m: num(row(rows, "moogle.range.talk")).unwrap_or(2.5),
                height_m: num(row(rows, "moogle.range.height")).unwrap_or(2.0),
                facing_deg: num(row(rows, "moogle.range.facing")).unwrap_or(70.0),
                clear_of,
                open_within_s: row(rows, "moogle.prompt.open").and_then(|o| num(o.get("open_within_s"))).unwrap_or(3.0),
            })
        }

        /// His facing in Elden Ring axes (unit, horizontal).
        pub fn facing_er(&self) -> Vec3 {
            crate::coords::facing_er(self.rot_y)
        }

        /// The talk range is clear of every Elden Ring talk prompt the sheet lists: nowhere are both prompts up.
        pub fn prompt_clear(&self) -> bool {
            self.clear_of.iter().all(|(_, d, r)| self.talk_m + r < *d)
        }
    }

    /// `mAA_BB_CC_DD` -> (area, BB, CC).
    pub fn parse_map(name: &str) -> Option<(u8, i32, i32)> {
        let parts: Vec<&str> = name.trim_start_matches('m').split('_').collect();
        if parts.len() < 3 {
            return None;
        }
        Some((parts[0].parse().ok()?, parts[1].parse().ok()?, parts[2].parse().ok()?))
    }

    /// The spot (map-local) in this frame's physics space, for a player at `player_local` of block `player_block` whose
    /// physics position is `player_physics`: None in another map (a legacy dungeon is one block: the same block only; the
    /// open world's tiles are 256 m). The same rule as the unlock scenes' (unlock/rules.rs `local_to_physics`).
    pub fn spot_physics(local: Vec3, map: (u8, i32, i32), player_block: (u8, i32, i32), overworld: bool, player_local: Vec3, player_physics: Vec3) -> Option<Vec3> {
        if map.0 != player_block.0 {
            return None;
        }
        let (dx, dz) = (map.1 - player_block.1, map.2 - player_block.2);
        if !overworld && (dx != 0 || dz != 0) {
            return None;
        }
        let shifted = Vec3::new(local.x + 256.0 * dx as f32, local.y, local.z + 256.0 * dz as f32);
        let p = player_physics + (shifted - player_local);
        p.is_finite().then_some(p)
    }

    /// (horizontal distance m, height difference m, the player faces him within `facing_deg`): Elden Ring axes; `facing`
    /// is the player's facing (horizontal). Right on top of him counts as facing.
    pub fn talk_geometry(player: Vec3, facing: Vec3, moogle: Vec3, facing_deg: f32) -> (f32, f32, bool) {
        let d = Vec2::new(moogle.x - player.x, moogle.z - player.z);
        let dist = d.length();
        let f = Vec2::new(facing.x, facing.z);
        let faces = dist < 0.2 || (f.length() > 1e-4 && d.normalize().dot(f.normalize()) >= facing_deg.to_radians().cos());
        (dist, moogle.y - player.y, faces)
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum Phase {
        /// Not drawn (another map, too far, hidden).
        Away,
        /// Drawn, no prompt.
        Shown,
        /// The "Talk" prompt is up.
        Prompt,
        /// Y was pressed; the Workshop was asked for and is awaited (seconds waited).
        Opening(f32),
        /// The Workshop is open.
        Open,
    }

    /// One step's facts.
    #[derive(Clone, Copy, Debug, Default)]
    pub struct Facts {
        /// The player is in the spot's map.
        pub in_hold: bool,
        /// A cutscene, a loading screen, the game over, no player.
        pub hidden: bool,
        /// 3D distance, horizontal distance, height difference (m), facing him.
        pub dist3: f32,
        pub dist: f32,
        pub dy: f32,
        pub facing: bool,
        /// Something else has the player: Elden Ring's own prompt, a KH menu, a conversation, a fight.
        pub blocked: bool,
        /// Y went down while the prompt was up.
        pub talk_pressed: bool,
        /// The Workshop is open now.
        pub workshop_open: bool,
        pub dt: f32,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Act {
        /// Start drawing him (load his pose on first use).
        Show,
        /// Stop drawing him.
        Hide,
        PromptOn,
        PromptOff,
        /// Y: his voice, his talk row, the Workshop asked for.
        Talk,
        /// The Workshop came up.
        Opened,
        /// The Workshop never came up: back to play.
        GiveUp,
        /// The Workshop closed: back to play.
        Closed,
    }

    /// (P8 fix) He is hidden only after this long out of sight / out of range / hidden: a frame without the player (a load
    /// step, a one-frame cutscene flag) never blinks him off and on.
    pub const HIDE_AFTER_S: f32 = 0.5;

    #[derive(Clone, Debug, PartialEq)]
    pub struct Director {
        pub phase: Phase,
        /// (P8 fix) Seconds not visible while shown.
        pub gone: f32,
    }

    impl Default for Director {
        fn default() -> Director {
            Director { phase: Phase::Away, gone: 0.0 }
        }
    }

    impl Director {
        pub fn visible(s: &Sheet, f: &Facts) -> bool {
            f.in_hold && !f.hidden && f.dist3 <= s.show_m
        }

        pub fn can_prompt(s: &Sheet, f: &Facts) -> bool {
            Director::visible(s, f) && f.dist <= s.talk_m && f.dy.abs() <= s.height_m && f.facing && !f.blocked
        }

        /// One step of the talk. The host carries out the acts in order.
        pub fn step(&mut self, s: &Sheet, f: &Facts) -> Vec<Act> {
            let mut acts = Vec::new();
            match self.phase {
                Phase::Away => {
                    if Director::visible(s, f) {
                        self.phase = Phase::Shown;
                        acts.push(Act::Show);
                        if Director::can_prompt(s, f) {
                            self.phase = Phase::Prompt;
                            acts.push(Act::PromptOn);
                        }
                    }
                }
                Phase::Shown | Phase::Prompt => {
                    let was_prompt = self.phase == Phase::Prompt;
                    if !Director::visible(s, f) {
                        if was_prompt {
                            acts.push(Act::PromptOff);
                            self.phase = Phase::Shown;
                        }
                        self.gone += f.dt;
                        if self.gone >= HIDE_AFTER_S {
                            acts.push(Act::Hide);
                            self.phase = Phase::Away;
                            self.gone = 0.0;
                        }
                    } else if was_prompt && f.talk_pressed && Director::can_prompt(s, f) {
                        self.gone = 0.0;
                        acts.extend([Act::PromptOff, Act::Talk]);
                        self.phase = Phase::Opening(0.0);
                    } else {
                        self.gone = 0.0;
                        if Director::can_prompt(s, f) != was_prompt {
                            acts.push(if was_prompt { Act::PromptOff } else { Act::PromptOn });
                            self.phase = if was_prompt { Phase::Shown } else { Phase::Prompt };
                        }
                    }
                }
                Phase::Opening(waited) => {
                    if f.workshop_open {
                        self.phase = Phase::Open;
                        acts.push(Act::Opened);
                    } else if waited + f.dt > s.open_within_s || f.hidden {
                        self.phase = Phase::Shown;
                        acts.push(Act::GiveUp);
                    } else {
                        self.phase = Phase::Opening(waited + f.dt);
                    }
                }
                Phase::Open => {
                    if !f.workshop_open {
                        self.phase = Phase::Shown;
                        acts.push(Act::Closed);
                    }
                }
            }
            acts
        }

        /// The Workshop refused the request outright (not in this build, another menu up): back to play at once.
        pub fn refused(&mut self) -> Vec<Act> {
            if matches!(self.phase, Phase::Opening(_)) {
                self.phase = Phase::Shown;
                return vec![Act::GiveUp];
            }
            Vec::new()
        }

        pub fn prompt_up(&self) -> bool {
            self.phase == Phase::Prompt
        }

        pub fn drawn(&self) -> bool {
            self.phase != Phase::Away
        }
    }

    /// (P8 fix) Seconds a different place must hold before he moves to it, and how different (cm).
    pub const RELATCH_CM: f32 = 100.0;
    pub const RELATCH_S: f32 = 0.25;

    /// (P8 fix, THE USER 2026-11-03: "the moogle in round table hold is spazzing out, his actual position should be stationary")
    /// HIS PLACE, TAKEN ONCE AND KEPT. The spot's KH3 place is worked out every frame from the player's physics position and
    /// his block-local position, which the game does not update together: their mismatch is the player's own motion of a
    /// frame, so the place worked out each frame shook by up to a frame of the player's running, and the Moogle and his
    /// Bonamik (pom-pom, wings, balloon) with it. Now the first place is kept; a frame without a place keeps it; only a
    /// place more than `RELATCH_CM` away that holds for `RELATCH_S` (a new KH3 frame after a load, a real move of the
    /// physics origin) moves him, at once and whole. KH3 space is continuous over the open world's origin shifts
    /// (coords.rs Frame::rebase), so a shift never moves him either.
    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub struct Anchor {
        pub at: Option<Vec3>,
        /// (the other place, seconds it has held)
        pending: Option<(Vec3, f32)>,
    }

    impl Anchor {
        pub const fn new() -> Anchor {
            Anchor { at: None, pending: None }
        }

        /// This frame's worked-out place (None: none this frame), `dt` s of the frame: the kept place.
        pub fn update(&mut self, candidate: Option<Vec3>, dt: f32) -> Option<Vec3> {
            match (self.at, candidate) {
                (None, Some(c)) => self.at = Some(c),
                (Some(at), Some(c)) if (c - at).length() > RELATCH_CM => {
                    let held = match self.pending {
                        Some((p, t)) if (c - p).length() <= RELATCH_CM => t + dt.max(0.0),
                        _ => 0.0,
                    };
                    if held >= RELATCH_S {
                        self.at = Some(c);
                        self.pending = None;
                    } else {
                        self.pending = Some((self.pending.filter(|(p, _)| (c - *p).length() <= RELATCH_CM).map_or(c, |(p, _)| p), held));
                    }
                }
                (Some(_), Some(_)) => self.pending = None,
                _ => {}
            }
            self.at
        }

        pub fn clear(&mut self) {
            *self = Anchor::default();
        }

        /// (SYNTH-MOOGLE) The kept place lives in Elden Ring's PHYSICS space (KH3 axes and cm, `coords::dir_er_to_kh` of
        /// the physics position): Sora's KH3 frame also moves under him when the game carries the body (sora.rs `Follow`,
        /// rides, warps), and a place kept in KH3 space was carried along with it (then put back a metre later). The
        /// physics origin itself moving (`moved`, the KH3 frame's own move this frame, in the same units) moves every
        /// physics position alike: when the new candidate is the kept place moved by just that, the place follows it at
        /// once (no `RELATCH_S` wait at the old, wrong place).
        pub fn follow_origin_shift(&mut self, candidate: Option<Vec3>, moved: Vec3) {
            if let (Some(at), Some(c)) = (self.at, candidate) {
                if moved.length() > RELATCH_CM && (c - at).length() > RELATCH_CM && (c - (at + moved)).length() <= RELATCH_CM {
                    self.at = Some(at + moved);
                    self.pending = None;
                }
            }
        }
    }

    /// (SYNTH-MOOGLE) ONE FRAME OF HIS PLACE: `candidate_er` the spot worked out this frame (physics space, None: none),
    /// `keep` he is drawn (a frame without a candidate keeps the place). The place is kept in physics space (KH3 axes and cm,
    /// `Anchor::follow_origin_shift`) and handed back in this frame's KH3 space: his floor place in KH3 space.
    pub fn kept_place(anchor: &mut Anchor, last_origin: &mut Option<Vec3>, frame: &crate::coords::Frame, candidate_er: Option<Vec3>, keep: bool, dt: f32) -> Option<Vec3> {
        let origin = crate::coords::dir_er_to_kh(frame.origin);
        let moved = last_origin.map_or(Vec3::ZERO, |o| origin - o);
        *last_origin = Some(origin);
        let candidate = candidate_er.map(crate::coords::dir_er_to_kh);
        anchor.follow_origin_shift(candidate, moved);
        if candidate.is_some() || keep { anchor.update(candidate, dt) } else { None }.map(|abs| abs - origin)
    }

    /// THE VOICE (KH3's shop voice rows): TalkNewArrival while the Workshop has something new, else TalkWelcome; one of the
    /// list by `seed`, never the line that played last when the list has another. (list, index).
    pub fn pick_voice(s: &Sheet, new_arrival: bool, last: Option<&str>, seed: u32) -> Option<String> {
        let list = if new_arrival && !s.new_arrival.is_empty() { &s.new_arrival } else { &s.welcome };
        if list.is_empty() {
            return None;
        }
        let mut k = seed as usize % list.len();
        if list.len() > 1 && last == Some(list[k].as_str()) {
            k = (k + 1) % list.len();
        }
        Some(list[k].clone())
    }
}

// ------------------------------------------------------------------------------------------------------------- the pose

/// His pose: kh-anim's Animator on his pack (Bonamik included), fixed steps, the latest two kept.
struct Poser {
    animator: kh_anim::Animator,
    /// The row playing: (sequence, length, looping) and its time.
    clip: (String, f32, bool),
    time: f32,
    /// The row it came from, its time and the blend left (s) (the talk row back into the idle).
    from: Option<(String, f32, f32)>,
    blend: f32,
    previous: kh_anim::Pose,
    current: kh_anim::Pose,
    accumulator: f32,
    steps: u64,
    /// (SYNTH-MOOGLE) His own space, the same for his whole life: the component at his hover above a floor at 0, turned by
    /// his yaw. KH3's shop actor never moves, so his Bonamik (antenna, pom-pom, wings) must never see a move either: the
    /// steps run here, never at his place in Sora's KH3 frame (that frame moves under him when the game carries the body,
    /// and a re-latched place jumped by a metre: the pom-pom whipped over his face). The pose is component space: the draw
    /// puts it at his place.
    space: Trs,
}

/// (SYNTH-MOOGLE) Seconds of steps his Poser runs before he is first drawn: KH3's actor is in the level from its load and
/// its Bonamik has long settled when the player sees it; a fresh Bonamik starts on the reference pose and the pom-pom falls
/// about 30 cm into his face over the first second (the trace build/sora_tmp/moogle_jerk/trace_before_60.csv, 0 .. 1.5 s).
pub const PREROLL_S: f32 = 4.0;

impl Poser {
    fn load(dir: &Path, s: &rules::Sheet) -> Result<Poser, String> {
        Poser::load_with(dir, s, true)
    }

    /// `bonamik` false: the clips alone (the tests' reference). Settled: `PREROLL_S` of steps already run.
    fn load_with(dir: &Path, s: &rules::Sheet, bonamik: bool) -> Result<Poser, String> {
        let mut p = Poser::unsettled(dir, s, bonamik)?;
        for _ in 0..(PREROLL_S / STEP).round() as u32 {
            p.step(s);
        }
        p.previous = p.current.clone();
        Ok(p)
    }

    /// A Poser on its first step (a fresh Bonamik on the reference pose): what every DEPLOY3 thread made.
    fn unsettled(dir: &Path, s: &rules::Sheet, bonamik: bool) -> Result<Poser, String> {
        let pack = kh_anim::Pack::load(dir)?;
        let animator = kh_anim::Animator::new(pack, kh_anim::Settings { leg_ik: false, bonamik, ..kh_anim::Settings::default() })?;
        if bonamik && animator.bonamik().is_none() {
            return Err("his pack has no Bonamik (data/bonamik.json)".to_string());
        }
        let pose = animator.pose().clone();
        let space = component_to_world(Vec3::Z * s.hover_cm, s.rot_y);
        Ok(Poser { animator, clip: (s.idle.0.clone(), s.idle.1.max(0.1), true), time: 0.0, from: None, blend: 0.0, previous: pose.clone(), current: pose, accumulator: 0.0, steps: 0, space })
    }

    /// The talk row once, blended in over `blend` s from what plays, then back to the idle (blended the same way).
    fn play_once(&mut self, row: &(String, f32), blend: f32) {
        let blend = blend.max(STEP);
        self.from = Some((self.clip.0.clone(), self.time, blend));
        self.clip = (row.0.clone(), row.1.max(0.1), false);
        self.time = 0.0;
        self.blend = blend;
    }

    /// The fixed steps of this frame (`dt` s of the game's frame), in his own space (`space`).
    fn advance(&mut self, dt: f32, s: &rules::Sheet) {
        self.accumulator = (self.accumulator + dt.clamp(0.0, 0.25)).min(STEP * 8.0);
        while self.accumulator >= STEP {
            self.accumulator -= STEP;
            self.step(s);
        }
    }

    fn step(&mut self, s: &rules::Sheet) {
        let (c2w, floor) = (self.space, Vec3::ZERO);
        let first = self.steps == 0;
        let from_t = if first { -1.0 } else { self.time };
        self.time += STEP;
        let (seq, len, looping) = self.clip.clone();
        let mut window = (from_t, self.time);
        if self.time >= len {
            if looping {
                self.time -= len;
                window = (from_t, self.time); // a wrap: `to < from`
            } else {
                // the talk row ended: back to the idle, the talk row blended out
                self.from = Some((seq.clone(), len, s.talk_blend.max(STEP)));
                self.clip = (s.idle.0.clone(), s.idle.1.max(0.1), true);
                self.time = 0.0;
                self.blend = s.talk_blend.max(STEP);
                window = (-1.0, 0.0);
            }
        }
        // the row it came from, blended out
        let blend_with = match self.from.as_mut() {
            Some((name, t, total)) if self.blend > 0.0 => {
                *t += STEP;
                self.blend -= STEP;
                Some((name.clone(), *t, (self.blend / total.max(STEP)).clamp(0.0, 1.0)))
            }
            _ => None,
        };
        if self.blend <= 0.0 {
            self.from = None;
        }
        let name = self.clip.0.clone();
        let names = [name.as_str()];
        let with = blend_with.as_ref().map(|(n, t, w)| (n.as_str(), *t, *w));
        let body = kh_anim::BodySlot { samples: [(name.as_str(), 1.0), (name.as_str(), 0.0)], blend_with: with, notify_sequences: &names, time: self.time, weight: 1.0, active: true, notify_window: window };
        self.animator.set_bonamik_ground(Some((floor, Vec3::Z)));
        let pose = self.animator.step(&kh_anim::Frame { dt: STEP, body, component_to_world: c2w }, &kh_anim::NoIkWorld).clone();
        self.previous = if first { pose.clone() } else { std::mem::replace(&mut self.current, pose.clone()) };
        self.current = pose;
        self.steps += 1;
    }

    /// The pose between the two latest steps.
    fn drawn(&self) -> kh_anim::Pose {
        let alpha = (self.accumulator / STEP).clamp(0.0, 1.0);
        self.previous.lerp(&self.current, alpha)
    }
}

/// The mesh component in KH3 space: at his feet, turned by his yaw minus 90 degrees (the gimmick's TresGimmickMesh0
/// RelativeRotation yaw -90; kh-partyhost poser `component_to_world`).
pub fn component_to_world(feet_kh: Vec3, yaw: f32) -> Trs {
    Trs { translation: feet_kh, rotation: Quat::from_rotation_z((yaw - 90.0).to_radians()), scale: Vec3::ONE }
}

// ------------------------------------------------------------------------------------------------------------- the host

/// The game frame's facts the host hands over (sora.rs, once a frame).
pub struct FrameIn {
    /// Seconds of the game's frame.
    pub dt: f32,
    /// KH3's origin in Elden Ring's physics space (sora.rs `self.world.frame`).
    pub frame: coords::Frame,
    /// A cutscene (the game's or ours).
    pub hidden: bool,
    /// The leader fights.
    pub in_battle: bool,
}

struct Live {
    sheet: Option<rules::Sheet>,
    read: bool,
    dir: rules::Director,
    poser: Option<Poser>,
    poser_failed: bool,
    /// His feet in KH3 space and his yaw, this frame (None: not placed).
    placed: Option<(Vec3, f32)>,
    /// (P8 fix) His floor place, taken once and kept (rules::Anchor). (SYNTH-MOOGLE) In Elden Ring's physics space (KH3 axes
    /// and cm: `Anchor::follow_origin_shift`), never in Sora's moving KH3 frame.
    anchor: rules::Anchor,
    /// (SYNTH-MOOGLE) The KH3 frame's origin of the frame before (physics space, KH3 axes and cm).
    origin: Option<Vec3>,
    last_voice: Option<String>,
    seed: u32,
    since_log: f32,
    told_off: bool,
}

/// His state. (SYNTH-MOOGLE, the user 2026-11-07: "still jerking around") ONE state for the process: it was `thread_local!`
/// on the belief that the host's frame and the party's drawing run on one game thread, but the game runs them on its worker
/// threads: the DEPLOY3 session log (er/logs/kh-er.log) read his sheet 6 times and made 6 Posers, each thread had its own
/// Director, place and pose, each advanced only on the frames that ran there, and the draw showed whichever thread drew:
/// his body, pom-pom and wings snapped between 6 poses out of phase (one of them a fresh Bonamik whose pom-pom was falling).
static LIVE: Mutex<Live> = Mutex::new(Live { sheet: None, read: false, dir: rules::Director { phase: rules::Phase::Away, gone: 0.0 }, anchor: rules::Anchor::new(), origin: None, poser: None, poser_failed: false, placed: None, last_voice: None, seed: 0x9E37_79B9, since_log: 0.0, told_off: false });

/// The pack and the sheets' folder (sora.rs at the host's start).
static PATHS: Mutex<Option<(PathBuf, PathBuf)>> = Mutex::new(None);
/// Y's down edge since the last frame while his prompt was up; the pad of the frame before; his prompt is up.
static Y_LATCH: AtomicBool = AtomicBool::new(false);
static Y_BEFORE: AtomicBool = AtomicBool::new(false);
static PROMPT_UP: AtomicBool = AtomicBool::new(false);

/// (sora.rs, at the host's start) The pack root and the loadout sheet's path (the Moogle's sheet lies beside it).
pub fn set_paths(pack: &Path, loadout: &Path) {
    let sheets = loadout.parent().map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    *PATHS.lock().unwrap_or_else(|e| e.into_inner()) = Some((pack.to_path_buf(), sheets));
}

/// (hud.rs `HudHost::input`, every frame) Y's down edge is his while his prompt is up (and hidden from Sora's situation
/// command), as Donald's talk (unlock/host.rs `note_pad`).
pub fn note_pad(held: &mut PadButtons) {
    let y = held.has(PadButton::FaceTop);
    let before = Y_BEFORE.swap(y, Ordering::Relaxed);
    if PROMPT_UP.load(Ordering::Relaxed) {
        if y && !before {
            Y_LATCH.store(true, Ordering::Relaxed);
        }
        held.set(PadButton::FaceTop, false);
    }
}

/// His prompt is up now.
pub fn prompt_up() -> bool {
    PROMPT_UP.load(Ordering::Relaxed)
}

fn read_sheet() -> Option<rules::Sheet> {
    let (_, dir) = PATHS.lock().ok()?.clone()?;
    let path = dir.join(rules::SHEET_FILE);
    match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| rules::Sheet::from_text(&t)) {
        Ok(s) if !s.approved => {
            erlog!("moogle: {}: the spot is not approved: no Moogle", path.display());
            None
        }
        Ok(s) => {
            erlog!("moogle: sheet {} read: {} at ({:.2}, {:.2}, {:.2}) yaw {:.0}; show {} m, talk {} m (clear of Elden Ring's prompts: {}), voices {} + {}", path.display(), s.map, s.pos.x, s.pos.y, s.pos.z, s.rot_y, s.show_m, s.talk_m, s.prompt_clear(), s.welcome.len(), s.new_arrival.len());
            Some(s)
        }
        Err(e) => {
            erlog!("moogle: sheet {} NOT read ({e}): no Moogle", path.display());
            None
        }
    }
}

/// The player's place this frame (Elden Ring axes).
#[derive(Clone, Copy, Debug)]
struct PlayerPlace {
    physics: Vec3,
    facing: Vec3,
    local: Vec3,
    block: (u8, i32, i32),
    overworld: bool,
}

/// Roundtable Hold's map (the Moogle sheet's spot is in it).
pub const ROUNDTABLE_HOLD: &str = "m11_10_00_00";

/// (the user, 2026-11-03: "keyblade-hero light should never activate in round table hold") The player is in Roundtable Hold:
/// his block is the Hold's map (the map, not the no-combat state: that one also covers Rennala's chamber and others). Without a
/// player this frame (a load) the last answer is kept, so the light cannot come up in the Hold before he stands there.
pub fn in_roundtable_hold() -> bool {
    static LAST: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if let Some(p) = player_place() {
        LAST.store(rules::parse_map(ROUNDTABLE_HOLD) == Some(p.block), Ordering::Relaxed);
    }
    LAST.load(Ordering::Relaxed)
}

fn player_place() -> Option<PlayerPlace> {
    let player = crate::game::player_mut()?;
    let b = player.current_block_id;
    let local = Vec3::new(player.block_position.x, player.block_position.y, player.block_position.z);
    let physics = crate::game::v3(&player.chr_ins.modules.physics.position);
    let yaw = coords::yaw_kh_of_er_quat(crate::game::quat(&player.chr_ins.modules.physics.orientation));
    (local.is_finite() && physics.is_finite()).then_some(PlayerPlace { physics, facing: coords::facing_er(yaw), local, block: (b.area(), b.block() as i32, b.region() as i32), overworld: b.is_overworld() })
}

// ---- THE WORKSHOP (P6b's screens, documentation/KH3_WEAPONS_PLAN.md 1.5 / 4). STUB until P6b publishes its open API: INT
// swaps the bodies of these three for P6b's calls (build/sora_tmp/weapons/P8/STATE.md "INT lines"). With the stub the talk
// plays his voice and his row, logs that the Workshop is not in this build, and goes back to play at once.

/// Asks KH3's Workshop to open on its list. False: refused (not in this build, another menu up).
/// (INT) P6b's API: `hud_workshop::open(Entry::Workshop, ..)` with Elden Ring's event flags (the milestones, the shop level).
fn workshop_request() -> bool {
    kh_host::hud_workshop::open(kh_host::hud_workshop::Entry::Workshop, "", &|f| crate::bossreward::game_flag(f) == Some(true))
}

/// The Workshop is open now: (INT) UP on the HUD (`is_up`; a request the HUD never takes is dropped at GiveUp).
fn workshop_is_open() -> bool {
    kh_host::hud_workshop::is_up()
}

/// The Workshop has a recipe the player has not seen (kh-host workshop.rs `recipe_is_new`): his TalkNewArrival voice.
/// (INT) Over the listed recipes of the running record (P6a's `recipes`, which only reads).
fn workshop_has_new() -> bool {
    kh_host::workshop::with_live(|ws, host| kh_host::workshop::recipes(ws, host).iter().any(|r| r.new))
}

fn play_voice(asset: &str, at_kh: Vec3) {
    // the sheet's names live for the run (the audio seam keeps `&'static str`)
    static NAMES: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
    let name = {
        let mut kept = NAMES.lock().unwrap_or_else(|e| e.into_inner());
        match kept.iter().find(|k| **k == asset) {
            Some(k) => *k,
            None => {
                let k: &'static str = Box::leak(asset.to_string().into_boxed_str());
                kept.push(k);
                k
            }
        }
    };
    // KH3 plays the shop voice from its UI (TresUIShopVoice): flat, on the menus' path, not paused with the world
    let play = kh_core::SoundPlay { id: kh_core::SoundId(0), name, at: kh_core::Attach::World(at_kh), volume: 1.0, pitch: 1.0, looping: false, spatial: false };
    crate::fx::ui_sounds(vec![kh_host::seam::AudioCommand::Play(kh_core::SoundHandle(VOICE_HANDLE), play)]);
}

/// ONCE A FRAME (sora.rs, beside the other talk frames): where he is, his prompt, his talk, his pose.
pub fn frame(control: &Control, input: &FrameIn) {
    {
        let mut guard = LIVE.lock().unwrap_or_else(|e| e.into_inner());
        let live = &mut *guard;
        if control.flag("moogle.on") == Some(false) {
            if !live.told_off {
                live.told_off = true;
                erlog!("moogle: moogle.on = 0: no Moogle");
            }
            off(live);
            return;
        }
        live.told_off = false;
        if !live.read {
            live.read = true;
            live.sheet = read_sheet();
        }
        let Some(sheet) = live.sheet.clone() else { return };
        let Some(map) = rules::parse_map(&sheet.map) else { return };
        // where he is this frame (the spot in the player's physics space; KH3 space for the drawing)
        let place = player_place();
        // (SYNTH-MOOGLE) the candidate and the kept place in PHYSICS space (KH3 axes and cm); his KH3 place is worked out from
        // it with this frame's KH3 frame, so a move of Sora's frame alone (the game carrying the body) never carries him
        let candidate = place.and_then(|p| rules::spot_physics(sheet.pos, map, p.block, p.overworld, p.local, p.physics));
        // (P8 fix) HIS PLACE IS KEPT (rules::Anchor): never re-worked each frame from the player's two positions; while the player is
        // in another map (no candidate) the kept place is not his
        let keep = live.dir.drawn();
        let floor_kh = rules::kept_place(&mut live.anchor, &mut live.origin, &input.frame, candidate, keep, input.dt);
        let at = floor_kh.filter(|_| place.is_some_and(|p| rules::spot_physics(sheet.pos, map, p.block, p.overworld, p.local, p.physics).is_some())).map(|k| input.frame.pos_kh_to_er(k));
        let mut facts = rules::Facts { hidden: input.hidden || place.is_none(), dt: input.dt, workshop_open: workshop_is_open(), ..rules::Facts::default() };
        if let (Some(p), Some(spot)) = (place, at) {
            let (dist, dy, faces) = rules::talk_geometry(p.physics, p.facing, spot, sheet.facing_deg);
            facts.in_hold = true;
            facts.dist3 = (spot - p.physics).length();
            facts.dist = dist;
            facts.dy = dy;
            facts.facing = faces;
        } else {
            facts.dist3 = f32::INFINITY;
            facts.dist = f32::INFINITY;
        }
        facts.blocked = input.in_battle || crate::hud::menu_open() || crate::conv::running().is_some() || crate::erprompt::er_up();
        facts.talk_pressed = Y_LATCH.swap(false, Ordering::Relaxed);
        let before = live.dir.phase;
        let mut acts = live.dir.step(&sheet, &facts);
        // (P8 fix) KH3's floating Moogle: his mesh at the gimmick's hover above the kept floor place; facing fixed
        let feet_kh = floor_kh.map(|f| f + Vec3::Z * sheet.hover_cm);
        if live.dir.drawn() {
            live.placed = feet_kh.map(|f| (f, sheet.rot_y)).or(live.placed);
        } else {
            live.placed = None;
            live.anchor.clear();
        }
        let mut k = 0;
        while k < acts.len() {
            match acts[k] {
                rules::Act::Show => {
                    if live.poser.is_none() && !live.poser_failed {
                        let dir = PATHS.lock().ok().and_then(|p| p.clone()).map(|(pack, _)| pack.join(&sheet.pack_dir));
                        match dir.ok_or("no pack path".to_string()).and_then(|d| Poser::load(&d, &sheet)) {
                            Ok(p) => {
                                erlog!("moogle: his pose is ready ({} bones, Bonamik on; warnings: {:?})", p.animator.skeleton().len(), p.animator.warnings);
                                live.poser = Some(p);
                            }
                            Err(e) => {
                                live.poser_failed = true;
                                erlog!("!!! moogle: his pack {} could not be read ({e}): he is not drawn", sheet.pack_dir);
                            }
                        }
                    }
                    kh_er_render::party::preload(&[sheet.draw_id]);
                    erlog!("moogle: THE MOOGLE IS SHOWN ({:.1} m from the player, in {})", facts.dist3, sheet.map);
                }
                rules::Act::Hide => erlog!("moogle: the Moogle is not drawn (the player left, or is hidden)"),
                rules::Act::PromptOn => PROMPT_UP.store(true, Ordering::Relaxed),
                rules::Act::PromptOff => PROMPT_UP.store(false, Ordering::Relaxed),
                rules::Act::Talk => {
                    let voice = rules::pick_voice(&sheet, workshop_has_new(), live.last_voice.as_deref(), live.seed);
                    live.seed = live.seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                    if let (Some(v), Some(f)) = (voice.as_ref(), feet_kh) {
                        play_voice(v, f + Vec3::Z * 80.0);
                    }
                    live.last_voice = voice.clone();
                    if let (Some(p), Some(row)) = (live.poser.as_mut(), sheet.talk.as_ref()) {
                        p.play_once(row, sheet.talk_blend);
                    }
                    let asked = workshop_request();
                    erlog!("moogle: TALK: voice {}, row {}, the Workshop {}", voice.as_deref().unwrap_or("-"), sheet.talk.as_ref().map_or("none (he keeps floating)", |t| t.0.as_str()), if asked { "asked for" } else { "NOT in this build (P6b's screens): back to play" });
                    if !asked {
                        acts.extend(live.dir.refused());
                    }
                }
                rules::Act::Opened => erlog!("moogle: the Workshop is open (the world stands)"),
                rules::Act::GiveUp => {
                    kh_host::hud_workshop::close(); // (INT) the request the HUD never took is dropped
                    erlog!("moogle: the Workshop did not open within {} s: back to play", sheet.open_within_s);
                }
                rules::Act::Closed => erlog!("moogle: the Workshop closed: back to play"),
            }
            k += 1;
        }
        if live.dir.phase != before {
            erlog!("moogle: {before:?} -> {:?} ({:.1} m, facing {}, blocked {})", live.dir.phase, facts.dist, facts.facing, facts.blocked);
        }
        // the conversation system's green "Talk" (and Y over Elden Ring's own prompt) while his prompt is up
        crate::conv::offer_talk(OWNER, live.dir.prompt_up());
        // (INTERACT-MARK) KH3's reaction-target marker on him: his reactor, 30 cm over his root (g_ex_mogli_01 TresReactor0)
        crate::reactmark::set_talk_point(OWNER, live.placed.filter(|_| live.dir.prompt_up()).map(|(root, _)| root + Vec3::Z * crate::reactmark::MOOGLE_REACTOR_CM));
        // his pose: only while he is drawn ((SYNTH-MOOGLE) the steps run in his own space; the draw puts the pose at his place)
        if let (true, Some(p), Some(_)) = (live.dir.drawn(), live.poser.as_mut(), live.placed) {
            p.advance(input.dt, &sheet);
        }
        live.since_log += input.dt;
        if control.on("moogle.log") && live.since_log >= 1.0 {
            live.since_log = 0.0;
            erlog!("moogle: {:?}; in the hold {}, {:.1} m (3D {:.1}), dy {:.2}, facing {}, blocked {}, drawn at {:?}", live.dir.phase, facts.in_hold, facts.dist, facts.dist3, facts.dy, facts.facing, facts.blocked, live.placed);
        }
    }
}

fn off(live: &mut Live) {
    if live.dir.prompt_up() {
        PROMPT_UP.store(false, Ordering::Relaxed);
    }
    crate::conv::offer_talk(OWNER, false);
    crate::reactmark::set_talk_point(OWNER, None); // (INTERACT-MARK)
    live.dir = rules::Director::default();
    live.placed = None;
    live.anchor.clear();
    live.origin = None;
}

/// (partydraw.rs, once a drawn frame) HIS DRAW for the renderer's party list, while he is shown and his pose is ready.
pub fn draw(glow_mask: bool) -> Option<kh_er_render::party::CharDraw> {
    let live = LIVE.lock().unwrap_or_else(|e| e.into_inner());
    if !live.dir.drawn() {
        return None;
    }
    let (feet, yaw) = live.placed?;
    let p = live.poser.as_ref().filter(|p| p.steps > 0)?;
    let pose = p.drawn();
    let draw_id = live.sheet.as_ref().map_or(kh_er_render::party::MOOGLE, |s| s.draw_id);
    Some(kh_er_render::party::CharDraw { chara: draw_id, body: Some(kh_er_render::Body { component_to_world: component_to_world(feet, yaw), local: pose.local, component: pose.component }), weapon: None, props: Vec::new(), glow_mask, blood: Default::default() })
}

// ------------------------------------------------------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::rules::*;
    use glam::Vec3;

    fn sheet() -> Sheet {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sheets/port").join(SHEET_FILE);
        Sheet::from_text(&std::fs::read_to_string(path).expect("sheets/port/kh-moogle.json")).expect("the sheet parses")
    }

    /// The player 1.5 m in front of him, facing him, nothing else up.
    fn near(s: &Sheet) -> Facts {
        assert!(1.5 <= s.talk_m && 1.5 <= s.show_m);
        Facts { in_hold: true, hidden: false, dist3: 1.5, dist: 1.5, dy: 0.0, facing: true, blocked: false, talk_pressed: false, workshop_open: false, dt: 1.0 / 60.0 }
    }

    #[test]
    fn the_sheet_has_the_approved_spot_and_the_ranges() {
        let s = sheet();
        assert!(s.approved, "the user approved spot A on 2026-11-03");
        assert_eq!(s.map, "m11_10_00_00");
        assert!((s.pos - Vec3::new(-290.5, -22.61, -306.5)).length() < 1e-3);
        assert_eq!(s.rot_y, 150.0);
        assert_eq!(s.pack_dir, "party/n_ex014", "(MOOGLE-HOVER) KH3's Twilight Town Moogle (g_ex_mogli_01, row moogle.model.tt)");
        assert_eq!(s.draw_id, kh_er_render::party::MOOGLE_TT, "kh-er-render party.rs MOOGLE_TT");
        assert_eq!(s.idle.0, "001", "(MOOGLE-HOVER) his swaying M_001, looped (row moogle.anim.idle.tt)");
        assert_eq!(s.talk, None, "(P8 fix) no talk row: KH3's shop actor plays only its idle in the world");
        assert_eq!(s.hover_cm, 105.0, "(MOOGLE-HOVER) the user's choice: his head at Sora's (KH3's own in Twilight Town: hover_cm_kh3_tt 85)");
        assert_eq!(s.talk_m, 2.5, "Elden Ring's own Talk radius");
        assert!(s.show_m > s.talk_m && s.height_m > 0.0 && s.facing_deg > 0.0 && s.facing_deg < 90.0);
        assert!(s.welcome.iter().chain(&s.new_arrival).all(|a| a.starts_with("/Game/Sound/SE/common/00000/se00000_moogle")), "KH3's Moogle voices only");
        assert!(!s.welcome.is_empty() && !s.new_arrival.is_empty());
    }

    #[test]
    fn the_talk_range_never_meets_an_elden_ring_prompt() {
        let s = sheet();
        assert!(s.clear_of.len() >= 2, "P8a's nearest talk parts are listed");
        for (part, d, r) in &s.clear_of {
            assert!(s.talk_m + r < *d, "{part}: {} + {r} m reaches {d} m", s.talk_m);
        }
        assert!(s.prompt_clear());
    }

    #[test]
    fn he_faces_away_from_the_wall_toward_hewg() {
        let s = sheet();
        let f = s.facing_er();
        // Hewg [9000] at (-286.72, -22.12, -313.56) (P8a's table); the navmesh edge 0.75 .. 0.85 m out on the (-0.5, 0.87) side
        let to_hewg = (Vec3::new(-286.72, -22.12, -313.56) - s.pos).normalize();
        assert!(f.dot(Vec3::new(to_hewg.x, 0.0, to_hewg.z).normalize()) > 0.9, "facing {f:?}");
        assert!(f.dot(Vec3::new(-0.5, 0.0, 0.866)) < -0.9, "his back to the wall");
    }

    #[test]
    fn talk_geometry_needs_the_range_and_the_facing() {
        let m = Vec3::new(0.0, 0.0, 2.0);
        let (d, dy, faces) = talk_geometry(Vec3::ZERO, Vec3::Z, m, 70.0);
        assert!((d - 2.0).abs() < 1e-4 && dy == 0.0 && faces);
        assert!(!talk_geometry(Vec3::ZERO, -Vec3::Z, m, 70.0).2, "his back to the Moogle");
        assert!(!talk_geometry(Vec3::ZERO, Vec3::X, m, 70.0).2, "90 degrees off");
        assert!(talk_geometry(Vec3::ZERO, Vec3::new(0.5, 0.0, 0.9).normalize(), m, 70.0).2, "29 degrees off");
        assert!(talk_geometry(Vec3::new(0.0, 0.0, 1.9), -Vec3::Z, m, 70.0).2, "right on top of him");
    }

    #[test]
    fn the_spot_is_placed_only_in_its_own_map() {
        let s = sheet();
        let map = parse_map(&s.map).unwrap();
        assert_eq!(map, (11, 10, 0));
        let player_local = Vec3::new(-300.0, -22.6, -306.0);
        let physics = Vec3::new(12.0, 5.0, -40.0);
        let at = spot_physics(s.pos, map, (11, 10, 0), false, player_local, physics).expect("in the hold");
        assert!((at - (physics + (s.pos - player_local))).length() < 1e-4);
        assert_eq!(spot_physics(s.pos, map, (60, 42, 37), true, player_local, physics), None, "Limgrave");
        assert_eq!(spot_physics(s.pos, map, (11, 0, 0), false, player_local, physics), None, "Leyndell (m11_00): another block of area 11");
        assert_eq!(spot_physics(s.pos, map, (11, 5, 0), false, player_local, physics), None, "Leyndell, Ashen Capital (m11_05)");
    }

    #[test]
    fn no_prompt_outside_roundtable_hold() {
        let s = sheet();
        let mut d = Director::default();
        // standing where the spot's numbers would be, but in another map: nothing at all
        let f = Facts { in_hold: false, dist3: f32::INFINITY, dist: f32::INFINITY, ..near(&s) };
        for _ in 0..10 {
            assert!(d.step(&s, &Facts { talk_pressed: true, ..f }).is_empty());
        }
        assert_eq!(d.phase, Phase::Away);
        assert!(!d.prompt_up() && !d.drawn());
    }

    #[test]
    fn shown_in_range_prompt_only_close_facing_and_free() {
        let s = sheet();
        let mut d = Director::default();
        let far = Facts { dist3: s.show_m - 1.0, dist: s.show_m - 1.0, ..near(&s) };
        assert_eq!(d.step(&s, &far), vec![Act::Show]);
        assert_eq!(d.phase, Phase::Shown);
        assert_eq!(d.step(&s, &Facts { dist3: s.show_m + 1.0, dist: s.show_m + 1.0, dt: HIDE_AFTER_S, ..far }), vec![Act::Hide], "out of show range: not drawn");
        assert_eq!(d.step(&s, &near(&s)), vec![Act::Show, Act::PromptOn]);
        assert!(d.prompt_up());
        assert_eq!(d.step(&s, &Facts { facing: false, ..near(&s) }), vec![Act::PromptOff], "turned away");
        assert_eq!(d.step(&s, &near(&s)), vec![Act::PromptOn]);
        assert_eq!(d.step(&s, &Facts { blocked: true, ..near(&s) }), vec![Act::PromptOff], "Elden Ring's prompt / a menu / a talk / a fight");
        assert_eq!(d.step(&s, &Facts { blocked: true, talk_pressed: true, ..near(&s) }), Vec::<Act>::new(), "Y while blocked does nothing");
        assert_eq!(d.step(&s, &Facts { dy: s.height_m + 0.5, ..near(&s) }), Vec::<Act>::new(), "on another floor");
        assert_eq!(d.step(&s, &Facts { dist: s.talk_m + 0.1, dist3: s.talk_m + 0.1, ..near(&s) }), Vec::<Act>::new());
        assert_eq!(d.step(&s, &near(&s)), vec![Act::PromptOn]);
        assert_eq!(d.step(&s, &Facts { hidden: true, dt: HIDE_AFTER_S, ..near(&s) }), vec![Act::PromptOff, Act::Hide], "a cutscene");
    }

    /// (P8 fix) A few frames without the player (a load step, a one-frame flag) never blink him off and on.
    #[test]
    fn a_short_gap_never_hides_him() {
        let s = sheet();
        let mut d = Director::default();
        d.step(&s, &near(&s));
        assert!(d.prompt_up());
        let gap = Facts { hidden: true, in_hold: false, dist3: f32::INFINITY, dist: f32::INFINITY, ..near(&s) };
        assert_eq!(d.step(&s, &gap), vec![Act::PromptOff], "the prompt goes at once");
        for _ in 0..10 {
            assert_eq!(d.step(&s, &gap), Vec::<Act>::new(), "but he stays drawn");
        }
        assert!(d.drawn());
        assert_eq!(d.step(&s, &near(&s)), vec![Act::PromptOn]);
        assert_eq!(d.gone, 0.0);
    }

    /// (P8 fix, THE USER: "the moogle ... is spazzing out, his actual position should be stationary") HIS DRAWN PLACE NEVER
    /// MOVES: the worked-out place shakes by up to a frame of the player's run (the game's two player positions disagree), the
    /// frame times are uneven, frames come without a place, the open world's physics origin shifts under him: the drawn
    /// component is the same, bit for bit. (SYNTH-MOOGLE) And Sora's KH3 frame moving alone (the game carrying the body:
    /// sora.rs `Follow`, a step at a time and a 1.5 m snap) never carries him: his place in Elden Ring stays the spot (it was
    /// kept in KH3 space and moved with Sora's frame until it was a metre off, then jumped back), and an origin shift moves
    /// it at once with every other physics position.
    #[test]
    fn his_drawn_place_is_stationary() {
        let s = sheet();
        let map = parse_map(&s.map).unwrap();
        let mut frame = crate::coords::Frame::at(Vec3::new(3.0, -1.0, 7.0));
        let (mut anchor, mut last_origin) = (Anchor::default(), None);
        let mut first: Option<Vec3> = None;
        let mut origin_shift = Vec3::ZERO;
        for k in 0..2000u32 {
            let dt = [1.0 / 30.0, 1.0 / 144.0, 1.0 / 60.0, 0.05, 1.0 / 90.0][k as usize % 5];
            if k == 900 {
                // the physics origin moves (an open-world rebase): every physics position and the KH3 frame shift alike
                let shift = Vec3::new(-256.0, 0.0, 512.0);
                origin_shift += shift;
                frame.rebase(shift);
            }
            if (300..360).contains(&k) {
                frame.rebase(Vec3::new(0.04, 0.0, -0.03)); // the game carries the body a step a frame: Sora's frame follows it
            }
            if k == 1200 {
                frame.rebase(Vec3::new(1.5, 0.0, 0.4)); // the game snaps the body 1.5 m
            }
            // the player runs about; his block-local place lags his physics place by up to a frame of a 7 m/s run
            let player_local = Vec3::new(-300.0 + (k as f32 * 0.37).sin() * 4.0, -22.6, -306.0 + (k as f32 * 0.23).cos() * 4.0);
            let lag = Vec3::new((k as f32 * 1.7).sin(), 0.0, (k as f32 * 2.3).cos()) * 7.0 * dt;
            let physics = player_local + Vec3::new(40.0, 2.0, -12.0) + origin_shift + lag;
            let candidate = if k % 97 == 3 { None } else { spot_physics(s.pos, map, map, false, player_local, physics) };
            let floor = kept_place(&mut anchor, &mut last_origin, &frame, candidate, true, dt).expect("placed");
            // where he is drawn in Elden Ring, the origin shift taken out
            let er = frame.pos_kh_to_er(floor) - origin_shift;
            match first {
                None => first = Some(er),
                Some(f) => assert!((f - er).length() < 1e-3, "frame {k}: drawn at {f:?} -> {er:?} in Elden Ring"),
            }
        }
        // a real move (a new KH3 frame after a load: 5 m) is taken once it holds, whole
        let at = anchor.at.unwrap();
        let moved = at + Vec3::new(500.0, 0.0, 0.0);
        assert_eq!(anchor.update(Some(moved), 0.1), Some(at), "not at once");
        assert_eq!(anchor.update(Some(moved), 0.1), Some(at));
        let mut got = None;
        for _ in 0..5 {
            got = anchor.update(Some(moved + Vec3::new(0.0, 3.0, 0.0)), 0.1);
        }
        assert_eq!(got, Some(moved + Vec3::new(0.0, 3.0, 0.0)), "held for RELATCH_S: moved whole");
        anchor.clear();
        assert_eq!(anchor.update(None, 0.1), None);
    }

    /// (MOOGLE-HOVER) A pack's head centre over a looped clip (cm above its component origin, i.e. above its floor when the
    /// mesh stands at 0): the middle of the bind-pose box of the vertices skinned mostly to `atama` (`centre`, measured from
    /// the pack's model.glb), carried by the posed `atama` bone. (lowest, highest, mean) over one cycle of `seq`.
    fn head_centre_over(dir: &std::path::Path, seq: &str, length: f32, centre: Vec3) -> (f32, f32, f32) {
        let clips = kh_anim::Settings { leg_ik: false, kine_driver: false, face: false, bonamik: false, first_update_holds_position: false };
        let mut a = kh_anim::Animator::new(kh_anim::Pack::load(dir).expect("the pack loads"), clips).expect("the animator");
        let sk = a.skeleton().clone();
        let head = sk.index_of("atama").expect("bone atama");
        let bind = kh_anim::pose::component_space(&sk, &sk.reference_pose())[head];
        let in_head = bind.rotation.conjugate() * (centre - bind.translation) / bind.scale;
        let names = [seq];
        let (mut lo, mut hi, mut sum, mut n) = (f32::MAX, f32::MIN, 0.0, 0);
        let steps = (length * 60.0) as usize;
        for k in 0..steps {
            let t = k as f32 / 60.0;
            let body = kh_anim::BodySlot { samples: [(seq, 1.0), (seq, 0.0)], blend_with: None, notify_sequences: &names, time: t, weight: 1.0, active: true, notify_window: (if k == 0 { -1.0 } else { t - 1.0 / 60.0 }, t) };
            let pose = a.step(&kh_anim::Frame { dt: 1.0 / 60.0, body, component_to_world: kh_anim::Trs::IDENTITY }, &kh_anim::NoIkWorld);
            let z = pose.component[head].transform_point(in_head).z;
            lo = lo.min(z);
            hi = hi.max(z);
            sum += z;
            n += 1;
        }
        (lo, hi, sum / n.max(1) as f32)
    }

    /// (MOOGLE-HOVER, THE USER 2026-11-05: "raise the moogle so it reaches about Sora's head") HIS HEAD FLOATS AT SORA'S HEAD:
    /// both are drawn 1:1 (KH3 cm, coords.rs CM_PER_M) on the same floor (Sora's component at the Tarnished's feet, the
    /// Moogle's floor = the spot's navmesh height), so the hover is Sora's head centre in his idle (IdleN = 0001, the hold has
    /// no fight) minus the Moogle's head centre in his idle. Measured from the packs (head boxes from each model.glb: Sora's
    /// atama vertices z 137.0 .. 161.8, middle (0, -3.17, 149.39); the Moogle's 29.4 .. 59.8, middle (0, 0.15, 44.59), the
    /// same for n_ex014 and n_ex028): Sora 147.24 .. 147.48 (mean 147.33); the Twilight Town Moogle in M_001 38.95 .. 45.58
    /// (mean 42.37): 104.96 -> the sheet's 105 (the field Moogle's M_010 gave 44.48 -> 103). KH3 draws the Twilight Town
    /// Moogle 85 cm above the street (hover_cm_kh3_tt): his head 20 cm under Sora's. Skipped without the packs.
    #[test]
    fn his_head_floats_at_soras_head() {
        let s = sheet();
        let pack = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack");
        let moogle = pack.join(&s.pack_dir);
        if !pack.join("manifest.json").is_file() || !moogle.join("manifest.json").is_file() {
            eprintln!("no {} / {}: skipped", pack.display(), moogle.display());
            return;
        }
        let sora = head_centre_over(&pack, "0001", 4.0, Vec3::new(0.0, -3.17, 149.39));
        let his = head_centre_over(&moogle, &s.idle.0, s.idle.1, Vec3::new(0.0, 0.15, 44.59));
        let gap = sora.2 - (s.hover_cm + his.2);
        eprintln!("Sora's head centre {:.2} .. {:.2} (mean {:.2}); his {:.2} .. {:.2} (mean {:.2}) + hover {}: {gap:.2} cm apart", sora.0, sora.1, sora.2, his.0, his.1, his.2, s.hover_cm);
        assert!(gap.abs() < 1.0, "his head centre is {gap:.2} cm from Sora's (wanted: about level)");
        // his head top (bind 59.8) is about Sora's (bind 161.8)
        assert!((s.hover_cm + 59.8 - 161.8).abs() < 5.0, "his head top at Sora's");
    }

    /// (MOOGLE-HOVER) THE TWILIGHT TOWN MOOGLE, TRACED (THE USER 2026-11-05: "The moogle should look and animate exactly like
    /// the one in Twilight Town"): tt_01_npc places g_tt_SynthesisShop87 = g_ex_mogli_01 (native TresShopActor, m_ShopID
    /// TT_MOOGLE1, m_DefaultIdlingMotName M_001, mesh n_ex014_0 with no mesh offset); TresShopActor plays that idle looped
    /// (0x140F9A330 / 0x140F9A210: flags 3) and no other clip (M_002 is the shop menu's portrait, n_ex014_UIActor
    /// ThinkingAnim). The sheet's Twilight Town rows win over the field Moogle's (n_ex028, kept for older builds), and KH3's own
    /// heights stay as evidence: 85 above the street in Twilight Town, 75 for the field Moogle.
    #[test]
    fn the_sheet_traces_kh3s_twilight_town_moogle() {
        let s = sheet();
        assert_eq!((s.idle.0.as_str(), s.idle.1), ("001", 2.0), "M_001 = ex_rt/body/001, 61 frames, 2 s");
        assert_eq!(s.talk, None, "no world clip on talk");
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sheets/port").join(SHEET_FILE);
        let json: serde_json::Value = serde_json::from_str(std::fs::read_to_string(path).expect("the sheet").trim_start_matches('\u{feff}')).expect("json");
        let rows = json["rows"].as_array().expect("rows");
        let row = |id: &str| rows.iter().find(|r| r[0] == id).cloned().unwrap_or_default();
        let tt = row("moogle.model.tt");
        assert_eq!((tt[1]["npc"].as_str(), tt[1]["class"].as_str(), tt[1]["shop_id"].as_str()), (Some("n_ex014"), Some("TresShopActor"), Some("ETresShopID::TT_MOOGLE1")));
        assert_eq!(tt[1]["hover_cm_kh3_tt"], 85.0, "KH3's own height in Twilight Town kept as evidence");
        assert_eq!(row("moogle.anim.idle.tt")[1]["kh3_play_flags"], 3, "looped");
        assert_eq!(row("moogle.anim.idle.tt")[6], "confirmed", "traced to ATresShopActor 0x140F9A330");
        // the rows older builds read are unchanged in kind: the field Moogle n_ex028 at draw id 8
        assert_eq!((row("moogle.model")[1]["pack_dir"].as_str(), row("moogle.model")[1]["draw_id"].as_u64()), (Some("party/n_ex028"), Some(8)));
        assert_eq!(row("moogle.model")[1]["hover_cm_kh3"], 75.0);
        assert_eq!(row("moogle.anim.idle")[1]["sequence"], "010");
    }

    /// (MOOGLE-HOVER, THE USER: "he has his own swaying hover animation that isn't playing right now in Elden Ring") HIS SWAY
    /// PLAYS AS KH3's ACTOR PLAYS IT: through the port's own pose path (Poser, fixed 60 Hz steps, Bonamik on), over 6 s of uneven
    /// frames, the clip is M_001 from the first step and never another (TresShopActor's only clip), its time wraps every 2 s
    /// (looped, rate 1.0), and the sway is there: his head rises and falls more than 5 cm a cycle and his wing tips swing more
    /// than 5 cm (the field Moogle's M_010 moves his head under 1 cm). Skipped without the pack.
    #[test]
    fn his_tt_float_sways() {
        let s = sheet();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack").join(&s.pack_dir);
        if !dir.join("manifest.json").is_file() {
            eprintln!("no {}: skipped", dir.display());
            return;
        }
        let mut p = super::Poser::load(&dir, &s).expect("his pack loads");
        let sk = p.animator.skeleton().clone();
        let head = sk.index_of("atama").expect("atama");
        let wing = sk.index_of("L_wing3").expect("L_wing3");
        let (mut clock, mut last, mut wraps) = (0.0f32, 0.0f32, Vec::new());
        let (mut head_lo, mut head_hi, mut wing_lo, mut wing_hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN), Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for k in 0..600usize {
            let dt = [1.0 / 60.0, 1.0 / 144.0, 1.0 / 30.0, 1.0 / 90.0][k % 4];
            clock += dt;
            p.advance(dt, &s);
            assert_eq!(p.clip.0, "001", "M_001 only, from the first step");
            if p.time < last {
                wraps.push(p.steps);
            }
            last = p.time;
            if p.steps > 30 {
                let (h, w) = (p.current.component[head].translation, p.current.component[wing].translation);
                head_lo = head_lo.min(h);
                head_hi = head_hi.max(h);
                wing_lo = wing_lo.min(w);
                wing_hi = wing_hi.max(w);
            }
        }
        assert!(clock > 5.9 && wraps.len() >= 2, "{} wraps in {clock:.1} s", wraps.len());
        // one wrap every 2 s of 60 Hz steps (the clip's length, rate 1.0): 120 steps, give or take the step a float sum rounds to
        for w in wraps.windows(2) {
            assert!((119..=121).contains(&(w[1] - w[0])), "a wrap after {} steps", w[1] - w[0]);
        }
        let mean = (wraps[wraps.len() - 1] - wraps[0]) as f32 / (wraps.len() - 1) as f32;
        assert!((mean - 120.0).abs() <= 0.5, "{mean} steps a cycle");
        assert!(head_hi.z - head_lo.z > 5.0, "his head bobs {:.1} cm", head_hi.z - head_lo.z);
        assert!((wing_hi - wing_lo).length() > 5.0, "his wing tip swings {:.1} cm", (wing_hi - wing_lo).length());
    }

    #[test]
    fn y_opens_the_workshop_and_closing_it_goes_back_to_play() {
        let s = sheet();
        let mut d = Director::default();
        d.step(&s, &near(&s));
        assert!(d.prompt_up());
        assert_eq!(d.step(&s, &Facts { talk_pressed: true, ..near(&s) }), vec![Act::PromptOff, Act::Talk]);
        assert!(matches!(d.phase, Phase::Opening(_)) && !d.prompt_up() && d.drawn());
        assert_eq!(d.step(&s, &near(&s)), Vec::<Act>::new(), "waiting for the screen");
        assert_eq!(d.step(&s, &Facts { workshop_open: true, ..near(&s) }), vec![Act::Opened]);
        for _ in 0..600 {
            assert_eq!(d.step(&s, &Facts { workshop_open: true, talk_pressed: true, ..near(&s) }), Vec::<Act>::new(), "open: Y is the Workshop's");
        }
        assert_eq!(d.step(&s, &near(&s)), vec![Act::Closed]);
        assert_eq!(d.phase, Phase::Shown);
        assert_eq!(d.step(&s, &near(&s)), vec![Act::PromptOn], "back to play: the prompt again");
        // a press needs the prompt up: the press that closed the Workshop never reopens it
        assert_eq!(d.step(&s, &near(&s)), Vec::<Act>::new());
    }

    #[test]
    fn a_workshop_that_never_opens_is_given_up() {
        let s = sheet();
        let mut d = Director::default();
        d.step(&s, &near(&s));
        d.step(&s, &Facts { talk_pressed: true, ..near(&s) });
        let mut acts = Vec::new();
        for _ in 0..((s.open_within_s * 60.0) as usize + 2) {
            acts.extend(d.step(&s, &near(&s)));
        }
        assert_eq!(acts, vec![Act::GiveUp, Act::PromptOn], "given up, then back to play: the prompt again");
        assert_eq!(d.phase, Phase::Prompt);
        // refused outright (the stub before P6b): back at once
        assert_eq!(d.step(&s, &Facts { talk_pressed: true, ..near(&s) }), vec![Act::PromptOff, Act::Talk]);
        assert_eq!(d.refused(), vec![Act::GiveUp]);
        assert_eq!(d.phase, Phase::Shown);
        assert!(d.refused().is_empty(), "nothing to refuse");
    }

    #[test]
    fn the_voice_is_kh3s_shop_voice_never_twice_running() {
        let s = sheet();
        let mut last: Option<String> = None;
        for seed in 0..40u32 {
            let v = pick_voice(&s, false, last.as_deref(), seed).unwrap();
            assert!(s.welcome.contains(&v));
            assert_ne!(Some(&v), last.as_ref(), "seed {seed}");
            last = Some(v);
        }
        assert!(s.new_arrival.contains(&pick_voice(&s, true, None, 3).unwrap()), "something new: TalkNewArrival");
        let empty = Sheet { welcome: Vec::new(), new_arrival: Vec::new(), ..s };
        assert_eq!(pick_voice(&empty, true, None, 0), None);
    }

    /// (P8 fix) His pack (tools/moogle_pack.js, KH3's floating n_ex028): the Animator reads it; over 10 s of uneven frame times
    /// his float M_010 loops in place without restarting (the clip's time only moves forward, wrapping at its end; the clip
    /// never changes), his root bones never move (no root motion), his body is the clip's alone, and Bonamik drives the
    /// pom-pom and the wings (not the removed balloon): they differ from the same steps without Bonamik. Skipped without the pack.
    #[test]
    fn his_float_loops_in_place_with_bonamik() {
        let s = sheet();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack").join(&s.pack_dir);
        if !dir.join("manifest.json").is_file() {
            eprintln!("no {} (node tools/moogle_pack.js): skipped", dir.display());
            return;
        }
        let mut p = super::Poser::load(&dir, &s).expect("his pack loads");
        let mut clips = super::Poser::load_with(&dir, &s, false).expect("his pack loads");
        let sk = p.animator.skeleton().clone();
        let driven = p.animator.bonamik().expect("Bonamik").driven_bones();
        for name in ["fusen2", "L_wing2", "R_wing2", "hair2"] {
            let i = sk.index_of(name).unwrap_or_else(|| panic!("bone {name}"));
            assert!(driven.contains(&i), "{name} is driven by his Bonamik");
        }
        // (P8 fix 2, the user: "Remove the balloon") the balloon and its string are neither drawn nor simulated
        // (MOOGLE-HOVER) the Twilight Town Moogle n_ex014 has no balloon at all; the field Moogle's is off
        for name in ["balloon1", "balloon2", "himo_up2", "himo_dawn2"] {
            if let Some(i) = sk.index_of(name) {
                assert!(!driven.contains(&i), "{name}: the balloon group is off");
            }
        }
        let roots: Vec<usize> = ["kh_sk", "trans"].iter().filter_map(|b| sk.index_of(b)).collect();
        let (mut last_time, mut wraps, mut time_run) = (0.0f32, 0, 0.0f32);
        let mut root_first: Option<Vec<kh_anim::Trs>> = None;
        let (mut gap, mut body_gap) = (0.0f32, 0.0f32);
        let hara = sk.index_of("hara1").unwrap();
        let fusen = sk.index_of("fusen2").unwrap();
        for k in 0..900usize {
            let dt = [1.0 / 30.0, 1.0 / 144.0, 1.0 / 60.0, 0.03, 1.0 / 90.0][k % 5];
            time_run += dt;
            p.advance(dt, &s);
            clips.advance(dt, &s);
            assert_eq!(p.clip.0, s.idle.0, "never another clip");
            if p.time < last_time {
                wraps += 1;
                assert!(last_time > s.idle.1 - 0.1, "a wrap only at the clip's end, never a restart ({last_time} -> {})", p.time);
            }
            last_time = p.time;
            let pose = p.drawn();
            let root: Vec<kh_anim::Trs> = roots.iter().map(|&r| pose.component[r]).collect();
            match &root_first {
                None => root_first = Some(root),
                Some(f) => assert!(f.iter().zip(&root).all(|(a, b)| (a.translation - b.translation).length() < 1e-4), "frame {k}: his root moved"),
            }
            if k > 60 {
                gap = gap.max((p.current.component[fusen].translation - clips.current.component[fusen].translation).length());
                body_gap = body_gap.max((p.current.component[hara].translation - clips.current.component[hara].translation).length());
            }
        }
        assert!(time_run > 9.5, "about 10 s");
        assert!(wraps >= 4, "the float loops ({wraps} wraps in {time_run:.1} s of a {} s clip)", s.idle.1);
        assert!(body_gap < 0.01, "his body is the clip's ({body_gap} cm)");
        assert!(gap > 0.1, "the pom-pom is Bonamik's ({gap} cm)");
        assert!(p.current.component.iter().all(|t| t.translation.is_finite() && t.translation.length() < 400.0), "nothing flies off");
    }

    /// (SYNTH-MOOGLE) The bones the smoothness test traces: his body (the clip's), the antenna and pom-pom (Bonamik), the
    /// wing tips (the clip's flap under Bonamik).
    const TRACED: [&str; 9] = ["center", "hara1", "atama", "hair1", "hair3", "fusen1", "fusen2", "L_wing3", "R_wing3"];

    /// One drawn frame: (game time, the clip's time, each TRACED bone in his own space, cm).
    type Drawn = (f32, f32, Vec<Vec3>);

    fn drawn_of(p: &super::Poser, bones: &[usize], t: f32) -> Drawn {
        let pose = p.drawn();
        (t, p.time, bones.iter().map(|&b| p.space.transform_point(pose.component[b].translation)).collect())
    }

    fn write_trace(name: &str, rows: &[Drawn]) {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/moogle_jerk");
        std::fs::create_dir_all(&dir).ok();
        let mut csv = String::from("t,clip_t");
        for b in TRACED {
            csv.push_str(&format!(",{b}_x,{b}_y,{b}_z"));
        }
        csv.push('\n');
        for (t, ct, at) in rows {
            csv.push_str(&format!("{t:.5},{ct:.5}"));
            for v in at {
                csv.push_str(&format!(",{:.4},{:.4},{:.4}", v.x, v.y, v.z));
            }
            csv.push('\n');
        }
        std::fs::write(dir.join(name), csv).ok();
    }

    /// The largest move of each TRACED bone from one drawn frame to the next, per 1/60 s (a longer frame may move more):
    /// (cm, at s).
    fn steps_of(rows: &[Drawn]) -> Vec<(f32, f32)> {
        let mut most = vec![(0.0f32, 0.0f32); TRACED.len()];
        for w in rows.windows(2) {
            let per = (super::STEP / (w[1].0 - w[0].0).max(super::STEP)).min(1.0);
            for (b, m) in most.iter_mut().enumerate() {
                let d = (w[1].2[b] - w[0].2[b]).length() * per;
                if d > m.0 {
                    *m = (d, w[1].0);
                }
            }
        }
        most
    }

    /// (SYNTH-MOOGLE, THE USER 2026-11-07: "The moogle in the round table hold is looking better but he is also still jerking
    /// around as seen in picture 3 to 4. fix this.") HE MOVES AS SMOOTHLY AS KH3's TWILIGHT TOWN MOOGLE: over 12 s of drawn
    /// frames at an uneven 60 fps (a 33 ms hitch now and then), his body never moves more than 1 cm a frame, his antenna and
    /// pom-pom never more than 1.5 cm, his wing tips never more than the clip's own flap (KH3's M_001 itself turns L_wing2 /
    /// L_wingA1 68 / 34 degrees in its last 1/30 s: the left wing tip's seam step is KH3's data), the loop seam (the clip's
    /// time wrapping) included, and the pom-pom stays up over his head (never the fall into his face of a fresh Bonamik).
    /// The DEPLOY3 build's six thread-local states (frame and draw on any of six game threads) are run alongside: the same
    /// check fails there by far. Traces: build/sora_tmp/moogle_jerk/trace_{before_threads,after,clips}.csv. Skipped without
    /// the pack.
    #[test]
    fn his_float_is_smooth_through_the_loop_seam() {
        let s = sheet();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack").join(&s.pack_dir);
        if !dir.join("manifest.json").is_file() {
            eprintln!("no {}: skipped", dir.display());
            return;
        }
        let mut p = super::Poser::load(&dir, &s).expect("his pack loads");
        let mut clips = super::Poser::load_with(&dir, &s, false).expect("his pack loads");
        let sk = p.animator.skeleton().clone();
        let bones: Vec<usize> = TRACED.iter().map(|b| sk.index_of(b).unwrap_or_else(|| panic!("bone {b}"))).collect();
        let dts = [0.0165f32, 0.0168, 0.0166, 0.0171, 0.0163, 0.0167, 0.0166, 0.0168, 0.0164, 0.0167, 0.0333, 0.0001];
        // the DEPLOY3 build: six threads, each with its own state made on its first frame; the frame and the draw each run on one
        let mut threads: Vec<Option<super::Poser>> = (0..6).map(|_| None).collect();
        let mut seed = 0x1234_5678u32;
        let mut pick = || {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            (seed >> 16) as usize % 6
        };
        let (mut after, mut reference, mut before) = (Vec::new(), Vec::new(), Vec::new());
        let (mut t, mut seams, mut last_clip) = (0.0f32, Vec::new(), p.time);
        let mut k = 0usize;
        while t < 12.0 {
            let dt = dts[k % dts.len()];
            k += 1;
            t += dt;
            p.advance(dt, &s);
            clips.advance(dt, &s);
            if p.time < last_clip {
                seams.push(t);
            }
            last_clip = p.time;
            after.push(drawn_of(&p, &bones, t));
            reference.push(drawn_of(&clips, &bones, t));
            let ran = pick();
            let state = threads[ran].get_or_insert_with(|| super::Poser::unsettled(&dir, &s, true).expect("his pack loads"));
            state.advance(dt, &s);
            if let Some(drawn) = threads[pick()].as_ref().filter(|q| q.steps > 0) {
                before.push(drawn_of(drawn, &bones, t));
            }
        }
        write_trace("trace_after.csv", &after);
        write_trace("trace_before_threads.csv", &before);
        write_trace("trace_clips.csv", &reference);
        assert!(seams.len() >= 5, "the loop wrapped {} times in 12 s", seams.len());
        let (most, clip_most, old) = (steps_of(&after), steps_of(&reference), steps_of(&before));
        for (b, name) in TRACED.iter().enumerate() {
            eprintln!("{name:8} most a frame: {:.2} cm at {:.3} s (the clip alone {:.2}; DEPLOY3's threads {:.2} at {:.3} s)", most[b].0, most[b].1, clip_most[b].0, old[b].0, old[b].1);
            let limit = match *name {
                "L_wing3" | "R_wing3" => clip_most[b].0 * 1.1 + 0.25,
                "hair3" | "fusen1" | "fusen2" => 1.5,
                _ => 1.0,
            };
            assert!(most[b].0 <= limit, "{name} moved {:.2} cm in a frame at {:.3} s (limit {limit:.2})", most[b].0, most[b].1);
        }
        // the loop seam: the frames around each wrap move as little as any other
        for &at in &seams {
            let near: Vec<Drawn> = after.iter().filter(|r| (r.0 - at).abs() < 0.05).cloned().collect();
            let seam = steps_of(&near);
            eprintln!("the seam at {at:.3} s: atama {:.2} cm, fusen2 {:.2} cm, L_wing3 {:.2} cm", seam[2].0, seam[6].0, seam[7].0);
            assert!(seam[2].0 <= 1.0 && seam[6].0 <= 1.5, "the seam at {at:.3} s: atama {:.2} cm, fusen2 {:.2} cm", seam[2].0, seam[6].0);
        }
        // the pom-pom stays up over his head from the first drawn frame (the fall of a fresh Bonamik: 30 cm, into his face)
        let low = after.iter().map(|r| r.2[6].z - r.2[2].z).fold(f32::MAX, f32::min);
        assert!(low > 30.0, "the pom-pom came down to {low:.1} cm over his head");
        // the DEPLOY3 build fails the same check by far
        assert!(old[2].0 > 5.0 && old[6].0 > 5.0, "the six thread states jump (atama {:.2}, fusen2 {:.2} cm)", old[2].0, old[6].0);
    }
}
