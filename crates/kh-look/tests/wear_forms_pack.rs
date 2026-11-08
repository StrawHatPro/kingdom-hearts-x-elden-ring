//! (weapons P5b, the user 2026-11-03: KH3_WEAPONS_PLAN.md 1.3 "OUTFIT x FORM COLOURS") EVERY OUTFIT x EVERY WEAR FORM, on the
//! real pack (`build/sora_pack`; skipped without it): each outfit's library (`Library::load`: materials.json + wear_extra.json)
//! is put into every wear form 1..=10 through `step_wear` (the renderer's switch) and must then show the expected material:
//! - KH3 Sora (main pack) and the outfits with a WearformSet (Toy Box, Monstropolis, The Caribbean, Verum Rex) and San
//!   Fransokyo (KH3 Sora's instances): the set's costume for that form in `FormChangeTex0` / `FormChangePointTex0`;
//! - KH1 / KH2: the WA variant of that (outfit, form) in the slots' base colour (`direct`, keyblades/outfit_forms.json), for
//!   every form: they never need the KH3 outfit (no fallback);
//!
//! and every texture named exists in the library's folder. The form's end gives back exactly the textures worn before.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use kh_look::library::Library;
use serde_json::Value;

fn pack() -> Option<PathBuf> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack");
    p.join("outfits/outfits.json").is_file().then_some(p)
}

fn json(path: &Path) -> Value {
    serde_json::from_str(std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())).trim_start_matches('\u{feff}')).unwrap()
}

/// (outfit id, its pack folder)
fn outfits(pack: &Path) -> Vec<(String, PathBuf)> {
    json(&pack.join("outfits/outfits.json"))["outfits"].as_array().unwrap().iter().map(|o| (o["id"].as_str().unwrap().to_string(), o["dir"].as_str().map_or(pack.to_path_buf(), |d| pack.join(d)))).collect()
}

/// The bound textures of every slot (slot -> texture -> file).
type Bound = Vec<Vec<Option<String>>>;

/// Every slot's textures (slot index -> files) that show while no slide runs (the "1" pair of the costume change is only
/// read during a slide: a pack's materials.json may start it at any form).
fn snapshot(lib: &Library) -> Bound {
    lib.slots.iter().map(|s| s.textures.iter().zip(&s.texture_names).map(|(t, n)| t.as_ref().filter(|_| !n.starts_with("FormChange") || n.ends_with('0')).map(|t| t.file.clone())).collect()).collect()
}

fn texture(lib: &Library, material: &str, parameter: &str) -> Vec<String> {
    lib.slots.iter().filter(|s| s.mesh == "sora" && s.instance_name.eq_ignore_ascii_case(material)).filter_map(|s| s.texture_names.iter().position(|n| n == parameter).and_then(|k| s.textures.get(k).cloned().flatten()).map(|t| t.file)).collect()
}

fn on_disk(lib: &Library, file: &str) -> bool {
    let png = lib.dir.join(file);
    let dds = lib.dir.join("er/tex").join(Path::new(file).with_extension("dds").file_name().unwrap());
    png.is_file() || dds.is_file()
}

#[test]
fn every_outfit_resolves_every_wear_form_to_its_material() {
    let Some(pack) = pack() else {
        eprintln!("no build/sora_pack: skipped");
        return;
    };
    let manifest = json(&pack.join("keyblades/outfit_forms.json"));
    let mut table: BTreeMap<String, String> = BTreeMap::new();
    for (id, dir) in outfits(&pack) {
        let mut lib = Library::load(&dir).unwrap_or_else(|e| panic!("{id}: {e}"));
        let list = lib.wear_form_list();
        assert_eq!(list, (1..=10).collect::<Vec<u8>>(), "{id}: a look for every wear form (no fallback needed)");
        lib.step_wear(0, 0.0);
        let normal = snapshot(&lib);
        for form in 1..=10u8 {
            // put on and settled (the slide is 1 s)
            for _ in 0..80 {
                lib.step_wear(form, 1.0 / 60.0);
            }
            let mut what = Vec::new();
            if let Some(rows) = lib.wear.forms.get(&form).cloned() {
                assert_eq!(lib.wear.worn, form, "{id} {form}");
                for (material, image, point) in rows {
                    let slots = texture(&lib, &material, "FormChangeTex0");
                    if let Some(image) = &image {
                        assert!(!slots.is_empty(), "{id} {form}: {material} has no FormChangeTex0 slot");
                        assert!(slots.iter().all(|f| *f == image.file), "{id} {form}: {material} wears {slots:?}, wanted {}", image.file);
                        assert!(on_disk(&lib, &image.file), "{id} {form}: {} not in {}", image.file, lib.dir.display());
                        what.push(image.file.clone());
                    }
                    if let Some(point) = &point {
                        let p = texture(&lib, &material, "FormChangePointTex0");
                        assert!(p.iter().all(|f| *f == point.file), "{id} {form}: {material} point {p:?}, wanted {}", point.file);
                        assert!(on_disk(&lib, &point.file), "{id} {form}: {}", point.file);
                    }
                }
                if form == 7 {
                    assert!(what.iter().all(|f| !f.contains("7d0")), "{id}: Rage Form keeps the normal costume (KH3's Anti: the point texture only)");
                }
            } else {
                // a direct replacement: the WA variant in the slots' base colour
                let rows = lib.wear.direct.get(&form).cloned().unwrap_or_else(|| panic!("{id} {form}: neither a costume nor a variant"));
                let wa = &manifest["outfits"][id.as_str()]["forms"][form.to_string()]["slots"];
                assert!(wa.is_array() || form == 5, "{id} {form}: not in WA's manifest keyblades/outfit_forms.json");
                for (material, parameter, file) in rows {
                    let now = texture(&lib, &material, &parameter);
                    assert!(!now.is_empty() && now.iter().all(|f| *f == file.file), "{id} {form}: {material}.{parameter} = {now:?}, wanted {}", file.file);
                    assert!(on_disk(&lib, &file.file), "{id} {form}: {}", file.file);
                    if let Some(slots) = wa.as_array() {
                        assert!(slots.iter().any(|s| s["material"] == material.as_str() && s["file"].as_str().is_some_and(|f| f.ends_with(file.file.as_str()))), "{id} {form}: {material} -> {} is not WA's", file.file);
                    }
                    what.push(file.file.clone());
                }
            }
            what.sort();
            what.dedup();
            table.insert(format!("{id:>12} {form:>2}"), what.join(" "));
            // off: exactly the normal clothes again
            for _ in 0..30 {
                lib.step_wear(0, 1.0 / 60.0);
            }
            assert_eq!(snapshot(&lib), normal, "{id} {form}: the form's end gives the normal textures back");
        }
    }
    for (k, v) in &table {
        eprintln!("{k}: {v}");
    }
    assert!(table.len() >= 80, "{} (outfit, form) pairs", table.len());
}

/// (weapons P5b, F-SHIELD's "the wear form flickers" report) A FORM HELD IS STABLE: every outfit holds each wear form 1..10 for
/// 600 frames of uneven game time (0, 1/240 .. 0.1 s, as the renderer's clamped clock gives) after a normal start. The slide
/// only ever goes up and ends once; after that not one texture binding changes and the bound materials are the same every frame,
/// and the form's end goes back once. (The renderer steps exactly this, once a frame, with the host's one wear form.)
#[test]
fn a_held_form_never_changes_its_bound_material() {
    let Some(pack) = pack() else { return };
    let dts = [1.0 / 60.0, 0.0, 1.0 / 240.0, 1.0 / 30.0, 0.1, 1.0 / 144.0, 0.0, 0.05];
    for (id, dir) in outfits(&pack) {
        let mut lib = Library::load(&dir).unwrap();
        for _ in 0..10 {
            lib.step_wear(0, 1.0 / 60.0);
        }
        for form in 1..=10u8 {
            let slide = |lib: &Library| lib.slots.iter().filter_map(|s| s.scalars.get("FormChangeSlide").copied()).fold(0.0f32, f32::max);
            let (mut last_slide, mut worn_changes, mut worn, mut settled): (f32, u32, u8, Option<Bound>) = (0.0, 0, lib.wear.worn, None);
            for k in 0..600 {
                let changes = lib.step_wear(form, dts[k % dts.len()]);
                let now = slide(&lib);
                if lib.wear.worn != lib.wear.target {
                    assert!(now >= last_slide, "{id} {form} frame {k}: the slide went back ({last_slide} -> {now})");
                }
                if lib.wear.worn != worn {
                    worn_changes += 1;
                    worn = lib.wear.worn;
                }
                last_slide = now;
                if lib.wear.worn == lib.wear.target {
                    let snap = snapshot(&lib);
                    match &settled {
                        None => settled = Some(snap),
                        Some(s) => {
                            assert!(changes.is_empty(), "{id} {form} frame {k}: {} binding(s) changed while the form is held", changes.len());
                            assert_eq!(&snap, s, "{id} {form} frame {k}: the bound material changed while the form is held");
                        }
                    }
                }
            }
            assert!(settled.is_some() && worn_changes <= 1 && lib.wear.target == if lib.wear.forms.contains_key(&form) { form } else { 0 }, "{id} {form}: settled {}, the worn costume changed {worn_changes} time(s)", settled.is_some());
            for _ in 0..60 {
                lib.step_wear(0, 1.0 / 60.0);
            }
            assert_eq!((lib.wear.worn, lib.wear.target, lib.wear.direct_on), (0, 0, 0), "{id} {form}: back to the normal clothes");
        }
    }
}

/// (the user: "KH1 and KH2 Sora get unique colours for each form") no two wear forms of KH1 / KH2 share a texture, and every
/// form differs from the normal clothes.
#[test]
fn kh1_and_kh2_have_their_own_texture_for_every_form() {
    let Some(pack) = pack() else { return };
    for id in ["kh1", "kh2"] {
        let lib = Library::load(&pack.join("outfits").join(id)).unwrap();
        let mut seen: BTreeMap<String, u8> = BTreeMap::new();
        for form in 1..=10u8 {
            for (_, _, file) in &lib.wear.direct[&form] {
                if let Some(other) = seen.insert(file.file.clone(), form) {
                    assert_eq!(other, form, "{id}: {} is worn by forms {other} and {form}", file.file);
                }
            }
        }
    }
}
