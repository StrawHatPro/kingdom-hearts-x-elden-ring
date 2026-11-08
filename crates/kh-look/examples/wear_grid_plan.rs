//! (weapons P5b) THE OUTFIT x WEAR FORM PICTURE PLAN, from the real switch: every outfit's library (`Library::load`, the
//! renderer's) is put into wear form 0..=10 with `Library::step_wear` and settled; each body slot's base colour as the switch
//! left it (its `FormChangeTex0`, else its base colour parameter) goes into a panel of kh-er-render's `kb_pics` plan (rows =
//! outfits, columns = wear forms). Rendered offline by:
//!
//! ```text
//! cargo run -p kh-look --example wear_grid_plan -- --out build/sora_tmp/weapons/P5b/shots/grid_plan.json [--outfits kh3,kh1,kh2]
//! cargo run --release -p kh-er-render --example kb_pics -- --plan build/sora_tmp/weapons/P5b/shots/grid_plan.json --out build/sora_tmp/weapons/P5b/shots/grid.png
//! ```

use std::path::{Path, PathBuf};

use kh_look::library::Library;
use serde_json::{json, Map, Value};

const BASE: [&str; 6] = ["FormChangeTex0", "BaseColorTex", "BaseColorTexture", "BaseTex", "EyeTexture", "Diffuse"];

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().map_err(|e| e.to_string())?;
    let pack = root.join("build/sora_pack");
    let out = arg("--out").map_or_else(|| root.join("build/sora_tmp/weapons/P5b/shots/grid_plan.json"), PathBuf::from);
    let forms: Vec<u8> = arg("--forms").map_or_else(|| (0..=10).collect(), |f| f.split(',').filter_map(|x| x.parse().ok()).collect());
    let index: Value = serde_json::from_str(std::fs::read_to_string(pack.join("outfits/outfits.json")).map_err(|e| e.to_string())?.trim_start_matches('\u{feff}')).map_err(|e| e.to_string())?;
    let wanted: Option<Vec<String>> = arg("--outfits").map(|o| o.split(',').map(str::to_string).collect());
    let rel = |p: &Path| p.strip_prefix(&root).unwrap_or(p).to_string_lossy().replace('\\', "/");
    let mut panels = Vec::new();
    let mut ids: Vec<(String, PathBuf, PathBuf)> = Vec::new();
    for o in index["outfits"].as_array().into_iter().flatten() {
        let id = o["id"].as_str().unwrap_or("").to_string();
        let (dir, glb) = match o["dir"].as_str() {
            Some(d) => (pack.join(d), pack.join(d).join("model.glb")),
            None => (pack.clone(), pack.join("sora.glb")),
        };
        ids.push((id, dir, glb));
    }
    if let Some(w) = &wanted {
        ids.retain(|(id, _, _)| w.contains(id));
        ids.sort_by_key(|(id, _, _)| w.iter().position(|x| x == id));
    }
    for (id, dir, glb) in &ids {
        let mut lib = Library::load(dir)?;
        lib.step_wear(0, 0.0);
        for &form in &forms {
            for _ in 0..90 {
                lib.step_wear(form, 1.0 / 60.0);
            }
            let mut slots = Map::new();
            for s in lib.slots.iter().filter(|s| s.mesh == "sora") {
                let Some(t) = BASE.iter().find_map(|p| s.texture_names.iter().position(|n| n == p).and_then(|k| s.textures.get(k).cloned().flatten())) else { continue };
                let file = if Path::new(&t.file).is_absolute() { PathBuf::from(&t.file) } else { lib.dir.join(&t.file) };
                if file.is_file() {
                    slots.insert(s.slot.clone(), Value::String(rel(&file)));
                }
            }
            panels.push(json!({ "label": format!("{} wear form {form}", id.to_uppercase()), "glb": rel(glb), "slots": slots }));
            for _ in 0..30 {
                lib.step_wear(0, 1.0 / 60.0);
            }
        }
    }
    let plan = json!({ "columns": forms.len(), "tile": [200, 330], "view": "front", "panels": panels });
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&out, serde_json::to_string_pretty(&plan).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    println!("{}: {} panels ({} outfits x {} wear forms)", out.display(), panels.len(), ids.len(), forms.len());
    Ok(())
}
