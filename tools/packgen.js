#!/usr/bin/env node
// (PACKGEN) THE ASSET PACK BUILD FROM A PLAYER'S OWN KINGDOM HEARTS III INSTALL: one deterministic run, in order.
//
//   node tools/packgen.js [phase ...] [--from STEP] [--only STEP[,STEP]] [--list] [--keep-going] [--resume]
//     phases: extract    the pak key from the player's executable, then the KH3 files the build reads (setup/pack_inputs.json)
//             convert    those files decoded / converted (assets_converted/: packages, textures, meshes, animations, sounds)
//             research   the research data the pack tools read (audit/sora_port, sheets/reflect + entities + systems,
//                        build/sora_tmp/...), made from the player's files by the research scripts (tools/research) and
//                        our notes (tools/packgen/notes)
//             pack       the pack tools, in the order the pack was built (build/sora_pack)
//             replay     the pack files made by the development streams' own tools, replayed from their recipes
//                        (tools/packgen/replay, tools/packgen_replay.js)
//             shaders    the Elden Ring shaders (kh-shaderc: the characters', the outfits', the party's, the effects')
//             gen        the generated Rust tables (build/gen/rust)
//     default: all seven. --workers N: the converters' parallel workers (default: the CPU count - 1, at most 8).
//
// Inputs: the KH3 files extracted and converted by the setup (assets/, assets_converted/: tools/pak_extract.js, ...), the
// player's KH3 executable (tools/config.json; base/KH3.exe when present), ELDEN RING's install for its item icons (--er DIR
// or KH_ER_GAME). Nothing from a development tree. Writes only under this repository (build/, audit/, sheets/).
//
// Research scripts: in a release checkout they are in tools/research/<path>, staged to build/sora_tmp/<path> before they
// run (they read and write relative to their own folder). In the development tree they are already in build/sora_tmp.
'use strict';
const fs = require('fs');
const path = require('path');
const cp = require('child_process');

const ROOT = path.resolve(__dirname, '..');
// (Linux) this run and every tool it starts list folders in Windows' (NTFS) order, as setup/setup.js does too
// (tools/lib/ntfs_order.js): a tool that follows the order it finds files in then writes the same files on Linux
if (process.platform !== 'win32') {
  const order = path.join(__dirname, 'lib', 'ntfs_order.js');
  require(order);
  if (!(process.env.NODE_OPTIONS || '').includes('ntfs_order.js')) process.env.NODE_OPTIONS = [process.env.NODE_OPTIONS, `--require "${order.replace(/[\\"]/g, (c) => '\\' + c)}"`].filter(Boolean).join(' ');
}
const RESEARCH_SRC = path.join(ROOT, 'tools', 'research');
const NOTES = path.join(ROOT, 'tools', 'packgen', 'notes');
const T = 'build/sora_tmp';
const args = process.argv.slice(2);
const opt = (k) => { const i = args.indexOf(k); return i >= 0 ? args[i + 1] : null; };
const PHASES = ['extract', 'convert', 'research', 'pack', 'replay', 'shaders', 'gen'];
const WORKERS = Math.max(1, Math.min(8, Number(opt('--workers')) || ((require('os').availableParallelism ? require('os').availableParallelism() : require('os').cpus().length) - 1)));
const wanted = args.filter((a) => PHASES.includes(a));
const phases = wanted.length ? wanted : PHASES;
const only = opt('--only') ? new Set(opt('--only').split(',')) : null;
const from = opt('--from');
const keepGoing = args.includes('--keep-going');
const erGame = opt('--er') || process.env.KH_ER_GAME || null;

// ---------------------------------------------------------------- the steps
// step: { id, phase, run: [script, ...args] | fn, cwd (relative to ROOT; default ROOT), files: research files (relative to
//         build/sora_tmp) the step needs staged, out: { produced: copy-to } (relative to ROOT), env }
const R = (id, cwd, run, files, out) => ({ id, phase: 'research', cwd, run, files, out });
// (a research file staged under another path than the development tree's: "<staged path><=<development path>", both
// relative to build/sora_tmp; the streams' tools that write next to themselves run from build/sora_tmp/packgen/<stream>)
const XW = 'packgen/xw/tools/xw_form_weapons.js<=er/xw/tools/xw_form_weapons.js';
const XWB = 'packgen/xwb/tools/xwb_chandelier.js<=er/xwb/tools/xwb_chandelier.js';
const ER90 = 'packgen/er90/tools/xsi_weapon_er_slots.js<=er/er90/tools/xsi_weapon_er_slots.js';
const VFB = 'packgen/vfb/tools/mk_shards.js<=er/party/vfb/tools/mk_shards.js';
const XWD = 'packgen/xwd/tools/mk_shards_xwd.js<=er/xwd/tools/mk_shards_xwd.js';
const KBA = 'weapons/AUDIT-A/stage/sound/pawn_triggers.js'; // (AUDIT-A SOUND) staged where it was: it finds the repository 6 folders up
const VFXR_M = 'weapons/VFX-RING/tools/M/stage_mesh.js'; // (VFX-RING M) staged where it was: it finds tools/sora_vfx_pack.js 6 folders up
const STEPS = [
  // ---- extract: the pak key (found in the player's executable) and the KH3 files the build reads
  { id: 'pak-footers', phase: 'extract', run: ['tools/pak_footers.js'] },
  { id: 'pak-key', phase: 'extract', run: ['tools/pak_key.js'] },
  { id: 'exe-check', phase: 'extract', run: ['tools/packgen_exe_check.js'] },          // the executable is the build the research tools read
  { id: 'extract', phase: 'extract', run: ['tools/packgen_extract.js'] },
  // ---- convert: the cooked files to JSON / glTF / PNG / WAV (assets_converted/)
  // (SETUP-TOOLS) census and packages resume from chunk files keyed by their position in the file list: when the extracted files
  // changed (an update that extracts more KH3 folders) the old chunks would stand for other files, so they start afresh
  { id: 'census', phase: 'convert', run: () => freshWhenInputsChanged('census', ['tools/asset_census.js', '--workers', String(WORKERS)]) },
  { id: 'bases', phase: 'convert', run: ['tools/packgen_bases.js'] },                         // every blueprint's native base, from all the paks
  { id: 'packages', phase: 'convert', run: () => freshWhenInputsChanged('packages', ['tools/package_convert.js', '--workers', String(WORKERS)]) },
  { id: 'textures', phase: 'convert', run: ['tools/texture_convert.js', 'export', '--workers', String(WORKERS)] },
  { id: 'meshes', phase: 'convert', run: ['tools/mesh_convert.js', 'export', '--workers', String(Math.max(1, Math.min(4, WORKERS)))] },
  { id: 'anim-index', phase: 'convert', run: ['tools/anim_convert.js', 'index', '--workers', String(WORKERS)] },
  { id: 'anims', phase: 'convert', run: ['tools/anim_convert.js', 'export', '--workers', String(WORKERS), '--budget', '86400'] },
  { id: 'sounds', phase: 'convert', run: ['tools/sead_convert.js', 'export', '--workers', String(Math.max(1, Math.min(4, WORKERS)))] },
  { id: 'loose', phase: 'convert', run: ['tools/loose_files.js'] },
  // ---- research: the executable and the game's tables
  { id: 'reflection', phase: 'research', run: ['tools/reflection.js'] },                       // audit/reflection.json (exe)
  { id: 'enums', phase: 'research', run: ['tools/packgen_enums.js'] },                         // sheets/reflect/enum-values.json
  { id: 'sheets', phase: 'research', run: ['tools/sheets_build.js'] },                         // sheets/entities, systems (paks)
  { id: 'vfx-defaults', phase: 'research', run: ['tools/sora_vfx_pack.js', '--defaults'] },    // audit/sora_port/vfx_class_defaults.json (exe)
  { id: 'bootstrap', phase: 'research', run: ['tools/packgen_bootstrap.js'] },                 // Sora's skeleton for the scripts below
  // ---- research: Sora's animation graph, leg IK, KineDriver, Bonamik, physics asset (copied into the pack's data/)
  R('sora-animgraph', T, ['graphdump.js', '--quiet'], ['graphdump.js'], { [`${T}/sora_animgraph.json`]: 'audit/sora_port/animgraph.json' }),
  R('sora-legik', T, ['legik_json.js'], ['legik_json.js'], { [`${T}/sora_legik.json`]: 'audit/sora_port/legik.json' }),
  R('sora-physics', T, ['bnmsub_physx.js'], ['bnmsub_physx.js'], { [`${T}/sora_physics_asset.json`]: 'audit/sora_port/physics_asset.json' }),
  R('sora-bonamik-notifies', '.', [`${T}/bnmsub_notifies.js`], ['bnmsub_notifies.js']),
  R('sora-bonamik', T, ['bnm_extract.js'], ['bnm_extract.js', 'bnm_plain.js'], { [`${T}/sora_bonamik.json`]: 'audit/sora_port/bonamik.json' }),
  R('sora-bonamik2', `${T}/bnm`, ['dump2.js'], ['dump2.js', 'anim.js', 'bnm_eval.js', 'chain.js', 'coll.js', 'cone.js', 'core.js', 'hfps.js', 'kdi.js', 'm.js', 'reset.js', 'wind.js'].map((f) => 'bnm/' + f),
    { [`${T}/sora_bonamik2.json`]: 'audit/sora_port/bonamik2.json' }),
  R('sora-kinedriver', '.', [`${T}/kd/build_json.js`], ['kd/build_json.js', 'kd/extract_raw.js'], { [`${T}/sora_kinedriver.json`]: 'audit/sora_port/kinedriver.json' }),
  // ---- research: the sound triggers (notifies, footsteps, voices, weapon / cloth sets)
  R('audio-notifies', `${T}/audio/triggers`, ['scan_notifies.js'], ['audio/triggers/lib.js', 'audio/triggers/scan_notifies.js']),
  R('audio-footsteps', `${T}/audio/triggers`, ['scan_footsteps.js'], ['audio/triggers/scan_footsteps.js']),
  R('audio-triggers', `${T}/audio/triggers`, ['build_outputs.js'], ['audio/triggers/build_outputs.js'], {
    [`${T}/audio/triggers/notify_rows.json`]: 'audit/sora_port/audio/notify_rows.json', [`${T}/audio/triggers/footstep_rows.json`]: 'audit/sora_port/audio/footstep_rows.json',
    [`${T}/audio/triggers/voice_rows.json`]: 'audit/sora_port/audio/voice_rows.json', [`${T}/audio/triggers/weapon_cloth_rows.json`]: 'audit/sora_port/audio/weapon_cloth_rows.json' }),
  // ---- research: sounds played from code and blueprints (the asset scans; the code reading is in the notes)
  ...['gen_static_se', 'gen_voice', 'gen_magic_proj', 'gen_prize', 'gen_seattach', 'gen_cues'].map((s) => R('audio-' + s.replace(/_/g, '-'), `${T}/audio/code`, [s + '.js'],
    ['audio/code/lib.js', 'audio/code/sndrefs.js', `audio/code/${s}.js`])),
  R('audio-bpscan', `${T}/audio/code`, ['bpscan.js', 'bp_sound_files.txt'], ['audio/code/bpscan.js', 'audio/code/bp_sound_files.txt']),
  R('audio-code', `${T}/audio/code`, ['build_rows.js'], ['audio/code/build_rows.js', 'audio/code/fx.js', 'audio/code/rows_manual.js'], {
    [`${T}/audio/code/code_rows.json`]: 'audit/sora_port/audio/code_rows.json', [`${T}/audio/code/helpers.json`]: 'audit/sora_port/audio/code_helpers.json',
    [`${T}/audio/code/tables.json`]: 'audit/sora_port/audio/code_tables.json' }),
  // ---- research: the SEAD configuration (WindowsEngine.ini)
  R('audio-config', `${T}/audio/bank`, ['config_extract.js'], ['audio/bank/config_extract.js']),
  R('audio-config-events', `${T}/audio/plugin`, ['cfg_events.js'], ['audio/plugin/cfg_events.js'], { [`${T}/audio/plugin/sead_config_events.json`]: 'audit/sora_port/audio/config_events.json' }),
  // ---- research: the effect inventory (which effects Sora's clips, weapon and blueprints use)
  R('vfx-notifies', `${T}/vfx/fxinv`, ['mine_notifies.js'], ['vfx/fxinv/lib.js', 'vfx/fxinv/mine_notifies.js']),
  R('vfx-animsets', `${T}/vfx/fxinv`, ['animsets.js'], ['vfx/fxinv/animsets.js']),
  R('vfx-sysindex', `${T}/vfx/fxinv`, ['sysindex.js'], ['vfx/fxinv/sysindex.js']),
  R('vfx-effect-list', `${T}/vfx/fxinv`, ['build_rows.js'], ['vfx/fxinv/build_rows.js']),
  // ---- research: the party
  R('party-animsets', `${T}/party/tools`, ['animsets.js'], ['party/tools/animsets.js']),
  { id: 'party-inventory', phase: 'research', run: ['tools/packgen_party_inventory.js'] },
  // ---- the pack (the order of the development build: docs in setup/PACK_PIPELINE.md)
  { id: 'sora', phase: 'pack', run: ['tools/sora_pack.js'] },
  { id: 'weapon', phase: 'pack', run: ['tools/sora_weapon_pack.js'] },
  { id: 'sora-again', phase: 'pack', run: ['tools/sora_pack.js'] },                            // with the weapon's attack sockets
  { id: 'materials', phase: 'pack', run: ['tools/sora_material_pack.js'] },
  { id: 'outfits', phase: 'pack', run: ['tools/sora_outfit_pack.js', '--no-shaders'] },
  { id: 'outfit-anims', phase: 'pack', run: ['tools/sora_outfit_anims.js'] },
  { id: 'outfit-forms', phase: 'pack', run: ['tools/sora_outfit_forms.js'] },
  { id: 'outfit-kdi', phase: 'pack', run: ['tools/sora_outfit_kdi.js'] },
  // (ER81 / ER85) the outfits' own shader sets: their world's post-process material in place of the KH World's
  { id: 'outfit-own-sets', phase: 'pack', run: () => {
    const { sets } = JSON.parse(fs.readFileSync(path.join(ROOT, 'tools', 'packgen', 'recipes', 'own_sets.json'), 'utf8'));
    for (const s of sets) {
      const out = `build/packgen/own/${s.outfit}`;
      runNode(['tools/sora_material_pack.js', '--own-khs', '--asset', s.asset, '--suffix', s.suffix, '--world', s.world, ...(s.control ? ['--control', s.control] : []), '--out', out], ROOT, { KH_OUTFIT_PACK: `build/sora_pack/outfits/${s.outfit}` });
      for (const f of [`materials_${s.suffix}.wgsl`, 'own_shaders.json']) { const d = path.join(ROOT, 'build/sora_pack/outfits', s.outfit, 'materials', f); fs.rmSync(d, { force: true }); fs.copyFileSync(path.join(ROOT, out, f), d); }
    }
    console.log(`   ${sets.length} own set(s)`);
  } },
  { id: 'outfit-kd-mesh', phase: 'pack', run: ['tools/sora_outfit_pack.js', '--kd-mesh'] },        // model_kd.glb (the KineDriver bones)
  { id: 'outfit-anticlip', phase: 'pack', run: ['tools/sora_outfit_anticlip.js'] },
  // (ER79) the leg guard of the outfits whose cloth goes into the legs (`enabled`; the development deploy took those only)
  { id: 'outfit-legguard', phase: 'pack', run: () => { runNode(['tools/sora_outfit_legguard.js', '--out', 'build/packgen/legguard']); let n = 0; for (const o of fs.readdirSync(path.join(ROOT, 'build/packgen/legguard/outfits'))) { const f = path.join(ROOT, 'build/packgen/legguard/outfits', o, 'legguard.json'); if (!fs.existsSync(f) || !JSON.parse(fs.readFileSync(f, 'utf8')).enabled) continue; const d = path.join(ROOT, 'build/sora_pack/outfits', o, 'legguard.json'); if (!fs.existsSync(d)) { fs.copyFileSync(f, d); n++; } } console.log(`   ${n} leg guard(s)`); } },
  { id: 'ui', phase: 'pack', run: ['tools/sora_ui_pack.js'] },
  // (BUTTON-ICONS) KH3's keyboard / mouse glyphs (IconTextData_km + IconText_km per language), new files of the UI folder
  { id: 'ui-button-icons', phase: 'pack', run: () => { fs.rmSync(path.join(ROOT, 'build/packgen/button_icons'), { recursive: true, force: true }); runNode(['tools/button_icons_pack.js', '--out=build/packgen/button_icons/ui']); copyNew('build/packgen/button_icons/ui', 'build/sora_pack/ui'); } },
  // (INTERACT-MARK) the interaction marker's blur filters (HudTarget, sprite 84), new files of the UI folder
  { id: 'ui-filters', phase: 'pack', run: () => { fs.rmSync(path.join(ROOT, 'build/packgen/ui_filters'), { recursive: true, force: true }); runNode(['tools/sora_ui_filters.js', 'HudTarget', '01_hud/hud_target/HudTarget', 'build/packgen/ui_filters/ui', '84']); copyNew('build/packgen/ui_filters/ui', 'build/sora_pack/ui'); } },
  { id: 'saveload', phase: 'pack', run: ['tools/sora_saveload_pack.js'] },
  { id: 'er-icons', phase: 'pack', run: () => { if (!erGame) throw new Error('the Elden Ring install is needed for its item icons: --er "<ELDEN RING>\\Game" (or KH_ER_GAME)'); runNode(['tools/er_icons.js', '--game', erGame]); } },
  { id: 'vfx', phase: 'pack', run: ['tools/sora_vfx_pack.js'] },
  { id: 'vfx-extra-textures', phase: 'pack', run: ['tools/sora_vfx_extra_textures.js'] },
  { id: 'vfx-host', phase: 'pack', run: ['tools/sora_vfx_host.js'] },
  { id: 'vfx-shaders', phase: 'pack', run: ['tools/sora_vfx_shaders.js'] },
  { id: 'vfx-rows', phase: 'pack', run: ['tools/sora_vfx_assets_rows.js'] },
  { id: 'savepoint', phase: 'pack', run: ['tools/sora_savepoint_pack.js'] },
  { id: 'party', phase: 'pack', run: ['tools/party_pack.js'] },
  { id: 'party-lookat', phase: 'pack', run: ['tools/party_lookat_pack.js'] },
  { id: 'party-menu-lights', phase: 'pack', run: ['tools/party_menu_lights.js'] },
  { id: 'party-audio-rows', phase: 'pack', run: ['tools/party_audio_pack.js'] },              // audit/sora_port/audio/party_rows.json
  { id: 'party-ai', phase: 'pack', run: ['tools/party_ai_pack.js'] },
  { id: 'party-team', phase: 'pack', run: ['tools/party_team_pack.js'] },
  // (SETUP-TOOLS) the XS abilities' staged pack (build/sora_tmp/xsi/pack) BEFORE the keyblades: their form clips leave out the
  // clips the XS overlay has (sora_keyblades_pack.js reads xsi_manifest.json); it reads only the main manifest
  { id: 'xsi', phase: 'pack', run: ['tools/xsi_pack.js'] },
  // (KEYBLADES) every keyblade (base + Re Mind), its formchanges' weapons, Sora's form clips, the keyblades' effects (their own
  // root keyblades/vfx), their sound rows (audit/sora_port/audio/keyblade_rows.json, read by the audio step below), the Moogle shop
  // NPCs; then every KH3 wear form's colours on the KH1 / KH2 outfits (after outfit-forms: it merges into their wear_extra.json)
  { id: 'keyblades', phase: 'pack', run: ['tools/sora_keyblades_pack.js'] },
  // (KBFX) every keyblade model's keychain (bonamik.json) and its skeleton's sockets (sockets.json), beside the model (P5)
  { id: 'keyblades-chains', phase: 'pack', run: ['tools/sora_weapon_pack.js', '--keyblades', 'build/sora_pack/keyblades'] },
  // (P8 / MOOGLE-HOVER) the synthesis Moogles in the friends' layout, from the keyblades step's npc folders: n_ex028 (the field
  // Moogle: the DLL merges its voices, PACK/party/n_ex028/audio) and n_ex014 (the Twilight Town Moogle the DLL draws)
  { id: 'moogle', phase: 'pack', run: () => { runNode(['tools/moogle_pack.js']); runNode(['tools/moogle_pack.js', '--npc', 'n_ex014']); } },
  { id: 'outfit-kb-forms', phase: 'pack', run: ['tools/sora_outfit_formcolours.js'] },
  { id: 'audio', phase: 'pack', run: ['tools/sora_audio_pack.js'] },
  { id: 'audio-tables', phase: 'pack', run: ['tools/sora_audio_tables.js'] },
  { id: 'audio-party', phase: 'pack', run: ['tools/sora_audio_pack.js', '--party-extra'], env: { KH_AUDIO_EXTRA_SOUNDS: 'tools/packgen/recipes/party_extra_sounds.json' } },
  { id: 'xsi-weapons', phase: 'pack', run: ['tools/xsi_weapons.js'] },
  { id: 'audio-xsi', phase: 'pack', run: ['tools/sora_audio_pack.js', '--xsi-extra'] },
  // (TITLE-AUDIO) KH3's title music (the title HUD's m_TitleBGM, BGM_title.mab of the `sounds` step) for Elden Ring's title
  // screen: its own audio root PACK/title/audio (kh-er fx.rs merges it; titlemusic.rs plays it on the title only)
  { id: 'title-audio', phase: 'pack', run: ['tools/title_audio_pack.js', '--out', 'build/sora_pack/title/audio'] },
  // (MIMIC-MUSIC) KH3's replica Xehanorts battle theme ("Replicas": BGM_xeha_bat__boss_1.mab of the `sounds` step) for Elden
  // Ring's two Mimic Tear boss fights: its own audio root PACK/mimic/audio (kh-er fx.rs merges it; mimic_music.rs plays it)
  { id: 'mimic-music', phase: 'pack', run: ['tools/mimic_music_pack.js', '--out', 'build/sora_pack/mimic/audio'] },
  // (FINAL-FIX + HM-MAGIC) the shotlocks' and the hm magic set's actor sounds: their own audio root PACK/shotlocks/audio
  { id: 'shotlocks-audio', phase: 'pack', run: ['tools/sora_shotlock_audio_root.js'] },
  // (AUDIT-A SOUND) the keyblade forms' audio root PACK/keyblades/audio: the effect-carried SE whose particle system names an
  // SeAttach package of another name, the forms' projectile SE and the pawn clips' voice notifies (our lists:
  // tools/packgen/recipes/keyblades_audio_{effects,sounds}.json; the notifies from the keyblades' model.json, AUDIT-A's script)
  { id: 'keyblades-audio', phase: 'pack', files: [KBA], run: () => {
    const w = path.join(ROOT, 'build/packgen/kb_audio'); fs.rmSync(w, { recursive: true, force: true }); fs.mkdirSync(w, { recursive: true });
    runNode([`${T}/${KBA}`]);
    runNode(['tools/sora_audio_pack.js', '--party-extra', '--out', 'build/packgen/kb_audio/audio'], ROOT, { KH_AUDIO_EXTRA_EFFECTS: 'tools/packgen/recipes/keyblades_audio_effects.json', KH_AUDIO_EXTRA_SOUNDS: 'tools/packgen/recipes/keyblades_audio_sounds.json', KH_AUDIO_EXTRA_TRIGGERS: `${T}/weapons/AUDIT-A/stage/sound/pawn_triggers.json` });
    fs.rmSync(path.join(ROOT, 'build/sora_pack/keyblades/audio'), { recursive: true, force: true });
    copyOver('build/packgen/kb_audio/audio', 'build/sora_pack/keyblades/audio');
  } },
  { id: 'xsi-deploy', phase: 'pack', run: () => copyNew('build/sora_tmp/xsi/pack', 'build/sora_pack') },
  // (XW / XW-b / ER90) the XS moves' form weapons as KH3 equips them (their skeletons, clips and materials), Chandelier's
  // pawn w_so088, and the form weapons' Elden Ring maps: the streams' own tools, staged under build/sora_tmp/packgen (their
  // outputs: build/sora_tmp/packgen/<stream>/pack), deployed as new files only, as the streams deployed them
  { id: 'xsi-form-weapons', phase: 'pack', files: [XW, XWB, ER90], run: () => {
    for (const s of ['xw', 'xwb', 'er90']) fs.rmSync(path.join(ROOT, T, 'packgen', s, 'pack'), { recursive: true, force: true });
    runNode([`${T}/packgen/xw/tools/xw_form_weapons.js`]);
    runNode([`${T}/packgen/xwb/tools/xwb_chandelier.js`]);
    runNode([`${T}/packgen/er90/tools/xsi_weapon_er_slots.js`, `${T}/packgen/er90/pack`]);
    for (const s of ['xw', 'xwb', 'er90']) copyNew(`${T}/packgen/${s}/pack`, 'build/sora_pack');
  } },
  { id: 'lip', phase: 'pack', run: ['tools/face_lip_pack.js', '--pack', 'build/sora_pack', '--out', 'build/sora_pack'] },
  // (AUDIT-C AN) Rage Form's three face clips (ANTI_00..02 = 9999EM / 9998EM / 9997EM) and face/face_extra.json, new files
  { id: 'face-rage', phase: 'pack', run: () => { fs.rmSync(path.join(ROOT, 'build/packgen/rage_face'), { recursive: true, force: true }); runNode(['tools/audit_c/rage_face_clips.js', 'build/packgen/rage_face']); copyNew('build/packgen/rage_face/face', 'build/sora_pack/face'); } },
  // (AUDIT-C ANLOOK) KH1's face / skin / hair with KH3's anti-mode owners in Rage Form (their shaders: the shader phase's `anti`)
  { id: 'outfit-anti', phase: 'pack', run: ['tools/outfit_anti_owners.js', 'kh1'] },
  // ---- the streams' pack files (recipes)
  // (SETUP-TOOLS) the AUX files (streams' manifests outside the pack: tools/packgen/replay/files_aux.json) are the replay's own
  // outputs: removed first, so that a build run again (or the clean room a release compiles its recipes in) starts as a fresh
  // one does; a leftover one became part of the recipe compiler's corpus once, and a recipe referenced its own file
  { id: 'replay-scratch', phase: 'replay', run: () => {
    const aux = path.join(ROOT, 'tools', 'packgen', 'replay', 'files_aux.json');
    if (fs.existsSync(aux)) for (const rel of Object.keys(JSON.parse(fs.readFileSync(aux, 'utf8')).files || {})) fs.rmSync(path.join(ROOT, rel), { force: true });
    runNode(['tools/packgen_replay.js', 'prepare']);
  } },
  // (ZETTA) the streams' own tools whose outputs the recipes copy: Zettaflare's sounds (the event's SE stream cut at the
  // matinee's times, Donald's event lines) and Donald's clips from the event kg702b / kg702c on his skeleton
  { id: 'zetta-audio', phase: 'replay', files: ['party/ZETTA/data/audio_extra.json', 'party/ZETTA/tools/zetta_audio.js'], run: () => {
    runNode(['tools/sora_audio_pack.js', '--party-extra', '--out', `${T}/party/ZETTA/data/audio_full`], ROOT, { KH_AUDIO_EXTRA_SOUNDS: `${T}/party/ZETTA/data/audio_extra.json` });
    runNode([`${T}/party/ZETTA/tools/zetta_audio.js`]);
  } },
  { id: 'zetta-clips', phase: 'replay', files: ['party/ZETTA/tools/zetta_clips.js'], run: () => {
    for (const d of ['anims', 'face']) fs.mkdirSync(path.join(ROOT, T, 'party/ZETTA/pack/party/n_ex001', d), { recursive: true }); // (the tool writes into them)
    runNode([`${T}/party/ZETTA/tools/zetta_clips.js`]);
  } },
  { id: 'replay-files', phase: 'replay', run: ['tools/packgen_replay.js', 'apply'] },
  // (SETUP-TOOLS) the keyblades' effect root and manifest AGAIN, now that the replay has laid the party's and the XS abilities'
  // effects: the keyblades pack leaves out every effect another root has (sora_keyblades_pack.js effectRoots) and names its
  // materials against the main root's, as it did in development (the pack step's first pass ran before the replay). Then the
  // root's cube maps and attach effects (VFX-2, VFX-XLATE).
  { id: 'keyblades-effects', phase: 'replay', run: () => { fs.rmSync(path.join(ROOT, 'build/sora_pack/keyblades/vfx'), { recursive: true, force: true }); runNode(['tools/sora_keyblades_pack.js', '--only', 'vfx,manifest']); } },
  // (VFX-2) the keyblades' effect materials' cube maps (Dead of Night's shards), then the root's own table made again so that it
  // reads them (tools/sora_vfx_shaders.js --root: the cube slots come from textures/cubes.json)
  { id: 'keyblades-cubes', phase: 'replay', run: () => { runNode(['tools/vfx_cube_pack.js', '--pack', 'build/sora_pack/keyblades/vfx']); runNode(['tools/sora_vfx_shaders.js', '--root', path.join(ROOT, 'build/sora_pack/keyblades/vfx'), '--xw-break']); } },
  // (VFX-XLATE) the keyblades' attach data assets as effects of their root
  { id: 'keyblades-attach', phase: 'replay', run: ['tools/vfx_attach_effects.js'] },
  // (VFX-RING M) the TailModelMarch meshes the keyblades' root lacks (kmd_cin00: Shooting Star's shot tail), converted as the
  // effect pack converts meshes (VFX-RING's own script; new files only)
  { id: 'keyblades-meshes', phase: 'replay', files: [VFXR_M], run: () => { fs.rmSync(path.join(ROOT, 'build/packgen/vfxring_mesh'), { recursive: true, force: true }); runNode([`${T}/${VFXR_M}`, path.join(ROOT, 'build/packgen/vfxring_mesh')]); copyNew('build/packgen/vfxring_mesh', 'build/sora_pack/keyblades/vfx/meshes'); } },
  // (VF-b / XW-d) Chandelier's shatter fragments for the character pipeline: the XS effect pack's fragment meshes, each with
  // its material slot renamed (the streams' mk_shards tools over the XS root as the recipes left it); their records and
  // materials are recipe files, the models (.glb) are written here
  { id: 'xsi-shards', phase: 'replay', files: [VFB, XWD], run: () => {
    const xsi = path.join(ROOT, 'build/sora_pack/xsi'), out = path.join(ROOT, 'build/packgen/shards');
    fs.mkdirSync(path.join(xsi, 'weapons/materials_xwb/er'), { recursive: true }); // (its shaders come with the shader phase)
    fs.rmSync(out, { recursive: true, force: true });
    runNode([`${T}/packgen/vfb/tools/mk_shards.js`, xsi, out]);
    runNode([`${T}/packgen/xwd/tools/mk_shards_xwd.js`, xsi, xsi, out]);
    let n = 0;
    for (const d of ['shards_vfb', 'shards_xwd']) for (const f of walkFiles(path.join(out, 'xsi/weapons', d))) {
      const t = path.join(xsi, 'weapons', d, f); if (fs.existsSync(t)) continue;
      fs.mkdirSync(path.dirname(t), { recursive: true }); fs.copyFileSync(path.join(out, 'xsi/weapons', d, f), t); n++;
    }
    console.log(`   ${n} fragment model(s) -> build/sora_pack/xsi/weapons`);
  } },
  // the effect tables of the three roots with the pack's owner ids (the built shaders and the overlays are keyed by them),
  // the party and XS ones with the streams' texture records, as the pack's tables were made (--no-twp-all: the world-position
  // rule of RSF round 21 is in the overlays, shading_xwd.json); then ZA's emitter-driven decal rows on every table (last)
  { id: 'vfx-tables', phase: 'replay', files: ['party/ZA/tools/decal_dyn.js'], run: () => {
    const extra = ['DON-KIT', 'DON-MAGIC', 'ZETTA', 'DON-SPELLB', 'AUD', 'HYB', 'YFX'].map((s) => path.join(ROOT, T, 'party', s, 'pack', 'vfx', 'manifest.json')).filter((f) => fs.existsSync(f)).map((f) => f.split(path.sep).join('/')).join(';');
    const keep = (n) => path.join(ROOT, 'tools', 'packgen', 'recipes', `vfx_owners_${n}.json`);
    runNode(['tools/sora_vfx_shaders.js', '--no-twp-all'], ROOT, { KH_KEEP_OWNER_IDS: keep('main') });
    runNode(['tools/sora_vfx_shaders.js', '--party', '--no-twp-all'], ROOT, { KH_KEEP_OWNER_IDS: keep('party'), KH_PARTY_EXTRA_MANIFEST: extra });
    runNode(['tools/sora_vfx_shaders.js', '--xsi', '--no-twp-all'], ROOT, { KH_KEEP_OWNER_IDS: keep('xsi'), KH_PARTY_EXTRA_MANIFEST: extra });
    for (const d of ['build/sora_pack/vfx/shaders', 'build/sora_pack/party/vfx/shaders', 'build/sora_pack/xsi/vfx/shaders']) {
      if (!fs.existsSync(path.join(ROOT, d, 'shading.json'))) continue;
      runNode([`${T}/party/ZA/tools/decal_dyn.js`, d, d]);
    }
  } },
  // the streams' overlays of those tables: their translator runs (tools/packgen_overlays.js), then the overlay files as
  // recipes over the runs (their shaders: the shader phase, tools/packgen_shaders.js vfx-overlays)
  { id: 'vfx-overlays', phase: 'replay', files: ['party/ZA/tools/decal_dyn.js'], run: ['tools/packgen_overlays.js'] },
  { id: 'replay-overlays', phase: 'replay', run: ['tools/packgen_replay.js', 'apply', '--overlays'] },
  // (SP2) the Elden Ring title picture of the load screens, cut from the player's own Elden Ring menu textures (the files
  // tools/er_icons.js read: build/sora_tmp/er/er43/game)
  { id: 'er-title', phase: 'replay', files: ['party/SP2/tools/logo_cut.js'], run: () => { runNode([`${T}/party/SP2/tools/logo_cut.js`, 'build/packgen/sp2']); const d = path.join(ROOT, 'build/sora_pack/saveload/textures'); fs.mkdirSync(d, { recursive: true }); fs.copyFileSync(path.join(ROOT, 'build/packgen/sp2/MenuWorldTitleER.png'), path.join(d, 'MenuWorldTitleER.png')); } },
  // (VFXR) the spawn points two Square Enix modules read from textures / vertex data, baked into the effects that use them
  { id: 'replay-bake', phase: 'replay', files: ['vfxr/tools/vfxr_bake.js'], run: () => { fs.rmSync(path.join(ROOT, 'build/packgen/vfxr_bake'), { recursive: true, force: true }); runNode([`${T}/vfxr/tools/vfxr_bake.js`, 'build/sora_pack', 'build/packgen/vfxr_bake']); copyOver('build/packgen/vfxr_bake', 'build/sora_pack'); } },
  // (SETUP-TOOLS) what the development streams changed in the keyblades' effect root after its pack (AUDIT-B's crack effects,
  // AUDIT-A's shield effects, the root's table and manifest): the late recipes (tools/packgen/replay/late_*.json), AFTER the bake
  // (a file the bake rewrites and a stream changed too would else be baked over the stream's version; the recipes are compiled
  // against the pack as the bake left it)
  { id: 'replay-late', phase: 'replay', run: ['tools/packgen_replay.js', 'apply', '--late'] },
  { id: 'replay-verify', phase: 'replay', run: ['tools/packgen_replay.js', 'verify'] },
  // ---- the Elden Ring shaders
  { id: 'shaders', phase: 'shaders', files: ['er/er87/tools/patch_udiv.js', 'er/er87/tools/hair_fade.js', 'er/er91/tools/hair91.js'], run: ['tools/packgen_shaders.js'] },
  // ---- the generated Rust tables
  { id: 'party-sheet', phase: 'gen', run: ['tools/party_sheet.js'] },
  { id: 'gen-port', phase: 'gen', run: ['tools/gen_port.js'] },
  { id: 'gen-party', phase: 'gen', run: ['tools/gen_party.js'] },
  // the tables made from KH3 data as the pack's data files (the released DLL reads them at start: kh-gendata), and the
  // run-time version of the generated Rust (build/gen/rust_runtime, what a release build compiles)
  { id: 'gen-runtime', phase: 'gen', run: ['tools/packgen_genruntime.js'] },
];

// ---------------------------------------------------------------- helpers
const rel = (p) => path.relative(ROOT, p).split(path.sep).join('/');
function runNode(argv, cwd = ROOT, env = {}) {
  const fullEnv = { ...process.env, KH_PACKGEN_ROOT: ROOT.split(path.sep).join('/') + '/', MSYS_NO_PATHCONV: '1' };
  for (const [k, v] of Object.entries(env)) fullEnv[k] = /^(tools|build|sheets|audit)\//.test(v) ? path.join(ROOT, v) : v;
  const r = cp.spawnSync(process.execPath, argv, { cwd, env: fullEnv, stdio: ['ignore', 'pipe', 'pipe'], encoding: 'utf8', maxBuffer: 1 << 28 });
  const out = `${r.stdout || ''}${r.stderr || ''}`;
  fs.mkdirSync(path.join(ROOT, 'build', 'packgen', 'logs'), { recursive: true });
  fs.appendFileSync(path.join(ROOT, 'build', 'packgen', 'logs', 'packgen.log'), `\n==== ${new Date().toISOString()} ${rel(cwd) || '.'}> node ${argv.join(' ')}\n${out}`);
  if (r.status !== 0) throw new Error(`node ${argv.join(' ')} failed (exit ${r.status}):\n${out.split('\n').slice(-12).join('\n')}`);
  return out;
}
function walkFiles(dir, base = dir, out = []) {
  if (!fs.existsSync(dir)) return out;
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) { const p = path.join(dir, e.name); if (e.isDirectory()) walkFiles(p, base, out); else out.push(path.relative(base, p)); }
  return out;
}
// (SETUP-TOOLS) the extracted files (assets/: path and size of each) as one hash; a converter whose chunks were made for another
// list runs with --fresh. The print is written after the step succeeded (build/packgen/convert_inputs_<step>.json).
function assetsPrint() {
  const h = require('crypto').createHash('sha256');
  const files = walkFiles(path.join(ROOT, 'assets')).map((f) => f.split(path.sep).join('/')).sort();
  for (const f of files) h.update(`${f}|${fs.statSync(path.join(ROOT, 'assets', f)).size};`);
  return { files: files.length, sha256: h.digest('hex') };
}
function freshWhenInputsChanged(step, argv) {
  const file = path.join(ROOT, 'build', 'packgen', `convert_inputs_${step}.json`);
  const now = assetsPrint();
  let before = null; try { before = JSON.parse(fs.readFileSync(file, 'utf8')); } catch { /* no print: a first run, or a build from before the prints */ }
  const fresh = !before || before.sha256 !== now.sha256;
  if (fresh) console.log(`   ${step}: the extracted files are new or changed (${now.files}): from the start (--fresh)`);
  runNode(fresh ? [...argv, '--fresh'] : argv);
  fs.mkdirSync(path.dirname(file), { recursive: true }); fs.writeFileSync(file, JSON.stringify(now, null, 1) + '\n');
}
// copy every file of SRC that DST does not have (the development deploys: new files only)
function copyNew(src, dst) {
  let n = 0;
  for (const f of walkFiles(path.join(ROOT, src))) {
    const d = path.join(ROOT, dst, f);
    if (fs.existsSync(d)) continue;
    fs.mkdirSync(path.dirname(d), { recursive: true }); fs.copyFileSync(path.join(ROOT, src, f), d); n++;
  }
  console.log(`   ${n} new file(s) ${src} -> ${dst}`);
}
// copy every file of SRC over DST (remove-then-copy: a pack file may be a hard link of another tree)
function copyOver(src, dst) {
  let n = 0;
  for (const f of walkFiles(path.join(ROOT, src))) { const d = path.join(ROOT, dst, f); fs.mkdirSync(path.dirname(d), { recursive: true }); fs.rmSync(d, { force: true }); fs.copyFileSync(path.join(ROOT, src, f), d); n++; }
  console.log(`   ${n} file(s) ${src} -> ${dst}`);
}
// research scripts and notes: staged into place (a release checkout keeps them in tools/research, tools/packgen/notes)
function stageResearch(files) {
  for (const entry of files || []) {
    const f = entry.split('<=')[0];
    const src = path.join(RESEARCH_SRC, f), dst = path.join(ROOT, T, f);
    if (!fs.existsSync(src)) { if (fs.existsSync(dst)) continue; throw new Error(`research file missing: tools/research/${f}`); }
    if (fs.existsSync(dst) && fs.readFileSync(dst).equals(fs.readFileSync(src))) continue;
    fs.mkdirSync(path.dirname(dst), { recursive: true }); fs.copyFileSync(src, dst);
  }
}
function stageNotes() {
  let n = 0;
  for (const f of walkFiles(NOTES)) {
    if (path.basename(f).startsWith('.')) continue;
    const dst = path.join(ROOT, f);
    if (fs.existsSync(dst) && fs.readFileSync(dst).equals(fs.readFileSync(path.join(NOTES, f)))) continue;
    fs.mkdirSync(path.dirname(dst), { recursive: true }); fs.copyFileSync(path.join(NOTES, f), dst); n++;
  }
  if (n) console.log(`   ${n} note(s) staged from tools/packgen/notes`);
}

// --research-files: the research files (relative to build/sora_tmp) a release copies into tools/research (release-tools/export.js);
// "A<=B": staged as A, the development tree's B
if (args.includes('--research-files')) { console.log(JSON.stringify([...new Set(STEPS.flatMap((s) => s.files || []))].sort(), null, 1)); process.exit(0); }
if (args.includes('--list')) { for (const s of STEPS) console.log(`${s.phase.padEnd(9)} ${s.id}`); process.exit(0); }
let started = !from;
const t0 = Date.now();
// the steps finished so far (build/packgen/done_steps.json): --resume skips them (a build that stopped goes on where it
// stopped; the setup passes --resume after a failed build, and deletes the list with --rebuild-pack)
const DONE_FILE = path.join(ROOT, 'build', 'packgen', 'done_steps.json');
const done = new Set((() => { try { return JSON.parse(fs.readFileSync(DONE_FILE, 'utf8')).steps; } catch { return []; } })());
const saveDone = () => { fs.mkdirSync(path.dirname(DONE_FILE), { recursive: true }); fs.writeFileSync(DONE_FILE, JSON.stringify({ steps: [...done] }, null, 1) + '\n'); };
const resume = args.includes('--resume');
let failed = 0;
for (const s of STEPS) {
  if (!phases.includes(s.phase)) continue;
  if (!started) { if (s.id !== from) continue; started = true; }
  if (only && !only.has(s.id)) continue;
  if (resume && done.has(s.id)) { console.log(`packgen ${s.phase} ${s.id} ... done before (--resume)`); continue; }
  done.delete(s.id); saveDone();
  const t = Date.now();
  process.stdout.write(`packgen ${s.phase} ${s.id} ... `);
  try {
    if (s.phase === 'research' && s.id === 'reflection') stageNotes();
    stageResearch(s.files);
    if (typeof s.run === 'function') s.run();
    else runNode(s.run, path.join(ROOT, s.cwd || '.'), s.env || {});
    for (const [a, b] of Object.entries(s.out || {})) { fs.mkdirSync(path.dirname(path.join(ROOT, b)), { recursive: true }); fs.copyFileSync(path.join(ROOT, a), path.join(ROOT, b)); }
    done.add(s.id); saveDone();
    console.log(`ok (${((Date.now() - t) / 1000).toFixed(1)} s)`);
  } catch (e) {
    failed++;
    console.log('FAILED');
    console.error(String(e.message || e));
    if (!keepGoing) process.exit(1);
  }
}
console.log(`packgen: ${failed ? failed + ' step(s) failed' : 'done'} in ${((Date.now() - t0) / 60000).toFixed(1)} min (log: build/packgen/logs/packgen.log)`);
process.exit(failed ? 1 : 0);
