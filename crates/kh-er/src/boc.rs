//! (IMPL_ER59, the user: "Boc is able to change Sora's appearance: KH1, KH2, and all of Sora's world outfits in 3")
//! SORA'S OUTFITS, OPENED BY BOC THE SEAMSTER. State note: build/sora_tmp/er/IMPL_ER59_STATE.md.
//!
//! WHAT IS OFFERED: the outfits of KH3's own data (`PACK/outfits/outfits.json`, tools/sora_outfit_pack.js: KH3's
//! default, KH2, KH1, Toy Box, Monstropolis, The Caribbean, San Fransokyo), in the KINGDOM HEARTS CAMP MENU as the
//! entry "Outfits" (kh-ui: a list drawn in the Game Help screen's category list; pad: up / down, A wears, B back).
//!
//! WHY THE CAMP MENU AND NOT BOC'S OWN "ALTER GARMENTS" (PARSED, Boc's talk scripts t223006000 (open world),
//! t223001400, t223001100, t223001105, extracted read-only into build/sora_tmp/er/er59/game): "Alter garments" (text
//! 22230000) is entry 1 of his list; choosing it runs talk command c1_142(110000, 110399) = the game's tailoring
//! window, and the script then waits for menu kind 26 to close. Nothing of it sets a flag, and the list's cursor is not
//! readable (talkdrive.rs): the moment that window opens cannot be told from another window with certainty, and
//! closing the game's window for the player would need presses the user could not see coming. So: NOT intercepted.
//! THE GATE IS BOC: the entry appears once the player has TALKED TO BOC (Elden Ring's popup menu carries one of his
//! talk ids, 223000000 ..= 223099999: `PopupState::talk_id`, the same field that MEASURED 1000 / 3000 for the grace /
//! Melina, talkdrive.rs). That is kept in Sora's save as the record `boc:met`.
//!
//! SAVED: the chosen outfit is the record `outfit:<id>` of Sora's save (`.khsave` version 3's once-only list,
//! kh_host::save): no new version, so an older DLL reads and keeps the file (it carries the records as they are).
//!
//! FORMS: an outfit whose pawn has a wear form for the form put on (`wear_forms`: Toy Box, Monstropolis, The Caribbean,
//! KH3's own: Second Form) slides into that form's costume itself (kh-er-render, the outfit's WearformSet). (ER69, the
//! user: "do NOT switch to KH3's outfit") One that has none KEEPS ITS CLOTHES and takes the form's look from
//! wear_extra.json (tools/sora_outfit_forms.js): San Fransokyo's clothes are KH3 Sora's own materials (KH3's Second Form
//! costume on them), KH1's and KH2's are recoloured by a table learnt from KH3's default -> Second Form textures.
//! `boc.form_outfit = 1`: the older rule (KH3's default outfit for the form).
//!
//! | key | default | meaning |
//! |---|---|---|
//! | `boc.outfits` | 1 | 0 = no Outfits entry and KH3's default outfit always (as before) |
//! | `boc.gate` | 1 | 0 = the entry without having talked to Boc (testing) |
//! | `boc.kh1_form_style` | 1 | (ER69) KH1 Sora's Second Form: 1 = the styled look (regions to black / silver / gold, trim lines), 0 = the plain recolour |
//! | `boc.form_outfit` | 0 | 1 = an outfit without the form's costume wears KH3's default outfit in a form (the older rule) |
//! | `boc.outfit` | - | testing aid: wear this outfit id now (not saved) |
//! | `boc.own_clips` | 1 | (ER69) 0 = an outfit plays Sora's clips (retargeted) even where it has its own |
//! | `boc.<outfit>.use_kh3` | kh1: `idle,battle_stand,stop,land`; others: none | (ER79, the user: "for KH1 Sora, can he use KH3 Sora's standing pose adjusted for his skeleton?") slots (kh-anim outfit_clips.rs: idle, battle_stand, stop, land, run_start, walk, run, jump, face, all) or clip names that play KH3 Sora's clip, retargeted onto the outfit, instead of its own; `0` = its own clips everywhere it has them. Read every frame: a change applies at once |
//! | `boc.outfit_kd` | 1 | (ER73) 0 = an outfit's own KineDriver off (its knee / hip / shoulder pieces follow Sora's helper bones, model.glb); read when an outfit is put on |
//! | `boc.leg_guard` | 1 | (ER79, the user: "I just want his pants to never clip into his leg") 0 = an outfit's leg guard off (legguard.json, enabled for kh1 and caribbean: the cloth bones pushed out of capsules around its legs after its Bonamik, every cloth vertex out of them in the skin shader `skin_cg.vs.dxil`); read when an outfit is put on |
//! | `boc.cloth_twosided` | 1 | (ER79, the user's 54.jpg: the shorts' hem looked hollow from below) 0 = an outfit's cloth slots (twosided.json, enabled for kh1) drawn one-sided again; on: both faces, the back faces shaded as the cloth's dark inside (the `_bli` shaders); read when an outfit is put on |
//! | `boc.<outfit>.own_shaders` | 1 | (ER81, the user: "please give verum rex Sora his own shaders so he looks correct") 0 = the outfit is drawn with the KH World set (KHS, the toon colour table) instead of its own (`own_shaders.json`: verumrex = KHS_CG + Quadratum's controller, WorldTypeInt 1); `boc.own_shaders = 0` = every outfit; a change reloads the outfit at once |
//! | `boc.<outfit>.own_preset` | the set's | (ER81) the KHS preset of the outfit's own set: verumrex `quadratum` (default: Quadratum's controller with the Pixar colour table off, matched to the official render the user gave) or `quadratum_game` (the game data's values, the table on: a red grain on the dark cloth); a change reloads the outfit |
//! | `boc.<outfit>.own_look` | 1 | (ER85, the user: "Verum Rex must look perfect") 0 = the outfit's own look file (`own_look.json`, verumrex: eyes and hair values chosen against the official render, his field brightness) not used; `boc.own_look = 0` = every outfit; read at the outfit's load |
//! | `r.own_field` | 1 | (ER85, the user's 58.jpg / 59.jpg) 0 = an outfit with its own set (Quadratum, San Fransokyo, Toy Box, Monstropolis, Caribbean) lit by the game's light as given, without the reshaping (undirected share held, key in front of his facing, neutral colours); `r.own_field_ambient` / `_studio` / `_neutral` / `_lift` tune it |
//! | `boc.grip_ik` | 1 | (ER83, the user: "KH1 Sora's other hand is still not gripping the keyblade properly", "his hand is not on his hip") 0 = KH1's hand contact IK off (kh-er-render handik.rs: where KH3 Sora's off-hand is on the Keyblade's grip or a hand on his hip / thigh, KH1's hand is pulled to the same place, mapped onto KH1's own body, fingers his); KH1 only; read every frame |
//! | `boc.grip_main` | 1 | (ER83) 0 = KH1's main hand keeps its retargeted fingers (on: KH3 Sora's fist copied onto them in the palm's frame); read every frame |
//! | `boc.kh1_voice` | 1 | (ER77) 0 = KH1 Sora speaks with KH3 Sora's voice; 1 = every voice line is one of the user's KH1 clips (or silent), fx.rs / kh-audio outfit.rs |

use std::path::Path;
use std::sync::Mutex;

use crate::control::Control;
use crate::erlog;

/// One outfit of `outfits.json`.
#[derive(Clone, Debug, PartialEq)]
pub struct Outfit {
    pub id: String,
    pub name: String,
    /// The wear forms its pawn has textures for (5 = Second Form).
    pub wear_forms: Vec<u8>,
    /// Its hip height over KH3 Sora's (KH1: 0.866).
    pub scale: f32,
}

/// Boc's talk scripts (t2230NNNNN, PARSED: t223006000 / t223001100 / t223001105 / t223001400).
pub fn is_boc_talk(talk_id: i32) -> bool {
    (223_000_000..=223_099_999).contains(&talk_id)
}

pub const MET: &str = "boc:met";
pub const OUTFIT_PREFIX: &str = "outfit:";

/// The outfit recorded in the save ("kh3" when none).
pub fn chosen(records: &[String]) -> String {
    records.iter().rev().find_map(|r| r.strip_prefix(OUTFIT_PREFIX)).unwrap_or("kh3").to_string()
}

/// Records the choice (one `outfit:` record at most).
pub fn choose(records: &mut Vec<String>, id: &str) {
    records.retain(|r| !r.starts_with(OUTFIT_PREFIX));
    if id != "kh3" {
        records.push(format!("{OUTFIT_PREFIX}{id}"));
    }
}

/// The list of `outfits.json`.
pub fn parse(text: &str) -> Vec<Outfit> {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(text.trim_start_matches('\u{feff}')) else { return Vec::new() };
    json["outfits"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|o| {
            Some(Outfit {
                id: o["id"].as_str()?.to_string(),
                name: o["name"].as_str().unwrap_or(o["id"].as_str()?).to_string(),
                wear_forms: o["wear_forms"].as_array().map(|a| a.iter().filter_map(|f| f.as_u64().map(|f| f as u8)).collect()).unwrap_or_default(),
                scale: o["scale"].as_f64().unwrap_or(1.0) as f32,
            })
        })
        .collect()
}

/// THE OUTFIT DRAWN: the chosen one, or KH3's default while a form is on that the chosen one has no costume for.
pub fn effective<'a>(outfits: &'a [Outfit], chosen: &str, wear_form: u8, form_fallback: bool) -> Option<&'a Outfit> {
    let own = outfits.iter().find(|o| o.id == chosen).or_else(|| outfits.iter().find(|o| o.id == "kh3"))?;
    if form_fallback && wear_form != 0 && !own.wear_forms.contains(&wear_form) {
        return outfits.iter().find(|o| o.id == "kh3").or(Some(own));
    }
    Some(own)
}

struct State {
    outfits: Vec<Outfit>,
    /// Boc's talk was seen and not yet written into the records.
    met_seen: bool,
    met: bool,
    chosen: String,
    drawn: String,
    camera_base: Option<f32>,
    scale: f32,
    /// The pack (the outfits' folders) and the folder of own clips the animator should take (None inside = his own).
    pack: std::path::PathBuf,
    /// (ER79) with the clips that stay KH3 Sora's (`boc.<outfit>.use_kh3`)
    clip_dir: Option<Option<(std::path::PathBuf, std::collections::BTreeSet<String>)>>,
    /// (ER79) outfit, own clips on, the KH3 list: what the animator was last given
    clips_key: String,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

/// Reads the list (`PACK/outfits/outfits.json`; none = no outfits, nothing changes). At Sora's start.
pub fn load(pack: &Path) {
    let file = pack.join("outfits").join("outfits.json");
    let outfits = std::fs::read_to_string(&file).map(|t| parse(&t)).unwrap_or_default();
    erlog!("boc: {} outfit(s) in {}: {}", outfits.len(), file.display(), outfits.iter().map(|o| o.id.as_str()).collect::<Vec<_>>().join(", "));
    if let Ok(mut g) = STATE.lock() {
        let (met, chosen) = g.as_ref().map(|s| (s.met, s.chosen.clone())).unwrap_or((false, "kh3".to_string()));
        *g = Some(State { outfits, met_seen: false, met, chosen, drawn: String::new(), camera_base: None, scale: 1.0, pack: pack.to_path_buf(), clip_dir: None, clips_key: String::new() });
    }
}

/// ONCE A FRAME (sora.rs `submit`): Boc's talk seen, and the outfit the renderer draws.
pub fn frame(control: &Control, talk_id: Option<i32>, wear_form: u8) {
    let Ok(mut g) = STATE.lock() else { return };
    let Some(s) = g.as_mut() else { return };
    if talk_id.is_some_and(is_boc_talk) && !s.met {
        s.met = true;
        s.met_seen = true;
        erlog!("boc: Boc's talk ({}) is up: Sora's Outfits are open in the Kingdom Hearts menu from now on", talk_id.unwrap_or(0));
    }
    let on = control.flag("boc.outfits") != Some(false);
    // (ER83) KH1's hands on the Keyblade / his hip (handik.rs), read every frame
    #[cfg(windows)]
    {
        kh_er_render::handik::set_enabled(control.flag("boc.grip_ik") != Some(false));
        kh_er_render::handik::set_main_fingers(control.flag("boc.grip_main") != Some(false));
    }
    let chosen = control.str("boc.outfit").map(str::to_string).unwrap_or_else(|| s.chosen.clone());
    let fallback = control.flag("boc.form_outfit") == Some(true);
    let want = if on { effective(&s.outfits, &chosen, wear_form, fallback).cloned() } else { None };
    let (id, scale) = want.map_or(("kh3".to_string(), 1.0), |o| (o.id, o.scale));
    // (ER81) an outfit's own shader set (its world's post-process material: Verum Rex's KHS_CG, Quadratum's controller):
    // `boc.<outfit>.own_shaders = 0` (`boc.own_shaders = 0`: every outfit) = the KH World set; a change reloads the drawn outfit
    #[cfg(windows)]
    {
        let mut off: Vec<String> = s.outfits.iter().filter(|o| control.flag(&format!("boc.{}.own_shaders", o.id)) == Some(false)).map(|o| o.id.clone()).collect();
        if control.flag("boc.own_shaders") == Some(false) {
            off.push("*".to_string());
        }
        kh_er_render::ownset::set_off(off);
        // (ER85) a set that is off by default (Toy Box, Monstropolis, the Caribbean): `boc.<outfit>.own_shaders = 1` switches it on
        kh_er_render::ownset::set_on(s.outfits.iter().filter(|o| control.flag(&format!("boc.{}.own_shaders", o.id)) == Some(true)).map(|o| o.id.clone()).collect());
        // `boc.<outfit>.own_preset = NAME`: another preset of its own set (verumrex: quadratum_game = the game data's values,
        // the Pixar colour table on; default quadratum = matched to the official render)
        kh_er_render::ownset::set_presets(s.outfits.iter().filter_map(|o| control.str(&format!("boc.{}.own_preset", o.id)).map(|p| (o.id.clone(), p.to_string()))).collect());
        // (ER85) `boc.<outfit>.own_look = 0` (`boc.own_look = 0`: every outfit): the outfit's own look file (own_look.json:
        // values chosen against a reference over its materials, its field light) not used; read at the outfit's load
        let mut look_off: Vec<String> = s.outfits.iter().filter(|o| control.flag(&format!("boc.{}.own_look", o.id)) == Some(false)).map(|o| o.id.clone()).collect();
        if control.flag("boc.own_look") == Some(false) {
            look_off.push("*".to_string());
        }
        kh_er_render::cgfield::set_look_off(look_off);
        // (ER85) the field light of an outfit drawn with its own set (kh-er-render cgfield.rs): `r.own_field = 0` = the game's
        // light as given (before ER85); `r.own_field_ambient` (the undirected share, 0.2), `r.own_field_studio` (how far a key
        // from behind him turns to the key in front of his facing, 0.85), `r.own_field_neutral` (0.6), `r.own_field_lift` (x his
        // brightness, times the outfit's own: 1)
        let d = kh_er_render::cgfield::FieldShape::default();
        kh_er_render::cgfield::set_shape(kh_er_render::cgfield::FieldShape { on: control.flag("r.own_field") != Some(false), ambient_max: control.f32("r.own_field_ambient").unwrap_or(d.ambient_max), studio_max: control.f32("r.own_field_studio").unwrap_or(d.studio_max), neutral_key: control.f32("r.own_field_neutral").unwrap_or(d.neutral_key), lift: control.f32("r.own_field_lift").unwrap_or(d.lift), ..d });
    }
    if id != s.drawn {
        // (ER73) the outfit's own KineDriver (Toy Box's and Monstropolis's legs), read at the load
        #[cfg(windows)]
        kh_er_render::outfit::set_kinedriver(control.flag("boc.outfit_kd") != Some(false));
        // (ER79) the outfit's leg guard (its cloth kept out of its legs) and its cloth drawn two-sided, read at the load
        #[cfg(windows)]
        kh_er_render::outfit::set_leg_guard(control.flag("boc.leg_guard") != Some(false));
        #[cfg(windows)]
        kh_er_render::outfit::set_cloth_twosided(control.flag("boc.cloth_twosided") != Some(false));
        erlog!("boc: OUTFIT {} -> {id} (chosen {chosen}, wear form {wear_form})", if s.drawn.is_empty() { "-" } else { &s.drawn });
        s.drawn = id.clone();
        #[cfg(windows)]
        kh_er_render::set_outfit(&id);
    }
    // (ER69) its own clips: the animator takes them at its next step (`boc.own_clips = 0`: his only); (ER79) without the
    // slots of `boc.<outfit>.use_kh3` (KH1: KH3 Sora's standing pose), re-given whenever one of the keys changes
    let own = control.flag("boc.own_clips") != Some(false);
    let key = format!("boc.{id}.use_kh3");
    let list = control.str(&key).unwrap_or(kh_anim::outfit_clips::default_for(&id)).to_string();
    let clips_key = format!("{id}|{own}|{list}");
    if clips_key != s.clips_key {
        let dir = s.pack.join("outfits").join(&id);
        let kh3 = kh_anim::outfit_clips::kh3_clips(&list);
        if id != "kh3" && own && dir.is_dir() {
            erlog!("boc: {id}'s own clips, KH3 Sora's (retargeted) for {key} = \"{list}\" ({} clip name(s))", kh3.len());
        }
        s.clip_dir = Some((id != "kh3" && own && dir.is_dir()).then_some((dir, kh3)));
        s.clips_key = clips_key;
    }
    s.scale = scale;
    // (ER69) KH1 Sora's Second Form look (`boc.kh1_form_style = 0`: the plain recolour)
    #[cfg(windows)]
    kh_er_render::set_form_style(u8::from(control.flag("boc.kh1_form_style") != Some(false)));
}

/// (ER77) The outfit drawn now ("kh3" when none / not yet known): fx.rs gives the KH1 outfit its own voice by it.
pub fn worn() -> String {
    STATE.lock().ok().and_then(|g| g.as_ref().map(|s| s.drawn.clone())).filter(|d| !d.is_empty()).unwrap_or_else(|| "kh3".to_string())
}

/// The outfit whose own voice table the audio side uses (kh-audio outfit.rs). THE RELEASED MOD HAS NONE: every outfit,
/// the KH1 outfit too, speaks with KH3 Sora's own battle voice (the same lines and selection as his normal outfit). The
/// development build's KH1 voice (the user's own recordings, `boc.kh1_voice`) is not part of the release.
pub fn voice_outfit(_worn: &str, _kh1_voice_key: Option<bool>) -> Option<&'static str> {
    None
}

/// (ER69) The worn outfit's body scale (sora.rs: the collision capsule; 1 = KH3 Sora's).
pub fn body_scale() -> f32 {
    STATE.lock().ok().and_then(|g| g.as_ref().map(|s| s.scale)).unwrap_or(1.0)
}

/// (ER69) The folder of the worn outfit's own clips, once after it changed (sora.rs `step_pose_over`).
pub fn take_clip_dir() -> Option<Option<(std::path::PathBuf, std::collections::BTreeSet<String>)>> {
    STATE.lock().ok().and_then(|mut g| g.as_mut().and_then(|s| s.clip_dir.take()))
}

/// (ER72, the user: the form's aura on KH1 Sora's own bones, scaled to his body) THE POSE THE EFFECTS HANG ON: with an
/// outfit on ((ER73) EVERY outfit, the user: "effects for keyblade need to match for KH1 sora's keyblade position": the
/// Keyblade's trail, glints, hit sparks and cast points hang on the worn outfit's hand, where its Keyblade is drawn), every bone of Sora's skeleton that the outfit has takes the
/// outfit's bone (its retargeted pose, kh-er-render outfit.rs, without its own Bonamik), its scale times the body scale
/// (an effect sized by its socket shrinks with him). `None`: his own pose (KH3's proportions, or no outfit).
#[cfg(windows)]
pub fn effect_pose(skeleton: &kh_anim::Skeleton, pose: &kh_anim::Pose) -> Option<kh_anim::Pose> {
    static CACHE: Mutex<Option<(String, std::sync::Arc<kh_er_render::outfit::Retarget>)>> = Mutex::new(None);
    let (id, scale, pack) = {
        let g = STATE.lock().ok()?;
        let s = g.as_ref()?;
        (s.drawn.clone(), s.scale, s.pack.clone())
    };
    let _ = scale;
    if id.is_empty() || id == "kh3" {
        return None;
    }
    let mut cache = CACHE.lock().ok()?;
    if cache.as_ref().map(|(c, _)| c != &id).unwrap_or(true) {
        let r = kh_er_render::outfit::retarget_for(&pack, &id, skeleton).ok().flatten()?;
        *cache = Some((id.clone(), std::sync::Arc::new(r)));
    }
    let r = cache.as_ref()?.1.clone();
    drop(cache);
    let mut out = pose.clone();
    out.component = r.effect_component(skeleton, &pose.local, &pose.component);
    Some(out)
}

/// IN THE HUD'S FRAME (kh-er hud.rs, under its lock): the camp menu's entry, a choice made there, the records.
pub fn camp(control: &Control, hud: &mut kh_host::hud::Hud) {
    let Ok(mut g) = STATE.lock() else { return };
    let Some(s) = g.as_mut() else { return };
    let records = &mut hud.inventory_mut().records;
    // the save's state (a character loaded) and what was seen since
    if s.met_seen && !records.iter().any(|r| r == MET) {
        records.push(MET.to_string());
    }
    s.met_seen = false;
    s.met = records.iter().any(|r| r == MET);
    s.chosen = chosen(records);
    if let Some(id) = hud.take_outfit_choice() {
        choose(&mut hud.inventory_mut().records, &id);
        s.chosen = id.clone();
        erlog!("boc: the Kingdom Hearts menu's Outfits: {id} chosen (record \"{OUTFIT_PREFIX}{id}\" in Sora's save at its next write)");
    }
    let open = control.flag("boc.outfits") != Some(false) && (s.met || control.flag("boc.gate") == Some(false)) && s.outfits.len() > 1;
    hud.outfit_menu = open.then(|| (s.outfits.iter().map(|o| (o.id.clone(), o.name.clone())).collect(), s.chosen.clone()));
}

/// (cam.rs) The camera's target height follows the outfit's body (KH1 Sora is shorter): `height` = the rig's
/// `target_socket_height`, rewritten from its first value times the outfit's scale.
pub fn camera_socket(height: &mut f32) {
    let Ok(mut g) = STATE.lock() else { return };
    let Some(s) = g.as_mut() else { return };
    let base = *s.camera_base.get_or_insert(*height);
    *height = base * s.scale;
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIST: &str = r#"{"outfits":[
        {"id":"kh3","name":"KINGDOM HEARTS III","wear_forms":[5],"scale":1},
        {"id":"kh1","name":"KINGDOM HEARTS","wear_forms":[],"scale":0.866},
        {"id":"toybox","name":"Toy Box","wear_forms":[5],"scale":1}]}"#;

    #[test]
    fn boc_is_known_by_his_talk_ids() {
        for id in [223006000, 223001100, 223001105, 223001400] {
            assert!(is_boc_talk(id));
        }
        for id in [1000, 3000, 224006000, 22230000] {
            assert!(!is_boc_talk(id));
        }
    }

    #[test]
    fn the_choice_lives_in_the_records() {
        let mut records = vec!["boss:margit".to_string()];
        assert_eq!(chosen(&records), "kh3");
        choose(&mut records, "kh1");
        choose(&mut records, "toybox");
        assert_eq!((chosen(&records), records.len()), ("toybox".to_string(), 2));
        choose(&mut records, "kh3");
        assert_eq!(records, vec!["boss:margit".to_string()]);
    }

    #[test]
    fn a_form_keeps_the_outfit_unless_asked() {
        let list = parse(LIST);
        assert_eq!(list.len(), 3);
        assert_eq!(list[1].scale, 0.866);
        let id = |chosen: &str, form: u8, fallback: bool| effective(&list, chosen, form, fallback).map(|o| o.id.clone()).unwrap();
        assert_eq!(id("kh1", 0, true), "kh1");
        assert_eq!(id("kh1", 5, true), "kh3", "the older rule: KH1 has no Second Form costume of its own");
        assert_eq!(id("kh1", 5, false), "kh1", "the rule: the outfit stays (its form look: wear_extra.json)");
        assert_eq!(id("toybox", 5, true), "toybox", "Toy Box has its own");
        assert_eq!(id("nope", 0, true), "kh3");
        assert!(parse("not json").is_empty());
    }

    #[test]
    fn every_outfit_speaks_with_kh3_soras_battle_voice_in_the_release() {
        for key in [None, Some(true), Some(false)] {
            assert_eq!(voice_outfit("kh1", key), None, "the KH1 outfit: KH3's voice ({key:?})");
        }
        for other in ["kh3", "kh2", "toybox", ""] {
            assert_eq!(voice_outfit(other, None), None, "{other}: KH3's voice");
        }
    }
}
