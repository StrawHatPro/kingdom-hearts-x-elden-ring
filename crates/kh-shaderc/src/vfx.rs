//! (effects) THE EFFECT MATERIALS for the Elden Ring renderer:
//!
//! ```text
//! cargo run --release -p kh-shaderc -- --vfx [--only NAME] [--jobs N]
//! ```
//!
//! Input: `crates/kh-er-render/shaders/er_vfx.wgsl` (the host's harness) with
//! `PACK/vfx/shaders/materials.wgsl` spliced in (the game's effect shaders, translated by
//! tools/sora_vfx_shaders.js: pixel stage per owner and vertex-factory class, the mesh particles'
//! vertex stage, the distortion pass, opacity masks) and `PACK/vfx/shaders/shading.json` (which
//! owner has what). Output: `PACK/vfx/er/`:
//!
//! | file | what |
//! |---|---|
//! | `quad.vs`, `plain.vs`, `mesh.vs` | the three generic vertex stages (sprites, ribbons / decals, mesh particles) |
//! | `v<id>.vs` | owner `<id>`'s own mesh-particle vertex shader (world position offset) |
//! | `s<id>.ps`, `m<id>.ps`, `d<id>.ps` | owner `<id>`'s pixel shader for sprites, mesh particles, decals |
//! | `xs<id>.ps`, `xm<id>.ps` | its distortion pass for sprites / mesh particles |
//! | `generic.ps` | the generic variant (a material without a translated shader) |
//! | `screen.vs`, `tone.ps`, `floor.ps` | the offline harness's own passes |
//! | `shaders.json` | every variant with `ok` and, for a failure, why |
//!
//! (all with the extension `.dxil`). A variant that fails (naga or dxc) is recorded and the
//! build goes on: the renderer draws such a material with the generic variant. The HLSL of the
//! failures is kept in `build/sora_tmp/er/fx/hlsl` for reading. Compiles on several threads.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::Value;

use crate::{preprocess, run_dxc, to_hlsl, VFX_HARNESS};

struct Job {
    name: String,
    stage: naga::ShaderStage,
    entry: &'static str,
    defines: BTreeSet<String>,
    /// For the count: "pixel", "vertex", "distort", "host".
    kind: &'static str,
}

fn defines(list: &[&str]) -> BTreeSet<String> {
    list.iter().map(|d| d.to_string()).collect()
}

pub fn run(project: &Path, pack: &Path, dxc: &Path, only: Option<&str>, jobs: Option<usize>) -> Result<(), String> {
    let dir = pack.join("vfx");
    let shading_path = dir.join("shaders").join("shading.json");
    let shading: Value = serde_json::from_str(&std::fs::read_to_string(&shading_path).map_err(|e| format!("{}: {e} (node tools/sora_vfx_shaders.js)", shading_path.display()))?).map_err(|e| e.to_string())?;
    let generated_path = dir.join("shaders").join("materials.wgsl");
    let generated = std::fs::read_to_string(&generated_path).map_err(|e| format!("{}: {e}", generated_path.display()))?;
    let out_dir = dir.join("er");
    // (VFXR) KH_SHADERC_HLSL_DIR: where the HLSL goes (default as before)
    let hlsl_dir: PathBuf = std::env::var_os("KH_SHADERC_HLSL_DIR").map(PathBuf::from).unwrap_or_else(|| project.join("build").join("sora_tmp").join("er").join("fx").join("hlsl"));
    std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&hlsl_dir).map_err(|e| e.to_string())?;
    let spliced = VFX_HARNESS.replace("//KH_GENERATED", &generated);

    // ---- what to build
    let mut list: Vec<Job> = vec![
        Job { name: "quad".into(), stage: naga::ShaderStage::Vertex, entry: "vs_quad", defines: defines(&["ER_QUAD", "KH_GENERIC"]), kind: "host" },
        Job { name: "plain".into(), stage: naga::ShaderStage::Vertex, entry: "vs_plain", defines: defines(&["ER_PLAIN", "KH_GENERIC"]), kind: "host" },
        Job { name: "mesh".into(), stage: naga::ShaderStage::Vertex, entry: "vs_mesh", defines: defines(&["ER_MESH", "KH_GENERIC"]), kind: "host" },
        Job { name: "generic".into(), stage: naga::ShaderStage::Fragment, entry: "fs_main", defines: defines(&["ER_FRAGMENT", "KH_GENERIC"]), kind: "host" },
        Job { name: "screen".into(), stage: naga::ShaderStage::Vertex, entry: "vs_screen", defines: defines(&["ER_SCREEN", "KH_GENERIC"]), kind: "host" },
        Job { name: "tone".into(), stage: naga::ShaderStage::Fragment, entry: "fs_tone", defines: defines(&["ER_SCREEN", "KH_GENERIC"]), kind: "host" },
        Job { name: "floor".into(), stage: naga::ShaderStage::Fragment, entry: "fs_floor", defines: defines(&["ER_SCREEN", "KH_GENERIC"]), kind: "host" },
        // (IMPL_ER53) the harness's forward-stage stand-in (a glow decal from a depth copy, effect panels): a NEW file
        Job { name: "fxtest".into(), stage: naga::ShaderStage::Fragment, entry: "fs_fxtest", defines: defines(&["ER_SCREEN", "KH_GENERIC"]), kind: "host" },
        // (IMPL_ER56) the same with the stencil-masked sky glow: a NEW file
        // (IMPL_ER65) + the sky glow masked by an early depth copy: a NEW file
        Job { name: "fxtest3".into(), stage: naga::ShaderStage::Fragment, entry: "fs_fxtest", defines: defines(&["ER_SCREEN", "KH_GENERIC"]), kind: "host" },
        Job { name: "fxtest2".into(), stage: naga::ShaderStage::Fragment, entry: "fs_fxtest", defines: defines(&["ER_SCREEN", "KH_GENERIC"]), kind: "host" },
        // (IMPL_ER78) the harness's light-pass stand-in for the hybrid (the diffuse light of the G-buffer's normals): a NEW file
        Job { name: "hylight".into(), stage: naga::ShaderStage::Fragment, entry: "fs_hylight", defines: defines(&["ER_SCREEN", "KH_GENERIC"]), kind: "host" },
        // (IMPL_ER80) the harness's glow source (the composite's second target), glow chain and TAA stand-ins: NEW files
        Job { name: "glowsrc".into(), stage: naga::ShaderStage::Fragment, entry: "fs_glowsrc", defines: defines(&["ER_SCREEN", "KH_GENERIC"]), kind: "host" },
        Job { name: "glowcomp".into(), stage: naga::ShaderStage::Fragment, entry: "fs_glowcomp", defines: defines(&["ER_SCREEN", "KH_GENERIC"]), kind: "host" },
        Job { name: "taa".into(), stage: naga::ShaderStage::Fragment, entry: "fs_taa", defines: defines(&["ER_SCREEN", "KH_GENERIC"]), kind: "host" },
        // (IMPL_ER80b) the harness's light-shaft source stand-in (the game's own composite shader draws it): a NEW file
        Job { name: "shaftsrc".into(), stage: naga::ShaderStage::Fragment, entry: "fs_shaftsrc", defines: defines(&["ER_SCREEN", "KH_GENERIC"]), kind: "host" },
        Job { name: "upadd".into(), stage: naga::ShaderStage::Fragment, entry: "fs_upadd", defines: defines(&["ER_SCREEN", "KH_GENERIC"]), kind: "host" },
    ];
    // (VFXR) the effect layer's post passes (kh-er-render vfxpost.rs): NEW files fxp_*.ps.dxil
    for entry in ["fs_fxp_bsrc", "fs_fxp_down", "fs_fxp_blur", "fs_fxp_bloom", "fs_fxp_radial", "fs_fxp_rcomp", "fs_fxp_sblur", "fs_fxp_filter", "fs_fxp_fringe"] {
        list.push(Job { name: entry.trim_start_matches("fs_").into(), stage: naga::ShaderStage::Fragment, entry, defines: defines(&["ER_SCREEN", "KH_GENERIC"]), kind: "host" });
    }
    // (HY) the harness's scene of Elden Ring's kind for the hybrid's proof (er_vfx.wgsl HY_SCENE): NEW files, built only with
    // `--only hylight2` / `--only hyfog` (never part of a whole build: the pack's files stay as they are)
    if matches!(only, Some("hylight2") | Some("hyfog")) {
        list.push(Job { name: "hylight2".into(), stage: naga::ShaderStage::Fragment, entry: "fs_hylight2", defines: defines(&["ER_SCREEN", "KH_GENERIC", "HY_SCENE"]), kind: "host" });
        list.push(Job { name: "hyfog".into(), stage: naga::ShaderStage::Fragment, entry: "fs_hyfog", defines: defines(&["ER_SCREEN", "KH_GENERIC", "HY_SCENE"]), kind: "host" });
    }
    // (ZFIN) the harness's stand-in for Elden Ring's characters (er_vfx.wgsl ER_CHARS, examples/effects.rs `--erchars`): a NEW
    // file, built only with `--only erchr`
    if only == Some("erchr") {
        list.push(Job { name: "erchr".into(), stage: naga::ShaderStage::Fragment, entry: "fs_erchr", defines: defines(&["ER_SCREEN", "KH_GENERIC", "ER_CHARS"]), kind: "host" });
    }
    let owners = shading.get("owners").and_then(Value::as_object).ok_or("shading.json: no owners")?;
    for (owner_name, owner) in owners.iter() {
        let Some(id) = owner.get("id").and_then(Value::as_u64) else { continue };
        for (class, letter, tag) in [("sprite", "s", "SPRITE"), ("mesh", "m", "MESH"), ("decal", "d", "DECAL")] {
            let Some(c) = owner.get(class).filter(|c| c.get("ok").and_then(Value::as_bool) == Some(true)) else { continue };
            let flag = |key: &str| c.get(key).and_then(Value::as_bool).unwrap_or(false);
            let mut d = defines(&["ER_FRAGMENT"]);
            d.insert(format!("KH_S{id}_{tag}"));
            if class == "mesh" {
                d.insert("KH_MESH".into());
            }
            if class == "decal" {
                d.insert("KH_DECAL".into());
            }
            if c.get("outputs").and_then(Value::as_u64).unwrap_or(1) >= 4 {
                d.insert("KH_DEFERRED".into());
            }
            // (ORBS) the prize orbs' lit balls (omt_prz_hpb_lit_00o, omt_prz_mpb_lit_00o): their own lit shading (er_vfx.wgsl kh_orb_shade)
            if owner_name.starts_with("omt_prz_") && class == "mesh" {
                d.insert("KH_ORB".into());
            }
            if flag("mask") {
                d.insert("KH_MASK".into());
            }
            list.push(Job { name: format!("{letter}{id}"), stage: naga::ShaderStage::Fragment, entry: "fs_main", defines: d.clone(), kind: "pixel" });
            if flag("distort") && class != "decal" {
                let mut x = d.clone();
                x.insert("KH_DISTORT".into());
                x.remove("KH_MASK");
                x.remove("KH_DEFERRED");
                list.push(Job { name: format!("x{letter}{id}"), stage: naga::ShaderStage::Fragment, entry: "fs_main", defines: x, kind: "distort" });
            }
            if class == "mesh" && flag("vs") {
                let mut v = defines(&["ER_MESH", "KH_VS", "KH_MESH"]);
                v.insert(format!("KH_S{id}_{tag}"));
                list.push(Job { name: format!("v{id}"), stage: naga::ShaderStage::Vertex, entry: "vs_mesh", defines: v, kind: "vertex" });
            }
        }
    }
    if let Some(only) = only {
        // (VFXR) `--only fxp`: every post pass of the effect layer (fxp_*)
        list.retain(|j| j.name == only || (only == "fxp" && j.name.starts_with("fxp_")));
    }
    let total = list.len();
    let threads = jobs.unwrap_or_else(|| std::thread::available_parallelism().map_or(4, |n| n.get())).clamp(1, 16);
    println!("kh-shaderc --vfx: pack {}, dxc {}, {total} variant(s) on {threads} thread(s)", pack.display(), dxc.display());

    // ---- build, several at a time
    let started = std::time::Instant::now();
    let queue = Mutex::new(list.into_iter().collect::<Vec<_>>());
    let results: Mutex<Vec<(String, &'static str, &'static str, Result<(String, u64), String>)>> = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| loop {
                let Some(job) = queue.lock().unwrap().pop() else { return };
                let (profile, extension) = if job.stage == naga::ShaderStage::Vertex { ("vs_6_0", "vs") } else { ("ps_6_0", "ps") };
                let file = format!("{}.{extension}.dxil", job.name);
                let hlsl_file = hlsl_dir.join(format!("{}.{extension}.hlsl", job.name));
                let built = preprocess(&spliced, &job.defines).and_then(|wgsl| to_hlsl(&wgsl, job.stage, job.entry)).and_then(|(text, hlsl_entry)| {
                    std::fs::write(&hlsl_file, &text).map_err(|e| e.to_string())?;
                    run_dxc(dxc, profile, &hlsl_entry, &hlsl_file, &out_dir.join(&file))?;
                    Ok((hlsl_entry, std::fs::metadata(out_dir.join(&file)).map(|m| m.len()).unwrap_or(0)))
                });
                match &built {
                    // (the HLSL of what compiled is not kept: nine hundred files nobody reads)
                    // (`--only NAME` keeps it, for reading)
                    Ok(_) if only.is_some() => {}
                    Ok(_) => {
                        let _ = std::fs::remove_file(&hlsl_file);
                    }
                    Err(_) => {
                        let _ = std::fs::remove_file(out_dir.join(&file));
                    }
                }
                results.lock().unwrap().push((job.name, extension, job.kind, built));
            });
        }
    });
    let mut results = results.into_inner().unwrap();
    results.sort_by(|a, b| (a.1, a.0.len(), &a.0).cmp(&(b.1, b.0.len(), &b.0)));

    // ---- the manifest and the count
    let mut written = Vec::new();
    let mut counts: std::collections::BTreeMap<&str, (u32, u32)> = std::collections::BTreeMap::new();
    let mut failures: Vec<(String, String)> = Vec::new();
    for (name, extension, kind, built) in &results {
        let count = counts.entry(kind).or_default();
        match built {
            Ok((entry, bytes)) => {
                count.0 += 1;
                written.push(serde_json::json!({ "name": name, "file": format!("{name}.{extension}.dxil"), "stage": extension, "kind": kind, "ok": true, "entry": entry, "bytes": bytes }));
            }
            Err(e) => {
                count.1 += 1;
                let first: String = e.lines().filter(|l| !l.trim().is_empty()).take(6).collect::<Vec<_>>().join(" | ");
                failures.push((format!("{name}.{extension}"), first.clone()));
                written.push(serde_json::json!({ "name": name, "stage": extension, "kind": kind, "ok": false, "why": first }));
            }
        }
    }
    for (kind, (ok, failed)) in &counts {
        println!("kh-shaderc --vfx: {kind}: {ok} built, {failed} failed");
    }
    for (name, why) in &failures {
        println!("  FAILED {name}: {}", why.chars().take(400).collect::<String>());
    }
    println!("kh-shaderc --vfx: {} of {total} in {:.1} s", total - failures.len(), started.elapsed().as_secs_f32());
    if only.is_none() {
        let manifest = serde_json::json!({
            "format": "kh3-er-vfx-shaders/1",
            "generator": concat!("crates/kh-shaderc ", env!("CARGO_PKG_VERSION"), " --vfx"),
            "source": { "shading": shading.get("generator").cloned().unwrap_or(Value::Null), "naga": "29 (wgsl-in -> hlsl-out, shader model 6.0)", "compiler": dxc.display().to_string() },
            "bindings": "b0 ErFrame, b1 KhParams (64 rows + misc + er); t0..t9 material textures, t10 / t11 shared wrap / clamp textures, t12 scene depth, t13 scene colour copy; sampler heap s0 space0, sampler index buffer t0 space1 (13 entries: one per texture, wrap, clamp, scene); vertex inputs LOC<n>: quad = 9 instance rows; plain = position, uv, colour, vertex colour, dynamic, misc, normal, tangent, particle; mesh = position, normal, tangent, uv, vertex colour + instance rows 5..10",
            "counts": counts.iter().map(|(k, (ok, failed))| (k.to_string(), serde_json::json!({ "ok": ok, "failed": failed }))).collect::<serde_json::Map<_, _>>(),
            "shaders": written,
        });
        std::fs::write(out_dir.join("shaders.json"), serde_json::to_string_pretty(&manifest).unwrap()).map_err(|e| e.to_string())?;
        println!("kh-shaderc --vfx: wrote {}", out_dir.join("shaders.json").display());
    }
    Ok(())
}
