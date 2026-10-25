//! The offline shader build of the Elden Ring renderer (see Cargo.toml).
//!
//! ```text
//! cargo run --release -p kh-shaderc [-- --pack DIR] [--dxc PATH] [--only VARIANT] [--hlsl DIR]
//! ```
//!
//! Input: `crates/kh-er-render/shaders/er_char.wgsl` (the host's harness), kh-look's shared harness
//! and `PACK/materials/materials.wgsl` (the game's shaders, translated by
//! tools/sora_material_pack.js). Output: `PACK/materials/er/skin.vs.dxil`, one
//! `<variant>.ps.dxil` per material variant the pack's slots use (`c<owner id>` + `m` masked +
//! `t` translucent + `s` subsurface) and `shaders.json`. The HLSL naga writes is kept in
//! `build/sora_tmp/er/er2/hlsl` (or `--hlsl DIR`) for reading.
//!
//! The binding contract with the renderer (kh-er-render/src/pipeline.rs has the same numbers):
//! cbuffers b0 frame, b1 material, b2 skin; textures t0..t13 the material's, t14 the colour table,
//! t15 the environment cube, t16 the scene depth copy; naga's sampler heap s0 (space 0) with the
//! index buffer t0 in space 1 (wrap, clamp, cube = heap entries 0, 1, 2).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use naga::back::hlsl;

const HARNESS: &str = include_str!("../../kh-er-render/shaders/er_char.wgsl");
/// (effects) The harness of the effect materials: see `vfx.rs` (`--vfx`).
const VFX_HARNESS: &str = include_str!("../../kh-er-render/shaders/er_vfx.wgsl");

mod vfx;
// (release) the repository kh-shaderc runs in (tools/packgen_shaders.js runs it there): a prebuilt binary names no build folder
const PROJECT: &str = ".";

/// `#ifdef NAME` / `#else` / `#endif` (nesting allowed), as the test app's preprocessor reads them.
fn preprocess(source: &str, defines: &BTreeSet<String>) -> Result<String, String> {
    let mut out = String::with_capacity(source.len());
    // (this level is emitting, some branch of this level was taken)
    let mut stack: Vec<(bool, bool)> = Vec::new();
    for (number, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        let live = stack.iter().all(|(on, _)| *on);
        if let Some(name) = trimmed.strip_prefix("#ifdef ") {
            let on = defines.contains(name.trim());
            stack.push((on, on));
        } else if let Some(name) = trimmed.strip_prefix("#ifndef ") {
            let on = !defines.contains(name.trim());
            stack.push((on, on));
        } else if trimmed == "#else" {
            let top = stack.last_mut().ok_or(format!("line {}: #else without #ifdef", number + 1))?;
            *top = (!top.1, true);
        } else if trimmed == "#endif" {
            stack.pop().ok_or(format!("line {}: #endif without #ifdef", number + 1))?;
        } else if trimmed.starts_with('#') {
            return Err(format!("line {}: directive not understood: {trimmed}", number + 1));
        } else if live {
            out.push_str(line);
        }
        // (every line keeps its number, so a compiler message points into the spliced text)
        out.push('\n');
    }
    if !stack.is_empty() {
        return Err("an #ifdef is not closed".to_string());
    }
    Ok(out)
}

fn target(space: u8, register: u32) -> hlsl::BindTarget {
    hlsl::BindTarget { space, register, binding_array_size: None, dynamic_storage_buffer_offsets_index: None, restrict_indexing: false }
}

fn options() -> hlsl::Options {
    let mut binding_map = hlsl::BindingMap::default();
    // (0 frame, 1 material, 2 skin, 3 the skin of the frame before)
    for binding in 0..4 {
        binding_map.insert(naga::ResourceBinding { group: 0, binding }, target(0, binding));
    }
    // (IMPL_ER59) 17..20: an outfit's extra material textures (er_char.wgsl KH_TEX_EXTRA)
    // (IMPL_ER78) 21 / 22: the game's diffuse light and shadow mask (er_char.wgsl KH_HYBRID)
    for binding in 0..23 {
        binding_map.insert(naga::ResourceBinding { group: 1, binding }, target(0, binding));
    }
    // Group 3: the GAME's own registers (passes that bind its descriptor tables): binding n = b<n> / t<n>.
    for binding in 0..14 {
        binding_map.insert(naga::ResourceBinding { group: 3, binding }, target(0, binding));
    }
    for binding in 20..66 {
        binding_map.insert(naga::ResourceBinding { group: 3, binding }, target(0, binding));
    }
    // A sampler's "register" is its place in the group's index buffer.
    // (the character harness uses 0..2; the effect harness one sampler per texture: 0..12)
    for binding in 0..16 {
        binding_map.insert(naga::ResourceBinding { group: 2, binding }, target(0, binding));
    }
    let mut sampler_buffer_binding_map = hlsl::SamplerIndexBufferBindingMap::default();
    sampler_buffer_binding_map.insert(hlsl::SamplerIndexBufferKey { group: 2 }, target(1, 0));
    hlsl::Options {
        shader_model: hlsl::ShaderModel::V6_0,
        binding_map,
        fake_missing_bindings: false,
        sampler_heap_target: hlsl::SamplerHeapBindTargets { standard_samplers: target(0, 0), comparison_samplers: target(2, 0) },
        sampler_buffer_binding_map,
        ..hlsl::Options::default()
    }
}

/// WGSL -> HLSL for one entry point; returns the text and the entry point's HLSL name.
fn to_hlsl(wgsl: &str, stage: naga::ShaderStage, entry: &str) -> Result<(String, String), String> {
    let module = naga::front::wgsl::parse_str(wgsl).map_err(|e| e.emit_to_string(wgsl))?;
    let info = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all()).validate(&module).map_err(|e| e.emit_to_string(wgsl))?;
    let options = options();
    let pipeline = hlsl::PipelineOptions { entry_point: Some((stage, entry.to_string())) };
    let mut text = String::new();
    let reflection = hlsl::Writer::new(&mut text, &options, &pipeline).write(&module, &info, None).map_err(|e| format!("naga hlsl: {e}"))?;
    let name = reflection.entry_point_names.into_iter().next().ok_or("no entry point written")?.map_err(|e| format!("naga hlsl entry point: {e}"))?;
    Ok((text, name))
}

fn find_dxc(given: Option<&str>) -> Result<PathBuf, String> {
    if let Some(path) = given.map(PathBuf::from).or_else(|| std::env::var_os("KH_DXC").map(PathBuf::from)) {
        return if path.is_file() { Ok(path) } else { Err(format!("{} is not a file", path.display())) };
    }
    // The newest Windows SDK that has one.
    let kits = Path::new(r"C:\Program Files (x86)\Windows Kits\10\bin");
    let mut versions: Vec<PathBuf> = std::fs::read_dir(kits).map_err(|e| format!("{}: {e} (give --dxc PATH)", kits.display()))?.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.join("x64").join("dxc.exe").is_file()).collect();
    versions.sort();
    versions.pop().map(|v| v.join("x64").join("dxc.exe")).ok_or(format!("no dxc.exe under {} (give --dxc PATH)", kits.display()))
}

fn run_dxc(dxc: &Path, profile: &str, entry: &str, hlsl_file: &Path, out: &Path) -> Result<(), String> {
    let result = Command::new(dxc)// (HLSL 2018: naga writes the ternary operator on vectors, as wgpu compiles it)
        .args(["-nologo", "-HV", "2018", "-Wno-parentheses-equality", "-T", profile, "-E", entry, "-O3", "-Fo"]).arg(out).arg(hlsl_file).output().map_err(|e| format!("{}: {e}", dxc.display()))?;
    if !result.status.success() || !out.is_file() {
        return Err(format!("dxc failed for {}:\n{}{}", hlsl_file.display(), String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr)));
    }
    Ok(())
}

/// Copies the cooked DDS of every texture the pack's materials name
/// (`assets_converted/textures/KINGDOM HEARTS III/Content/<asset path>.dds`) to
/// `PACK/materials/er/tex/<the PNG's name>.dds` when it is one of the block formats the renderer
/// reads (DXT1, DXT5, ATI2, BC7) and has the size the pack records. Returns (files, bytes).
fn copy_textures(project: &Path, materials_dir: &Path, out_dir: &Path) -> Result<(u32, u64), String> {
    let text = std::fs::read_to_string(materials_dir.join("materials.json")).map_err(|e| e.to_string())?;
    let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let sources = project.join("assets_converted").join("textures").join("KINGDOM HEARTS III").join("Content");
    let tex = out_dir.join("tex");
    std::fs::create_dir_all(&tex).map_err(|e| e.to_string())?;
    let (mut files, mut bytes) = (0u32, 0u64);
    let mut done: BTreeSet<String> = BTreeSet::new();
    let materials = json.get("materials").and_then(|m| m.as_object()).ok_or("materials.json: no materials")?;
    // (IMPL_ER23B) + the wear forms' textures (Second Form's costume: `wear_forms.<form>.materials[].image / point`)
    let wear: Vec<serde_json::Value> = json.get("wear_forms").and_then(|w| w.as_object()).into_iter().flatten().flat_map(|(_, f)| f.get("materials").and_then(|m| m.as_array()).cloned().unwrap_or_default()).flat_map(|m| [m.get("image").cloned(), m.get("point").cloned()]).flatten().collect();
    for texture in materials.values().filter_map(|m| m.get("textures").and_then(|t| t.as_array())).flatten().chain(wear.iter()) {
        let (Some(file), Some(asset)) = (texture.get("file").and_then(|f| f.as_str()), texture.get("asset").and_then(|a| a.as_str())) else { continue };
        if !done.insert(file.to_string()) {
            continue;
        }
        let Some(path) = asset.split('.').next().and_then(|p| p.strip_prefix("/Game/")) else { continue };
        let source = sources.join(format!("{path}.dds"));
        let Ok(data) = std::fs::read(&source) else { continue };
        let word = |at: usize| data.get(at..at + 4).map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        let four = data.get(84..88).unwrap_or(&[0; 4]);
        let block = matches!(four, b"DXT1" | b"DXT5" | b"ATI2") || (four == b"DX10" && matches!(word(128), 71 | 72 | 77 | 78 | 83 | 98 | 99));
        let sized = texture.get("width").is_none() || (texture.get("width").and_then(|w| w.as_u64()) == Some(word(16) as u64) && texture.get("height").and_then(|h| h.as_u64()) == Some(word(12) as u64));
        if !block || !sized || word(16) % 4 != 0 || word(12) % 4 != 0 {
            continue;
        }
        let stem = Path::new(file).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        std::fs::write(tex.join(format!("{stem}.dds")), &data).map_err(|e| e.to_string())?;
        files += 1;
        bytes += data.len() as u64;
    }
    Ok((files, bytes))
}

fn main() {
    if let Err(e) = run() {
        eprintln!("kh-shaderc: FAILED: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |name: &str| args.iter().position(|a| a == name).and_then(|k| args.get(k + 1)).map(String::as_str);
    let project = PathBuf::from(PROJECT);
    let pack = value("--pack").map(PathBuf::from).unwrap_or_else(|| project.join("build").join("sora_pack"));
    let hlsl_dir = value("--hlsl").map(PathBuf::from).unwrap_or_else(|| project.join("build").join("sora_tmp").join("er").join("er2").join("hlsl"));
    let only = value("--only");
    let dxc = find_dxc(value("--dxc"))?;
    // (effects) `--vfx`: the effect materials of PACK/vfx instead of the character's (vfx.rs).
    if args.iter().any(|a| a == "--vfx") {
        return vfx::run(&project, &pack, &dxc, only, value("--jobs").and_then(|v| v.parse().ok()));
    }
    // (ZDRAW) Zettaflare's target from its own model into the finale's mask (kh-er-render shaders/er_zdraw.wgsl, src/zdraw.rs):
    // NEW files, built with `--only zdraw` only, into `--out DIR` (default build/sora_tmp/er/zdraw/pack/materials/er: never
    // the live pack)
    if only == Some("zdraw") {
        const ZDRAW: &str = include_str!("../../kh-er-render/shaders/er_zdraw.wgsl");
        let out_dir = value("--out").map(PathBuf::from).unwrap_or_else(|| project.join("build").join("sora_tmp").join("er").join("zdraw").join("pack").join("materials").join("er"));
        std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&hlsl_dir).map_err(|e| e.to_string())?;
        // (zdraw_body: the offline harness's stand-in for the game's own draw of the body)
        for (file, stage, entry, profile, ext) in [("zdraw", naga::ShaderStage::Vertex, "zdraw_vs", "vs_6_0", "vs"), ("zdraw", naga::ShaderStage::Fragment, "zdraw_ps", "ps_6_0", "ps"), ("zdraw_up", naga::ShaderStage::Vertex, "zdraw_up_vs", "vs_6_0", "vs"), ("zdraw_up", naga::ShaderStage::Fragment, "zdraw_up_ps", "ps_6_0", "ps"), ("zdraw_imm", naga::ShaderStage::Vertex, "zdraw_imm_vs", "vs_6_0", "vs"), ("zdraw_imm", naga::ShaderStage::Fragment, "zdraw_imm_ps", "ps_6_0", "ps"), ("zdraw_body", naga::ShaderStage::Vertex, "zdraw_body_vs", "vs_6_0", "vs"), ("zdraw_body", naga::ShaderStage::Fragment, "zdraw_body_ps", "ps_6_0", "ps")] {
            let (text, name) = to_hlsl(ZDRAW, stage, entry).map_err(|e| format!("zdraw: {e}"))?;
            let hlsl_file = hlsl_dir.join(format!("{file}.{ext}.hlsl"));
            std::fs::write(&hlsl_file, &text).map_err(|e| e.to_string())?;
            run_dxc(&dxc, profile, &name, &hlsl_file, &out_dir.join(format!("{file}.{ext}.dxil")))?;
            println!("  {file}.{ext}.dxil -> {}", out_dir.display());
        }
        return Ok(());
    }
    let library = kh_look::library::Library::load(&pack)?;
    // (ER81) an outfit's OWN SET: `--materials FILE` (the outfit's materials.wgsl with another post-process material:
    // tools/sora_material_pack.js --own-khs), `--suffix S` (every material variant written as `<variant>_S[_pl|_bl|_hy2|_gb2]`,
    // nothing else built; `--only bli` adds `_S_bli`) and `--out DIR` (default PACK/materials/er). Without them: as before.
    let suffix = value("--suffix");
    if suffix.is_some_and(|s| s.is_empty() || !s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())) {
        return Err("--suffix: lower-case letters and digits only".to_string());
    }
    let own = suffix.map(|s| format!("_{s}")).unwrap_or_default();
    let generated_path = value("--materials").map(PathBuf::from).unwrap_or_else(|| library.dir.join("materials.wgsl"));
    let generated = std::fs::read_to_string(&generated_path).map_err(|e| format!("{}: {e}", generated_path.display()))?;
    // (ER party S4) PARTY MODE (`--party`, for PACK/party/<npc> and PACK/party/team): the party's materials.wgsl holds its
    // KH_C<id> blocks only; the world's post-process material (kh_khs) is the MAIN pack's (`--main-pack DIR`, default
    // build/sora_pack), spliced in after them as kh-app merges the libraries; the skin has room for 320 joints (Goofy has
    // 207); the owners that read the pre-skinned position (materials.json `pre_skinned_position`) get KH_PSP, every shader
    // of the set KH_PSP_OUT (one vertex layout for the set). Without `--party` nothing of this applies.
    let party = args.iter().any(|a| a == "--party");
    let mut psp_ids: BTreeSet<u16> = BTreeSet::new();
    let generated = if party {
        let owners: serde_json::Value = std::fs::read_to_string(library.dir.join("materials.json")).ok().and_then(|t| serde_json::from_str(t.trim_start_matches('﻿')).ok()).unwrap_or_default();
        for owner in owners["owners"].as_object().into_iter().flatten().map(|(_, o)| o) {
            if owner["pre_skinned_position"].as_bool() == Some(true) || owner["pre_skinned_position"].is_object() {
                if let Some(id) = owner["id"].as_u64() {
                    psp_ids.insert(id as u16);
                }
            }
        }
        if generated.contains("fn kh_khs(") {
            generated
        } else {
            let main = value("--main-pack").map(PathBuf::from).unwrap_or_else(|| project.join("build").join("sora_pack"));
            let main_text = std::fs::read_to_string(main.join("materials").join("materials.wgsl")).map_err(|e| format!("--party: the main pack's materials.wgsl: {e}"))?;
            let at = main_text.find("// /Game/Shader/KHS_set").ok_or("--party: the main pack's materials.wgsl has no KHS block")?;
            println!("kh-shaderc: (party) the world's post-process material from {} spliced in; owners with the pre-skinned position: {psp_ids:?}", main.display());
            format!("{generated}
{}", &main_text[at..])
        }
    } else {
        generated
    };
    let out_dir = value("--out").map(PathBuf::from).unwrap_or_else(|| library.dir.join("er"));
    std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&hlsl_dir).map_err(|e| e.to_string())?;
    let spliced = HARNESS.replace("//KH_COMMON", kh_look::CHAR_COMMON_WGSL).replace("//KH_GENERATED", &generated);
    println!("kh-shaderc: pack {} ({}), dxc {}", pack.display(), library.generator, dxc.display());

    let mut written: Vec<serde_json::Value> = Vec::new();
    // (IMPL_ER59) an outfit's materials that bind more than 14 textures: kh_t14.. declared (er_char.wgsl KH_TEX_EXTRA)
    let tex_extra = generated.contains("kh_t14");
    // (ER69) an outfit's pack (PACK/outfits/<id>: its model.glb beside materials/): the skin has room for 320 joints
    let joints_extra = pack.join("model.glb").is_file() || party;
    let mut build = |name: &str, stage: naga::ShaderStage, entry: &str, mut defines: BTreeSet<String>| -> Result<(), String> {
        if tex_extra {
            defines.insert("KH_TEX_EXTRA".to_string());
        }
        if joints_extra {
            defines.insert("KH_JOINTS_EXTRA".to_string());
        }
        // (ER party S4) party mode: one vertex layout for the set; KH_PSP for the owners that read the pre-skinned position
        if party {
            defines.insert("KH_PSP_OUT".to_string());
            if psp_ids.iter().any(|id| defines.contains(&format!("KH_C{id}"))) {
                defines.insert("KH_PSP".to_string());
            }
        }
        let started = std::time::Instant::now();
        let wgsl = preprocess(&spliced, &defines)?;
        let (text, hlsl_entry) = to_hlsl(&wgsl, stage, entry).map_err(|e| format!("{name}: {e}"))?;
        let (profile, extension) = if stage == naga::ShaderStage::Vertex { ("vs_6_0", "vs") } else { ("ps_6_0", "ps") };
        let hlsl_file = hlsl_dir.join(format!("{name}.{extension}.hlsl"));
        std::fs::write(&hlsl_file, &text).map_err(|e| e.to_string())?;
        let file = format!("{name}.{extension}.dxil");
        run_dxc(&dxc, profile, &hlsl_entry, &hlsl_file, &out_dir.join(&file))?;
        let bytes = std::fs::metadata(out_dir.join(&file)).map(|m| m.len()).unwrap_or(0);
        println!("  {file}: {bytes} bytes ({} lines of HLSL, {:.1} s)", text.lines().count(), started.elapsed().as_secs_f32());
        written.push(serde_json::json!({ "name": name, "file": file, "stage": extension, "entry": hlsl_entry, "bytes": bytes, "defines": defines.iter().collect::<Vec<_>>() }));
        Ok(())
    };

    // (NF) `--rebuild LIST`: rebuild EXACTLY the shaders a JSON file names ({"jobs": [{"name", "stage": "ps" | "vs", "entry",
    // "defines": [..]}]}: the defines each pack file was built with, from its index (shaders*.json) or its name), with this
    // run's --pack / --materials / --party / --main-pack / --out, and nothing else (no index is written: the pack's own
    // indexes keep describing the files). build/sora_tmp/party/NF/tools/rebuild_plan.js writes the lists.
    if let Some(list) = value("--rebuild") {
        let text = std::fs::read_to_string(list).map_err(|e| format!("--rebuild {list}: {e}"))?;
        let json: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("--rebuild {list}: {e}"))?;
        let mut n = 0;
        for job in json["jobs"].as_array().into_iter().flatten() {
            let name = job["name"].as_str().ok_or("--rebuild: a job without a name")?;
            let stage = if job["stage"].as_str() == Some("vs") { naga::ShaderStage::Vertex } else { naga::ShaderStage::Fragment };
            let entry = job["entry"].as_str().ok_or("--rebuild: a job without an entry")?;
            let defines: BTreeSet<String> = job["defines"].as_array().into_iter().flatten().filter_map(|d| d.as_str().map(str::to_string)).collect();
            build(name, stage, entry, defines)?;
            n += 1;
        }
        println!("kh-shaderc: (NF) rebuilt {n} shader(s) of {list} into {}", out_dir.display());
        return Ok(());
    }
    // (ER91) THE ER91 HAIR PATH (kh-er-render hair.rs / hair91.rs): `--hair91 OWNERS` (as `--hair`; `ID:6` = the fur's six
    // screen phases, which keeps ER87's coverage) with `--materials` = the ER91 translation (er91/tools/hair91.js: the strand
    // mask's clamp-U / wrap-V sampler, the dither phase input and the final test's margin) builds ONLY, for every masked
    // variant of those owners and every set the DLL may pick ("", _pl, _bl, _bli, _hy2): `<variant>[_S]<set>_h91` (the core:
    // the continuous supersampled coverage at least 1/2, opaque), `_h91f` (the fringe below it, alpha), `_h91m` / `_h91n` (the
    // 4x hair pass: the same two halves with their true coverage, premultiplied, + motion vector and depth), and
    // `<variant>[_S]_gb2_h91` (the hybrid's G-buffer pixels = the core's); `shaders_h91[_S].json`. New names only.
    if let Some(spec) = value("--hair91") {
        let mut owners: Vec<(u16, bool)> = Vec::new();
        for part in spec.split(',').filter(|p| !p.trim().is_empty()) {
            let (id, six) = match part.split_once(':') {
                Some((id, n)) if n.trim() == "6" => (id, true),
                Some((_, n)) if n.trim() != "5" => return Err(format!("--hair91 {part}: 5 or 6 phases")),
                Some((id, _)) => (id, false),
                None => (part, false),
            };
            owners.push((id.trim().parse().map_err(|_| format!("--hair91 {part}: an owner id"))?, six));
        }
        let mut variants: BTreeSet<(u16, bool, bool, bool)> = BTreeSet::new();
        for slot in library.slots.iter().filter(|s| s.masked && !s.translucent && owners.iter().any(|(id, _)| *id == s.owner_id)) {
            variants.insert((slot.owner_id, slot.masked, slot.translucent, slot.subsurface));
        }
        if variants.is_empty() {
            return Err(format!("--hair91 {spec}: no masked slot of those owners in {}", pack.display()));
        }
        for (id, masked, translucent, subsurface) in variants {
            let six = owners.iter().any(|(o, s)| *o == id && *s);
            let base = format!("{}{own}", kh_look::library::variant_name(id, masked, translucent, subsurface));
            let mut common = BTreeSet::from(["ER_FRAGMENT".to_string(), format!("KH_C{id}"), "KH_MASKED".to_string(), "KH_HAIR".to_string()]);
            common.insert(if six { "KH_HAIR6" } else { "KH_HAIR91" }.to_string());
            if subsurface {
                common.insert("KH_SUBSURFACE".to_string());
            }
            common.extend(value("--hair-define").into_iter().flat_map(|v| v.split(',')).filter(|d| !d.is_empty()).map(str::to_string));
            let sets: [(&str, &[&str]); 6] = [
                ("", &[]),
                ("_pl", &["KH_EXTRA_LIGHTS"]),
                ("_bl", &["KH_EXTRA_LIGHTS", "KH_BLOOD"]),
                ("_bli", &["KH_EXTRA_LIGHTS", "KH_BLOOD", "KH_INTERIOR"]),
                ("_hy2", &["KH_EXTRA_LIGHTS", "KH_BLOOD", "KH_HYBRID"]),
                // (ER91 in the live tree, with HY's third hybrid: its set and, below, its hair G-buffer writer `_gb3_h91`)
                ("_hy3", &["KH_EXTRA_LIGHTS", "KH_BLOOD", "KH_INTERIOR", "KH_HYBRID", "KH_HY3"]),
            ];
            // (`--hair-sets LIST`, as HY's `--hair`: only those sets)
            let wanted_sets: Option<Vec<String>> = value("--hair-sets").map(|v| v.split(',').map(|s| s.trim().to_string()).collect());
            for (set, extra) in sets {
                if wanted_sets.as_ref().is_some_and(|w| !w.iter().any(|s| s == set)) {
                    continue;
                }
                if set == "_hy3" {
                    let mut defines = common.clone();
                    defines.extend(extra.iter().map(|d| d.to_string()));
                    defines.insert("KH_GBUFFER".to_string());
                    build(&format!("{base}_gb3_h91"), naga::ShaderStage::Fragment, "gb_main", defines)?;
                }
                for (tail, pass) in [("_h91", &["KH_HAIR_CORE"][..]), ("_h91f", &["KH_HAIR_FRINGE"]), ("_h91m", &["KH_HAIR_CORE", "KH_HAIR91_MSAA"]), ("_h91n", &["KH_HAIR_FRINGE", "KH_HAIR91_MSAA"])] {
                    let mut defines = common.clone();
                    defines.extend(pass.iter().map(|d| d.to_string()));
                    defines.extend(extra.iter().map(|d| d.to_string()));
                    // (the 4x pass shaded per sample, the coverage at each sample's point: true supersampling of the strands'
                    // alpha, their shading and the cards' edges; er91/shots z_s1: the per-pixel shading left a fine grain)
                    if tail.ends_with('m') || tail.ends_with('n') {
                        defines.insert("KH_HAIR91_SAMPLE".to_string());
                        if !six {
                            defines.insert("KH_HAIR91_POINT".to_string());
                        }
                    }
                    build(&format!("{base}{set}{tail}"), naga::ShaderStage::Fragment, "fs_main", defines)?;
                }
            }
            let mut defines = common.clone();
            defines.extend(["KH_EXTRA_LIGHTS", "KH_BLOOD", "KH_HYBRID", "KH_GBUFFER"].iter().map(|d| d.to_string()));
            build(&format!("{base}_gb2_h91"), naga::ShaderStage::Fragment, "gb_main", defines)?;
        }
        let manifest = serde_json::json!({
            "format": "kh3-er-shaders/1",
            "generator": concat!("crates/kh-shaderc ", env!("CARGO_PKG_VERSION")),
            "note": "(ER91) the hair path: <variant>[_suffix]<set>_h91 (core), _h91f (fringe), _h91m / _h91n (the 4x hair pass), <variant>[_suffix]_gb2_h91 (the hybrid's G-buffer)",
            "source": { "materials": generated_path.display().to_string(), "hair91": spec, "naga": "29 (wgsl-in -> hlsl-out, shader model 6.0)", "compiler": dxc.display().to_string() },
            "shaders": written,
        });
        let index = format!("shaders_h91{own}.json");
        std::fs::write(out_dir.join(&index), serde_json::to_string_pretty(&manifest).unwrap()).map_err(|e| e.to_string())?;
        println!("kh-shaderc: (ER91) wrote {}", out_dir.join(&index).display());
        return Ok(());
    }
    // (ER91) the 4x hair pass's own screen shaders (er_char.wgsl ER_H91): NEW files, built with `--only h91` only
    if only == Some("h91") {
        build("h91", naga::ShaderStage::Vertex, "h91_vs", BTreeSet::from(["ER_H91".to_string()]))?;
        build("h91init", naga::ShaderStage::Fragment, "h91_init_ps", BTreeSet::from(["ER_H91".to_string()]))?;
        build("h91init1", naga::ShaderStage::Fragment, "h91_init1_ps", BTreeSet::from(["ER_H91".to_string()]))?;
        build("h91comp", naga::ShaderStage::Fragment, "h91_comp_ps", BTreeSet::from(["ER_H91".to_string()]))?;
        build("h91velo", naga::ShaderStage::Fragment, "h91_velo_ps", BTreeSet::from(["ER_H91".to_string()]))?;
        return Ok(());
    }
    // (ER87) THE HAIR PATH (kh-er-render hair.rs): `--hair OWNERS` (owner ids, `9,10`; `ID:6` = a mask whose dither has six
    // phases, the fur's) builds ONLY, for every masked variant of those owners and every set the DLL may pick ("", _pl, _bl,
    // _bli, _hy2), `<variant>[_S]<set>_hair` (er_char.wgsl KH_HAIR + KH_HAIR_CORE: the fully covered pixels, opaque) and
    // `..._hairf` (KH_HAIR_FRINGE: the partly covered ones, alpha = coverage), and `shaders_hair[_S].json`. New names only.
    // Meant with `--materials FILE` (the translation with the hair mask's remainder restored: tools/sora_material_pack.js
    // ER87) and `--out DIR`.
    if let Some(spec) = value("--hair") {
        let mut owners: Vec<(u16, bool)> = Vec::new();
        for part in spec.split(',').filter(|p| !p.trim().is_empty()) {
            let (id, six) = match part.split_once(':') {
                Some((id, n)) if n.trim() == "6" => (id, true),
                Some((_, n)) if n.trim() != "5" => return Err(format!("--hair {part}: 5 or 6 phases")),
                Some((id, _)) => (id, false),
                None => (part, false),
            };
            owners.push((id.trim().parse().map_err(|_| format!("--hair {part}: an owner id"))?, six));
        }
        let mut variants: BTreeSet<(u16, bool, bool, bool)> = BTreeSet::new();
        for slot in library.slots.iter().filter(|s| s.masked && !s.translucent && owners.iter().any(|(id, _)| *id == s.owner_id)) {
            variants.insert((slot.owner_id, slot.masked, slot.translucent, slot.subsurface));
        }
        if variants.is_empty() {
            return Err(format!("--hair {spec}: no masked slot of those owners in {}", pack.display()));
        }
        for (id, masked, translucent, subsurface) in variants {
            let six = owners.iter().any(|(o, s)| *o == id && *s);
            let base = format!("{}{own}", kh_look::library::variant_name(id, masked, translucent, subsurface));
            let sets: [(&str, &[&str]); 6] = [
                ("", &[]),
                ("_pl", &["KH_EXTRA_LIGHTS"]),
                ("_bl", &["KH_EXTRA_LIGHTS", "KH_BLOOD"]),
                ("_bli", &["KH_EXTRA_LIGHTS", "KH_BLOOD", "KH_INTERIOR"]),
                ("_hy2", &["KH_EXTRA_LIGHTS", "KH_BLOOD", "KH_HYBRID"]),
                // (HY) the third hybrid (`--only hy3` above); its G-buffer writer for the hair's core follows
                ("_hy3", &["KH_EXTRA_LIGHTS", "KH_BLOOD", "KH_INTERIOR", "KH_HYBRID", "KH_HY3"]),
            ];
            // (HY) `--hair-sets LIST` (e.g. `_hy3`): only those sets (default: all, as before)
            let wanted_sets: Option<Vec<String>> = value("--hair-sets").map(|v| v.split(',').map(|s| s.trim().to_string()).collect());
            for (set, extra) in sets {
                if wanted_sets.as_ref().is_some_and(|w| !w.iter().any(|s| s == set)) {
                    continue;
                }
                // (HY) `<variant>[_S]_gb3_hair`: the hair core's G-buffer writer (gb_main with KH_HAIR: the core's pixels only)
                if set == "_hy3" {
                    let mut defines = BTreeSet::from(["ER_FRAGMENT".to_string(), format!("KH_C{id}"), "KH_MASKED".to_string(), "KH_HAIR".to_string(), "KH_GBUFFER".to_string()]);
                    if subsurface {
                        defines.insert("KH_SUBSURFACE".to_string());
                    }
                    if six {
                        defines.insert("KH_HAIR6".to_string());
                    }
                    defines.extend(extra.iter().map(|d| d.to_string()));
                    build(&format!("{base}_gb3_hair"), naga::ShaderStage::Fragment, "gb_main", defines)?;
                }
                for (tail, pass) in [("_hair", "KH_HAIR_CORE"), ("_hairf", "KH_HAIR_FRINGE")] {
                    let mut defines = BTreeSet::from(["ER_FRAGMENT".to_string(), format!("KH_C{id}"), "KH_MASKED".to_string(), "KH_HAIR".to_string(), pass.to_string()]);
                    if subsurface {
                        defines.insert("KH_SUBSURFACE".to_string());
                    }
                    if six {
                        defines.insert("KH_HAIR6".to_string());
                    }
                    defines.extend(extra.iter().map(|d| d.to_string()));
                    // (`--hair-define NAME,..`: extra definitions, for diagnostic builds)
                    defines.extend(value("--hair-define").into_iter().flat_map(|v| v.split(',')).filter(|d| !d.is_empty()).map(str::to_string));
                    build(&format!("{base}{set}{tail}"), naga::ShaderStage::Fragment, "fs_main", defines)?;
                }
            }
        }
        let manifest = serde_json::json!({
            "format": "kh3-er-shaders/1",
            "generator": concat!("crates/kh-shaderc ", env!("CARGO_PKG_VERSION")),
            "note": "(ER87) the hair path: <variant>[_suffix]<set>_hair (fully covered pixels, opaque) and _hairf (the fringe, alpha = coverage)",
            "source": { "materials": generated_path.display().to_string(), "hair": spec, "naga": "29 (wgsl-in -> hlsl-out, shader model 6.0)", "compiler": dxc.display().to_string() },
            "shaders": written,
        });
        let index = if value("--hair-sets").is_some() { format!("shaders_hair_sets{own}.json") } else { format!("shaders_hair{own}.json") };
        std::fs::write(out_dir.join(&index), serde_json::to_string_pretty(&manifest).unwrap()).map_err(|e| e.to_string())?;
        println!("kh-shaderc: (ER87) wrote {}", out_dir.join(&index).display());
        return Ok(());
    }
    // (ER81) an own set builds its material variants only
    if suffix.is_none() {
    if only.is_none() || only == Some("skin") {
        build("skin", naga::ShaderStage::Vertex, "vs_main", BTreeSet::from(["ER_VERTEX".to_string()]))?;
    }
    // (ER79) an outfit's skin with its leg guard (er_char.wgsl KH_CLOTH_GUARD): `--only skincg` only (a NEW file, read by
    // kh-er-render gfx.rs when the outfit has a legguard.json and `boc.leg_guard` is on)
    if only == Some("skincg") {
        build("skin_cg", naga::ShaderStage::Vertex, "vs_main", BTreeSet::from(["ER_VERTEX".to_string(), "KH_CLOTH_GUARD".to_string()]))?;
    }
    // (ER88) the velocity draw's skin with the own-record camera route (er_char.wgsl ER_VELO_OWN, kh-er-render taa.rs): NEW
    // files, built with `--only velo88` only
    if only == Some("velo88") {
        build("skin_v88", naga::ShaderStage::Vertex, "vs_main", BTreeSet::from(["ER_VERTEX".to_string(), "ER_VELO_OWN".to_string()]))?;
        // (only where the pack has the leg-guard skin: an outfit with legguard.json)
        if library.dir.join("er").join("skin_cg.vs.dxil").is_file() {
            build("skin_cg_v88", naga::ShaderStage::Vertex, "vs_main", BTreeSet::from(["ER_VERTEX".to_string(), "KH_CLOTH_GUARD".to_string(), "ER_VELO_OWN".to_string()]))?;
        }
    }
    // The contact shadow (a screen-space decal from the scene depth).
    if only.is_none() || only == Some("blob") {
        build("blob", naga::ShaderStage::Vertex, "blob_vs", BTreeSet::from(["ER_BLOB".to_string()]))?;
        build("blob", naga::ShaderStage::Fragment, "blob_ps", BTreeSet::from(["ER_BLOB".to_string()]))?;
    }
    // The discovery tool that copies a constant buffer of the game into a texture.
    if only.is_none() || only == Some("dump") {
        for register in 4..14 {
            build(&format!("dump{register}"), naga::ShaderStage::Fragment, "dump_ps", BTreeSet::from(["ER_DUMP".to_string(), format!("ER_DUMP_B{register}")]))?;
        }
    }
    // (IMPL_ER17) His real shadow: the depth-only vertex stage for the game's cascade atlas.
    if only.is_none() || only == Some("shadow") {
        build("shadow", naga::ShaderStage::Vertex, "shadow_vs", BTreeSet::from(["ER_SHADOW".to_string()]))?;
    }
    // His motion vectors for the game's velocity target.
    if only.is_none() || only == Some("velo") {
        build("velo", naga::ShaderStage::Fragment, "velo_ps", BTreeSet::from(["ER_VELO".to_string()]))?;
    }
    // The light probes around his feet (the game-light pass).
    if only.is_none() || only == Some("probe") {
        build("probe", naga::ShaderStage::Fragment, "probe_ps", BTreeSet::from(["ER_PROBE".to_string()]))?;
    }
    // (IMPL_ER65) The late re-composite of his colour over the scene (a NEW file).
    if only.is_none() || only == Some("recomp") {
        build("recomp", naga::ShaderStage::Fragment, "recomp_ps", BTreeSet::from(["ER_RECOMP".to_string()]))?;
    }
    // (IMPL_ER80) The late re-composite drawn before the game's TAA, its far condition a smooth weight: a NEW file.
    if only.is_none() || only == Some("recomp2") {
        build("recomp2", naga::ShaderStage::Fragment, "recomp2_ps", BTreeSet::from(["ER_RECOMP2".to_string()]))?;
    }
    // (ZFIN) Zettaflare's finale on an Elden Ring character (er_char.wgsl ER_FINALE, kh-er-render finale.rs): a NEW file,
    // built with `--only finale` only
    if only == Some("finale") {
        build("finale", naga::ShaderStage::Fragment, "finale_ps", BTreeSet::from(["ER_FINALE".to_string()]))?;
        // its stencil value learnt at run time (the probe), the white shape kept for its fade (g) and the fade
        build("finale_probe", naga::ShaderStage::Fragment, "finale_probe_ps", BTreeSet::from(["ER_FINALE".to_string()]))?;
        build("finale_g", naga::ShaderStage::Fragment, "finale_g_ps", BTreeSet::from(["ER_FINALE".to_string()]))?;
        build("finale_fade", naga::ShaderStage::Fragment, "finale_fade_ps", BTreeSet::from(["ER_FINALE".to_string()]))?;
        // (round 3) the silhouette breaking apart: its snapshot, its pieces (a vertex stage of their own)
        build("finale_snap", naga::ShaderStage::Fragment, "finale_snap_ps", BTreeSet::from(["ER_FINALE".to_string()]))?;
        build("finale_frag", naga::ShaderStage::Vertex, "finale_frag_vs", BTreeSet::from(["ER_FINALE".to_string()]))?;
        build("finale_frag", naga::ShaderStage::Fragment, "finale_frag_ps", BTreeSet::from(["ER_FINALE".to_string()]))?;
        // (round 5) the beam's colour over the picture, from the mask
        build("finale_comp", naga::ShaderStage::Fragment, "finale_comp_ps", BTreeSet::from(["ER_FINALE".to_string()]))?;
        // (ZFIN round 9) the comp at the game's Present in display colours, and its occlusion written into the mask before it
        build("finale_occl", naga::ShaderStage::Fragment, "finale_occl_ps", BTreeSet::from(["ER_FINALE".to_string()]))?;
        build("finale_disp", naga::ShaderStage::Fragment, "finale_disp_ps", BTreeSet::from(["ER_FINALE".to_string()]))?;
    }
    // (HY) The hybrid's capture of the game's light (er_char.wgsl ER_HYCAP): a NEW file, built with `--only hycap` only.
    if only == Some("hycap") {
        // (RSF2 round 5) with the frame block's hybrid rows (KH_HY3 follows the extra lights and the blood, as in the *_hy3 set)
        build("hycap", naga::ShaderStage::Fragment, "hycap_ps", ["ER_HYCAP", "KH_EXTRA_LIGHTS", "KH_BLOOD", "KH_HYBRID", "KH_HY3"].iter().map(|d| d.to_string()).collect())?;
        // (RSF2 round 11) the G-buffer's targets 3 and 4 cleared under the KH characters (er_char.wgsl hygb_ps): a NEW file
        build("hygb", naga::ShaderStage::Fragment, "hygb_ps", ["ER_HYCAP", "KH_EXTRA_LIGHTS", "KH_BLOOD", "KH_HYBRID", "KH_HY3"].iter().map(|d| d.to_string()).collect())?;
        // (RSF2 round 14) the KH characters' depth into the game's half-size effect depth (er_char.wgsl hdmerge_ps / hdfill_ps)
        build("hdmerge", naga::ShaderStage::Fragment, "hdmerge_ps", ["ER_HYCAP", "KH_EXTRA_LIGHTS", "KH_BLOOD", "KH_HYBRID", "KH_HY3"].iter().map(|d| d.to_string()).collect())?;
        build("hdfill", naga::ShaderStage::Fragment, "hdfill_ps", ["ER_HYCAP", "KH_EXTRA_LIGHTS", "KH_BLOOD", "KH_HYBRID", "KH_HY3"].iter().map(|d| d.to_string()).collect())?;
        // (RSF round 19) the KH surfaces' light probes (er_char.wgsl hcprobe_ps / hcprobedown_ps), read by hycap at t9
        build("hcprobe", naga::ShaderStage::Fragment, "hcprobe_ps", ["ER_HYCAP", "KH_EXTRA_LIGHTS", "KH_BLOOD", "KH_HYBRID", "KH_HY3"].iter().map(|d| d.to_string()).collect())?;
        build("hcprobedown", naga::ShaderStage::Fragment, "hcprobedown_ps", ["ER_HYCAP", "KH_EXTRA_LIGHTS", "KH_BLOOD", "KH_HYBRID", "KH_HY3"].iter().map(|d| d.to_string()).collect())?;
    }
    // (IMPL_ER80b) His mask for the game's light shafts at a reduced size (from the stencil of the full one): a NEW file.
    if only.is_none() || only == Some("maskdown") {
        build("maskdown", naga::ShaderStage::Fragment, "maskdown_ps", BTreeSet::from(["ER_MASKDOWN".to_string()]))?;
    }
    // (IMPL_ER64) The light probes + the ring of taps around his feet (the world's lights on him): a NEW file.
    if only.is_none() || only == Some("probe2") {
        build("probe2", naga::ShaderStage::Fragment, "probe_ps", BTreeSet::from(["ER_PROBE".to_string(), "ER_PROBE_RING".to_string()]))?;
    }
    // (IMPL_ER86) probe2 + the WORLD PROBE (taps on the ground in fixed world directions, kh-er-render stable.rs): a NEW file,
    // built with `--only probe3` only (a whole build writes the files it wrote before)
    // (RSF2) probe4: probe3 + taps on a KH character's own G-buffer (white albedo) left out (ER_PROBE_SKIP_KH): a NEW file,
    // built with `--only probe4` only
    if only == Some("probe4") {
        build("probe4", naga::ShaderStage::Fragment, "probe_ps", BTreeSet::from(["ER_PROBE".to_string(), "ER_PROBE_RING".to_string(), "ER_PROBE_WORLD".to_string(), "ER_PROBE_SKIP_KH".to_string()]))?;
    }
    if only == Some("probe3") {
        build("probe3", naga::ShaderStage::Fragment, "probe_ps", BTreeSet::from(["ER_PROBE".to_string(), "ER_PROBE_RING".to_string(), "ER_PROBE_WORLD".to_string()]))?;
    }
    // (IMPL_ER39) The light pass: the effects' particle lights and the hero light, additive, from the scene depth.
    if only.is_none() || only == Some("plight") {
        build("plight", naga::ShaderStage::Fragment, "plight_ps", BTreeSet::from(["ER_PLIGHT".to_string()]))?;
    }
    // (IMPL_ER44) The same pass + his body's shadow from the Keyblade light (capsules): a new file, the DLL takes it when there.
    if only.is_none() || only == Some("plight2") {
        build("plight2", naga::ShaderStage::Fragment, "plight_ps", BTreeSet::from(["ER_PLIGHT".to_string(), "ER_PLIGHT_CAPS".to_string()]))?;
    }
    }
    let mut variants: BTreeSet<(u16, bool, bool, bool)> = BTreeSet::new();
    for slot in &library.slots {
        variants.insert((slot.owner_id, slot.masked, slot.translucent, slot.subsurface));
    }
    let variants_pl = variants.clone();
    let variants_bl = variants.clone();
    let variants_hy = variants.clone();
    let variants_hy3 = variants.clone();
    let variants_bli = variants.clone();
    for (id, masked, translucent, subsurface) in variants {
        let name = format!("{}{own}", kh_look::library::variant_name(id, masked, translucent, subsurface));
        if only.is_some_and(|o| o != name) {
            continue;
        }
        let mut defines = BTreeSet::from(["ER_FRAGMENT".to_string(), format!("KH_C{id}")]);
        for (on, define) in [(masked, "KH_MASKED"), (translucent, "KH_TRANSLUCENT"), (subsurface, "KH_SUBSURFACE")] {
            if on {
                defines.insert(define.to_string());
            }
        }
        build(&name, naga::ShaderStage::Fragment, "fs_main", defines)?;
    }
    // (IMPL_ER44) THE SAME VARIANTS WITH SORA'S OWN POINT LIGHTS (the hero light and the fire lights on him):
    // `<variant>_pl.ps.dxil`, KH_EXTRA_LIGHTS (a longer frame block, read only by these). New names, so a DLL that
    // knows nothing of them keeps loading the files above unchanged. `--only pl` builds all of them.
    for (id, masked, translucent, subsurface) in variants_pl {
        let name = format!("{}{own}_pl", kh_look::library::variant_name(id, masked, translucent, subsurface));
        if only.is_some_and(|o| o != name && o != "pl") {
            continue;
        }
        let mut defines = BTreeSet::from(["ER_FRAGMENT".to_string(), "KH_EXTRA_LIGHTS".to_string(), format!("KH_C{id}")]);
        for (on, define) in [(masked, "KH_MASKED"), (translucent, "KH_TRANSLUCENT"), (subsurface, "KH_SUBSURFACE")] {
            if on {
                defines.insert(define.to_string());
            }
        }
        build(&name, naga::ShaderStage::Fragment, "fs_main", defines)?;
    }
    // (IMPL_ER60) THE *_pl SET + THE BLOOD LAYER (er_char.wgsl kh_blood; its block follows the lights' block, so these are
    // always built with KH_EXTRA_LIGHTS): `<variant>_bl.ps.dxil`, new names. With no blood they draw what *_pl draws.
    // `--only bl` builds all of them.
    for (id, masked, translucent, subsurface) in variants_bl {
        let name = format!("{}{own}_bl", kh_look::library::variant_name(id, masked, translucent, subsurface));
        if only.is_some_and(|o| o != name && o != "bl") {
            continue;
        }
        let mut defines = BTreeSet::from(["ER_FRAGMENT".to_string(), "KH_EXTRA_LIGHTS".to_string(), "KH_BLOOD".to_string(), format!("KH_C{id}")]);
        for (on, define) in [(masked, "KH_MASKED"), (translucent, "KH_TRANSLUCENT"), (subsurface, "KH_SUBSURFACE")] {
            if on {
                defines.insert(define.to_string());
            }
        }
        build(&name, naga::ShaderStage::Fragment, "fs_main", defines)?;
    }
    // (ER79) THE *_bl SET + THE CLOTH'S INSIDE (er_char.wgsl KH_INTERIOR: a back face of a slot with an interior factor is
    // shaded as the inside, darkened): `<variant>_bli.ps.dxil`, new names, only with `--only bli` (an outfit whose cloth is
    // drawn two-sided: twosided.json). Without an interior factor they draw exactly what *_bl draws.
    if only == Some("bli") {
        for (id, masked, translucent, subsurface) in variants_bli {
            let name = format!("{}{own}_bli", kh_look::library::variant_name(id, masked, translucent, subsurface));
            let mut defines = BTreeSet::from(["ER_FRAGMENT".to_string(), "KH_EXTRA_LIGHTS".to_string(), "KH_BLOOD".to_string(), "KH_INTERIOR".to_string(), format!("KH_C{id}")]);
            for (on, define) in [(masked, "KH_MASKED"), (translucent, "KH_TRANSLUCENT"), (subsurface, "KH_SUBSURFACE")] {
                if on {
                    defines.insert(define.to_string());
                }
            }
            build(&name, naga::ShaderStage::Fragment, "fs_main", defines)?;
        }
    }
    // (IMPL_ER78) THE HYBRID: the *_bl code + the game's light (`<variant>_hy`, entry fs_main) and the G-buffer writers
    // (`<variant>_gb`, entry gb_main), all with KH_EXTRA_LIGHTS + KH_BLOOD + KH_HYBRID (their frame block follows the two
    // others). New names (IMPL_ER78b: `_hy2` / `_gb2`, the G-buffer writer with three targets); `--only hy` builds both sets
    // (`--only hy_only` / `--only gb` one of them).
    for (id, masked, translucent, subsurface) in variants_hy {
        let base = format!("{}{own}", kh_look::library::variant_name(id, masked, translucent, subsurface));
        let mut defines = BTreeSet::from(["ER_FRAGMENT".to_string(), "KH_EXTRA_LIGHTS".to_string(), "KH_BLOOD".to_string(), "KH_HYBRID".to_string(), format!("KH_C{id}")]);
        for (on, define) in [(masked, "KH_MASKED"), (translucent, "KH_TRANSLUCENT"), (subsurface, "KH_SUBSURFACE")] {
            if on {
                defines.insert(define.to_string());
            }
        }
        let name = format!("{base}_hy2");
        if only.is_none_or(|o| o == name || o == "hy" || o == "hy_only") {
            build(&name, naga::ShaderStage::Fragment, "fs_main", defines.clone())?;
        }
        let name = format!("{base}_gb2");
        if only.is_none_or(|o| o == name || o == "hy" || o == "gb") {
            let mut defines = defines;
            defines.insert("KH_GBUFFER".to_string());
            build(&name, naga::ShaderStage::Fragment, "gb_main", defines)?;
        }
    }
    // (HY) THE HYBRID, THIRD VERSION (er_char.wgsl KH_HY3: the light read from the renderer's own capture of the game's pass,
    // validated per pixel, the undirected share left to KH3's indirect term, his own sun shadow, the debug views):
    // `<variant>[_S]_hy3` (fs_main) and `_gb3` (gb_main), NEW names, built only with `--only hy3`. The colour set is the *_bli
    // code (blood + the cloth's inside, which draws what *_bl draws where a slot has no interior factor) + the hybrid; in party
    // mode the *_bl code + the hybrid (the party's sets have no inside). With the hybrid off for a frame it draws what the set the
    // DLL picks today draws.
    if only == Some("hy3") {
        for (id, masked, translucent, subsurface) in variants_hy3 {
            let base = format!("{}{own}", kh_look::library::variant_name(id, masked, translucent, subsurface));
            let mut defines = BTreeSet::from(["ER_FRAGMENT".to_string(), "KH_EXTRA_LIGHTS".to_string(), "KH_BLOOD".to_string(), "KH_HYBRID".to_string(), "KH_HY3".to_string(), format!("KH_C{id}")]);
            if !party {
                defines.insert("KH_INTERIOR".to_string());
            }
            for (on, define) in [(masked, "KH_MASKED"), (translucent, "KH_TRANSLUCENT"), (subsurface, "KH_SUBSURFACE")] {
                if on {
                    defines.insert(define.to_string());
                }
            }
            build(&format!("{base}_hy3"), naga::ShaderStage::Fragment, "fs_main", defines.clone())?;
            let mut defines = defines;
            defines.insert("KH_GBUFFER".to_string());
            build(&format!("{base}_gb3"), naga::ShaderStage::Fragment, "gb_main", defines)?;
        }
    }
    // The textures as the game cooked them (block compressed, its own mips), beside the PNGs:
    // the Elden Ring renderer takes these when they are there (a quarter of the memory).
    if suffix.is_none() && (only.is_none() || only == Some("tex")) {
        let copied = copy_textures(&project, &library.dir, &out_dir)?;
        println!("kh-shaderc: {} block-compressed texture(s), {:.1} MB, in {}", copied.0, copied.1 as f64 / 1_048_576.0, out_dir.join("tex").display());
    }
    if only.is_none() || suffix.is_some() || only == Some("hy3") {
        let manifest = serde_json::json!({
            "format": "kh3-er-shaders/1",
            "generator": concat!("crates/kh-shaderc ", env!("CARGO_PKG_VERSION")),
            "source": { "materials": library.generator, "naga": "29 (wgsl-in -> hlsl-out, shader model 6.0)", "compiler": dxc.display().to_string() },
            "bindings": "b0 frame, b1 material (KhParams), b2 skin; t0..t13 material textures, t14 colour table, t15 environment cube, t16 scene depth; sampler heap s0 space0, sampler index buffer t0 space1 (0 wrap, 1 clamp, 2 cube); vertex inputs LOC0..LOC7 = position, normal, tangent, uv0, uv1, uv2, joints (uint4), weights",
            "shaders": written,
        });
        // (ER81) an own set: shaders_<suffix>.json (the main shaders.json is not touched)
        // (HY) `--only hy3`: shaders_hy3[_<suffix>].json, a NEW name (no existing manifest is rewritten)
        let index = match (suffix, only == Some("hy3")) {
            (Some(s), true) => format!("shaders_hy3_{s}.json"),
            (None, true) => "shaders_hy3.json".to_string(),
            (Some(s), false) => format!("shaders_{s}.json"),
            (None, false) => "shaders.json".to_string(),
        };
        std::fs::write(out_dir.join(&index), serde_json::to_string_pretty(&manifest).unwrap()).map_err(|e| e.to_string())?;
        println!("kh-shaderc: wrote {}", out_dir.join(&index).display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (IMPL_ER78) the hybrid's two entries translate for every variant of the main pack (naga WGSL -> HLSL; dxc is the
    /// build's), the colour one with the game's light at t21 / t22, the G-buffer one with three targets (IMPL_ER78b). Skipped without the pack.
    #[test]
    fn the_hybrid_variants_translate() {
        let pack = PathBuf::from(PROJECT).join("build").join("sora_pack");
        let Ok(library) = kh_look::library::Library::load(&pack) else { return };
        let Ok(generated) = std::fs::read_to_string(library.dir.join("materials.wgsl")) else { return };
        let spliced = HARNESS.replace("//KH_COMMON", kh_look::CHAR_COMMON_WGSL).replace("//KH_GENERATED", &generated);
        let mut seen = BTreeSet::new();
        for slot in &library.slots {
            if !seen.insert((slot.owner_id, slot.masked, slot.translucent, slot.subsurface)) {
                continue;
            }
            let mut defines = BTreeSet::from(["ER_FRAGMENT".to_string(), "KH_EXTRA_LIGHTS".to_string(), "KH_BLOOD".to_string(), "KH_HYBRID".to_string(), format!("KH_C{}", slot.owner_id)]);
            for (on, define) in [(slot.masked, "KH_MASKED"), (slot.translucent, "KH_TRANSLUCENT"), (slot.subsurface, "KH_SUBSURFACE")] {
                if on {
                    defines.insert(define.to_string());
                }
            }
            let (colour, _) = to_hlsl(&preprocess(&spliced, &defines).unwrap(), naga::ShaderStage::Fragment, "fs_main").unwrap();
            if !slot.translucent {
                assert!(colour.contains("register(t21)") && colour.contains("register(t22)"), "variant {}", slot.variant());
            }
            defines.insert("KH_GBUFFER".to_string());
            let (gbuffer, _) = to_hlsl(&preprocess(&spliced, &defines).unwrap(), naga::ShaderStage::Fragment, "gb_main").unwrap();
            assert!(gbuffer.contains("SV_Target2") && !gbuffer.contains("SV_Target3"), "variant {}", slot.variant());
        }
        assert!(!seen.is_empty());
    }

    /// (HY) the third hybrid's entries translate for every variant of the main pack (the colour one reads the capture at t21 /
    /// t22, the G-buffer one writes three targets, its hair core too), and the capture's own shader reads five game targets.
    /// Skipped without the pack.
    #[test]
    fn the_third_hybrid_variants_translate() {
        let pack = PathBuf::from(PROJECT).join("build").join("sora_pack");
        let Ok(library) = kh_look::library::Library::load(&pack) else { return };
        let Ok(generated) = std::fs::read_to_string(library.dir.join("materials.wgsl")) else { return };
        let spliced = HARNESS.replace("//KH_COMMON", kh_look::CHAR_COMMON_WGSL).replace("//KH_GENERATED", &generated);
        let mut seen = BTreeSet::new();
        for slot in &library.slots {
            if !seen.insert((slot.owner_id, slot.masked, slot.translucent, slot.subsurface)) {
                continue;
            }
            let mut defines = BTreeSet::from(["ER_FRAGMENT", "KH_EXTRA_LIGHTS", "KH_BLOOD", "KH_INTERIOR", "KH_HYBRID", "KH_HY3"].map(str::to_string));
            defines.insert(format!("KH_C{}", slot.owner_id));
            for (on, define) in [(slot.masked, "KH_MASKED"), (slot.translucent, "KH_TRANSLUCENT"), (slot.subsurface, "KH_SUBSURFACE")] {
                if on {
                    defines.insert(define.to_string());
                }
            }
            let (colour, _) = to_hlsl(&preprocess(&spliced, &defines).unwrap(), naga::ShaderStage::Fragment, "fs_main").unwrap_or_else(|e| panic!("{}: {e}", slot.variant()));
            if !slot.translucent {
                assert!(colour.contains("register(t21)") && colour.contains("register(t22)"), "variant {}", slot.variant());
            }
            defines.insert("KH_GBUFFER".to_string());
            let (gbuffer, _) = to_hlsl(&preprocess(&spliced, &defines).unwrap(), naga::ShaderStage::Fragment, "gb_main").unwrap();
            assert!(gbuffer.contains("SV_Target2") && !gbuffer.contains("SV_Target3"), "variant {}", slot.variant());
            if slot.masked && !slot.translucent {
                defines.insert("KH_HAIR".to_string());
                to_hlsl(&preprocess(&spliced, &defines).unwrap(), naga::ShaderStage::Fragment, "gb_main").unwrap_or_else(|e| panic!("{} hair gb: {e}", slot.variant()));
            }
        }
        let (capture, _) = to_hlsl(&preprocess(&spliced, &["ER_HYCAP", "KH_EXTRA_LIGHTS", "KH_BLOOD", "KH_HYBRID", "KH_HY3"].iter().map(|d| d.to_string()).collect()).unwrap(), naga::ShaderStage::Fragment, "hycap_ps").unwrap();
        let regs = |h: &str| h.match_indices("register(").map(|(i, _)| h[i..].split(')').next().unwrap_or("").to_string()).chain(h.match_indices("SV_Target").map(|(i, _)| h[i..i + 10].to_string())).collect::<Vec<_>>().join(" "); // (PORT30 housekeeping) the bindings in the message
        // (PORT30 housekeeping) since RSF round 19 the module also holds the probe passes (hcprobe_ps: 6 targets, one per
        // direction): hycap_ps's own two targets are checked in its output struct, not over the whole module
        let out_struct = |h: &str, name: &str| h.find(&format!("struct {name}")).map(|i| h[i..].split("};").next().unwrap_or("").to_string()).unwrap_or_default();
        let hycap_out = out_struct(&capture, "HyCapOut");
        assert!(capture.contains("register(t4)") && hycap_out.contains("SV_Target1") && !hycap_out.contains("SV_Target2"), "hycap_ps: {} | {hycap_out}", regs(&capture));
        // (RSF2 round 11) the G-buffer's targets 3 / 4 under the KH characters
        let (gbclear, _) = to_hlsl(&preprocess(&spliced, &["ER_HYCAP", "KH_EXTRA_LIGHTS", "KH_BLOOD", "KH_HYBRID", "KH_HY3"].iter().map(|d| d.to_string()).collect()).unwrap(), naga::ShaderStage::Fragment, "hygb_ps").unwrap();
        let hygb_out = out_struct(&gbclear, "HyGbOut"); // (PORT30 housekeeping) its own output struct, as hycap_ps's
        assert!(gbclear.contains("register(t4)") && hygb_out.contains("SV_Target1") && !hygb_out.contains("SV_Target2") && gbclear.contains("discard"), "hygb_ps: {hygb_out}");
    }

    /// (ER79) The outfit skin with the leg guard translates (KH_JOINTS_EXTRA + KH_CLOTH_GUARD), reads the palette's last
    /// entries, and the plain skin is untouched by it.
    #[test]
    fn the_leg_guard_skin_translates() {
        let pack = PathBuf::from(PROJECT).join("build").join("sora_pack").join("outfits").join("kh1");
        let Ok(library) = kh_look::library::Library::load(&pack) else { return };
        let Ok(generated) = std::fs::read_to_string(library.dir.join("materials.wgsl")) else { return };
        let spliced = HARNESS.replace("//KH_COMMON", kh_look::CHAR_COMMON_WGSL).replace("//KH_GENERATED", &generated);
        let mut defines = BTreeSet::from(["ER_VERTEX".to_string(), "KH_JOINTS_EXTRA".to_string()]);
        if generated.contains("kh_t14") {
            defines.insert("KH_TEX_EXTRA".to_string());
        }
        let (plain, _) = to_hlsl(&preprocess(&spliced, &defines).unwrap(), naga::ShaderStage::Vertex, "vs_main").unwrap();
        assert!(!plain.contains("kh_guard"));
        defines.insert("KH_CLOTH_GUARD".to_string());
        let (guarded, _) = to_hlsl(&preprocess(&spliced, &defines).unwrap(), naga::ShaderStage::Vertex, "vs_main").unwrap();
        assert!(guarded.contains("kh_guard") && guarded.contains("319"), "the guard reads the palette's last entry");
        // (ER79) the cloth's inside: the *_bli fragment set translates for every variant of the outfit
        for slot in &library.slots {
            let mut defines = BTreeSet::from(["ER_FRAGMENT".to_string(), "KH_EXTRA_LIGHTS".to_string(), "KH_BLOOD".to_string(), "KH_INTERIOR".to_string(), "KH_JOINTS_EXTRA".to_string(), format!("KH_C{}", slot.owner_id)]);
            if generated.contains("kh_t14") {
                defines.insert("KH_TEX_EXTRA".to_string());
            }
            for (on, define) in [(slot.masked, "KH_MASKED"), (slot.translucent, "KH_TRANSLUCENT"), (slot.subsurface, "KH_SUBSURFACE")] {
                if on {
                    defines.insert(define.to_string());
                }
            }
            to_hlsl(&preprocess(&spliced, &defines).unwrap(), naga::ShaderStage::Fragment, "fs_main").unwrap();
        }
    }

    /// (ER party S4) PARTY MODE: Donald's materials with the main pack's post-process material spliced in translate for
    /// every variant (the hybrid's colour and G-buffer entries), one vertex layout for the set (KH_PSP_OUT: location 9),
    /// and the owners that read the pre-skinned position (w_do070 / w_do130: KH_PSP) read it. Skipped without the packs.
    #[test]
    fn the_party_variants_translate() {
        let project = PathBuf::from(PROJECT);
        let pack = project.join("build").join("sora_pack").join("party").join("n_ex001");
        let (Ok(library), Ok(own), Ok(main)) = (kh_look::library::Library::load(&pack), std::fs::read_to_string(pack.join("materials").join("materials.wgsl")), std::fs::read_to_string(project.join("build").join("sora_pack").join("materials").join("materials.wgsl"))) else { return };
        assert!(!own.contains("fn kh_khs("), "the party's text has no post-process material of its own");
        let generated = format!("{own}
{}", &main[main.find("// /Game/Shader/KHS_set").unwrap()..]);
        let spliced = HARNESS.replace("//KH_COMMON", kh_look::CHAR_COMMON_WGSL).replace("//KH_GENERATED", &generated);
        let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(pack.join("materials").join("materials.json")).unwrap()).unwrap();
        let psp: BTreeSet<u16> = json["owners"].as_object().unwrap().values().filter(|o| o["pre_skinned_position"].is_object()).filter_map(|o| o["id"].as_u64().map(|i| i as u16)).collect();
        assert!(!psp.is_empty());
        let (vs, _) = to_hlsl(&preprocess(&spliced, &BTreeSet::from(["ER_VERTEX".to_string(), "KH_JOINTS_EXTRA".to_string(), "KH_PSP_OUT".to_string()])).unwrap(), naga::ShaderStage::Vertex, "vs_main").unwrap();
        assert!(vs.contains("320") && vs.contains("LOC9"), "320 joints, the pre-skinned position out");
        let mut seen = BTreeSet::new();
        for slot in &library.slots {
            if !seen.insert((slot.owner_id, slot.masked, slot.translucent, slot.subsurface)) {
                continue;
            }
            let mut defines = BTreeSet::from(["ER_FRAGMENT".to_string(), "KH_EXTRA_LIGHTS".to_string(), "KH_BLOOD".to_string(), "KH_HYBRID".to_string(), "KH_JOINTS_EXTRA".to_string(), "KH_PSP_OUT".to_string(), format!("KH_C{}", slot.owner_id)]);
            if psp.contains(&slot.owner_id) {
                defines.insert("KH_PSP".to_string());
            }
            for (on, define) in [(slot.masked, "KH_MASKED"), (slot.translucent, "KH_TRANSLUCENT"), (slot.subsurface, "KH_SUBSURFACE")] {
                if on {
                    defines.insert(define.to_string());
                }
            }
            to_hlsl(&preprocess(&spliced, &defines).unwrap(), naga::ShaderStage::Fragment, "fs_main").unwrap_or_else(|e| panic!("{}: {e}", slot.variant()));
            defines.insert("KH_GBUFFER".to_string());
            to_hlsl(&preprocess(&spliced, &defines).unwrap(), naga::ShaderStage::Fragment, "gb_main").unwrap_or_else(|e| panic!("{} gb: {e}", slot.variant()));
        }
        assert!(seen.len() >= 10, "Donald and his weapons: {} variants", seen.len());
    }

    #[test]
    fn ifdefs_select_and_keep_line_numbers() {
        let source = "a\n#ifdef X\nb\n#else\nc\n#endif\n#ifdef Y\n#ifdef X\nd\n#endif\ne\n#endif\nf";
        let with = |names: &[&str]| preprocess(source, &names.iter().map(|n| n.to_string()).collect()).unwrap();
        assert_eq!(with(&["X"]), "a\n\nb\n\n\n\n\n\n\n\n\n\nf\n");
        assert_eq!(with(&[]), "a\n\n\n\nc\n\n\n\n\n\n\n\nf\n");
        assert_eq!(with(&["X", "Y"]), "a\n\nb\n\n\n\n\n\nd\n\ne\n\nf\n");
        assert_eq!(with(&["Y"]), "a\n\n\n\nc\n\n\n\n\n\ne\n\nf\n");
        assert!(preprocess("#ifdef X\n", &BTreeSet::new()).is_err() && preprocess("#endif\n", &BTreeSet::new()).is_err());
    }
}
