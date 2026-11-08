//! (MIMIC-RENDER) THE MIMIC TEAR BOSSES' SORA COPIES AT FULL QUALITY (THE USER 2026-11-05: "Sora's copy should only be silver for
//! three seconds like the rest"). A boss copy of a Sora leader (kh-er mimic_boss.rs) is a SECOND SORA: drawn by the renderer's
//! own Sora route (gfx/sora_copy_draw.rs: his translated KH3 shaders, the early depth with the characters' stencil, the
//! hybrid's G-buffer, the KH colour pass reading the game's captured light, the hair fringe, the glow mask, the motion vectors,
//! the real shadow) in BOTH Styles (`r.hybrid_native` on / off picks the same shader sets as the player's), with his own pose,
//! his own Keyblade and his own form weapon parts; and silver ONLY while Elden Ring's liquid silver is on him.
//!
//! ELDEN RING'S LIQUID SILVER, AS TRACED (regulation 2.7.2.0, `build/sora_tmp/party/SUMMON/reg2.js`, the paramdex defs):
//! - the bosses' events (MIMIC-BOSS STATE.md section 1: 12022865 / 30202810) give the boss SpEffect **16316** when its pool has
//!   formed it: `effectEndurance` **3** s, `vfxId` 53202 ([`LIQUID_SILVER_SPEFFECT`], [`LIQUID_SILVER_SECONDS`]);
//! - SpEffectVfx 53202: `wetAspectType` 18, `materialParamId` **-1**, `materialParamInitValue` 0, `materialParamTargetValue` 0,
//!   `materialParamFadeTime` **0** (the row's only fade is the material-parameter fade, and it is off), no init / midst / finish
//!   effect or sound, `playCategory` 7, `playPriority` 5; `unknown_0x96` / `0x98` = 150 as in 1412 of the 1513 rows (not a wet field);
//! - WetAspectParam 18: base colour (200, 200, 200) at 0.8, metallic 255 at 1.0, shininess 255 at 0.85, and NOTHING ELSE: the row
//!   has no time, fade or drying field (`baseColorR/G/B/A`, `metallic`, `metallicRate`, `shininessRate`, `shininess`, reserves);
//! - the only wetness clock in the regulation is WeatherParam `WetTime` (the weather's own wetting), not a SpEffect's.
//!   So ELDEN RING HAS NO FADE OF ITS OWN for this look in its data: the wet aspect is on while 16316 is on and off when its 3 s end
//!   ([`LIQUID_SILVER_FADE_SECONDS`] = 0). (Whether the engine eases a wet aspect off internally could not be read: there is no
//!   disassembly of eldenring.exe in this project; nothing is invented for it.)
//!
//! THE SILVER WEIGHT ([`SoraCopyDraw::silver`], the host's): 1 = the Mimic Tear surface exactly as `mimic::submit_mimic` draws it
//! today; 0 = Sora's normal KH3 look. Because the game has no fade, the timeline is a step ([`silver_after_formed`]) and a weight
//! between is drawn by the nearer end ([`route`], at [`SILVER_HALF`]): no blend is made up.
//! A copy that is FORMING (rising out of the pool, melting into it: [`MimicForm::body`] not whole) or stands over its pool is
//! always drawn by the Mimic route: that liquid is silver by nature (its forming writers and pool exist only there).
//!
//! WHAT IS HIS OWN AND WHAT IS SHARED WITH THE PLAYER (one set of assets is loaded):
//! - his own: the pose, the Keyblade in his hand ([`SoraCopyDraw::keyblade`], when that model is loaded: every slotted
//!   keyblade's base model is, kh-er-render keyblade.rs `wanted_models`), the form weapon parts ([`SoraCopyDraw::forms`], drawn
//!   when their models are loaded: those of the player's hand keyblade), the Keyblade's opacity, his bone-driven material
//!   parameters (eyes, mouth, the face's head frame), no blood, no camera fade, no Rage Form look;
//! - the player's: the OUTFIT (the worn one: a second outfit would need its own model, skeleton, library and shader sets loaded;
//!   the copy wears the leader's outfit from the fight's start and outfits change only at Boc's).
//! - (MIMIC-BOSS hotfix 3, MB3-FORMLEAK) HIS OWN (was the player's): the WEAR FORM (his costume's textures from descriptor tables of his own, his
//!   `FormChangeSlide`, his Rage Form look: crate::copy_wear) and the effects' material drives (vfx.rs `copy_drive_slot`).

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use glam::{Quat, Vec3};
use kh_anim::Trs;

use crate::mimic::{MimicDraw, MimicForm};

/// SpEffect 16316, the Mimic Tear bosses' liquid silver (vfx 53202 -> WetAspectParam 18).
pub const LIQUID_SILVER_SPEFFECT: u32 = 16316;
/// Its `effectEndurance` (s).
pub const LIQUID_SILVER_SECONDS: f32 = 3.0;
/// Elden Ring's own fade of the look at its end (s): none in the data (SpEffectVfx 53202 `materialParamFadeTime` 0,
/// `materialParamId` -1; WetAspectParam has no time field).
pub const LIQUID_SILVER_FADE_SECONDS: f32 = 0.0;
/// A weight at or above this is drawn silver, under it in his KH3 look (the game has no blend between: the nearer end).
pub const SILVER_HALF: f32 = 0.5;
/// Copies drawn at most (the KH route; the silver ones share the Mimic route's four).
pub const MAX_COPIES: usize = 4;

/// ONE SORA COPY OF THIS FRAME.
#[derive(Clone, Debug)]
pub struct SoraCopyDraw {
    /// His pose, as `FrameInput::body` (the main pack's skeleton, GAME space).
    pub body: crate::Body,
    /// His Keyblade as `FrameInput::weapon` (None = not drawn).
    pub weapon: Option<crate::Weapon>,
    /// His form weapon parts / XS form weapons as `FrameInput::forms` (keyblade.rs `draw_form_weapon_parts` /
    /// `draw_equipped_parts` on HIS pose); a part whose model is not loaded is not drawn.
    pub forms: Vec<crate::FormWeapon>,
    /// The keyblade in his hand by id (`w_so080`) or item key (`WEP_KEYBLADE_SO_06`); None = the player's.
    pub keyblade: Option<String>,
    /// His outfit id and wear form as his core has them (None = the worn ones): his outfit is drawn as the worn one (see the
    /// module), told in [`status`] when it differs. (MIMIC-BOSS hotfix 3, MB3-FORMLEAK) his wear form is HIS (with `owner`): crate::copy_wear.
    pub outfit: Option<String>,
    pub wear_form: Option<u8>,
    /// 1 = Elden Ring's liquid silver (the Mimic Tear surface of `submit_mimic`), 0 = his KH3 look ([`silver_after_formed`]).
    pub silver: f32,
    /// His rise out of the pool / melt into it ([`MimicForm::WHOLE`]: all of him).
    pub form: MimicForm,
    /// (MIMIC-BOSS drives fix) the owner of HIS material drives (kh-host `char_drives`: copy member m = m + 1): his form glow,
    /// in-light and aura on his slots instead of the player's (vfx.rs `copy_drive_slot`). None = the slots as the player has them.
    pub owner: Option<u8>,
}

impl SoraCopyDraw {
    /// A whole copy in his KH3 look, the player's Keyblade, outfit and wear form.
    pub fn new(body: crate::Body, weapon: Option<crate::Weapon>) -> SoraCopyDraw {
        SoraCopyDraw { body, weapon, forms: Vec::new(), keyblade: None, outfit: None, wear_form: None, silver: 0.0, form: MimicForm::WHOLE, owner: None }
    }

    /// The route he is drawn by this frame.
    pub fn route(&self) -> Route {
        route(self.silver, &self.form)
    }

    /// The same copy as the Mimic route's draw (his pose, his Keyblade, his forming).
    pub fn as_mimic(&self) -> MimicDraw {
        MimicDraw { body: self.body.clone(), weapon: self.weapon.clone(), form: self.form, wear_form: self.wear_form, owner: self.owner /* (MIMIC-BOSS hotfix 3, MB3-FORMLEAK) his own look on the Mimic route too */ }
    }
}

/// How a copy is drawn this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    /// By the Mimic route (`mimic::submit_mimic`'s draw: the Mimic Tear surface through the game's G-buffer, his forming, his pool).
    Silver,
    /// By the renderer's Sora route: his KH3 look at full quality (gfx/sora_copy_draw.rs).
    Kh,
}

/// THE ROUTE of a weight and a forming: forming / melting / over his pool = silver (the liquid); else the nearer end of the weight.
pub fn route(silver: f32, form: &MimicForm) -> Route {
    let liquid = form.body() != crate::mimic::BodyRoute::Whole || form.pool().is_some();
    if liquid || !silver.is_finite() || silver >= SILVER_HALF {
        Route::Silver
    } else {
        Route::Kh
    }
}

/// THE SILVER WEIGHT OF A BOSS COPY (Elden Ring's timeline, kh-er mimic_boss.rs): `formed_for` = seconds since the copy stood
/// whole out of its pool (None: not yet formed: forming is silver). 1 for SpEffect 16316's 3 s, then Elden Ring's fade (none:
/// 0 at once).
pub fn silver_after_formed(formed_for: Option<f32>) -> f32 {
    silver_with(formed_for, LIQUID_SILVER_SECONDS, LIQUID_SILVER_FADE_SECONDS)
}

/// [`silver_after_formed`] with the effect's length and a fade (s) given.
pub fn silver_with(formed_for: Option<f32>, seconds: f32, fade: f32) -> f32 {
    match formed_for {
        None => 1.0,
        Some(t) if !t.is_finite() || t < seconds => 1.0,
        Some(t) if fade > 0.0 => (1.0 - (t - seconds) / fade).clamp(0.0, 1.0),
        Some(_) => 0.0,
    }
}

/// THE KEYBLADE IN A COPY'S HAND (keyblade.rs `hand` for another keyblade): his own when its base model is loaded, the
/// Kingdom Key when his is the Kingdom Key or an unknown one, the player's hand for None; while the keyblades load: none.
pub fn hand_for(keyblade: Option<&str>) -> crate::keyblade::Hand {
    let Some(k) = keyblade.filter(|k| !k.is_empty()) else { return crate::keyblade::hand() };
    let Some(t) = crate::keyblade::table() else { return crate::keyblade::Hand::KingdomKey };
    hand_in(t.keyblade(k).map(|kb| kb.base.as_str()), &crate::keyblade::loaded(), crate::keyblade::hand() == crate::keyblade::Hand::Loading)
}

/// [`hand_for`] from the keyblade's base model (None: unknown), the loaded models and whether a load runs.
pub fn hand_in(base: Option<&str>, loaded: &[String], loading: bool) -> crate::keyblade::Hand {
    match base {
        Some(b) if b != crate::keyblade::KINGDOM_KEY && loaded.iter().any(|m| m == b) => crate::keyblade::Hand::Model(format!("{}{b}", crate::keyblade::PREFIX)),
        Some(b) if b != crate::keyblade::KINGDOM_KEY && loading => crate::keyblade::Hand::Loading,
        _ => crate::keyblade::Hand::KingdomKey,
    }
}

/// THE MIMIC ROUTE'S LIST WITH THE SILVER COPIES: the host's Mimic draws (or none) and the copies drawn silver this frame, the
/// Mimic route's draws first. Nothing added = the list as it was (the same Arc).
pub fn with_silver(mimics: Option<Arc<Vec<MimicDraw>>>, copies: &[SoraCopyDraw]) -> Option<Arc<Vec<MimicDraw>>> {
    let silver: Vec<MimicDraw> = copies.iter().filter(|c| c.route() == Route::Silver).map(SoraCopyDraw::as_mimic).collect();
    if silver.is_empty() {
        return mimics;
    }
    let mut all: Vec<MimicDraw> = mimics.map(|m| m.as_ref().clone()).unwrap_or_default();
    all.extend(silver);
    Some(Arc::new(all))
}

/// The copies drawn by the KH route this frame (at most [`MAX_COPIES`]).
pub fn kh_copies(copies: &[SoraCopyDraw]) -> Vec<&SoraCopyDraw> {
    copies.iter().filter(|c| c.route() == Route::Kh).take(MAX_COPIES).collect()
}

/// THE REACH OF THE KH-ROUTE COPIES (the hybrid's capture and G-buffer box, hybrid.rs bone_balls): added to the player's; none
/// added = the player's reach as it was.
pub fn reach_of(copies: &[&SoraCopyDraw]) -> Vec<(Vec3, Vec3, f32)> {
    let mut out = Vec::new();
    for c in copies {
        out.extend(crate::hybrid::bone_balls(&c.body.component, &c.body.component_to_world));
        if let Some(w) = c.weapon.as_ref().filter(|w| w.opacity > 0.5) {
            for (a, b, r) in crate::shadowcaps::keyblade_capsules(w) {
                out.push((a, b, r));
            }
        }
        for f in &c.forms {
            let p = f.to_world.translation;
            if p.is_finite() {
                out.push((p, p, 150.0));
            }
        }
    }
    out
}

static COPIES: Mutex<Option<(Arc<Vec<SoraCopyDraw>>, Instant)>> = Mutex::new(None);
static WANTED: AtomicBool = AtomicBool::new(false);
static STATUS: Mutex<String> = Mutex::new(String::new());

/// THE SORA COPIES OF THIS FRAME (any thread, once a frame after crate::submit); lapses 500 ms after the latest call (as the
/// Mimic list); an empty list draws none. The silver ones are drawn by the Mimic route, the others by the Sora route.
pub fn submit(draws: Vec<SoraCopyDraw>) {
    if !draws.is_empty() {
        WANTED.store(true, Ordering::Relaxed);
        // (the silver ones are the Mimic route's: its states made as for a submitted mimic)
        crate::mimic::preload_mimic();
    }
    if let Ok(mut g) = COPIES.lock() {
        *g = if draws.is_empty() { None } else { Some((Arc::new(draws), Instant::now())) };
    }
}

/// No copy is drawn until the next submission.
pub fn clear() {
    if let Ok(mut g) = COPIES.lock() {
        *g = None;
    }
}

/// The copies in force now (the host's).
pub(crate) fn draws_now() -> Option<Arc<Vec<SoraCopyDraw>>> {
    let g = COPIES.lock().ok()?;
    g.as_ref().filter(|(d, at)| !d.is_empty() && crate::mimic::in_force(*at, Instant::now())).map(|(d, _)| d.clone())
}

/// A copy was ever submitted (or the testing aid is on).
pub fn wanted() -> bool {
    WANTED.load(Ordering::Relaxed) || test_copies().is_some()
}

/// One line for the host's once-a-second log.
pub fn status() -> String {
    let s = STATUS.lock().map(|s| s.clone()).unwrap_or_default();
    format!("(MIMIC-RENDER) Sora copies: {}", if s.is_empty() { if wanted() { "submitted, not drawn yet".to_string() } else { "none submitted".to_string() } } else { s })
}

pub(crate) fn set_status(text: String) {
    if let Ok(mut s) = STATUS.lock() {
        *s = text;
    }
}

/// What of a copy is drawn as the player's (told in the status): his outfit when it differs from the worn one; (MIMIC-BOSS hotfix 3, MB3-FORMLEAK) his
/// wear form only when he has no look of his own (no `owner`: the testing aid).
pub fn shared_note(copy: &SoraCopyDraw, worn_outfit: Option<&str>, worn_form: u8) -> Option<String> {
    let mut notes = Vec::new();
    if let Some(o) = copy.outfit.as_deref().filter(|o| Some(*o) != worn_outfit) {
        notes.push(format!("his outfit {o} drawn as the worn {}", worn_outfit.unwrap_or("kh3")));
    }
    if let Some(w) = copy.wear_form.filter(|w| *w != worn_form && copy.owner.is_none()) {
        notes.push(format!("his wear form {w} drawn as the worn {worn_form}"));
    }
    (!notes.is_empty()).then(|| notes.join(", "))
}

// ---------------------------------------------------------------------------------------------------- the testing aid

/// (testing aid, the offline harnesses only: environment `KH_SORA_COPY_TEST = X,Y[,SILVER[,YAW[,LAG]]]`, several copies
/// separated by `;`) a copy of the frame's own Sora moved by (X, Y) cm in game space, turned YAW degrees about his feet, posed
/// as the player was LAG frames before (his own pose), at the silver weight SILVER (default 0): the real renderer's copy path
/// offline, no host submitting one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TestCopy {
    pub offset: Vec3,
    pub silver: f32,
    pub yaw_deg: f32,
    pub lag: usize,
}

/// The aid's copies from its text (None: no copy given).
pub fn parse_test(text: &str) -> Option<Vec<TestCopy>> {
    let copies: Vec<TestCopy> = text
        .split(';')
        .filter_map(|one| {
            let v: Vec<f32> = one.split(',').filter_map(|p| p.trim().parse().ok()).collect();
            (v.len() >= 2).then(|| TestCopy { offset: Vec3::new(v[0], v[1], 0.0), silver: v.get(2).copied().unwrap_or(0.0), yaw_deg: v.get(3).copied().unwrap_or(0.0), lag: v.get(4).map_or(0, |l| l.max(0.0) as usize) })
        })
        .collect();
    (!copies.is_empty()).then_some(copies)
}

fn test_copies() -> Option<&'static Vec<TestCopy>> {
    static TEST: std::sync::OnceLock<Option<Vec<TestCopy>>> = std::sync::OnceLock::new();
    TEST.get_or_init(|| std::env::var("KH_SORA_COPY_TEST").ok().and_then(|t| parse_test(&t))).as_ref()
}

/// A transform turned `yaw` about the point `pivot` (game space, about +Z), then moved by `offset`.
pub fn moved(t: &Trs, pivot: Vec3, yaw: Quat, offset: Vec3) -> Trs {
    Trs { translation: yaw * (t.translation - pivot) + pivot + offset, rotation: yaw * t.rotation, scale: t.scale }
}

type Posed = (crate::Body, Option<crate::Weapon>, Vec<crate::FormWeapon>);
static HISTORY: Mutex<VecDeque<Posed>> = Mutex::new(VecDeque::new());

/// The test copies of a frame from the player's frame input (the aid's pose history stepped once a call).
fn test_draws(input: &crate::FrameInput) -> Option<Vec<SoraCopyDraw>> {
    let copies = test_copies()?;
    let body = input.body.clone()?;
    let mut history = HISTORY.lock().ok()?;
    history.push_front((body, input.weapon.clone(), input.forms.clone()));
    let keep = copies.iter().map(|c| c.lag).max().unwrap_or(0) + 1;
    history.truncate(keep);
    let out = copies
        .iter()
        .filter_map(|c| {
            let (body, weapon, forms) = history.get(c.lag).or_else(|| history.back())?;
            let pivot = body.component_to_world.translation;
            let yaw = Quat::from_rotation_z(c.yaw_deg.to_radians());
            let mut d = SoraCopyDraw::new(crate::Body { component_to_world: moved(&body.component_to_world, pivot, yaw, c.offset), local: body.local.clone(), component: body.component.clone() }, weapon.clone().map(|mut w| {
                w.to_world = moved(&w.to_world, pivot, yaw, c.offset);
                w
            }));
            d.forms = forms.iter().cloned().map(|mut f| {
                f.to_world = moved(&f.to_world, pivot, yaw, c.offset);
                f
            }).collect();
            d.silver = c.silver;
            Some(d)
        })
        .collect();
    Some(out)
}

/// THE FRAME'S COPIES: the host's, else (testing aid) the copies `KH_SORA_COPY_TEST` asks for. None: no copy (the frame is the
/// player's alone, as before).
pub(crate) fn draws_or_test(input: &crate::FrameInput) -> Option<Arc<Vec<SoraCopyDraw>>> {
    draws_now().or_else(|| {
        let d = test_draws(input).filter(|d| !d.is_empty())?;
        crate::mimic::preload_mimic();
        Some(Arc::new(d))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body_at(x: f32) -> crate::Body {
        crate::Body { component_to_world: Trs { translation: Vec3::new(x, 0.0, 0.0), ..Trs::IDENTITY }, local: vec![Trs::IDENTITY; 3], component: vec![Trs::IDENTITY; 3] }
    }

    #[test]
    fn copy_the_silver_is_er_s_3_seconds_of_16316_then_off_at_once() {
        // (regulation 2.7.2.0: SpEffect 16316 effectEndurance 3, vfx 53202 materialParamFadeTime 0 / materialParamId -1)
        assert_eq!((LIQUID_SILVER_SPEFFECT, LIQUID_SILVER_SECONDS, LIQUID_SILVER_FADE_SECONDS), (16316, 3.0, 0.0));
        assert_eq!(silver_after_formed(None), 1.0, "forming: silver");
        assert_eq!(silver_after_formed(Some(0.0)), 1.0);
        assert_eq!(silver_after_formed(Some(2.999)), 1.0);
        assert_eq!(silver_after_formed(Some(3.0)), 0.0, "the effect's end: his KH3 look");
        assert_eq!(silver_after_formed(Some(60.0)), 0.0);
        assert_eq!(silver_after_formed(Some(f32::NAN)), 1.0, "no clock: kept silver, never a flash");
        // a fade, were one given: linear from the end over its length
        assert_eq!(silver_with(Some(3.5), 3.0, 1.0), 0.5);
        assert_eq!(silver_with(Some(4.5), 3.0, 1.0), 0.0);
        // the timeline drawn: silver through the effect, his KH3 look after
        let whole = MimicForm::WHOLE;
        assert_eq!(route(silver_after_formed(Some(1.0)), &whole), Route::Silver);
        assert_eq!(route(silver_after_formed(Some(3.2)), &whole), Route::Kh);
    }

    #[test]
    fn copy_the_route_is_the_nearer_end_and_the_liquid_is_always_silver() {
        let whole = MimicForm::WHOLE;
        assert_eq!(route(1.0, &whole), Route::Silver, "1 = exactly the Mimic route's draw");
        assert_eq!(route(0.0, &whole), Route::Kh, "0 = his KH3 look");
        assert_eq!(route(0.49, &whole), Route::Kh);
        assert_eq!(route(SILVER_HALF, &whole), Route::Silver);
        assert_eq!(route(f32::NAN, &whole), Route::Silver);
        // forming, melting, nothing of him, or over his pool: the Mimic route whatever the weight (its writers and pool are there)
        let f = |height: f32, pool: f32| MimicForm { height, feet_z: 0.0, tall: 180.0, pool };
        for form in [f(0.5, 0.0), f(0.0, 60.0), f(0.0, 0.0), f(1.0, 30.0)] {
            assert_eq!(route(0.0, &form), Route::Silver, "{form:?}");
        }
        assert_eq!(route(0.0, &f(1.0, 0.2)), Route::Kh, "a pool under half a centimetre is none");
    }

    #[test]
    fn copy_no_copy_leaves_the_player_s_frame_as_it_was() {
        // nothing submitted: no copy in force, the Mimic list untouched (the same Arc), no copy on the KH route, no reach added
        clear();
        assert!(draws_now().is_none());
        let mimics = Some(Arc::new(vec![MimicDraw::new(body_at(1.0), None)]));
        let same = with_silver(mimics.clone(), &[]);
        assert!(Arc::ptr_eq(mimics.as_ref().unwrap(), same.as_ref().unwrap()), "the very list");
        assert!(with_silver(None, &[]).is_none());
        assert!(kh_copies(&[]).is_empty());
        assert!(reach_of(&[]).is_empty());
        // a KH-route copy only: the Mimic list untouched as well
        let kh = SoraCopyDraw::new(body_at(100.0), None);
        let same = with_silver(mimics.clone(), std::slice::from_ref(&kh));
        assert!(Arc::ptr_eq(mimics.as_ref().unwrap(), same.as_ref().unwrap()));
        assert_eq!(kh_copies(std::slice::from_ref(&kh)).len(), 1);
        // submit / lapse (the only test that touches the shared list)
        submit(vec![kh.clone()]);
        assert!(draws_now().is_some_and(|d| d.len() == 1) && wanted());
        submit(Vec::new());
        assert!(draws_now().is_none(), "an empty list draws none");
        assert!(status().starts_with("(MIMIC-RENDER)"));
    }

    #[test]
    fn copy_silver_copies_join_the_mimic_route_and_the_kh_ones_are_capped() {
        let mut silver = SoraCopyDraw::new(body_at(200.0), None);
        silver.silver = 1.0;
        let kh = SoraCopyDraw::new(body_at(300.0), None);
        let list = with_silver(Some(Arc::new(vec![MimicDraw::new(body_at(1.0), None)])), &[silver.clone(), kh.clone()]).unwrap();
        assert_eq!(list.len(), 2, "the spirit's draw first, then the silver copy");
        assert_eq!(list[1].body.component_to_world.translation.x, 200.0);
        assert_eq!(list[1].form, MimicForm::WHOLE);
        let only = with_silver(None, std::slice::from_ref(&silver)).unwrap();
        assert_eq!(only.len(), 1);
        let many = vec![kh.clone(); 9];
        assert_eq!(kh_copies(&many).len(), MAX_COPIES);
        // the reach: his bones' balls (one per bone)
        assert_eq!(reach_of(&[&kh]).len(), crate::hybrid::bone_balls(&kh.body.component, &kh.body.component_to_world).len());
    }

    #[test]
    fn copy_his_keyblade_is_his_own_when_loaded() {
        use crate::keyblade::Hand;
        let loaded = vec!["w_so080".to_string()];
        assert_eq!(hand_in(Some("w_so080"), &loaded, false), Hand::Model("kb:w_so080".to_string()));
        assert_eq!(hand_in(Some(crate::keyblade::KINGDOM_KEY), &loaded, false), Hand::KingdomKey);
        assert_eq!(hand_in(None, &loaded, false), Hand::KingdomKey, "an unknown keyblade: the Kingdom Key");
        assert_eq!(hand_in(Some("w_so150"), &loaded, true), Hand::Loading, "being loaded: none drawn, as the player's");
        assert_eq!(hand_in(Some("w_so150"), &loaded, false), Hand::KingdomKey);
    }

    #[test]
    fn copy_the_shared_look_is_told() {
        let mut c = SoraCopyDraw::new(body_at(0.0), None);
        assert_eq!(shared_note(&c, None, 0), None);
        c.wear_form = Some(5);
        c.outfit = Some("kh2".to_string());
        let note = shared_note(&c, Some("kh3"), 0).unwrap();
        assert!(note.contains("outfit kh2") && note.contains("wear form 5"), "{note}");
        c.wear_form = Some(0);
        c.outfit = Some("kh3".to_string());
        assert_eq!(shared_note(&c, Some("kh3"), 0), None);
        // (MIMIC-BOSS hotfix 3, MB3-FORMLEAK) a copy with his own look wears his own form: nothing of it is the player's
        c.wear_form = Some(5);
        c.owner = Some(9);
        assert_eq!(shared_note(&c, Some("kh3"), 0), None);
    }

    #[test]
    fn copy_the_testing_aid_moves_turns_and_delays_him() {
        assert_eq!(parse_test("150,0"), Some(vec![TestCopy { offset: Vec3::new(150.0, 0.0, 0.0), silver: 0.0, yaw_deg: 0.0, lag: 0 }]));
        let two = parse_test("150,0,1; -150,20,0,180,12").unwrap();
        assert_eq!(two.len(), 2);
        assert_eq!((two[0].silver, two[1].yaw_deg, two[1].lag), (1.0, 180.0, 12));
        assert_eq!(parse_test(""), None);
        assert_eq!(parse_test("x"), None);
        // turned about his feet, then moved
        let t = Trs { translation: Vec3::new(10.0, 0.0, 5.0), ..Trs::IDENTITY };
        let m = moved(&t, Vec3::ZERO, Quat::from_rotation_z(std::f32::consts::PI), Vec3::new(100.0, 0.0, 0.0));
        assert!((m.translation - Vec3::new(90.0, 0.0, 5.0)).length() < 1e-4, "{}", m.translation);
    }
}
