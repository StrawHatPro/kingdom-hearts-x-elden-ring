// The registry of tools/gen_port.js (sheets -> Rust for the Sora port). THIS is the file an
// implementer edits to get a new sheet into the Rust build; the generator's header
// (tools/gen_port.js) is the reference for every key. Three lists:
//
// params  parameter sheets (columns id, value, unit, source, evidence, ref_addr, ref_conf, note;
//         one row = one constant). Keyed by sheet name (the file is sheets/port/<name>.json).
//           file           generated file under build/gen/rust/
//           struct, prefix the Rust struct and the prefix of its constants (<PREFIX>_PARAMS ...)
//           from, doc      banner text and the struct's doc comment
//           informational  { 'row.id': 'what the code uses instead' }: rows that record a
//                          question, not a code input; left out of the struct whether checked or
//                          not. Any OTHER row with an unchecked (null) value fails the generator.
//         A parameter sheet under sheets/port/ that is NOT listed here is still generated, with
//         default names (sora-combat-params.json -> sora_combat_params.rs, SoraCombatParams,
//         SORA_COMBAT_PARAMS), but only while every one of its rows has a checked, typable value:
//         otherwise the tool prints NOT GENERATED for it and goes on (so an unfinished research
//         sheet never breaks the build). Register a sheet here, with its informational list, as
//         soon as code depends on it: from then on a null is an error.
//
// tables  table-shaped sheets (one row = one attack / spell / ability / sound ...), generated
//         ONLY when listed here, from a small schema. Keyed by sheet name.
//           file, row, table   generated file, the row struct's name, the static's name
//           doc                doc comment of the row struct
//           key                (optional) a column whose values name the rows: one
//                              `pub const <key_prefix>_<VALUE>: usize` per row and, for a string
//                              key, `pub fn <table>_by_<key>(&str) -> Option<&'static Row>`
//           key_prefix         prefix of those constants (default: the table's name)
//           columns            { 'sheet column': { type, optional, field, doc } }; columns not
//                              listed are not generated. type: 'str', 'f32', 'i32', 'u32', 'bool',
//                              'f32[]', 'i32[]', 'str[]', or 'enum' with `enum: 'ETresX'` (the
//                              cell is the value's game name or its number; the enum must be in
//                              `enums` below). optional: true -> Option<T>, a null cell is None;
//                              without it a null cell fails the generator. field: the Rust field
//                              name (default: the column name in snake case).
//
// enums   reflected enums (sheets/reflect/enum-values) to generate into tres_enums.rs as Rust
//         enums with the game's values: add a name here when code needs it.
module.exports = {
  params: {
    'sora-movement': {
      file: 'sora_params.rs', struct: 'SoraParams', prefix: 'SORA', from: 'port/sora-movement',
      doc: 'Sora\'s tunables, one field per row of the sheet. Units are KH3\'s own: centimetres, seconds, degrees.',
      informational: {
        'tick.order': 'open question Q1 of docs/sora-movement-spec.md; the core implements the order the spec assumes (controller, state queue, movement, then animation) and docs/sora-port-plan.md lists it under the assumptions to confirm at run time',
        'move.penetration_pullback': 'a run-time global of the exe (float at [[0x149D86A48]]) not yet measured; the core uses the stock UE 4.17 pull-back of UPrimitiveComponent::MoveComponent (crates/kh-core/src/cmc.rs move_updated)',
        'anim.run_start_root_distance': 'superseded: the root track of the five RunStart samples is taken per frame from build/sora_pack/manifest.json root_motion.tracks (sora_anim.rs)',
      },
    },
    'sora-combat-params': {
      file: 'sora_combat_params.rs', struct: 'SoraCombatParams', prefix: 'SORA_COMBAT', from: 'port/sora-combat-params',
      doc: 'The constants of melee combat, one field per row of the sheet (docs/sora-combat-spec.md). Centimetres, seconds, degrees.',
      informational: {},
    },
    // ---- enemy-side hit reaction (docs/sora-enemy-reaction-spec.md; crates/kh-react includes it) ----
    'sora-enemy-reaction': {
      file: 'sora_enemy_reaction.rs', struct: 'EnemyReactionParams', prefix: 'ENEMY_REACTION', from: 'port/sora-enemy-reaction',
      doc: 'What a generic KH3 enemy does when a hit lands: the enemy damage parameters, the handler constants and the median clip lengths, one field per row (docs/sora-enemy-reaction-spec.md). Centimetres, seconds, degrees.',
      informational: {
        'clip.land': 'not measured: kh-react uses the marked stand-in ClipTimes::LAND_STAND_IN (0.3 s); a host supplies its own landing time',
      },
    },
    // ---- (prize) prize orbs (docs/sora-prize-spec.md; crates/kh-prize includes it) ----
    'sora-prizes': {
      file: 'sora_prizes.rs', struct: 'PrizeParams', prefix: 'PRIZE', from: 'port/sora-prizes',
      doc: "KH3's prize orbs: movement, life, attraction and values, one field per row (docs/sora-prize-spec.md); rows test_* are choices of the test level, rows er_* the Elden Ring host's one drop rule. Centimetres, seconds, degrees.",
      informational: {
        'get.target_height': "not read (the getter's virtual +0x6E0): kh-prize flies to the point the host gives (its capsule centre)",
      },
    },
    // ---- (boss finish) the boss-finish presentation (docs/sora-bossfinish-spec.md; kh-core/src/combat/finish.rs includes it) ----
    'sora-bossfinish': {
      file: 'sora_bossfinish.rs', struct: 'BossFinishParams', prefix: 'BOSS_FINISH', from: 'port/sora-bossfinish',
      doc: "KH3's boss-finish presentation as the boss maps' level blueprints script it: the global game speed, the three camera cuts, the shake, the screen blur, the white-out, the sound, one field per row (docs/sora-bossfinish-spec.md); rows port_* and battle_* are choices of the port. Centimetres, seconds, degrees.",
      informational: {},
    },
    // ---- action states (docs/sora-actions-spec.md; kh-core/src/actions.rs includes it) ----
    'sora-actions': {
      file: 'sora_actions.rs', struct: 'ActionParams', prefix: 'ACTION', from: 'port/sora-actions',
      doc: 'The constants and rule texts of the action / traversal states, one field per row (docs/sora-actions-spec.md; Part II rows replace Part I rows of the same id). Centimetres, seconds, degrees.',
      informational: {
        'guard.cancel_time': 'Part II: the Guard TresEnableChange handler never sets m_bEnableChange, so there is no cancel time: a queued jump starts at the clip end (states/guard.rs)',
        'guard.hit_cancel_time': 'as guard.cancel_time: the hit clip has no cancel time either; dodge / air slide chain from guard.hit_enable_input',
        'glide.landing_exit': 'Part II Q2: nothing ends a glide on ground contact (states/glide.rs implements that reading; port option Tuning::glide_ends_on_ground, default off)',
      },
    },
    'sora-camera': {
      file: 'sora_camera_params.rs', struct: 'CameraParams', prefix: 'CAMERA', from: 'port/sora-camera',
      doc: 'The field camera\'s constants, one field per row of the sheet (docs/sora-camera-spec.md section 9). Centimetres, seconds, degrees.',
      informational: {
        'fade.character_near': 'open question of docs/sora-camera-spec.md 3.4: no fade of Sora by camera distance exists in the camera code; nothing is implemented',
      },
    },
    // ---- magic side (kh-core/src/magic/tables.rs): the five sora-magic-* sheets as lookup tables ----
    ...require("./port_gen_defs_magic.js"),
    // ---- status side (kh-core/src/status/tables.rs includes it) ----
    'sora-stats-rules': {
      file: 'sora_stats_rules.rs', struct: 'StatsRules', prefix: 'STATS_RULES', from: 'port/sora-stats-rules',
      doc: 'The constants and formula texts of docs/sora-stats-spec.md, one field per row (the formula rows are text: the code that implements each cites its row).',
      informational: {
        'prize.values': 'an object, not a scalar: the prize amounts are the constants of kh-core/src/status/runtime.rs apply_prize (HP 1 / 5, MP 3 / 5 or 2 s / 6 s of charge, focus 3 / 5, munny 1 / 5 / 10), pinned by a test against this row\'s text',
      },
    },
    // ---- sound (crates/kh-audio/src/rules.rs includes it) ----
    'sora-sound-rules': {
      file: 'sora_sound_rules.rs', struct: 'SoundRules', prefix: 'SOUND_RULES', from: 'port/sora-sound-rules',
      doc: 'The constants of docs/sora-audio-spec.md, one field per row (formula rows are text: the code of kh-audio that implements each cites its row). Linear volumes, frequency ratios, seconds, cm; the auto-SE rows are metres.',
      informational: {
        'asset.volume_default': 'open question 1 of docs/sora-audio-spec.md (the SQEXSEADSound constructor was not checked); kh-audio uses the stock USoundWave default 1.0 for an asset that does not serialize Volume (crates/kh-audio/src/pack.rs ASSET_VOLUME_DEFAULT_ASSUMED)',
        'curve.shapes': 'an array of formula texts, not a scalar: the six fade curves are the function crates/kh-audio/src/driver.rs curve(), pinned by a test against the formulas',
      },
    },
    // ---- (XSI, 2026-10-16) the 16 XS abilities (tools/sora_xs_sheet.js; kh-core/src/abilities/xs.rs includes both) ----
    'sora-xs-abilities': {
      file: 'sora_xs_abilities.rs', struct: 'XsAbilityParams', prefix: 'XS_ABILITY', from: 'port/sora-xs-abilities',
      doc: 'Every number of the 16 abilities of XS_SORA_ABILITIES.md (our design unless a KH3 source is named), one field per row. Centimetres, seconds, degrees.',
      informational: {},
    },
    'sora-xs-battle-levels': {
      file: 'sora_xs_battle_levels.rs', struct: 'XsBattleLevels', prefix: 'XS_BATTLE_LEVELS', from: 'port/sora-xs-battle-levels',
      doc: 'Hidden Potential: the battle level of a powerful Elden Ring foe by region depth (our design).',
      informational: {},
    },
    // (DON1) the Donald overhaul's shared numbers (crates/kh-party/src/donald/; build/sora_tmp/party/don/make_sheets.js; DON_PLAN.md)
    'party-don-core': {
      file: 'party_don_core.rs', struct: 'DonCoreParams', prefix: 'DON_CORE', from: 'port/party-don-core',
      doc: 'Donald\'s attunement, Resonance, Arcane Flow, status and reaction numbers (documentation/DONALD_OVERHAUL.md; DON1 readings where the doc leaves a number open), one field per row. Centimetres, seconds, shares.',
      informational: {},
    },
    // (DON-UI) the look of Donald's HUD pieces and his menu's switches (crates/kh-party/src/donald/hud.rs; kh_ui hud_resonance / hud_arcane)
    'party-don-hud': {
      file: 'party_don_hud.rs', struct: 'DonHudParams', prefix: 'DON_HUD', from: 'port/party-don-hud',
      doc: 'Donald\'s HUD (documentation/DONALD_OVERHAUL.md 6.1, 6.3, 6.5, 7, 9): the Resonance ring, pips, ultimate mark, stars, Zettaflare tracker and status icons placed on KH3\'s HUD movies, and his menu\'s switches (DON-UI), one field per row. Stage pixels, shares, seconds.',
      informational: {},
    },
    // (ZETTA) Zettaflare's gameplay numbers (crates/kh-party/src/donald/zettaflare.rs; build/sora_tmp/party/ZETTA/tools/make_zetta_sheets.js)
    'party-don-zetta': {
      file: 'party_don_zetta.rs', struct: 'DonZettaParams', prefix: 'DON_ZETTA', from: 'port/party-don-zetta',
      doc: 'Zettaflare rebuilt from the KH3 event kg702b (Donald\'s Zettaflare at Terra-Xehanort): the matinee times the sequence keys on and the numbers of the gameplay rebuild (the channel\'s length, the turn, the no-target aim), one field per row. Seconds, centimetres, degrees.',
      informational: {},
    },
    // (DON-MAGIC) the spell-specific numbers of Donald's casts (crates/kh-party/src/donald/magic_fx.rs; build/sora_tmp/party/DON-MAGIC/make_magic_sheet.js)
    'party-don-magic': {
      file: 'party_don_magic.rs', struct: 'DonMagicParams', prefix: 'DON_MAGIC', from: 'port/party-don-magic',
      doc: 'The spell-specific numbers of Donald\'s casts that party-don-spells has no column for (documentation/DONALD_OVERHAUL.md 4.3, 4.5, 6; DON-MAGIC readings where the doc leaves a number open), one field per row. Centimetres, seconds, shares.',
      informational: {},
    },
  },
  tables: {
    // (DON1) every castable spell of Donald's overhaul and its levels (crates/kh-party/src/donald/spells.rs)
    'party-don-spells': {
      file: 'party_don_spells.rs', row: 'DonSpellRow', table: 'DON_SPELLS', key: 'id', key_prefix: 'DON_SPELL',
      doc: 'One castable spell (and level) of Donald\'s overhaul (documentation/DONALD_OVERHAUL.md 4.3, 6): its MP prices in % of max MP (-1 = not castable that way), the full-power / once-per-period rules, Arcane Flow charges, and the effect numbers the spell streams read (their meaning per line is in the row\'s source).',
      columns: {
        id: { type: 'str' }, name: { type: 'str' }, element: { type: 'str', doc: 'fire | blizzard | thunder | water | aero | light | none' }, element2: { type: 'str', doc: 'a hybrid\'s second element, else "-"' },
        line: { type: 'str', doc: 'l1 (tiers) | l2 (3 levels) | l3 (ultimate) | hy (hybrid) | y (Arcane, on Y) | rel (Release burst)' }, level: { type: 'u32' },
        cost_free: { type: 'f32', doc: 'MP (fixed, like Sora; Y rows: % of max MP) when its element is not attuned (-1 = never castable so)' }, cost_att: { type: 'f32', doc: 'MP (fixed; Y rows: % of max MP) when attuned' }, cost_quick: { type: 'f32', doc: 'MP within res.quick_window of reaching Resonance 3 (-1 = no quick price)' },
        full_power: { type: 'bool' }, once_per_period: { type: 'bool' }, arcane: { type: 'u32', doc: 'Arcane Flow charges it costs (Y spells), else 0' },
        cast: { type: 'f32', doc: 's' }, power: { type: 'f32' }, extra: { type: 'f32' }, radius: { type: 'f32', doc: 'cm' }, range: { type: 'f32', doc: 'cm' }, angle: { type: 'f32', doc: 'deg' }, push: { type: 'f32', doc: 'cm' },
        duration: { type: 'f32', doc: 's' }, count: { type: 'u32' }, applies: { type: 'str', doc: 'statuses, comma separated: burn | chill:N | frozen | wet | reveal | blind | launch | -' }, target: { type: 'str', doc: 'enemy | ally | self | party | area_self | area_target' }, source: { type: 'str' },
      },
    },
    // (ZETTA) what the KH3 event kg702b does for Zettaflare, one row per clip / effect / sound (crates/kh-party/src/donald/zettaflare.rs)
    'party-don-zetta-fx': {
      file: 'party_don_zetta_fx.rs', row: 'DonZettaFxRow', table: 'DON_ZETTA_FX', key: 'id', key_prefix: 'DON_ZETTA_FX',
      doc: 'One thing the KH3 event kg702b (cutscene_kg01_kg702, matinee kg702b 58.767 .. 79.633 s) does for Donald\'s Zettaflare: a clip of his, an effect (on him, his staff, the target, or placed in a frame of him / of the target, with its move track), a cut of the event\'s SE stream, a voice line. Times are the matinee\'s.',
      columns: {
        id: { type: 'str' }, use: { type: 'u32', doc: '1 = played; 0 = a camera cheat of the event\'s shots (kept for the record)' }, ult: { type: 'u32', doc: '1 = also played by the ultimate casts (tinted to the element)' }, kind: { type: 'str', doc: 'clip | fx | se | voice | look (round 5: offset = exposure_ev, key_ev, bloom_threshold for kh_core::host::cinematic; fade = the blend, s)' },
        start: { type: 'f32', doc: 'matinee s' }, end: { type: 'f32', doc: 'matinee s; -1 = it ends by itself' }, asset: { type: 'str', doc: 'clip sequence / effect name / sound asset path' },
        frame: { type: 'str', doc: 'donald_socket | weapon | world_donald | beam (the beam frame: the target centre, X from his staff to it in 3D) | beam_socket (on the target, in the beam frame) | beam_stream (the beam laid along the line, stretched to it) | end (the ending: his feet at its start, X along the beam line) | end_beam (the ending beam: its start at his staff, X along it in 3D) | end_impact (the final cloud: centred above the real impact; offset = camera back, elevation deg, camera height; path[0] = the event distance) | end_glow (round 5c: the light at the source, at the ending beam start, sized with the beam width; path[0] = seconds to nothing) | none (target_socket / world_target: round 1)' }, socket: { type: 'str' },
        offset: { type: 'f32[]', doc: 'cm in the frame (X forward, Y right, Z up)' }, rotation: { type: 'f32[]', doc: 'pitch, yaw, roll (deg) in the frame' },
        path: { type: 'f32[]', doc: 'a move track: (matinee s, x, y, z) per key after the start, in the frame' }, end_mode: { type: 'str', doc: 'loop | once (clips); deactivate | fadeout | loopend (effects); fadeout (sounds); none' },
        fade: { type: 'f32', doc: 's' }, scale: { type: 'f32' }, source: { type: 'str' },
      },
    },
    // (ZETTA) the Zettaflare's cinematic camera: the KH3 event's camera keys of Donald's and the target's shots, in their frames
    'party-don-zetta-cam': {
      file: 'party_don_zetta_cam.rs', row: 'DonZettaCamRow', table: 'DON_ZETTA_CAM', key: 'id', key_prefix: 'DON_ZETTA_CAM',
      doc: 'One key of the KH3 event kg702b\'s camera for Donald\'s Zettaflare (TresMBCameraData_3, linear between keys): the camera in a frame of Donald or of the target, its FOV. The reaction shots of Mickey and Goofy are left out.',
      columns: {
        id: { type: 'str' }, shot: { type: 'str' }, frame: { type: 'str', doc: 'donald | beam (the target centre, X from his staff to it in 3D) | subject | end (the ending: his feet at its start, X along the beam line)' }, part: { type: 'str', doc: 'channel | cast | end' }, time: { type: 'f32', doc: 'matinee s' },
        position: { type: 'f32[]', doc: 'cm in the frame (X forward, Y right, Z up)' }, rotation: { type: 'f32[]', doc: 'pitch, yaw, roll (deg) in the frame' }, fov: { type: 'f32', doc: 'horizontal deg' }, source: { type: 'str' },
      },
    },
    // (DON-PROG) Donald's ability AP / levels / test order and the progression numbers (aggro, auras, AI Donald): crates/kh-party/src/donald/ability_info.rs
    'party-don-progression': {
      file: 'party_don_progression.rs', row: 'DonProgRow', table: 'DON_PROG', key: 'id', key_prefix: 'DON_PROG',
      doc: 'One row of the progression of Donald (documentation/DONALD_OVERHAUL.md 4.2, 5.4, 9, 10, 11): an ability (ability.<name>: its AP, the level he learns it at (0 = not by level), its place in the loadout of the test level (0 = not by default)) or a number of the aggro / aura / AI rules (value).',
      columns: {
        id: { type: 'str' }, ap: { type: 'i32' }, level: { type: 'u32' }, priority: { type: 'u32' }, value: { type: 'f32' }, unit: { type: 'str' }, source: { type: 'str' },
      },
    },
    // (GOOFY-BAL) the friend leaders' Strength / Magic / Defense per level (crates/kh-party/src/balance.rs; built by tools/party_balance_sheet.js)
    'party-balance-levels': {
      file: 'party_balance_levels.rs', row: 'BalanceLevelRow', table: 'PARTY_BALANCE_LEVELS',
      doc: 'A friend LEADER\'s Strength / Magic / Defense before his weapon and armour at each level 1..99 (port design, the user 2026-10-16 23:50: balance Donald and Goofy like Sora).',
      columns: { id: { type: 'str' }, chara: { type: 'str' }, level: { type: 'u32' }, strength: { type: 'i32' }, magic: { type: 'i32' }, defense: { type: 'i32' }, source: { type: 'str' } },
    },
    // (NF) every KH character's camera volumes (KH3's near-camera / see-through fade: crates/kh-camera/src/camfade.rs; written by
    // build/sora_tmp/party/NF/tools/make_camfade_sheets.js from the game's blueprints)
    'sora-camera-fade-volumes': {
      file: 'sora_camera_fade_volumes.rs', row: 'CamFadeVolumeRow', table: 'CAMERA_FADE_VOLUMES',
      doc: 'One camera volume of a KH character: a TresBodyCollPrimitive of his blueprint with m_bEnableCamera (MyBodyName "Camera"); the camera touching it fades him (FUN_1405D87F0 -> FUN_1405C9660).',
      columns: { id: { type: 'str' }, chara: { type: 'str', doc: 'the character id (p_ex001, n_ex001, ...)' }, bone: { type: 'str', doc: 'AttachToName on TresCharMesh0 (a bone; UE names compare without case)' }, shape: { type: 'str', doc: 'SPHERE | CAPSULE' }, radius: { type: 'f32', doc: 'MyShapeSize.X (cm)' }, half_height: { type: 'f32', doc: 'CAPSULE: MyShapeSize.Z, the half height with the radius (cm); 0 for a sphere' }, offset: { type: 'f32[]', doc: 'RelativeLocation in the bone (cm)' }, rotation: { type: 'f32[]', doc: 'RelativeRotation (pitch, yaw, roll; degrees): the capsule axis is its local Z' }, function: { type: 'str', doc: 'm_OverlapCameraFunction: FADE | RESET (RESET pushes the camera out instead)' }, source: { type: 'str' } },
    },
    // (GOOFY-GG) the GUARD gauge's max per level of Goofy (crates/kh-party/src/guard.rs `Guard::max_for_level`)
    'party-guard-levels': {
      file: 'party_guard_levels.rs', row: 'GuardLevelRow', table: 'PARTY_GUARD_LEVELS',
      doc: 'GUARD max of Goofy (n_ex002) at each level 1..99 (port design, the user 2026-10-16: GUARD grows with levels).',
      columns: { id: { type: 'str' }, chara: { type: 'str' }, level: { type: 'u32' }, guard_max: { type: 'f32' }, source: { type: 'str' } },
    },
    // ---- melee combat (docs/sora-combat-spec.md; crates/kh-core/src/combat/tables.rs includes them) ----
    'sora-combat-attacks': {
      file: 'sora_combat_attacks.rs', row: 'AttackKindRow', table: 'ATTACK_KINDS', key: 'kind', key_prefix: 'KIND', hex_key: true,
      doc: 'One attack kind of TresPlayerState_Attack (the value of state+0x1B0): its animation and the per-kind settings of PlayAttack 0x1413378B0 (docs/sora-combat-spec.md 4.3). Clip timings are NOT taken from here (the sheet rounds them): the notifies of the generated animation table are.',
      columns: {
        kind: { type: 'i32' }, label: { type: 'str' }, anim: { type: 'str', doc: 'row name of the anim set' }, clip: { type: 'str' }, phase: { type: 'str' }, role: { type: 'str' }, requires: { type: 'str' },
        root_len: { type: 'f32', optional: true, doc: 'state+0x1D8; None = the kind does not use root scaling' }, stop_dist: { type: 'f32' }, z_gain: { type: 'f32' },
        air_mode: { type: 'yesno' }, keep_face: { type: 'yesno' }, length: { type: 'f32' }, frames: { type: 'i32' }, root_motion: { type: 'yesno' }, root_fwd: { type: 'f32' }, root_up: { type: 'f32' },
        enable_input: { type: 'f32', optional: true }, enable_change: { type: 'f32', optional: true, doc: '(XSI) None: the clip has no TresEnableChange (7419, the SoKc clips)' }, attack_rows: { type: 'str[]' },
      },
    },
    'sora-combat-hits': {
      file: 'sora_combat_hits.rs', row: 'HitWindowRow', table: 'HIT_WINDOWS',
      doc: 'One hit window (TresAnimNotifyState_Attack) of a melee clip (docs/sora-combat-spec.md 8). start / end are the rounded times of the sheet: the hit detection runs off the notify itself.',
      columns: { id: { type: 'str' }, anim: { type: 'str' }, clip: { type: 'str' }, start: { type: 'f32' }, end: { type: 'f32' }, group: { type: 'str' }, attack_row: { type: 'str' }, hit_se: { type: 'str', doc: '"-" = none' }, interval: { type: 'f32', optional: true }, extra: { type: 'str', doc: '"-" = none' } },
    },
    'sora-combat-collisions': {
      file: 'sora_combat_collisions.rs', row: 'AtkCollisionRow', table: 'ATK_COLLISIONS',
      doc: 'One shape of a TresAtkCollComponent shape group: those of the keyblade (owner w_so_KeybladeBase) and of the pawn (docs/sora-combat-spec.md 8.1).',
      columns: { id: { type: 'str' }, owner: { type: 'str' }, group: { type: 'str' }, shape: { type: 'str' }, size_x: { type: 'f32' }, size_y: { type: 'f32' }, size_z: { type: 'f32' }, socket1: { type: 'str' }, socket2: { type: 'str' },
        loc1: { type: 'f32[]' }, loc2: { type: 'f32[]' }, rot: { type: 'f32[]' }, attach1: { type: 'str' }, attach2: { type: 'str' }, map_hit: { type: 'str' }, sweep: { type: 'yesno' }, default_row: { type: 'str' }, phys: { type: 'yesno' } },
    },
    // (party stream D6b) tools/sora_combat_shots_sheet.js
    'sora-combat-shots': {
      file: 'sora_combat_shots.rs', row: 'ShotShapeRow', table: 'SHOT_SHAPES',
      doc: 'One attack shape of a team finisher shot actor (p_ex001_ProjectileSet: UNISON_FIRE / _BLIZZARD / _THUNDER, FUSION_SPIN) with the actor life, particle, hit and fire effects.',
      columns: { id: { type: 'str' }, shot: { type: 'str' }, class: { type: 'str', field: 'class_name' }, group: { type: 'str' }, row: { type: 'str' }, shape: { type: 'str' }, mesh: { type: 'str' }, life: { type: 'f32' }, no_destroy: { type: 'yesno' },
        size: { type: 'f32[]' }, inc_size: { type: 'f32[]' }, max_size: { type: 'f32[]' }, size_curve: { type: 'f32[]' }, loc: { type: 'f32[]' }, scale: { type: 'f32[]' }, inc_scale: { type: 'f32[]' }, max_scale: { type: 'f32[]' },
        interval: { type: 'f32' }, map_hit: { type: 'str' }, particle: { type: 'str' }, particle_loc: { type: 'f32[]' }, hit_effect: { type: 'str' }, hit_se: { type: 'str' }, hit_offset: { type: 'f32' }, fire_effect: { type: 'str' }, fire_se: { type: 'str' }, spawn: { type: 'str' } },
    },
    'sora-combat-attack-data': {
      file: 'sora_combat_attack_data.rs', row: 'AttackDataRow', table: 'ATTACK_DATA', key: 'id', key_prefix: 'ATK',
      doc: 'One row of p_ex001_AttackData (TresAttackDataTable) as the hit windows name it (docs/sora-combat-spec.md 8).',
      columns: { id: { type: 'str' }, power: { type: 'f32' }, damage_kind: { type: 'enum', enum: 'ETresDamageKind' }, attribute: { type: 'enum', enum: 'ETresDamageAttribute' }, power_scale: { type: 'f32' }, parabola_angle: { type: 'f32' }, move_length: { type: 'f32' }, brake: { type: 'f32' },
        knockback: { type: 'str' }, exec_rate: { type: 'f32' }, effect_time: { type: 'f32' }, bad_status: { type: 'str' }, bad_gen_rate: { type: 'f32' }, bad_time: { type: 'f32' }, bad_param: { type: 'f32' }, team_check: { type: 'str' },
        reaction_power: { type: 'i32' }, scrum: { type: 'yesno' }, armor_attack: { type: 'i32' }, form_point: { type: 'i32' }, magic: { type: 'yesno' }, invalid_guard: { type: 'yesno' }, rapid_fire: { type: 'yesno' }, killer: { type: 'yesno' },
        attraction: { type: 'yesno' }, counter: { type: 'yesno' }, weak_guard: { type: 'yesno' }, reflect_level: { type: 'i32' }, reflect_reaction: { type: 'str' }, revenge_point: { type: 'f32' }, phys_force: { type: 'str' } },
    },
    'sora-combat-rules': {
      file: 'sora_combat_rules.rs', row: 'ComboRuleRow', table: 'COMBO_RULES',
      doc: 'The combo decision procedure as ordered rules (docs/sora-combat-spec.md 4). The procedure is code (combat/decide.rs); each branch there names its rule id, and the table is carried so that a test can check every rule has its branch.',
      columns: { id: { type: 'str' }, order: { type: 'i32' }, phase: { type: 'str' }, condition: { type: 'str' }, result: { type: 'str' }, kind: { type: 'str', doc: '"-" = the rule fixes no kind' } },
    },
    // (second combat implementer, 2026-10-11: the strand tables combat/ compiles against)
    'sora-combat-reaction-rules': {
      file: 'sora_combat_reaction_rules.rs', row: 'ReactionRuleRow', table: 'REACTION_RULES',
      doc: 'The 33 rows of the guard / armour / body classification Classify 0x14061FE20 (combat strand C 8.2.6). The procedure is code (combat/classify.rs); each branch there names its row and a test walks the table.',
      columns: { id: { type: 'str' }, body_type: { type: 'str' }, condition: { type: 'str' }, victim_result: { type: 'str' }, attacker_result: { type: 'str' }, no_react: { type: 'yesno' }, recorded: { type: 'yesno' }, clear_bad_status: { type: 'yesno' } },
    },
    'sora-combat-form-commands': {
      file: 'sora_combat_form_commands.rs', row: 'CommandRow', table: 'COMMAND_ROWS',
      doc: 'Rows of Load/Common/TresCmdData (TresCommandKindData) that the formchange gauge and the situation-command queue read (combat strand F 10.1, 11): Type, Cate, Attr, Param0 (queue lifetime, s), Param1 (form level), Param3 (form duration, s).',
      columns: { id: { type: 'str' }, kind: { type: 'i32' }, Type: { type: 'i32', field: 'cmd_type' }, Cate: { type: 'i32', field: 'cate' }, Attr: { type: 'i32', field: 'attr' }, Param0: { type: 'f32', field: 'param0' }, Param1: { type: 'f32', field: 'param1' }, Param3: { type: 'f32', field: 'param3' } },
    },
    'sora-combat-form-abilities': {
      file: 'sora_combat_form_abilities.rs', row: 'FormAbilityRow', table: 'FORM_ABILITIES',
      doc: 'p_ex001_FormAbility: the abilities wearing TRES_WEAR_FORM_05 (Second Form) adds, one equipment copy each (combat strand F 10.5).',
      columns: { id: { type: 'str' }, wear_form: { type: 'str' }, ability: { type: 'str' }, ability_id: { type: 'i32' }, copies: { type: 'i32' } },
    },
    // ---- status side (docs/sora-stats-spec.md; kh-core/src/status/tables.rs) ----
    'sora-levels': {
      file: 'sora_levels.rs', row: 'LevelRow', table: 'LEVELS',
      doc: 'One level of p_ex001_LevelUpData (row index = level - 1): the TOTAL EXP the level needs and the absolute Strength / Magic / Defense / AP of the level.',
      columns: {
        id: { type: 'str', field: 'level_name', doc: 'the row name, "1".."99"' },
        exp_total: { type: 'i32', doc: 'm_Exp: total EXP at which the level is reached' },
        attack: { type: 'i32', doc: 'm_AttackPower (absolute)' },
        magic: { type: 'i32', doc: 'm_MagicPower (absolute)' },
        defense: { type: 'i32', doc: 'm_DefensePower (absolute)' },
        ap: { type: 'i32', doc: 'm_AbilityPoint (absolute)' },
        ability_a: { type: 'str', doc: 'm_AbilityKind1 (ability path 0), "-" = none' },
        ability_b: { type: 'str', doc: 'm_AbilityKind2 (ability path 1), "-" = none' },
        ability_c: { type: 'str', doc: 'm_AbilityKind3 (ability path 2), "-" = none' },
      },
    },
    'sora-abilities': {
      file: 'sora_abilities.rs', row: 'AbilityRow', table: 'ABILITIES', key: 'id', key_prefix: 'ABILITY',
      doc: 'One ETresAbilityKind value with its TresAbilityData row and the stats spec\'s catalogue entry.',
      columns: {
        id: { type: 'str', doc: 'the enum value\'s game name (AIR_RECOVERY)' },
        enum_value: { type: 'enum', enum: 'ETresAbilityKind', field: 'kind' },
        name: { type: 'str', doc: 'English name' },
        category: { type: 'str' },
        ap: { type: 'i32', optional: true, doc: 'm_UseAP: AP one equipped copy costs (None: the row has no data, one party row)' },
        max_equip: { type: 'i32', optional: true, doc: 'm_MaxEquip (not a cap on owned copies: stats spec 5.1)' },
        owner: { type: 'str', doc: 'sora / party / cuisine / unused' },
        copies: { type: 'i32', doc: 'copies Sora owns at level 99 with every victory bonus' },
        system: { type: 'str', doc: 'the catalogue group of stats spec 5.4' },
        status: { type: 'str', doc: 'full / partial / hook-only / none: how far the effect was read' },
        port: { type: 'str', doc: '(X1) in / excluded: excluded = never granted, listed, equipped or loaded (decision dec029)' },
        help_text: { type: 'str', doc: '(XSI) the help line the game shows (- = none): the Abilities screen fallback for the port abilities, which the UI pack ability data does not list' },
      },
    },
    'sora-difficulty': {
      file: 'sora_difficulty.rs', row: 'DifficultyRow', table: 'DIFFICULTY', key: 'id', key_prefix: 'DIFFICULTY',
      doc: 'One game level (ETresGameLevelID; row index = its value): docs/sora-stats-spec.md section 3.',
      columns: {
        id: { type: 'str' },
        game_level: { type: 'i32' },
        base_hp: { type: 'i32' },
        base_mp: { type: 'i32' },
        hp_up: { type: 'i32[]', doc: 'amounts of HP_UP3 / 5 / 10 / 15 / 30 (victory-bonus kinds 1..5)' },
        mp_up: { type: 'i32[]', doc: 'amounts of MP_UP3 / 5 / 10 / 20 (victory-bonus kinds 8..11)' },
        player_damage: { type: 'f32', doc: 'factor on damage dealt by the player side' },
        enemy_damage: { type: 'f32', doc: 'factor on damage dealt by the other side' },
        enemy_attack_add: { type: 'i32' },
        ap_add: { type: 'i32' },
        mp_charge_time: { type: 'f32', doc: 'seconds' },
        start_abilities: { type: 'str[]' },
      },
    },
    // (DF, 2026-10-15) KH3 Critical's start abilities from the flagged victory-bonus rows (tools/sora_critical_sheet.js)
    'sora-critical-start': {
      file: 'sora_critical_start.rs', row: 'CriticalStartRow', table: 'CRITICAL_START',
      doc: 'One victory-bonus row whose ability Critical mode gives at the start (m_InitAbilityEquipOnCriticalMode; skip 0x140586280 case 1).',
      columns: {
        id: { type: 'str' },
        group: { type: 'str', doc: 'init (m_Critical* lists, granted by Status::from_loadout) / vbonus (flagged victory-bonus rows)' },
        ability: { type: 'str', doc: 'ETresAbilityKind game name' },
        mode: { type: 'str', doc: 'EQUIP (owned and equipped) / HAVE (owned)' },
        ap: { type: 'i32' },
        order: { type: 'i32' },
        port: { type: 'str', doc: '(dec032) in / excluded: excluded = not part of the port Critical start' },
      },
    },
    'sora-equipment': {
      file: 'sora_equipment.rs', row: 'EquipmentRow', table: 'EQUIPMENT',
      doc: 'One equipment row (TresEquipItemData), a Kingdom Key upgrade level (cumulative totals) or a slot count.',
      columns: {
        id: { type: 'str' },
        kind: { type: 'str', doc: 'keyblade / keyblade-upgrade / armour / accessory / slot' },
        name: { type: 'str' },
        ap: { type: 'i32', doc: 'm_AP (for kind slot: the slot count)' },
        attack: { type: 'i32', doc: 'm_AttackPlus' },
        magic: { type: 'i32', doc: 'm_MagicPlus' },
        defense: { type: 'i32', doc: 'm_DefensePlus' },
        resist: { type: 'i32[]', doc: 'm_AttrResist* by ETresDamageAttribute 0..7, percent' },
        abilities: { type: 'str[]', doc: 'm_AppendAbility: copies supplied while worn (no AP)' },
      },
    },
  },
  enums: [
    'ETresStateID',          // kh_core::StateId: the player states' ids (TSID_STATE_IDLE = 1 ...)
    'ETresAbilityKind',      // kh_core::status: equipped abilities are keyed by these values
    'ETresCommandKind',      // kh_core::ui_model: command menu entries
    'ETresDamageKind',       // kh_core::host::HitRecord::damage_kind (TresAttackDataTable.m_DamageKind)
    'ETresDamageAttribute',  // kh_core::host::HitRecord::attribute (TresAttackDataTable.m_DamageAttribute)
  ],
};
