//! (IMPL_ER59, the user: "Boc is able to change Sora's appearance: KH1, KH2, and all of Sora's world outfits in 3")
//! SORA'S OUTFITS, OPENED BY BOC THE SEAMSTER. State note: build/sora_tmp/er/IMPL_ER59_STATE.md.
//!
//! WHAT IS OFFERED: the outfits of KH3's own data (`PACK/outfits/outfits.json`, tools/sora_outfit_pack.js: KH3's
//! default, KH2, KH1, Toy Box, Monstropolis, The Caribbean, San Fransokyo), on the KINGDOM HEARTS "Outfits" screen
//! (kh-ui: a list drawn in the Game Help screen's category list; pad: up / down, A wears, B closes).
//!
//! (THE USER 2026-11-02: "Move the Outfits selection to Boc, replacing his armor alteration menu with it completely
//! (this means it is only accessible by speaking with Boc)") ONLY BOC OPENS IT: his "Alter garments" (EventTextForTalk
//! 22230000, entry 1 of his talk list) runs talk command c1_142(110000, 110399), the game's alteration window, and his
//! script waits while menu type 26 reads open. The talk hooks now catch THAT COMMAND (talkkh/alter.rs: every one of his
//! scripts, read with conv/tools/esdparse.js; the grace's own 142 (talk 1000, a system script) is left alone): Elden
//! Ring's window never opens, [`talk_frame`] opens the Outfits screen (kh-host `Hud::open_outfits`), and while it is up
//! his script reads the window as open; B closes it, the script reads it closed and goes back to his talk list (his
//! other options and his quest as they were). THE CAMP MENU HAS NO OUTFITS ENTRY ANY MORE (IMPL_ER59 put one there,
//! gated by having talked to him; that gate, `boc.gate`, is gone with it) and no other menu reaches it. An outfit
//! already worn (the save's record) stays worn whether or not Boc can be reached. The record `boc:met` (his talk seen:
//! `PopupState::talk_id` in 223000000 ..= 223099999, talkdrive.rs) is still written, for an older DLL's gate.
//! (Before 2026-11-02 the command was NOT intercepted: the list's cursor is not readable, so the window's opening could
//! not be told apart; catching the command itself needs no cursor.)
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
//! (weapons P5b, the user 2026-11-03: "OUTFIT x FORM COLOURS") EVERY WEAR FORM (1 Brave .. 6 Final, 7 Rage, 8 Light, 9 Dark,
//! 10 Double; kh-core `Form.wear_form`, sora.rs -> `kh_er_render::set_wear_form`) takes the same path: KH3's default outfit wears
//! every costume of p_ex001_WearformSet; Toy Box / Monstropolis / Caribbean / Verum Rex / San Fransokyo their own set's
//! (wear_extra.json `forms`, tools/sora_outfit_forms.js); KH1 / KH2 WA's (outfit, form) colour variants (`direct`,
//! tools/sora_outfit_formcolours.js) and NEVER KH3's outfit (`no_fallback`, even with `boc.form_outfit = 1`). An outfit
//! changed at Boc during a form wears the form at once (kh-look `Library::step_wear`: a library's first frame).
//!
//! | key | default | meaning |
//! |---|---|---|
//! | `boc.outfits` | 1 | 0 = no Outfits screen (Boc's "Alter garments" is Elden Ring's window again) and KH3's default outfit always |
//! | `boc.gate` | - | (THE USER 2026-11-02) NOT READ ANY MORE: Boc's "Alter garments" is the only way in, there is no camp entry to gate |
//! | `boc.kh1_form_style` | 1 | (ER69) KH1 Sora's Second Form: 1 = the styled look (regions to black / silver / gold, trim lines), 0 = the plain recolour |
//! | `boc.form_outfit` | 0 | 1 = an outfit without the form's costume wears KH3's default outfit in a form (the older rule) |
//! | `boc.outfit` | - | testing aid: wear this outfit id now (not saved) |
//! | `test.wear_form` | - | (weapons P5b) TEST PROFILE ONLY: show wear form N (1..10) without the form (the outfit x form check); 0 = off |
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
//! | `boc.unlock` | 1 | (OUTFIT-UNLOCK, the user 2026-11-05) 1 = the needles open the outfits ([`needles`]: KH3's from the start, KH1 / KH2 with the Sewing Needle given (Elden Ring flag 31159206), every other with the Gold Sewing Needle given (1036489208); records `boc:needle` / `boc:gold_needle`, KH3's OBTAINED popup); an outfit not opened is not listed and not drawn; 0 = every outfit, the rule before |
//! | `boc.outfit_price` | 50000 | (OUTFIT-UNLOCK, the user 2026-11-05: "it costs 50,000 munny") the munny one outfit change costs on the Outfits screen (the camp menu's entry and Boc's alike): KH3's Yes / No with the outfit and the price first, KH3's "You don't have enough munny." when short; the outfit worn is free; 0 = free |
//! | (camp entry) | - | (OUTFIT-UNLOCK, the user 2026-11-05: "once you have the sewing kit, you can access the costume menu") the camp menu shows "Outfits" (before Config) while Elden Ring's Tailoring Tools (goods 8163) or Golden Tailoring Tools (8188) are owned; hidden otherwise, as KH3 hides a camp entry not open yet (Cuisine, Gummiphone) |
//! | `boc.gold_anytime` | 1 | (OUTFIT-UNLOCK, the user 2026-11-05) 1 = Boc offers "Give the Gold Sewing Needle" whenever it is held, the Sewing Needle was given and the gold one not (his talk script's own flag check answered: talkkh/hooks.rs -> [`talk_flag_read`]); 0 = Elden Ring's own requirements |
//!
//! (OUTFIT-UNLOCK) THE OLD ROUTE REPLACED: before 2026-11-05 every outfit of `outfits.json` was listed as soon as Boc's
//! "Alter garments" was reachable (in Elden Ring: after the Sewing Needle). Now the list holds only the opened ones; a save
//! wearing an outfit not opened yet draws KH3's (its record kept: it comes back once opened).

use std::path::Path;
use std::sync::Mutex;

use crate::control::Control;
use crate::erlog;

// (OUTFIT-UNLOCK) Boc's needles -> the outfits (pure rules)
pub mod needles;

/// One outfit of `outfits.json`.
#[derive(Clone, Debug, PartialEq)]
pub struct Outfit {
    pub id: String,
    pub name: String,
    /// The wear forms its pawn has textures for (5 = Second Form). (weapons P5b) + the forms of its library's wear_extra.json
    /// `forms` (tools/sora_outfit_forms.js: every form of its WearformSet: KH3 Sora's and the world outfits' 1..10).
    pub wear_forms: Vec<u8>,
    /// Its hip height over KH3 Sora's (KH1: 0.866).
    pub scale: f32,
    /// (weapons P5b) The wear forms it has a colour VARIANT for (wear_extra.json `direct`: KH1 / KH2 by WA's manifest,
    /// tools/sora_outfit_formcolours.js; KH1's Second Form by tools/sora_outfit_forms.js).
    pub variants: Vec<u8>,
    /// (weapons P5b) Never drawn as KH3's outfit in a form, even with `boc.form_outfit = 1` (KH1 / KH2: WA's variants).
    pub no_fallback: bool,
}

/// (weapons P5b) The outfits that wear WA's (outfit, form) variants and never fall back to KH3's outfit in a form.
pub const VARIANT_OUTFITS: [&str; 2] = ["kh1", "kh2"];

impl Outfit {
    /// (weapons P5b) It has a look of its own for wear form `form` (its costume or a variant).
    pub fn has_form(&self, form: u8) -> bool {
        self.wear_forms.contains(&form) || self.variants.contains(&form)
    }

    /// (weapons P5b) Takes in its library's `wear_extra.json` (`forms` -> own costumes, `direct` -> variants; the plain 100+
    /// variants are not wear forms; `kb_forms` = WA's manifest: no fallback).
    pub fn add_wear_extra(&mut self, text: &str) {
        let Ok(json) = serde_json::from_str::<serde_json::Value>(text.trim_start_matches('\u{feff}')) else { return };
        let keys = |k: &str| -> Vec<u8> { json[k].as_object().into_iter().flatten().filter_map(|(f, _)| f.parse::<u8>().ok()).filter(|f| (1..100).contains(f)).collect() };
        for (list, more) in [(&mut self.wear_forms, keys("forms")), (&mut self.variants, keys("direct"))] {
            list.extend(more);
            list.sort_unstable();
            list.dedup();
        }
        self.no_fallback |= json["kb_forms"].is_object();
    }
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
                variants: Vec::new(),
                no_fallback: o["id"].as_str().is_some_and(|id| VARIANT_OUTFITS.contains(&id)),
            })
        })
        .collect()
}

/// (weapons P5b) The folder of an outfit's library (`PACK/materials` for KH3's own, `PACK/<dir>/materials` for the others).
fn library_dir(pack: &Path, outfits_json: &serde_json::Value, id: &str) -> std::path::PathBuf {
    let dir = outfits_json["outfits"].as_array().into_iter().flatten().find(|o| o["id"] == id).and_then(|o| o["dir"].as_str());
    dir.map_or_else(|| pack.join("materials"), |d| pack.join(d).join("materials"))
}

/// THE OUTFIT DRAWN (weapons P5b: for every wear form 1..10, Rage Form's 7 included): the chosen one when it has a look of its
/// own for the form (its own KH3 costume, else a colour variant); else KH3's default outfit if `form_fallback`
/// (`boc.form_outfit = 1`), else still the chosen one (its clothes as they are). KH1 / KH2 never fall back.
pub fn effective<'a>(outfits: &'a [Outfit], chosen: &str, wear_form: u8, form_fallback: bool) -> Option<&'a Outfit> {
    let own = outfits.iter().find(|o| o.id == chosen).or_else(|| outfits.iter().find(|o| o.id == "kh3"))?;
    if form_fallback && wear_form != 0 && !own.no_fallback && !own.has_form(wear_form) {
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
    /// (OUTFIT-UNLOCK) the outfits the records open (None: `boc.unlock = 0`, every outfit); written by [`camp`]
    unlocked: Option<Vec<String>>,
    /// (OUTFIT-UNLOCK) the chosen outfit told as not opened (one log line per outfit)
    locked_told: String,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

/// Reads the list (`PACK/outfits/outfits.json`; none = no outfits, nothing changes). At Sora's start.
pub fn load(pack: &Path) {
    let file = pack.join("outfits").join("outfits.json");
    let text = std::fs::read_to_string(&file).unwrap_or_default();
    let mut outfits = parse(&text);
    // (weapons P5b) each library's wear_extra.json: its other wear forms and its colour variants
    let json: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).unwrap_or_default();
    for o in &mut outfits {
        if let Ok(extra) = std::fs::read_to_string(library_dir(pack, &json, &o.id).join("wear_extra.json")) {
            o.add_wear_extra(&extra);
        }
    }
    erlog!("boc: {} outfit(s) in {}: {}", outfits.len(), file.display(), outfits.iter().map(|o| format!("{} (forms {:?}, variants {:?}{})", o.id, o.wear_forms, o.variants, if o.no_fallback { ", no fallback" } else { "" })).collect::<Vec<_>>().join(", "));
    if let Ok(mut g) = STATE.lock() {
        let (met, chosen) = g.as_ref().map(|s| (s.met, s.chosen.clone())).unwrap_or((false, "kh3".to_string()));
        *g = Some(State { outfits, met_seen: false, met, chosen, drawn: String::new(), camera_base: None, scale: 1.0, pack: pack.to_path_buf(), clip_dir: None, clips_key: String::new(), unlocked: None, locked_told: String::new() /* (OUTFIT-UNLOCK) */ });
    }
    unlock::restart(); // (OUTFIT-UNLOCK) a new start: the first poll is the catch-up
}

/// ONCE A FRAME (sora.rs `submit`): Boc's talk seen, and the outfit the renderer draws.
pub fn frame(control: &Control, talk_id: Option<i32>, wear_form: u8) {
    // (weapons P5b) the wear form shown (`form.costume`, the test aid `test.wear_form`) to the renderer and the outfit choice
    let wear_form = wear_form_shown(control, wear_form);
    #[cfg(windows)]
    kh_er_render::set_wear_form(wear_form);
    // (OUTFIT-UNLOCK) the needles' flags -> the records, the popups (before the state's lock: it takes the HUD's)
    #[cfg(windows)]
    unlock::frame(control, talk_id.is_some_and(is_boc_talk));
    let Ok(mut g) = STATE.lock() else { return };
    let Some(s) = g.as_mut() else { return };
    if talk_id.is_some_and(is_boc_talk) && !s.met {
        s.met = true;
        s.met_seen = true;
        erlog!("boc: Boc's talk ({}) is up: his \"Alter garments\" opens Sora's Outfits", talk_id.unwrap_or(0));
    }
    let on = control.flag("boc.outfits") != Some(false);
    // (ER83) KH1's hands on the Keyblade / his hip (handik.rs), read every frame
    #[cfg(windows)]
    {
        kh_er_render::handik::set_enabled(control.flag("boc.grip_ik") != Some(false));
        kh_er_render::handik::set_main_fingers(control.flag("boc.grip_main") != Some(false));
    }
    let mut chosen = control.str("boc.outfit").map(str::to_string).unwrap_or_else(|| s.chosen.clone());
    // (OUTFIT-UNLOCK) an outfit the needles have not opened is not drawn (the testing aid `boc.outfit` is not held back); only
    // once the first poll of this start has read the flags (a loaded save's catch-up first: no flash of KH3's outfit)
    if held_back(&chosen, s.unlocked.as_deref(), unlock::polled(), control.str("boc.outfit").is_some()) {
        if s.locked_told != chosen {
            erlog!("boc: (OUTFIT-UNLOCK) the save's outfit {chosen} is not opened by Boc's needles yet ({}): KH3's outfit is drawn (the record stays; it comes back once opened)", if needles::tier_of(&chosen) == needles::Tier::Needle { "the Sewing Needle" } else { "the Gold Sewing Needle" });
            s.locked_told = chosen.clone();
        }
        chosen = "kh3".to_string();
    }
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

/// (weapons P5b) THE WEAR FORM SHOWN (sora.rs: `kh_er_render::set_wear_form` and [`frame`] take it): the pawn's
/// (`Form.wear_form`, every form 1..10 and Rage's 7), 0 with `form.costume = 0` (his normal clothes always), and the TEST AID
/// `test.wear_form = N` (TEST PROFILE ONLY, held while set: wear form N's look without the form, for the outfit x form check in
/// game; 0 / empty = off). Read every frame.
pub fn wear_form_shown(control: &Control, pawn: u8) -> u8 {
    shown(control.flag("form.costume") == Some(false), control.str("test.wear_form"), pawn)
}

fn shown(costume_off: bool, test: Option<&str>, pawn: u8) -> u8 {
    match test.and_then(|t| t.trim().parse::<u8>().ok()).filter(|f| (1..=10).contains(f)) {
        Some(form) => form,
        None if costume_off => 0,
        None => pawn,
    }
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

/// IN THE HUD'S FRAME (kh-er hud.rs, under its lock): the list Boc's Outfits screen shows, a choice made there, the records.
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
        erlog!("boc: Boc's Outfits screen: {id} chosen (record \"{OUTFIT_PREFIX}{id}\" in Sora's save at its next write)");
    }
    // (OUTFIT-UNLOCK) the outfits Boc's needles have opened (`boc.unlock = 0`: all of them)
    s.unlocked = opened(&s.outfits, &hud.inventory_mut().records, unlock::on());
    // (THE USER 2026-11-02) the list for Boc's screen (Hud::open_outfits); no camp menu entry is made of it any more
    // (OUTFIT-UNLOCK) only the opened ones; the worn one shown is KH3's while the chosen one is not opened
    hud.outfit_menu = offered(control, s.outfits.len()).then(|| menu_of(&s.outfits, s.unlocked.as_deref(), &s.chosen));
    // (OUTFIT-UNLOCK, the user 2026-11-05: "once you have the sewing kit, you can access the costume menu that lets you change your
    // clothes, but it costs 50,000 munny") the camp menu's Outfits entry while Elden Ring's Tailoring Tools are owned; every change
    // on the screen (camp or Boc) costs `boc.outfit_price` munny (default 50000; the one worn is free)
    hud.outfits_entry = control.flag("boc.outfits") != Some(false) && unlock::kit_owned();
    hud.outfit_price = control.str("boc.outfit_price").and_then(|v| v.trim().parse::<i32>().ok()).unwrap_or(needles::OUTFIT_PRICE).max(0);
    // (OF) THE USER 2026-11-03: "make sure Sora's HUD changes for his costumes": the HUD's portraits of Sora show the outfit
    // drawn (a Boc change and a loaded save's outfit alike, the frame it is drawn; KH3's own before any is drawn)
    hud.set_sora_outfit(if s.drawn.is_empty() { "kh3" } else { &s.drawn });
}

/// (OUTFIT-UNLOCK) The ids of the pack's outfits the records open (None: the rule is off, every outfit).
fn opened(outfits: &[Outfit], records: &[String], rule_on: bool) -> Option<Vec<String>> {
    rule_on.then(|| outfits.iter().filter(|o| needles::is_unlocked(records, &o.id)).map(|o| o.id.clone()).collect())
}

/// (OUTFIT-UNLOCK) Boc's Outfits screen: the opened outfits (id, name) in the pack's order, and the worn one it marks (KH3's
/// while the chosen one is not opened).
fn menu_of(outfits: &[Outfit], opened: Option<&[String]>, chosen: &str) -> (Vec<(String, String)>, String) {
    let shown = |id: &str| opened.is_none_or(|u| u.iter().any(|x| x == id));
    let worn = if shown(chosen) { chosen.to_string() } else { "kh3".to_string() };
    (outfits.iter().filter(|o| shown(&o.id)).map(|o| (o.id.clone(), o.name.clone())).collect(), worn)
}

/// (OUTFIT-UNLOCK) The outfit to draw for the chosen one: KH3's when the needles have not opened it, once the flags were read
/// (`polled`); the testing aid `boc.outfit` (`aid`) is never held back.
fn held_back(chosen: &str, opened: Option<&[String]>, polled: bool, aid: bool) -> bool {
    !aid && polled && opened.is_some_and(|u| !u.iter().any(|x| x == chosen))
}

/// (THE USER 2026-11-02) Boc's "Alter garments" is the Outfits screen: `boc.outfits` not 0 and more than KH3's own
/// outfit in the pack (else his window stays Elden Ring's).
fn offered(control: &Control, outfits: usize) -> bool {
    control.flag("boc.outfits") != Some(false) && outfits > 1
}

/// (THE USER 2026-11-02) The talk hooks must run for Boc's "Alter garments" (even with `talk.kh = 0`).
#[cfg(windows)]
pub fn wants_talk_hooks(control: &Control) -> bool {
    // (OUTFIT-UNLOCK) ... and for the Gold Sewing Needle's offer (`boc.gold_anytime`)
    crate::talkkh::hooks::COMPLETE.load(std::sync::atomic::Ordering::Relaxed) && (control.flag("boc.outfits") != Some(false) || control.flag("boc.gold_anytime") != Some(false))
}

/// (OUTFIT-UNLOCK, the user 2026-11-05: "If you have it, and have already given him the sewing needle, you can give him the
/// gold sewing needle as part of his quest") FROM THE TALK-SCRIPT QUERY DETOUR (talkkh/hooks.rs, game thread): what Boc's
/// talk reads for GetEventFlag(`flag`) instead of the game's answer (None: the game's). See [`needles`].
#[cfg(windows)]
pub(crate) fn talk_flag_read(talk_id: i32, flag: u32) -> Option<bool> {
    unlock::talk_flag_read(talk_id, flag)
}

// (OUTFIT-UNLOCK) the game half: the flags polled, the records, the popups, the talk's flag reads
mod unlock;
// (OUTFIT-UNLOCK, PORT) the game's GetEventFlag case and the flag manager's global, for the offline address check
#[cfg(all(test, windows))]
pub(crate) use unlock::{FLAG_CASE_IN_FN, LOCATED};

/// (THE USER 2026-11-02: "Move the Outfits selection to Boc, replacing his armor alteration menu with it completely")
/// ONCE A FRAME (sora.rs, after the talk adapter and the KH shop): talkkh/alter.rs swallowed Boc's alteration window
/// and left a request; here the Outfits screen is opened for it (kh-host `Hud::open_outfits`, pad-first, KH3's menu
/// sounds). When it has closed the request is done: his script reads the window as closed and goes back to his talk
/// list. Not up within 3 s (another menu of ours is up, no HUD, no outfits) or a cutscene: given up the same way (his
/// talk goes on; Elden Ring's window still never opens). Logs: `boc:` lines.
#[cfg(windows)]
pub fn talk_frame(control: &Control, cutscene: bool, dt: f32) {
    use crate::talkkh::hooks::with_shared;
    static WAIT: Mutex<f32> = Mutex::new(0.0);
    let count = STATE.lock().ok().and_then(|g| g.as_ref().map(|s| s.outfits.len())).unwrap_or(0);
    let on = offered(control, count);
    let Some((request, notes)) = with_shared(|s| {
        s.alter.on = on;
        (s.alter.request.clone(), std::mem::take(&mut s.alter.notes))
    }) else {
        return;
    };
    for n in notes {
        erlog!("boc: {n}");
    }
    let mut wait = WAIT.lock().unwrap_or_else(|e| e.into_inner());
    let step = alter_step(request.as_ref().map(|r| r.opened), cutscene, || crate::hud::with_hud(|hud| hud.open_outfits()).unwrap_or(false), || crate::hud::with_hud(|hud| (hud.take_outfits_closed() || !hud.outfits_up(), hud.outfits_shown())).unwrap_or((true, false)), &mut wait, dt);
    match step {
        AlterStep::Opened => {
            if let Some(r) = request {
                with_shared(|s| {
                    if let Some(q) = s.alter.request.as_mut().filter(|q| q.serial == r.serial) {
                        q.opened = true;
                    }
                });
                erlog!("boc: the Outfits screen opens for talk {} in place of Elden Ring's alteration window (rows {}..{} not used)", r.talk_id, r.rows.0, r.rows.1);
            }
        }
        AlterStep::Done(why) => {
            if why == ALTER_NOT_SHOWN {
                crate::hud::with_hud(|hud| hud.drop_outfits_request());
            }
            with_shared(|s| s.alter.done(why));
            if why != ALTER_CLOSED {
                erlog!("boc: Boc's alteration window is given up: {why}");
            }
        }
        AlterStep::Wait => {}
    }
}

/// What [`alter_step`] says to do this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
enum AlterStep {
    Wait,
    /// The HUD took the request for the Outfits screen.
    Opened,
    /// The request is done (why): the script reads the window as closed.
    Done(&'static str),
}

const ALTER_CLOSED: &str = "closed";
const ALTER_NOT_SHOWN: &str = "the Outfits screen did not come up";
/// Seconds a request may wait for the Outfits screen.
const ALTER_WAIT: f32 = 3.0;

/// (THE USER 2026-11-02) THE REQUEST'S STEPS (pure; [`talk_frame`] gives it the HUD): `opened` = the request's state (None:
/// no request); `open` asks the HUD for the screen; `state` = (closed, shown) of the HUD's screen.
fn alter_step(opened: Option<bool>, cutscene: bool, open: impl FnOnce() -> bool, state: impl FnOnce() -> (bool, bool), wait: &mut f32, dt: f32) -> AlterStep {
    let step = match opened {
        None => AlterStep::Wait,
        Some(false) if cutscene => AlterStep::Done("a cutscene began"),
        Some(false) if open() => AlterStep::Opened,
        Some(false) => {
            *wait += dt;
            if *wait > ALTER_WAIT { AlterStep::Done("the Outfits screen could not open (another menu is up, no HUD, or no outfits)") } else { AlterStep::Wait }
        }
        Some(true) => match state() {
            (true, _) => AlterStep::Done(ALTER_CLOSED),
            (false, true) => {
                *wait = 0.0;
                AlterStep::Wait
            }
            (false, false) => {
                *wait += dt;
                if *wait > ALTER_WAIT { AlterStep::Done(ALTER_NOT_SHOWN) } else { AlterStep::Wait }
            }
        },
    };
    if matches!(step, AlterStep::Opened | AlterStep::Done(_)) || opened.is_none() {
        *wait = 0.0;
    }
    step
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
        // (weapons P5b) the older rule no longer applies to KH1 / KH2 (WA's variants; the user: no fallback to KH3's outfit)
        assert_eq!(id("kh1", 5, true), "kh1", "KH1 never falls back");
        assert_eq!(id("toybox", 3, true), "kh3", "the older rule: Toy Box has no form-3 costume in this list");
        assert_eq!(id("kh1", 5, false), "kh1", "the rule: the outfit stays (its form look: wear_extra.json)");
        assert_eq!(id("toybox", 5, true), "toybox", "Toy Box has its own");
        assert_eq!(id("nope", 0, true), "kh3");
        assert!(parse("not json").is_empty());
    }

    /// (weapons P5b) EVERY WEAR FORM: the outfit drawn for (outfit, wear form 1..10) with the older fallback rule ON (the strictest
    /// case): KH3's own costumes for all forms, KH1 / KH2 their variants and never KH3's outfit, a world outfit its own costumes,
    /// one with nothing for a form KH3's outfit (only then); with the rule off the chosen outfit always.
    #[test]
    fn every_wear_form_finds_its_look_and_kh1_kh2_never_fall_back() {
        let mut list = parse(LIST);
        assert!(list[1].no_fallback && !list[0].no_fallback && !list[2].no_fallback, "KH1 is a variant outfit by id");
        list[0].add_wear_extra(r#"{"forms":{"1":{},"2":{},"3":{},"4":{},"6":{},"7":{},"8":{},"9":{},"10":{}},"direct":{}}"#);
        list[1].add_wear_extra(r#"{"direct":{"1":[],"2":[],"3":[],"4":[],"5":[],"6":[],"7":[],"8":[],"9":[],"10":[],"105":[]},"kb_forms":{}}"#);
        assert_eq!(list[0].wear_forms, (1..=10).collect::<Vec<u8>>(), "KH3's outfit: p_ex001_WearformSet, all of it");
        assert_eq!(list[1].variants, (1..=10).collect::<Vec<u8>>(), "the plain 105 is not a wear form");
        let id = |list: &[Outfit], chosen: &str, form: u8, fallback: bool| effective(list, chosen, form, fallback).map(|o| o.id.clone()).unwrap();
        for form in 1..=10 {
            assert_eq!(id(&list, "kh3", form, true), "kh3");
            assert_eq!(id(&list, "kh1", form, true), "kh1", "KH1, wear form {form}: its variant, never KH3's outfit");
            assert_eq!(id(&list, "toybox", form, false), "toybox");
            assert_eq!(id(&list, "toybox", form, true), if form == 5 { "toybox" } else { "kh3" }, "Toy Box without its other forms: the older rule");
        }
        // no variant and no costume, but a KH1 / KH2 outfit: still never KH3's
        let bare = parse(r#"{"outfits":[{"id":"kh3","wear_forms":[5]},{"id":"kh2","wear_forms":[]}]}"#);
        assert_eq!(id(&bare, "kh2", 3, true), "kh2");
        // a variant (no costume of its own) is enough for any other outfit too
        list[2].add_wear_extra(r#"{"direct":{"3":[]}}"#);
        assert_eq!(id(&list, "toybox", 3, true), "toybox");
        // garbage changes nothing
        let before = list[2].clone();
        list[2].add_wear_extra("not json");
        assert_eq!(list[2], before);
    }

    /// (weapons P5b, the user: "Changing outfit at Boc during a form re-applies the wear form at once") the outfit drawn follows a
    /// Boc change while the form stays on (the renderer then loads the new outfit's library, which wears the form on its first
    /// frame: kh-look `a_library_loaded_during_a_form_wears_it_at_once`), and the form's end keeps the new outfit.
    #[test]
    fn a_boc_change_during_a_form_draws_the_new_outfit_in_that_form() {
        let mut list = parse(LIST);
        list[1].add_wear_extra(r#"{"direct":{"2":[]},"kb_forms":{}}"#);
        let mut records = Vec::new();
        for (chosen, want) in [("kh3", "kh3"), ("kh1", "kh1"), ("toybox", "toybox"), ("kh3", "kh3")] {
            choose(&mut records, chosen);
            assert_eq!(effective(&list, &chosen_of(&records), 2, false).map(|o| o.id.as_str()), Some(want), "in Wisdom Form, {chosen} chosen at Boc");
        }
        choose(&mut records, "kh1");
        assert_eq!(effective(&list, &chosen_of(&records), 0, false).map(|o| o.id.as_str()), Some("kh1"), "the form ends: KH1 stays");
    }

    #[test]
    fn the_wear_form_shown_is_the_pawns_unless_switched_off_or_the_test_aid_holds_one() {
        assert_eq!(shown(false, None, 2), 2);
        assert_eq!(shown(true, None, 2), 0, "form.costume = 0");
        assert_eq!(shown(false, Some(" 7 "), 0), 7, "test.wear_form = 7: Rage Form's look");
        assert_eq!(shown(true, Some("10"), 0), 10, "the aid wins");
        for off in ["0", "", "11", "x"] {
            assert_eq!(shown(false, Some(off), 5), 5, "{off:?}: the aid is off");
        }
    }

    /// (weapons P5b, F-SHIELD's "the wear form flickers" report) THE RENDERER'S FRAME, AS gfx.rs RUNS IT, on the real pack (skipped
    /// without it): the one wear form the host hands over (`wear_form_shown`) -> F-RAGE's `Rage::step` + `library_form` -> kh-look
    /// `step_wear`, every frame. Held for 600 frames, every form (Rage's 7 with its fade included) reaches the library ONCE and
    /// the bound material never changes after that; its end goes back once.
    #[cfg(windows)]
    #[test]
    fn a_held_form_is_stable_through_the_renderers_frame_rage_included() {
        let pack = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack");
        if !pack.join("outfits/outfits.json").is_file() {
            return;
        }
        for dir in [pack.clone(), pack.join("outfits/kh1"), pack.join("outfits/kh2")] {
            let mut lib = kh_look::library::Library::load(&dir).unwrap();
            for form in [3u8, 4, 5, 7, 6] {
                let mut rage = kh_er_render::rage::Rage::default();
                let (mut clock, mut handed, mut switches) = (0.0f32, 0u8, 0u32);
                type Bound = Vec<Vec<Option<String>>>;
                let mut bound: Option<Bound> = None;
                let snapshot = |lib: &kh_look::library::Library| -> Bound { lib.slots.iter().map(|s| s.textures.iter().zip(&s.texture_names).map(|(t, n)| t.as_ref().filter(|_| !n.starts_with("FormChange") || n.ends_with('0')).map(|t| t.file.clone())).collect()).collect() };
                for k in 0..600 {
                    clock += [1.0 / 60.0, 1.0 / 30.0, 1.0 / 144.0][k % 3];
                    let wear = shown(false, None, form);
                    rage.step(wear, clock);
                    let to_library = rage.library_form(wear);
                    if to_library != handed {
                        switches += 1;
                        handed = to_library;
                    }
                    let changes = lib.step_wear(to_library, 1.0 / 60.0);
                    if k >= 200 {
                        let now = snapshot(&lib);
                        assert!(changes.is_empty(), "{} form {form} frame {k}: bindings changed while held", dir.display());
                        assert_eq!(bound.get_or_insert_with(|| now.clone()), &now, "{} form {form} frame {k}", dir.display());
                    }
                }
                assert_eq!((switches, handed), (1, form), "{}: wear form {form} reaches the library once", dir.display());
                for _ in 0..120 {
                    clock += 1.0 / 60.0;
                    rage.step(0, clock);
                    lib.step_wear(rage.library_form(0), 1.0 / 60.0);
                }
                assert_eq!((rage.library_form(0), lib.wear.target, lib.wear.direct_on), (0, 0, 0), "{}: form {form} ended", dir.display());
            }
        }
    }

    fn chosen_of(records: &[String]) -> String {
        chosen(records)
    }

    /// (weapons P5b) THE REAL PACK (skipped without it): every outfit has a look for every wear form 1..10 (no outfit falls back
    /// to KH3's outfit even with the older rule on), and KH1 / KH2 have WA's variants for all of them.
    #[test]
    fn the_pack_gives_every_outfit_every_wear_form() {
        let pack = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack");
        let Ok(text) = std::fs::read_to_string(pack.join("outfits/outfits.json")) else { return };
        let json: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).unwrap();
        let mut list = parse(&text);
        for o in &mut list {
            if let Ok(extra) = std::fs::read_to_string(library_dir(&pack, &json, &o.id).join("wear_extra.json")) {
                o.add_wear_extra(&extra);
            }
        }
        assert!(list.len() >= 8, "{list:?}");
        for o in &list {
            for form in 1..=10 {
                assert!(o.has_form(form), "{}: no look for wear form {form} ({:?} / {:?})", o.id, o.wear_forms, o.variants);
                assert_eq!(effective(&list, &o.id, form, true).map(|e| e.id.as_str()), Some(o.id.as_str()));
            }
        }
        for id in VARIANT_OUTFITS {
            let o = list.iter().find(|o| o.id == id).unwrap();
            assert!(o.no_fallback && o.variants == (1..=10).collect::<Vec<u8>>(), "{o:?}");
        }
        assert_eq!(list.iter().find(|o| o.id == "kh3").unwrap().wear_forms, (1..=10).collect::<Vec<u8>>(), "KH3's default outfit: all of p_ex001_WearformSet");
    }

    /// (THE USER 2026-11-02: "Move the Outfits selection to Boc") A REQUEST LEFT BY BOC'S "ALTER GARMENTS": the screen is
    /// asked for at once; once up, the request lasts while it is shown and is done when it closes (his talk list comes
    /// back); a screen that never opens or never comes up, or a cutscene, gives the request up (his talk goes on).
    #[test]
    fn boc_s_request_opens_the_screen_and_closing_returns_to_his_talk() {
        let mut wait = 0.0;
        let never = || -> bool { panic!("not asked") };
        let no_state = || -> (bool, bool) { panic!("not asked") };
        assert_eq!(alter_step(None, false, never, no_state, &mut wait, 0.1), AlterStep::Wait, "no request");
        assert_eq!(alter_step(Some(false), false, || true, no_state, &mut wait, 0.1), AlterStep::Opened);
        // asked, not on screen yet (the camp's opening), then up, then B
        assert_eq!(alter_step(Some(true), false, never, || (false, false), &mut wait, 0.5), AlterStep::Wait);
        assert_eq!(alter_step(Some(true), false, never, || (false, true), &mut wait, 0.5), AlterStep::Wait);
        assert_eq!(wait, 0.0, "shown: no waiting counted");
        for _ in 0..600 {
            assert_eq!(alter_step(Some(true), false, never, || (false, true), &mut wait, 0.1), AlterStep::Wait, "it stays up as long as the player likes");
        }
        assert_eq!(alter_step(Some(true), false, never, || (true, false), &mut wait, 0.1), AlterStep::Done(ALTER_CLOSED), "closed: back to Boc's talk list");
        // the HUD refuses (another menu up): given up after 3 s
        let mut wait = 0.0;
        for _ in 0..29 {
            assert_eq!(alter_step(Some(false), false, || false, no_state, &mut wait, 0.1), AlterStep::Wait);
        }
        assert!(matches!(alter_step(Some(false), false, || false, no_state, &mut wait, 0.2), AlterStep::Done(why) if why != ALTER_CLOSED));
        // taken but never shown: given up (the HUD's request dropped)
        let mut wait = 0.0;
        for _ in 0..29 {
            alter_step(Some(true), false, never, || (false, false), &mut wait, 0.1);
        }
        assert_eq!(alter_step(Some(true), false, never, || (false, false), &mut wait, 0.2), AlterStep::Done(ALTER_NOT_SHOWN));
        assert!(matches!(alter_step(Some(false), true, never, no_state, &mut 0.0, 0.1), AlterStep::Done(_)), "a cutscene");
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

    /// (OUTFIT-UNLOCK) Boc's screen lists only what the needles opened, in the pack's order, and an outfit not opened is not
    /// drawn once the flags were read; the rule off (`boc.unlock = 0`) is the old list; the testing aid is never held back.
    #[test]
    fn boc_s_screen_lists_and_draws_only_what_the_needles_opened() {
        let pack = parse(r#"{"outfits":[{"id":"kh3","name":"KINGDOM HEARTS III"},{"id":"kh2","name":"KINGDOM HEARTS II"},{"id":"kh1","name":"KINGDOM HEARTS"},{"id":"toybox","name":"Toy Box"},{"id":"verumrex","name":"Quadratum"}]}"#);
        let ids = |m: &(Vec<(String, String)>, String)| (m.0.iter().map(|(i, _)| i.as_str()).collect::<Vec<_>>().join(","), m.1.clone());
        let mut records = vec!["outfit:toybox".to_string()];
        // before any needle: KH3's only; the save's Toy Box is marked as KH3's and not drawn once polled
        let none = opened(&pack, &records, true);
        assert_eq!(ids(&menu_of(&pack, none.as_deref(), &chosen(&records))), ("kh3".to_string(), "kh3".to_string()));
        assert!(held_back("toybox", none.as_deref(), true, false));
        assert!(!held_back("toybox", none.as_deref(), false, false), "not before the first poll (a load's catch-up)");
        assert!(!held_back("toybox", none.as_deref(), true, true), "the testing aid boc.outfit");
        assert!(!held_back("kh3", none.as_deref(), true, false));
        // the Sewing Needle
        needles::take(&mut records, true, false);
        let needle = opened(&pack, &records, true);
        assert_eq!(ids(&menu_of(&pack, needle.as_deref(), "kh1")), ("kh3,kh2,kh1".to_string(), "kh1".to_string()));
        assert!(held_back("toybox", needle.as_deref(), true, false));
        // the Gold Sewing Needle: everything, the save's Toy Box back
        needles::take(&mut records, true, true);
        let gold = opened(&pack, &records, true);
        assert_eq!(ids(&menu_of(&pack, gold.as_deref(), &chosen(&records))), ("kh3,kh2,kh1,toybox,verumrex".to_string(), "toybox".to_string()));
        assert!(!held_back("toybox", gold.as_deref(), true, false));
        // the rule off: the old list, nothing held back
        let off = opened(&pack, &[], false);
        assert_eq!(off, None);
        assert_eq!(ids(&menu_of(&pack, off.as_deref(), "verumrex")).0, "kh3,kh2,kh1,toybox,verumrex");
        assert!(!held_back("verumrex", off.as_deref(), true, false));
    }
}
