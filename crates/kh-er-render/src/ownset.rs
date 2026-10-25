//! (ER81, the user: "please give verum rex Sora his own shaders so he looks correct") AN OUTFIT'S OWN SHADER SET.
//!
//! KH3 draws every character through a post-process material chosen by its WORLD's controller (KHS_Blueprint_C,
//! `WorldType`): "KH World" -> /Game/Shader/KHS_set/postprocess/KHS (the 16-step colour table: the toon look), "CG World"
//! -> KHS_CG, "Real World" -> KHS_Real, Pooh's -> KHS_Poo; the controller also writes KHS_Control `WorldTypeInt`, which
//! the base passes read to pick their world branch. Verum Rex Sora (p_dl001) only appears in Quadratum (ss_01, a CG
//! World: KHS_CG, WorldTypeInt 1). Every outfit pack carries the KH World set; an outfit with an `own_shaders.json`
//! (tools/sora_material_pack.js --own-khs) also carries its world's set under a suffix:
//! `materials/er/<variant>_<suffix>[_pl|_bl|_bli|_hy2|_gb2].ps.dxil` (kh-shaderc --suffix), the post-process block of
//! that material (its collections and its world's controller as presets).
//!
//! Per-outfit DATA: the renderer takes the set when the file is there, complete (every set the DLL may pick for the
//! outfit's slots exists with the suffix), and the outfit's key is on (kh-er `boc.<outfit>.own_shaders`, default 1;
//! `boc.own_shaders = 0` all off). Off, absent or incomplete: exactly the KH World set as before. A change of the key
//! reloads the outfit (the worker's next job).

use std::path::Path;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Mutex;

use kh_look::library::Library;
use kh_look::Khs;

/// The file of an outfit's own set (beside its materials.json).
pub const FILE: &str = "own_shaders.json";
/// (ER85) A newer set of the same outfit under a new suffix (the pack's files are never rewritten): read in place of `FILE`
/// when it is there (verumrex: `cg2`, the eye's hue / saturation of a grey without the 0 / 0 that blackened the sclera).
pub const FILE_V2: &str = "own_shaders_v2.json";

/// The outfits whose own set is switched OFF by the host (kh-er `boc.<id>.own_shaders = 0`); `*` = all.
static OFF: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// What the drawn assets are: 0 = no own set in that outfit (nothing to switch), 1 = drawn with its own set,
/// 2 = it has one, drawn with the KH World set (switched off or incomplete).
static DRAWN: AtomicU8 = AtomicU8::new(0);
/// The outfit the DRAWN state is for, whether its set was complete, and the world preset it was drawn with.
static DRAWN_FOR: Mutex<(String, bool, String)> = Mutex::new((String::new(), false, String::new()));
/// (outfit, preset) the host asks for in place of the set's own world preset (kh-er `boc.<outfit>.own_preset`).
static PRESET: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

/// The host's keys: per outfit, the world preset of its own set to use (absent: the set's `world_preset`).
pub fn set_presets(presets: Vec<(String, String)>) {
    if let Ok(mut p) = PRESET.lock() {
        *p = presets;
    }
}

fn preset_wanted(outfit: &str) -> Option<String> {
    PRESET.lock().ok().and_then(|p| p.iter().find(|(o, _)| o == outfit).map(|(_, n)| n.clone()))
}

/// The host's keys: `off` = the outfits whose own set is not wanted (`*` = every outfit).
pub fn set_off(off: Vec<String>) {
    if let Ok(mut o) = OFF.lock() {
        *o = off;
    }
}

pub fn wanted(outfit: &str) -> bool {
    let off = OFF.lock().map(|o| o.iter().any(|x| x == "*" || x == outfit)).unwrap_or(false);
    // (ER85) a set whose file says `default_on: false` is drawn only when the host switches it on (`boc.<id>.own_shaders = 1`)
    let default_off = DEFAULT_OFF.lock().map(|d| d.iter().any(|x| x == outfit)).unwrap_or(false);
    !off && (!default_off || ON.lock().map(|o| o.iter().any(|x| x == outfit)).unwrap_or(false))
}

/// (ER85) The outfits whose own set the host switched ON by key (`boc.<id>.own_shaders = 1`), for sets that are off by default.
static ON: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// (ER85) The outfits whose own set file says `default_on: false` (recorded at their load).
static DEFAULT_OFF: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub fn set_on(on: Vec<String>) {
    if let Ok(mut o) = ON.lock() {
        *o = on;
    }
}

fn note_default(outfit: &str, on: bool) {
    if let Ok(mut d) = DEFAULT_OFF.lock() {
        d.retain(|x| x != outfit);
        if !on {
            d.push(outfit.to_string());
        }
    }
}

/// The outfit drawn has an own set whose state differs from the key: it is loaded again.
pub fn reload_wanted() -> bool {
    let (id, complete, preset) = DRAWN_FOR.lock().map(|d| d.clone()).unwrap_or_default();
    // (only for the outfit actually drawn: a load that failed leaves the drawn one, and is not retried from here)
    if crate::outfit::loaded() != id {
        return false;
    }
    match DRAWN.load(Ordering::Relaxed) {
        1 => !wanted(&id) || preset_wanted(&id).is_some_and(|p| p != preset),
        2 => complete && wanted(&id),
        _ => false,
    }
}

/// An outfit's own set as the renderer uses it.
pub struct OwnSet {
    /// `_<suffix>`, put after the variant's name.
    pub suffix: String,
    pub asset: String,
    pub world_preset: String,
}

/// The shader sets the renderer may pick (gfx.rs `read_shaders`): the plain name and these endings.
const SETS: [&str; 6] = ["", "_pl", "_bl", "_bli", "_hy2", "_gb2"];

/// Every set the pack has for ALL of the slots (by its plain names) also exists for all of them with `suffix`.
pub fn complete(dir: &Path, library: &Library, suffix: &str) -> Result<(), String> {
    for set in SETS {
        let plain = library.slots.iter().all(|s| dir.join(format!("{}{set}.ps.dxil", s.variant())).is_file());
        if !plain {
            continue;
        }
        if let Some(s) = library.slots.iter().find(|s| !dir.join(format!("{}{suffix}{set}.ps.dxil", s.variant())).is_file()) {
            return Err(format!("{}{suffix}{set}.ps.dxil missing", s.variant()));
        }
    }
    Ok(())
}

/// Reads `look/materials/own_shaders.json`; when the outfit's key is on and the set is complete, puts its post-process
/// block in place of the library's (the KH World presets' names are kept as aliases of the world's preset, so the
/// host's `r.khs` and the portrait's "camp" find it) and returns the set. Otherwise the library is untouched.
pub fn apply(look: &Path, outfit: &str, library: &mut Library) -> Option<OwnSet> {
    let file = [FILE_V2, FILE].iter().map(|n| look.join("materials").join(n)).find(|p| p.is_file()).unwrap_or_else(|| look.join("materials").join(FILE));
    let remember = |state: u8, complete: bool, preset: &str| {
        DRAWN.store(state, Ordering::Relaxed);
        // (ER85) the field light of an outfit drawn with its own set (cgfield.rs)
        crate::cgfield::set_active(state == 1);
        if let Ok(mut d) = DRAWN_FOR.lock() {
            *d = (outfit.to_string(), complete, preset.to_string());
        }
    };
    if !file.is_file() {
        remember(0, false, "");
        return None;
    }
    let parsed = std::fs::read_to_string(&file).map_err(|e| e.to_string()).and_then(|t| serde_json::from_str::<serde_json::Value>(&t).map_err(|e| e.to_string()));
    let json = match parsed {
        Ok(j) => j,
        Err(e) => {
            crate::rlog!("render: (ER81) {}: {e}: the KH World set", file.display());
            remember(2, false, "");
            return None;
        }
    };
    let suffix = json.get("suffix").and_then(|s| s.as_str()).filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())).map(|s| format!("_{s}"));
    let khs = Khs::parse(&json);
    // (the key's preset when the set has it, else the set's own world preset)
    let world = preset_wanted(outfit).filter(|p| khs.presets.iter().any(|(n, ..)| n == p)).unwrap_or_else(|| json.get("world_preset").and_then(|s| s.as_str()).unwrap_or("").to_string());
    let Some(suffix) = suffix.filter(|_| !khs.presets.is_empty() && khs.presets.iter().any(|(n, ..)| *n == world)) else {
        crate::rlog!("render: (ER81) {}: no suffix, post-process block or world preset: the KH World set", file.display());
        remember(2, false, "");
        return None;
    };
    if let Err(e) = complete(&look.join("materials").join("er"), library, &suffix) {
        crate::rlog!("render: (ER81) {outfit}'s own shader set is incomplete ({e}): the KH World set");
        remember(2, false, "");
        return None;
    }
    note_default(outfit, json.get("default_on").and_then(|v| v.as_bool()).unwrap_or(true));
    if !wanted(outfit) {
        crate::rlog!("render: (ER81) {outfit}'s own shader set switched off (boc.{outfit}.own_shaders = 0, or off by default: = 1 switches it on): the KH World set");
        remember(2, true, "");
        return None;
    }
    let asset = json.get("khs").and_then(|k| k.get("asset")).and_then(|a| a.as_str()).unwrap_or("").to_string();
    let own = with_aliases(khs, &library.khs, &world);
    library.khs = own;
    crate::rlog!("render: (ER81) {outfit} drawn with its own shader set: *{suffix}* ({asset}; world preset {world}; WorldTypeInt {}, PixarCoreControl {})", library.khs.preset_state(&world, true).c0[1].y, library.khs.preset_state(&world, true).c0[9].x);
    // (ER85) the outfit's own look file (values over its materials, its field light: cgfield.rs)
    crate::cgfield::apply_look(look, outfit, library);
    remember(1, true, &world);
    Some(OwnSet { suffix, asset, world_preset: world })
}

/// The own block with every preset name of `before` it lacks added as a copy of `world` (the host asks for its
/// world preset by the KH World's names: "twilight" by default).
pub fn with_aliases(mut own: Khs, before: &Khs, world: &str) -> Khs {
    let Some(w) = own.presets.iter().find(|(n, ..)| n == world).cloned() else { return own };
    for (name, ..) in &before.presets {
        if !own.presets.iter().any(|(n, ..)| n == name) {
            own.presets.push((name.clone(), format!("(ER81) alias of {world}: {}", w.1), w.2.clone(), w.3.clone()));
        }
    }
    own
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn project() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
    }

    /// The staged Verum Rex set (er81) parses: KHS_CG, the Quadratum controller (a CG World: WorldTypeInt 1), its
    /// boards; the KH World names are aliases of it; the camp preset is its own. Skipped without the files.
    #[test]
    fn the_verum_rex_set_is_quadratums() {
        let file = project().join("build/sora_tmp/er/er81/pack/outfits/verumrex/materials").join(FILE);
        let Ok(text) = std::fs::read_to_string(&file) else { return };
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(json["suffix"], "cg");
        assert_eq!(json["khs"]["asset"], "/Game/Shader/KHS_set/postprocess/KHS_CG");
        let own = Khs::parse(&json);
        assert!(own.table.is_none(), "KHS_CG samples no colour table");
        let mut before = Khs::default();
        before.presets.push(("twilight".into(), String::new(), serde_json::Value::Null, serde_json::Value::Null));
        let own = with_aliases(own, &before, "quadratum");
        let q = own.preset_state("quadratum", true);
        let t = own.preset_state("twilight", true);
        assert_eq!(q.c0[1].y, 1.0, "WorldTypeInt of a CG World");
        assert_eq!(q.c0, t.c0);
        assert_eq!(q.k0, t.k0);
        // Board A of ss_01's controller: rotate 20, colour (0.547, 0.728, 1)
        assert_eq!(q.k0[7].x, 20.0);
        assert!((q.k0[11].x - 0.547).abs() < 1e-3 && (q.k0[11].z - 1.0).abs() < 1e-6);
        assert_eq!(own.preset_state("camp", true).c0[1].y, 1.0);
        // the default look is matched to the official render (the Pixar colour table off: --control PixarCoreControl=0);
        // the game data's own value stays as quadratum_game (boc.verumrex.own_preset = quadratum_game)
        assert_eq!(json["world_preset"], "quadratum");
        assert_eq!(q.c0[9].x, 0.0);
        assert_eq!(own.preset_state("camp", true).c0[9].x, 0.0);
        let game = own.preset_state("quadratum_game", true);
        assert_eq!((game.c0[9].x, game.c0[1].y), (1.0, 1.0));
        assert_eq!(game.k0, q.k0);
    }

    #[test]
    fn the_keys_switch_per_outfit_and_all() {
        // (ER85) a set off by default is drawn only when switched on
        note_default("toybox", false);
        assert!(!wanted("toybox"));
        set_on(vec!["toybox".into()]);
        assert!(wanted("toybox"));
        set_on(Vec::new());
        note_default("toybox", true);
        assert!(wanted("toybox"));
        set_off(vec!["verumrex".into()]);
        assert!(!wanted("verumrex") && wanted("kh1"));
        set_off(vec!["*".into()]);
        assert!(!wanted("verumrex") && !wanted("kh1"));
        set_off(Vec::new());
        assert!(wanted("verumrex"));
    }
}
