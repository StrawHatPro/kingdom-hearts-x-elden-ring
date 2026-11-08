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
    // (weapons) KH3's Moogle shop / Workshop / Keyblade Forge rules: written by tools/kh3_weapon_sheets.js (documentation/KH3_WEAPON_DATA.md)
    'kh3-workshop-rules': {
      file: 'kh3_workshop_rules.rs', struct: 'Kh3WorkshopRules', prefix: 'KH3_WORKSHOP', from: 'port/kh3-workshop-rules',
      doc: 'KH3\'s Moogle shop, Workshop (synthesis, Collector\'s Goals, Photo Missions) and Keyblade Forge rules, one row per rule, from the decompiled exe and the level scripts.',
      informational: {},
    },
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
    // (HM-MAGIC) sora-hm-magic: written by tools/sora_hm_magic.js
    'sora-hm-magic': {
      file: 'sora_hm_magic.rs', row: 'HmMagicRow', table: 'HM_MAGIC', key: 'id', key_prefix: 'HMM',
      doc: 'KH3\'s turn-type magic set p_ex001_hm_MagicSet as data (kh-core magic/hm_set.rs, states/fire_magic.rs, magic/projectile.rs): its units, the TresProj_Mag*CL actors, the formchanges that install it, their MagicTurn / MagicSpin clips and the form weapon\'s ATK_01 over them.',
      columns: { id: { type: 'str', doc: '<what>.<key>' }, what: { type: 'str', doc: 'the row kind' }, form: { type: 'str', doc: 'the form code (HM, CL, CL2, SK, FL, HM2) or -' }, code: { type: 'i32', doc: 'the command (set, form), the tier (actor), frames (muzzle), else 0' }, logical: { type: 'str', doc: 'the unit key (set), the element (actor), the form command (form), the logical clip name (anim), ATK_01 (muzzle)' }, clip: { type: 'str', doc: 'the CL class (set, actor), the magic set (form), the animation-table row (anim, muzzle)' }, length: { type: 'f32', doc: 'the clip length, s' }, flags: { type: 'str[]', doc: 'strings by kind (see the note)' }, rows: { type: 'str[]', doc: 'the attack rows (actor)' }, effects: { type: 'str[]', doc: 'the effects (actor)' }, sounds: { type: 'str[]', doc: 'the sounds (actor)' }, values: { type: 'f32[]', doc: 'numbers by kind (see the note)' }, ref_addr: { type: 'str', doc: 'decompiled function' }, ref_conf: { type: 'str', doc: 'confirmed = read in the code or the data; inferred; guess' }, note: { type: 'str' } },
    },
    // (AUDIT-A PLACEMENT) sora-form-placement: written by tools/sora_form_placement.js (build/sora_tmp/weapons/AUDIT-A/PLACEMENT.md)
    'sora-form-placement': {
      file: 'sora_form_placement.rs', row: 'FormPlacementRow', table: 'FORM_PLACEMENT', key: 'id', key_prefix: 'FPL',
      doc: 'Where KH3 puts the form weapons while their own clips move them off their sockets (kh-core combat/forms/placement.rs): every TresAnimNotifyState_ChangeAttachParent window of the form weapons\' own clips (the weapon on another socket of Sora) and every TresAnimNotify_ThrowWeapon of Sora\'s form clips (the weapon detached, flying with its movement component).',
      columns: { id: { type: 'str', doc: '<what>.<model>.<logical> (parent / clip), throw.<form>.<grow>.<sequence>.<model>' }, what: { type: 'str', doc: 'the row kind' }, model: { type: 'str', doc: 'the weapon model (the pack\'s keyblades/<model>)' }, cmds: { type: 'i32[]', doc: 'the form commands whose parts include the model (sora-keyblade-forms units / grow units / transform units)' }, logical: { type: 'str', doc: 'Sora\'s socket: the new parent (parent), the part\'s own socket (throw), the socket (socket)' }, clip: { type: 'str', doc: 'the weapon clip\'s file stem (parent / clip), Sora\'s sequence with the notify (throw), the socket\'s bone (socket)' }, names: { type: 'str[]', doc: 'the logical name of the weapon clip (parent / clip); [the weapon\'s throw clip logical name, its sequence, Sora\'s sequence of that logical name] (throw)' }, length: { type: 'f32', doc: 'the weapon clip length, s' }, flags: { type: 'str[]', doc: 'throw: [grow level, follow | keep (bRotationFollowsVelocity), loop | once, the state class, thrown | base | ignored, homing | straight | no_movement]' }, values: { type: 'f32[]', doc: 'parent: [begin, end] s of the weapon clip; socket: [location x, y, z, rotation pitch, yaw, roll]; throw: see the note' } },
    },
    // (weapons F-SECOND) sora-form-second: written by tools/sora_form_second.js (documentation/KH3_WEAPONS_PLAN.md 1.3)
    'sora-form-second': {
      file: 'sora_form_second.rs', row: 'FormSecondRow', table: 'FORM_SECOND', key: 'id', key_prefix: 'FSE',
      doc: 'KH3\'s Second Form S / D (Starlight, Dead of Night) as data for kh-core (combat/forms/second.rs, states/style_finish.rs slot 155): Ancient Light\'s STYLEFINISH body, the Starlight weapon\'s hit shapes of M_1435, the rows\' differences from Second Form.',
      columns: { id: { type: 'str', doc: '<what>.<form>.<key>' }, what: { type: 'str', doc: 'the row kind' }, code: { type: 'i32', doc: 'finish sub-step (finish), the form command (diff), else 0' }, logical: { type: 'str', doc: 'the group (shape), the socket (socket), the clip KH3 plays (finish), the command (diff)' }, clip: { type: 'str', doc: 'the owner (shape), the bone (socket), the animation table row (finish), the Keyblade (diff)' }, flags: { type: 'str[]', doc: 'strings by kind (see the note)' }, windows: { type: 'str[]', doc: 'the clip\'s hit windows start-end@group@row, s' }, values: { type: 'f32[]', doc: 'numbers by kind (see the note)' }, ref_addr: { type: 'str', doc: 'decompiled function' }, ref_conf: { type: 'str', doc: 'confirmed = read in the code or the data' }, note: { type: 'str' } },
    },
    // (weapons F-ULTIMATE) sora-form-ultimate: written by tools/sora_form_ultimate.js (documentation/KH3_WEAPONS_PLAN.md 1.3)
    'sora-form-ultimate': {
      file: 'sora_form_ultimate.rs', row: 'FormUltimateRow', table: 'FORM_ULTIMATE', key: 'id', key_prefix: 'FSW',
      doc: 'KH3\'s Ultimate Form as data for its form package (kh-core combat/forms/ultimate.rs, states/combo_sw.rs): COMBO_SW\'s attack kinds and the guard counter, the replacement anim set, the Reflec guard, FINISH_SW, the weapon shapes and their per-frame tracks, the projectiles.',
      columns: { id: { type: 'str', doc: '<what>.SW.<key>' }, what: { type: 'str', doc: 'the row kind' }, form: { type: 'str', doc: 'SW (Ultimate Form, 0x87)' }, code: { type: 'i32', doc: 'attack kind (attack, shot), the class id (decide), finish sub-step (finish), effect group (guard), frames (track), else 0' }, logical: { type: 'str', doc: 'the logical anim name KH3 plays (attack / anim / finish / change / guard), the group (shape / track), the projectile (proj), the row (row)' }, clip: { type: 'str', doc: 'the animation table row (attack / finish / change / guard: a form-set clip by its sequence, a base / XS row by its logical name); the sequence (track / shot); the owner (shape); the class (proj)' }, length: { type: 'f32', doc: 'the clip length, s' }, group: { type: 'i32', doc: 'the effect group the play passes (PlayAttack +0x198)' }, root_len: { type: 'f32', doc: 'root length (+0x1D8), 0 = none' }, stop_dist: { type: 'f32', doc: 'stop distance (+0x24C)' }, z_gain: { type: 'f32', doc: 'z gain (+0x14C)' }, air_mode: { type: 'bool', doc: '+0x1DF (attack); the air variant (guard)' }, chan_xy: { type: 'bool', doc: '+0x234 bit 0' }, chan_root: { type: 'bool', doc: '+0x234 bit 1' }, flags: { type: 'str[]', doc: 'the case\'s other writes / the kind\'s strings (see the note)' }, windows: { type: 'str[]', doc: 'the clip\'s hit windows start-end@group@row@interval, s (track: the group\'s windows)' }, rows: { type: 'str[]', doc: 'the attack rows of those windows' }, effects: { type: 'str[]', doc: 'the clip\'s effects of that group (time@asset)' }, sounds: { type: 'str[]', doc: 'the clip\'s sounds / voices / swing aliases (time@asset)' }, values: { type: 'f32[]', doc: 'numbers by kind (see the note)' }, ref_addr: { type: 'str', doc: 'decompiled function' }, ref_conf: { type: 'str', doc: 'confirmed = read in the code or the data; inferred; guess' }, note: { type: 'str' } },
    },
    // (weapons F-NANO) sora-form-nano: written by tools/sora_form_nano.js (documentation/KH3_WEAPONS_PLAN.md 1.3)
    'sora-form-nano': {
      file: 'sora_form_nano.rs', row: 'FormNanoRow', table: 'FORM_NANO', key: 'id', key_prefix: 'FNA',
      doc: 'KH3\'s Nano Arms as data for its form package (kh-core combat/forms/nano.rs, states/combo_gm.rs): COMBO_GM\'s attack kinds, AnimSet8, the weapon\'s and the gate\'s hit shapes and their tracks, FINISH_GM, the projectiles, the attack rows.',
      columns: { id: { type: 'str', doc: '<what>.<key>' }, what: { type: 'str', doc: 'the row kind' }, code: { type: 'i32', doc: 'attack kind (attack), finish sub-step (finish), frames (track, gate), projectile stage (proj), else 0' }, logical: { type: 'str', doc: 'the logical anim name KH3 plays (attack / anim / finish), the group (shape, track), the projectile (proj), the row (row)' }, clip: { type: 'str', doc: 'the animation-table row (attack / anim / finish), the owner (shape), the sequence (track, gate), the blueprint (proj)' }, length: { type: 'f32', doc: 'the clip length, s' }, root_len: { type: 'f32', doc: 'root length (+0x1D8), 0 = none' }, stop_dist: { type: 'f32', doc: 'stop distance (+0x24C)' }, z_gain: { type: 'f32', doc: 'z gain (+0x14C)' }, air_mode: { type: 'bool', doc: '+0x1DF' }, chan_xy: { type: 'bool', doc: '+0x234 bit 0' }, chan_root: { type: 'bool', doc: '+0x234 bit 1' }, flags: { type: 'str[]', doc: 'the case\'s other writes / strings by kind (see the note)' }, windows: { type: 'str[]', doc: 'the clip\'s hit windows a-b, s' }, rows: { type: 'str[]', doc: 'the attack rows of those windows (row@group); proj / shape: the default row' }, values: { type: 'f32[]', doc: 'numbers by kind (see the note)' }, ref_addr: { type: 'str', doc: 'decompiled function' }, ref_conf: { type: 'str', doc: 'confirmed = read in the code or the data' }, note: { type: 'str' } },
    },
    // (weapons F-HAMMER) sora-form-hammer: written by tools/sora_form_hammer.js (documentation/KH3_WEAPONS_PLAN.md 1.3)
    'sora-form-hammer': {
      file: 'sora_form_hammer.rs', row: 'FormHammerRow', table: 'FORM_HAMMER', key: 'id', key_prefix: 'FHM',
      doc: 'KH3\'s Hyper Hammer / Drill Punch / Boom Hammer / Clock Drill as data for their form package (kh-core combat/forms/hammer.rs, states/combo_hm.rs): the attack kinds of COMBO_HM / COMBO_DR, the replacement anim sets, FINISH_DR (the drill / the Clock), the row renames, the weapon shapes and their per-frame tracks.',
      columns: { id: { type: 'str', doc: '<what>.<form>.<key>' }, what: { type: 'str', doc: 'the row kind (AUDIT-B F-HAMMER: wnotify = a weapon or the Clock: its own clip notify, tools/lib/wnotify_rows.js; phys = the phys shape groups of a form)' }, form: { type: 'str', doc: 'HM (Hyper Hammer 0x81), DR (Drill Punch 0x84), HM2 (Boom Hammer 0x92), DR2 (Clock Drill 0x93)' }, code: { type: 'i32', doc: 'attack kind (attack), finish sub-step (finish), frames (track), the attack kind of a rename (0 any, -1 the dodge), the command (start), else 0' }, logical: { type: 'str', doc: 'the logical anim name KH3 plays (attack / anim / finish / start), the group (shape / track), the old row (rename), the row (row)' }, clip: { type: 'str', doc: 'the animation table row (attack / finish / start / track: the sequence of a form-set clip, a logical name of the base / XS rows); the new row (rename); the owner (shape); the model (pawn)' }, length: { type: 'f32', doc: 'the clip length, s' }, group: { type: 'i32', doc: 'the effect group the play passes (PlayAttack +0x198)' }, root_len: { type: 'f32', doc: 'root length (+0x1D8), 0 = none' }, stop_dist: { type: 'f32', doc: 'stop distance (+0x24C)' }, z_gain: { type: 'f32', doc: 'z gain (+0x14C)' }, air_mode: { type: 'bool', doc: '+0x1DF' }, chan_xy: { type: 'bool', doc: '+0x234 bit 0' }, chan_root: { type: 'bool', doc: '+0x234 bit 1' }, flags: { type: 'str[]', doc: 'the case\'s other writes / the kind\'s strings (see the note)' }, windows: { type: 'str[]', doc: 'the clip\'s hit windows start-end@group@row@interval, s (renamed rows)' }, rows: { type: 'str[]', doc: 'the attack rows of those windows' }, effects: { type: 'str[]', doc: 'the clip\'s effects of that group (time@asset)' }, sounds: { type: 'str[]', doc: 'the clip\'s sounds / voices (time@asset)' }, values: { type: 'f32[]', doc: 'numbers by kind (see the note)' }, ref_addr: { type: 'str', doc: 'decompiled function' }, ref_conf: { type: 'str', doc: 'confirmed = read in the code or the data; inferred; guess' }, note: { type: 'str' } },
    },
    // (weapons F-SHIP) sora-form-ship: written by tools/sora_form_ship.js (documentation/KH3_WEAPONS_PLAN.md 1.3)
    'sora-form-ship': {
      file: 'sora_form_ship.rs', row: 'FormShipRow', table: 'FORM_SHIP', key: 'id', key_prefix: 'FSHIP',
      doc: 'KH3\'s Highwind and Storm Flag (Wheel of Fate) as data for their form package (kh-core combat/forms/ship.rs, states/combo_hl.rs, states/combo_fl.rs): COMBO_HL / COMBO_FL\'s attack kinds, the decision thresholds, the flag throw, the spear rain, the warp, the replacement anim sets, FINISH_FL (the Kraken), the collision shapes.',
      columns: { id: { type: 'str', doc: '<what>.<form>.<key>' }, what: { type: 'str', doc: 'the row kind' }, form: { type: 'str', doc: 'HL (Highwind, 0x8C) or FL (Storm Flag, 0x8D); ALL = every form and none (AUDIT-B W-CORE: the Final Blow aura rows)' }, code: { type: 'i32', doc: 'attack kind (attack, chain, warp), finish sub-step (finish), the form command (decide, start), else 0' }, logical: { type: 'str', doc: 'the logical anim name KH3 plays (attack / chain / anim / finish), the projectile (proj / rain), the blueprint (pawn), the group (shape), the row (row), the command (start)' }, clip: { type: 'str', doc: 'the clip = the animation table row (the sequence); the owner (shape); the model (pawn); the class (proj)' }, length: { type: 'f32', doc: 'the clip length, s' }, root_len: { type: 'f32', doc: 'root length (+0x1D8), 0 = none' }, stop_dist: { type: 'f32', doc: 'stop distance (+0x24C)' }, z_gain: { type: 'f32', doc: 'z gain (+0x14C)' }, dir_z: { type: 'f32', doc: 'DirZ written by the case (+0x1D4), 0 = none' }, air_mode: { type: 'bool', doc: '+0x1DF' }, chan_xy: { type: 'bool', doc: '+0x234 bit 0' }, chan_root: { type: 'bool', doc: '+0x234 bit 1' }, flags: { type: 'str[]', doc: 'the case\'s other writes / the kind\'s strings (see the note)' }, windows: { type: 'str[]', doc: 'the clip\'s hit windows a-b, s' }, rows: { type: 'str[]', doc: 'the attack rows of those windows' }, groups: { type: 'str[]', doc: 'the collision groups of those windows' }, effects: { type: 'str[]', doc: 'the clip\'s effects (time@asset)' }, sounds: { type: 'str[]', doc: 'the clip\'s sounds / voices (time@asset)' }, values: { type: 'f32[]', doc: 'numbers by kind (see the note)' }, ref_addr: { type: 'str', doc: 'decompiled function' }, ref_conf: { type: 'str', doc: 'confirmed = read in the code or the data; inferred; guess' }, note: { type: 'str' } },
    },
    // (weapons F-STAFF) sora-form-staff: written by tools/sora_form_staff.js (documentation/KH3_WEAPONS_PLAN.md 1.3)
    'sora-form-staff': {
      file: 'sora_form_staff.rs', row: 'FormStaffRow', table: 'FORM_STAFF', key: 'id', key_prefix: 'FST',
      doc: 'KH3\'s Mirage Staff as data for its form package (kh-core combat/forms/staff.rs, states/combo_wa.rs): COMBO_WA\'s attack kinds, the replacement anim set, the stick steering, MaxCombo / Finishers, the mirror images and the projectiles, FINISH_WA\'s tower.',
      columns: { id: { type: 'str', doc: '<what>.<key>' }, what: { type: 'str', doc: 'the row kind' }, code: { type: 'i32', doc: 'attack kind (attack), finish sub-step (finish), the form command (max, start), else 0' }, logical: { type: 'str', doc: 'the logical anim name KH3 plays (attack / anim / finish), the projectile name (proj), the blueprint (pawn), the group (shape), the row (row)' }, clip: { type: 'str', doc: 'the clip = the animation table row (the sequence); the blueprint class (proj); the pawn\'s clip (pawn); the owner (shape)' }, length: { type: 'f32', doc: 'the clip length, s' }, z_gain: { type: 'f32', doc: 'z gain (+0x14C)' }, stop_dist: { type: 'f32', doc: 'stop distance (+0x24C)' }, air_mode: { type: 'bool', doc: '+0x1DF' }, flags: { type: 'str[]', doc: 'the case\'s writes / the kind\'s strings (see the note)' }, windows: { type: 'str[]', doc: 'the clip\'s hit windows a-b, s' }, rows: { type: 'str[]', doc: 'the attack rows of those windows; the projectile\'s row' }, effects: { type: 'str[]', doc: 'the clip\'s effects (time@asset); the projectile\'s particles' }, sounds: { type: 'str[]', doc: 'the clip\'s sounds / voices (time@asset)' }, values: { type: 'f32[]', doc: 'numbers by kind (see the note)' }, ref_addr: { type: 'str', doc: 'decompiled function' }, ref_conf: { type: 'str', doc: 'confirmed = read in the code or the data; inferred; guess' }, note: { type: 'str' } },
    },
    // (weapons F-CLAWS) sora-form-claws: written by tools/sora_form_claws.js (documentation/KH3_WEAPONS_PLAN.md 1.3)
    'sora-form-claws': {
      file: 'sora_form_claws.rs', row: 'FormClawsRow', table: 'FORM_CLAWS', key: 'id', key_prefix: 'FCL',
      doc: 'KH3\'s Agile Claws / Twin Yo-Yos / Blizzard Claws / Blizzard Blades as data for their form package (kh-core combat/forms/claws.rs, states/combo_cl.rs / combo_yo.rs / combo_sk.rs): the attack kinds, the replacement anim sets, FINISH_YO / FINISH_SK, the CL2 row rename, the weapon shapes and their per-frame tracks.',
      columns: { id: { type: 'str', doc: '<what>.<form>.<key>' }, what: { type: 'str', doc: 'the row kind' }, form: { type: 'str', doc: 'CL (Agile Claws 0x8A), CL2 (Blizzard Claws 0x90), YO (Twin Yo-Yos 0x82), SK (Blizzard Blades 0x85)' }, code: { type: 'i32', doc: 'attack kind (attack), finish sub-step (finish), frames (track), else 0' }, logical: { type: 'str', doc: 'the logical anim name KH3 plays (attack / anim / finish / loco), the group (shape / track), the old row (rename), the row (row)' }, clip: { type: 'str', doc: 'the animation table row (attack / finish / track: the sequence of a form-set clip, a logical name of the base / XS rows); the new row (rename); the owner (shape); the model (pawn)' }, length: { type: 'f32', doc: 'the clip length, s' }, group: { type: 'i32', doc: 'the effect group the play passes (PlayAttack +0x198)' }, root_len: { type: 'f32', doc: 'root length (+0x1D8), 0 = none' }, stop_dist: { type: 'f32', doc: 'stop distance (+0x24C)' }, z_gain: { type: 'f32', doc: 'z gain (+0x14C)' }, air_mode: { type: 'bool', doc: '+0x1DF' }, chan_xy: { type: 'bool', doc: '+0x234 bit 0' }, chan_root: { type: 'bool', doc: '+0x234 bit 1' }, flags: { type: 'str[]', doc: 'the case\'s other writes / the kind\'s strings (see the note)' }, windows: { type: 'str[]', doc: 'the clip\'s hit windows start-end@group@row@interval, s (CL2: renamed rows)' }, rows: { type: 'str[]', doc: 'the attack rows of those windows' }, effects: { type: 'str[]', doc: 'the clip\'s effects of that group (time@asset)' }, sounds: { type: 'str[]', doc: 'the clip\'s sounds / voices (time@asset)' }, values: { type: 'f32[]', doc: 'numbers by kind (see the note)' }, ref_addr: { type: 'str', doc: 'decompiled function' }, ref_conf: { type: 'str', doc: 'confirmed = read in the code or the data; inferred; guess' }, note: { type: 'str' } },
    },
    // (weapons F-ARROWGUN) sora-form-arrowgun: written by tools/sora_form_arrowgun.js (documentation/KH3_WEAPONS_PLAN.md 1.3)
    'sora-form-arrowgun': {
      file: 'sora_form_arrowgun.rs', row: 'FormArrowgunRow', table: 'FORM_ARROWGUN', key: 'id', key_prefix: 'FAG',
      doc: 'KH3\'s gun formchanges (Double Arrowguns, Magic Launcher, Hunny Blasters, Hunny Launcher) as data for their form package (kh-core combat/forms/arrowgun.rs, states/combo_dw.rs, states/combo_bz.rs): COMBO_DW / COMBO_BZ\'s attack kinds and shots, the replacement anim sets, the gun projectiles, the launchers\' melee shapes, FINISH_BZ, the attack rows.',
      columns: { id: { type: 'str', doc: '<what>.<form>.<key>' }, what: { type: 'str', doc: 'the row kind (AUDIT-A ARROWGUN: wnotify = a form weapon or transform unit: its own clip notify, tools/lib/wnotify_rows.js)' }, form: { type: 'str', doc: 'DW / BZ / DW2 / BZ2 (anim, attack: DW = both COMBO_DW forms), SS / PO (proj, row: Shooting Star / Hunny Spout)' }, code: { type: 'i32', doc: 'attack kind (attack), finish command (finish), else 0' }, logical: { type: 'str', doc: 'the logical anim name (anim, attack, finish), the class (proj), the group (shape), the row (row)' }, clip: { type: 'str', doc: 'the clip = the animation table row (the sequence); the particle (proj); the weapon (shape)' }, length: { type: 'f32', doc: 'the clip length, s' }, group: { type: 'i32', doc: 'the effect group the play passes in DW2 / BZ2 (PlayAttack +0x198), 0 = none' }, flags: { type: 'str[]', doc: 'strings by kind (see the note)' }, values: { type: 'f32[]', doc: 'numbers by kind (see the note)' }, ref_addr: { type: 'str', doc: 'decompiled function' }, ref_conf: { type: 'str', doc: 'confirmed = read in the code or the data; inferred; guess' }, note: { type: 'str' } },
    },
    // (weapons F-RAGE) sora-form-rage: written by tools/sora_form_rage.js (documentation/KH3_WEAPONS_PLAN.md 6e)
    'sora-form-rage': {
      file: 'sora_form_rage.rs', row: 'FormRageRow', table: 'FORM_RAGE', key: 'id', key_prefix: 'FRG',
      doc: 'KH3\'s Rage Form as data for its form package (kh-core combat/forms/rage.rs, states/combo_anti.rs): the HP-loss offer, the counts and the cooldown, COMBO_ANTI\'s attack kinds, AnimSet13, RISKCHARGE / FINISH_AN, the aura, the attack rows.',
      columns: { id: { type: 'str', doc: '<what>.<key>' }, what: { type: 'str', doc: 'the row kind (sock / shape: COMBAT-FIX, the hit sockets per frame, the pawn groups)' }, code: { type: 'i32', doc: 'attack kind (attack, chain), finish command (finish), effect group (aura), else 0' }, logical: { type: 'str', doc: 'the logical anim name KH3 plays (attack / chain / finish / anim), the rule (rule), the particle (aura), the row (row)' }, clip: { type: 'str', doc: 'the clip = the animation-table row; the socket (aura)' }, length: { type: 'f32', doc: 'the clip length, s' }, root_len: { type: 'f32', doc: 'root length (+0x1D8), 0 = none' }, stop_dist: { type: 'f32', doc: 'stop distance (+0x24C)' }, z_gain: { type: 'f32', doc: 'z gain (+0x14C)' }, air_mode: { type: 'bool', doc: '+0x1DF' }, keep_face: { type: 'bool', doc: '+0x1F4 bit 4' }, chan_xy: { type: 'bool', doc: '+0x234 bit 0' }, chan_root: { type: 'bool', doc: '+0x234 bit 1' }, flags: { type: 'str[]', doc: 'the case\'s other writes (see the note)' }, windows: { type: 'str[]', doc: 'the clip\'s hit windows a-b, s' }, rows: { type: 'str[]', doc: 'the attack rows of those windows' }, effects: { type: 'str[]', doc: 'the clip\'s effects (time@group:asset) and shots (shot time@name)' }, sounds: { type: 'str[]', doc: 'the clip\'s sounds / voices (time@asset)' }, values: { type: 'f32[]', doc: 'numbers by kind (see the note)' }, ref_addr: { type: 'str', doc: 'decompiled function' }, ref_conf: { type: 'str', doc: 'confirmed = read in the code or the data; inferred; guess' }, note: { type: 'str' } },
    },
    // (weapons F-LIGHTDARK) sora-form-lightdark: written by tools/sora_form_lightdark.js (documentation/KH3_WEAPONS_PLAN.md 1.3)
    'sora-form-lightdark': {
      file: 'sora_form_lightdark.rs', row: 'FormLightdarkRow', table: 'FORM_LIGHTDARK', key: 'id', key_prefix: 'FLD',
      doc: 'KH3 Re Mind\'s Light Form, Dark Form and Double Form as data for their form package (kh-core combat/forms/lightdark.rs, states/combo_lf.rs / combo_df.rs / combo_tf.rs): PlayAttack\'s cases of COMBO_LF / COMBO_DF / COMBO_TF, AnimSet22 / AnimSet23, the form guard and dodge, FINISH_TF, the weapons\' hit shapes, the projectiles, the hit sockets of both hands.',
      columns: { id: { type: 'str', doc: '<what>.<form>.<key>' }, what: { type: 'str', doc: 'the row kind' }, form: { type: 'str', doc: 'LF / DF / TF (attack, rule, guard, dodge, finish, proj), LD / TF (anim: AnimSet22 / AnimSet23), R / L (shape, weapon, sock: the hand)' }, code: { type: 'i32', doc: 'attack kind (attack), finish sub-step (finish), frames (sock), else 0' }, logical: { type: 'str', doc: 'the logical anim name (anim, attack, finish, guard, dodge), the group (shape), the projectile (proj), the animation-table row (sock), the row (row)' }, clip: { type: 'str', doc: 'the animation-table row; the blueprint (shape, proj); the sequence (sock)' }, length: { type: 'f32', doc: 'the clip length, s' }, group: { type: 'i32', doc: 'the effect group of the play (PlayAttack +0x198, Guard +0xEC, DodgeRoll +0x90)' }, root_len: { type: 'f32', doc: 'root length (+0x1D8), 0 = none' }, stop_dist: { type: 'f32', doc: 'stop distance (+0x24C)' }, z_gain: { type: 'f32', doc: 'z gain (+0x14C)' }, air_mode: { type: 'bool', doc: '+0x1DF' }, keep_face: { type: 'bool', doc: '+0x1F4 bit 4' }, chan_xy: { type: 'bool', doc: '+0x234 bit 0' }, chan_root: { type: 'bool', doc: '+0x234 bit 1' }, flags: { type: 'str[]', doc: 'strings by kind (see the note)' }, windows: { type: 'str[]', doc: 'the clip\'s hit windows a-b, s' }, rows: { type: 'str[]', doc: 'the attack rows of those windows / shapes' }, effects: { type: 'str[]', doc: 'the clip\'s effects of that group (time@group:asset)' }, sounds: { type: 'str[]', doc: 'the clip\'s sounds / voices (time@asset)' }, values: { type: 'f32[]', doc: 'numbers by kind (see the note)' }, ref_addr: { type: 'str', doc: 'decompiled function' }, ref_conf: { type: 'str', doc: 'confirmed = read in the code or the data; inferred; guess' }, note: { type: 'str' } },
    },
    // (weapons F-SHIELD) sora-form-shield: written by tools/sora_form_shield.js (documentation/KH3_WEAPONS_PLAN.md 1.3)
    'sora-form-shield': {
      file: 'sora_form_shield.rs', row: 'FormShieldRow', table: 'FORM_SHIELD', key: 'id', key_prefix: 'FSH',
      doc: 'KH3\'s Counter Shield and Frying Pan as data for their form package (kh-core combat/forms/shield.rs, states/combo_sh.rs): COMBO_SH\'s attack kinds, the replacement anim sets, the grow levels, FINISH_SH, the row rename, the collision shapes.',
      columns: { id: { type: 'str', doc: '<what>.<form>.<key>' }, what: { type: 'str', doc: 'the row kind' }, form: { type: 'str', doc: 'SH (Counter Shield, 0x89) or SH2 (Frying Pan, 0x91)' }, code: { type: 'i32', doc: 'attack kind (attack, chain), grow level (grow), finish sub-step (finish), else 0' }, logical: { type: 'str', doc: 'the logical anim name KH3 plays (attack / chain / anim / finish), the guard clip (grow), the blueprint (pawn), the old row (rename), the group (shape), the row (row)' }, clip: { type: 'str', doc: 'the clip = the animation table row (the sequence); the new row (rename); the owner (shape); the model (pawn)' }, length: { type: 'f32', doc: 'the clip length, s' }, group: { type: 'i32', doc: 'the effect group the play passes (PlayAttack +0x198)' }, root_len: { type: 'f32', doc: 'root length (+0x1D8), 0 = none' }, stop_dist: { type: 'f32', doc: 'stop distance (+0x24C)' }, z_gain: { type: 'f32', doc: 'z gain (+0x14C)' }, air_mode: { type: 'bool', doc: '+0x1DF' }, traction: { type: 'bool', doc: 'm_bEnableTraction at the play' }, chan_xy: { type: 'bool', doc: '+0x234 bit 0' }, chan_root: { type: 'bool', doc: '+0x234 bit 1' }, flags: { type: 'str[]', doc: 'the case\'s other writes / the kind\'s strings (see the note)' }, windows: { type: 'str[]', doc: 'the clip\'s hit windows a-b, s' }, rows: { type: 'str[]', doc: 'the attack rows of those windows (Frying Pan: renamed); grow: the weapon parts' }, effects: { type: 'str[]', doc: 'the clip\'s effects of that group (time@asset)' }, sounds: { type: 'str[]', doc: 'the clip\'s sounds / voices (time@asset)' }, values: { type: 'f32[]', doc: 'numbers by kind (see the note)' }, ref_addr: { type: 'str', doc: 'decompiled function' }, ref_conf: { type: 'str', doc: 'confirmed = read in the code or the data; inferred; guess' }, note: { type: 'str' } },
    },
    // (weapons P3) sora-keyblade-forms: written by tools/sora_keyblade_forms.js (documentation/KH3_WEAPONS_PLAN.md 1.3)
    'sora-keyblade-forms': {
      file: 'sora_keyblade_forms.rs', row: 'KeybladeFormRow', table: 'KEYBLADE_FORMS', key: 'id', key_prefix: 'KBFORM',
      doc: 'One KH3 formchange as data for the form framework (kh-core combat/form.rs): the Keyblade, command, level chain, time, finish commands, wear form, form abilities, transformation clip, replacement anim set, projectiles and weapon parts.',
      columns: { id: { type: 'str', doc: 'form.<code> (the CHANGE_<code> command)' }, cmd: { type: 'i32', doc: 'the formchange command (ETresCommandKind value)' }, command: { type: 'str', doc: 'its name' }, name: { type: 'str', doc: 'English (the command\'s LocKey text)' }, keyblade: { type: 'str', doc: 'the Keyblade (kh3-keyblades id) whose form it is, - for Rage Form' }, keyblade_name: { type: 'str', doc: 'English' }, weapon_type: { type: 'u32', doc: 'KH3\'s weapon type of that Keyblade (= its ETresItemDefWeapon value; AwardAtMax / StartStyle switch on it); 0 = none' }, weapon_form: { type: 'str', doc: 'ETresWeaponForm of the equipment variant' }, weapon_form_value: { type: 'u32', doc: 'its value' }, stage: { type: 'i32', doc: 'form level = TresCmdData Param1: 1 first stage, 2 second stage' }, first: { type: 'str', doc: 'a second stage: the first-stage form it is offered in, else -' }, next: { type: 'str', doc: 'a first stage: the form a full gauge offers in it (AwardAtMax at form level 1), else -' }, next_cmd: { type: 'i32', doc: 'that form\'s command, 0 = none' }, grow_levels: { type: 'u32', doc: 'ETresWeaponGrowLvl variants of its weapon (1..3)' }, time: { type: 'f32', doc: 'form time, s (TresCmdData Param3; 30 when 0)' }, command_time: { type: 'f32', doc: 'how long the command stays offered, s (Param0)' }, finish: { type: 'i32', doc: 'its finish command (the first finish level for the Second Form family), 0 = none' }, finish_command: { type: 'str', doc: 'its name or -' }, finish_levels: { type: 'i32[]', doc: 'the Second Form family: the finish command of finish level 1, 2, 3 (SetFinishLevel); else empty' }, end_delete: { type: 'i32[]', doc: 'the commands EndStyle deletes for it (its next stage\'s offer or its finish commands)' }, wear_form: { type: 'u32', doc: 'ETresCharWearForm value Sora wears (5 Second Form, 7 Rage Form ...)' }, wear_form_name: { type: 'str', doc: 'p_ex001_WearformSet m_FormName' }, wear_at: { type: 'str', doc: 'when the transformation applies it: style_change = STYLECHANGE sub 3 (Second Form family, Light / Dark / Double Form), weapon_change = with the weapon variant right after its clip starts' }, transform_clip: { type: 'str', doc: 'the transformation clip (StyleChange Tick sub 0)' }, anim_sets: { type: 'str[]', doc: 'm_ReplaceAnimSets: the replacement anim sets of the form' }, magic_set: { type: 'str', doc: 'm_ReplaceMagicSet' }, projectile_set: { type: 'str', doc: 'm_ReplaceProjectileSet' }, projectiles: { type: 'str[]', doc: 'its projectile kinds (name=blueprint)' }, units: { type: 'str[]', doc: 'weapon parts at grow level 1: unit@part:socket=weapon blueprint' }, grow_units: { type: 'str[]', doc: 'weapon parts of grow levels 2 / 3: G<n>:unit@part:socket=blueprint' }, transform_units: { type: 'str[]', doc: 'm_TransformSetData: the Keyblade\'s transform part' }, abilities: { type: 'str[]', doc: 'p_ex001_FormAbility m_Ability of the wear form (ETresAbilityKind; a repeat = another copy)' }, text_abilities: { type: 'str[]', doc: 'p_ex001_FormAbility m_TextAbility (TEXT_*)' }, moves: { type: 'str[]', doc: 'TresKeybladeAbilityData m_FormAbilityMap: the moves the Abilities screen lists' }, move_names: { type: 'str[]', doc: 'English' }, wear_textures: { type: 'str[]', doc: 'Sora\'s costume texture swaps (material=texture)' }, ui_asset: { type: 'str', doc: 'm_UIDataAsset' }, act_logo: { type: 'str[]', doc: 'the formchange banner movies' }, charge_effect: { type: 'str', doc: 'm_CmnFormChangeChargeEffect' }, start_extra: { type: 'str', doc: 'StartStyle\'s per-form tail (for the F-package\'s on_start)' }, trigger: { type: 'str', doc: 'what offers the command' }, moveset: { type: 'str', doc: 'second_form = the framework\'s own; pending = no moveset yet (never offered, refused); else the F-package' } },
    },
    // (weapons) kh3-keyblades: written by tools/kh3_weapon_sheets.js (documentation/KH3_WEAPON_DATA.md)
    'kh3-keyblades': {
      file: 'kh3_keyblades.rs', row: 'Kh3KeybladeRow', table: 'KH3_KEYBLADES', key: 'id', key_prefix: 'KH3_KB',
      doc: 'One Keyblade of KH3 (base game + Re Mind): item, stats, abilities, Forge levels, shotlocks, formchanges, assets and how KH3 gives it.',
      columns: { id: { type: 'str', doc: 'm_KeyName (WEP_KEYBLADE_SO_00 ..)' }, item_id: { type: 'u32', doc: 'item id 0x300 + index' }, row: { type: 'str', doc: 'TresItemData row I03nnn' }, weapon_def: { type: 'str', doc: 'ETresItemDefWeapon name (the upgrade / ability tables key by it)' }, weapon_def_value: { type: 'u32', doc: 'its value = the item index' }, name: { type: 'str', doc: 'English name' }, name_id: { type: 'str', doc: 'm_NameTextID' }, help: { type: 'str', doc: 'English description (markup stripped)' }, help_id: { type: 'str', doc: 'm_HelpTextID' }, strength: { type: 'i32', doc: 'm_AttackPlus at level 0' }, magic: { type: 'i32', doc: 'm_MagicPlus at level 0' }, abilities: { type: 'str[]', doc: 'm_AppendAbility (ETresAbilityKind)' }, ability_names: { type: 'str[]', doc: 'English' }, max_level: { type: 'u32', doc: 'highest m_WeaponLevel of its Forge rows' }, init_level: { type: 'u32', doc: 'highest level with m_bInitAchieved (the level it has when obtained)' }, shotlocks: { type: 'str[]', doc: 'm_ShootflowAbilitys (ETresTextAbilityKind)' }, shotlock_names: { type: 'str[]', doc: 'English' }, wear_forms: { type: 'str[]', doc: 'm_WearForms (Sora\'s formchange costumes)' }, formchanges: { type: 'str[]', doc: 'CHANGE_* commands of its formchanges (TresCmdData), see kh3-keyblade-forms' }, formchange_names: { type: 'str[]', doc: 'English' }, icon: { type: 'str', doc: 'm_IconAsset (item icon)' }, forge_icon: { type: 'str', doc: 'TresItemWeaponIconData KeybladeIconAsset' }, picker_icon: { type: 'str', doc: 'TresItemWeaponIconData PickerIconAsset' }, equip: { type: 'str', doc: 'TresEquipItemData m_EquipSubclass (TresEquipKeybladeData)' }, weapon_bp: { type: 'str', doc: 'the Keyblade\'s own weapon blueprint (form 01, grow 01)' }, mesh: { type: 'str', doc: 'its skeletal mesh' }, projectile_set: { type: 'str', doc: 'm_ReplaceProjectileSet of form 01 (shotlock projectiles)' }, max_num: { type: 'u32', doc: 'm_MaxNum' }, can_sell: { type: 'bool', doc: 'm_bCanSell' }, how: { type: 'str[]', doc: 'how KH3 gives it (kh3-weapon-sources)' }, where: { type: 'str[]', doc: 'where / when' } },
    },
    // (weapons) kh3-keyblade-levels: written by tools/kh3_weapon_sheets.js (documentation/KH3_WEAPON_DATA.md)
    'kh3-keyblade-levels': {
      file: 'kh3_keyblade_levels.rs', row: 'Kh3KeybladeLevelRow', table: 'KH3_KEYBLADE_LEVELS', key: 'id', key_prefix: 'KH3_KBLV',
      doc: 'One Keyblade Forge level of KH3 (TresItemWeaponEnhanceData): stat gains, the added ability and the materials it costs (no munny).',
      columns: { id: { type: 'str', doc: '<keyblade>.lv<n>' }, keyblade: { type: 'str', doc: 'kh3-keyblades id' }, level: { type: 'u32', doc: 'm_WeaponLevel 1..10' }, row: { type: 'str', doc: 'IW_n' }, flag_index: { type: 'u32', doc: 'm_FlagIndex: the save bit of the level' }, init_achieved: { type: 'bool', doc: 'm_bInitAchieved: the level is done when the Keyblade is obtained' }, strength_plus: { type: 'i32', doc: 'm_AttackPlus' }, magic_plus: { type: 'i32', doc: 'm_MagicPlus' }, strength: { type: 'i32', doc: 'base + every level up to this one' }, magic: { type: 'i32', doc: 'base + every level up to this one' }, ability: { type: 'str', doc: 'm_AppendAbility or -' }, ability_name: { type: 'str', doc: 'English' }, materials: { type: 'str[]', doc: 'm_Material0..5 (MAT_ITEMnn)' }, counts: { type: 'i32[]', doc: 'm_MatNum0..5' }, material_names: { type: 'str[]', doc: 'English' }, munny: { type: 'u32', doc: 'munny cost (0: the Forge takes none)' } },
    },
    // (weapons) kh3-keyblade-forms: written by tools/kh3_weapon_sheets.js (documentation/KH3_WEAPON_DATA.md)
    'kh3-keyblade-forms': {
      file: 'kh3_keyblade_forms.rs', row: 'Kh3KeybladeFormRow', table: 'KH3_KEYBLADE_FORMS', key: 'id', key_prefix: 'KH3_KBFORM',
      doc: 'One weapon form (and grow level) of a KH3 Keyblade (TresEquipKeybladeData m_EquipMap): the formchange command, its time and level, finisher, the weapon parts and where they attach, replacement anim / magic / projectile sets, the moves the Abilities screen lists, the form\'s abilities and Sora\'s costume textures.',
      columns: { id: { type: 'str', doc: '<keyblade>.F<form>.G<grow>' }, keyblade: { type: 'str', doc: 'kh3-keyblades id' }, weapon_form: { type: 'str', doc: 'ETresWeaponForm' }, weapon_form_value: { type: 'u32', doc: 'its value' }, grow: { type: 'u32', doc: 'ETresWeaponGrowLvl 1..3' }, wear_form: { type: 'str', doc: 'm_OwnerWearForm (ETresCharWearForm; NORMAL = no formchange)' }, wear_form_name: { type: 'str', doc: 'p_ex001_WearformSet m_Forms[wear form].m_FormName' }, formchange: { type: 'bool', doc: 'a formchange (wear form not NORMAL)' }, command: { type: 'str', doc: 'the CHANGE_* command (TresCmdData) or -' }, command_name: { type: 'str', doc: 'English' }, form_level: { type: 'i32', doc: 'command Param1 (1 first form, 2 second-stage form)' }, form_time: { type: 'i32', doc: 'command Param3, s' }, command_time: { type: 'i32', doc: 'command Param0: s the formchange command stays offered' }, finisher: { type: 'str', doc: 'FINISH_<code> command if TresCmdData has one, else -' }, ui_asset: { type: 'str', doc: 'm_UIDataAsset' }, act_logo: { type: 'str[]', doc: 'its Scaleform movies (the formchange banner)' }, units: { type: 'str[]', doc: 'm_Equips: unit@part:socket=weapon blueprint' }, transform_units: { type: 'str[]', doc: 'm_TransformSetData (the Keyblade\'s transform part)' }, anim_sets: { type: 'str[]', doc: 'm_ReplaceAnimSets' }, magic_set: { type: 'str', doc: 'm_ReplaceMagicSet' }, projectile_set: { type: 'str', doc: 'm_ReplaceProjectileSet' }, projectiles: { type: 'str[]', doc: 'its Projectiles (name=blueprint)' }, moves: { type: 'str[]', doc: 'TresKeybladeAbilityData m_FormAbilityMap[form] (ETresTextAbilityKind)' }, move_names: { type: 'str[]', doc: 'English' }, form_abilities: { type: 'str[]', doc: 'p_ex001_FormAbility m_Ability of the wear form' }, form_text_abilities: { type: 'str[]', doc: 'p_ex001_FormAbility m_TextAbility' }, wear_textures: { type: 'str[]', doc: 'p_ex001_WearformSet texture replacements (material=texture)' }, charge_effect: { type: 'str', doc: 'm_CmnFormChangeChargeEffect' } },
    },
    // (weapons) kh3-weapon-assets: written by tools/kh3_weapon_sheets.js (documentation/KH3_WEAPON_DATA.md)
    'kh3-weapon-assets': {
      file: 'kh3_weapon_assets.rs', row: 'Kh3WeaponAssetRow', table: 'KH3_WEAPON_ASSETS', key: 'id', key_prefix: 'KH3_WASSET',
      doc: 'One weapon (or shotlock projectile) blueprint of KH3 that a Keyblade, staff or shield uses, with the meshes, materials, anim sets, effects and sounds it references.',
      columns: { id: { type: 'str', doc: 'blueprint path' }, kind: { type: 'str', doc: 'whose' }, used_by: { type: 'str[]', doc: 'weapon ids that use it' }, parent: { type: 'str', doc: 'parent class' }, mesh: { type: 'str', doc: 'SkeletalMesh' }, materials: { type: 'str[]', doc: 'material instances of the mesh' }, skeleton: { type: 'str' }, physics: { type: 'str', doc: 'PhysicsAsset' }, bonamik: { type: 'str', doc: 'SQEX_BonamikAsset (chain)' }, anim_class: { type: 'str', doc: 'anim blueprint' }, anim_sets: { type: 'str[]', doc: 'MyAnimSets of the anim blueprint' }, effects: { type: 'str[]', doc: 'particle systems / attach data' }, sounds: { type: 'str[]', doc: 'sound sets' }, projectiles: { type: 'str[]', doc: 'projectile blueprints it names' }, other: { type: 'str[]', doc: 'other references' } },
    },
    // (weapons) kh3-party-weapons: written by tools/kh3_weapon_sheets.js (documentation/KH3_WEAPON_DATA.md)
    'kh3-party-weapons': {
      file: 'kh3_party_weapons.rs', row: 'Kh3PartyWeaponRow', table: 'KH3_PARTY_WEAPONS', key: 'id', key_prefix: 'KH3_PW',
      doc: 'One of Donald\'s staves or Goofy\'s shields: stats, abilities, prices, shop levels, assets and how KH3 gives it.',
      columns: { id: { type: 'str', doc: 'm_KeyName' }, item_id: { type: 'u32' }, row: { type: 'str' }, weapon_def: { type: 'str', doc: 'ETresItemDefWeapon' }, chara: { type: 'str' }, name: { type: 'str' }, help: { type: 'str' }, strength: { type: 'i32', doc: 'm_AttackPlus' }, magic: { type: 'i32', doc: 'm_MagicPlus' }, defense: { type: 'i32', doc: 'm_DefensePlus' }, abilities: { type: 'str[]', doc: 'm_AppendAbility' }, ability_names: { type: 'str[]' }, buy: { type: 'u32', doc: 'm_BuyPrice' }, sell: { type: 'u32', doc: 'm_SellPrice' }, can_sell: { type: 'bool' }, max_num: { type: 'u32' }, shop_level: { type: 'u32', doc: 'm_ShopLevel (0 = not sold)' }, gumi_level: { type: 'u32', doc: 'm_GumiShopLevel' }, icon: { type: 'str' }, equip: { type: 'str', doc: 'm_EquipSubclass' }, weapon_bps: { type: 'str[]' }, mesh: { type: 'str' }, how: { type: 'str[]' }, where: { type: 'str[]' } },
    },
    // (weapons) kh3-synthesis: written by tools/kh3_weapon_sheets.js (documentation/KH3_WEAPON_DATA.md)
    'kh3-synthesis': {
      file: 'kh3_synthesis.rs', row: 'Kh3SynthesisRow', table: 'KH3_SYNTHESIS', key: 'id', key_prefix: 'KH3_SYN',
      doc: 'One recipe of the Moogle Workshop (TresItemSynthesisData): the item, its unlock condition and materials.',
      columns: { id: { type: 'str', doc: 'IS_n' }, flag_index: { type: 'u32', doc: 'm_FlagIndex (save bits: made / seen / count)' }, reward: { type: 'str', doc: 'm_RewardItem' }, reward_name: { type: 'str' }, reward_kind: { type: 'str', doc: 'item category' }, reward_item_id: { type: 'i32' }, reward_max: { type: 'u32', doc: 'm_MaxNum (carry cap)' }, condition: { type: 'str', doc: 'ETresItemSynthesisCondition' }, condition_row: { type: 'str', doc: 'IC_n / IM_n or -' }, materials: { type: 'str[]' }, counts: { type: 'i32[]' }, material_names: { type: 'str[]' } },
    },
    // (weapons) kh3-collect-goals: written by tools/kh3_weapon_sheets.js (documentation/KH3_WEAPON_DATA.md)
    'kh3-collect-goals': {
      file: 'kh3_collect_goals.rs', row: 'Kh3CollectGoalRow', table: 'KH3_COLLECT_GOALS', key: 'id', key_prefix: 'KH3_GOAL',
      doc: 'One Collector\'s Goal of the Moogle Workshop (TresItemCollectData): the condition, its reward, and what it unlocks.',
      columns: { id: { type: 'str', doc: 'IC_n' }, flag_index: { type: 'u32' }, condition: { type: 'str', doc: 'ETresItemCollectCondition' }, n: { type: 'u32', doc: 'm_ConditionN' }, x: { type: 'str', doc: 'm_ConditionX as MAT_ITEMnn or -' }, y: { type: 'str', doc: 'm_ConditionY' }, rank: { type: 'str', doc: 'm_ConditionR (ETresItemRank)' }, reward_kind: { type: 'str' }, reward_item: { type: 'str' }, reward_name: { type: 'str' }, text_id: { type: 'str', doc: 'the goal\'s list text' }, text: { type: 'str', doc: 'English template ({0} = n / material)' }, unlocks_recipes: { type: 'str[]', doc: 'recipes with this condition row' }, unlocks_buy: { type: 'str[]', doc: 'items whose m_UnlockFlagIndex = this flag index (ENABLE_BUY)' } },
    },
    // (weapons) kh3-photo-missions: written by tools/kh3_weapon_sheets.js (documentation/KH3_WEAPON_DATA.md)
    'kh3-photo-missions': {
      file: 'kh3_photo_missions.rs', row: 'Kh3PhotoMissionRow', table: 'KH3_PHOTO_MISSIONS', key: 'id', key_prefix: 'KH3_PHOTO',
      doc: 'One Photo Mission of the Moogle Workshop (TresItemMotifData): its story gate and the recipes it unlocks.',
      columns: { id: { type: 'str', doc: 'IM_n' }, flag_index: { type: 'u32' }, name: { type: 'str' }, name_id: { type: 'str' }, help: { type: 'str' }, help_id: { type: 'str' }, flag_name: { type: 'str', doc: 'm_GameFlagName (listed when this flag >= the label)' }, flag_label: { type: 'str', doc: 'm_GameFlagLabel' }, seven_princess: { type: 'bool', doc: 'm_bSevenPrincessFlag (all 12 teammates in the album)' }, prev: { type: 'str', doc: 'm_PrevMotifName or -' }, ui_priority: { type: 'i32' }, unlocks_recipes: { type: 'str[]' } },
    },
    // (weapons) kh3-materials: written by tools/kh3_weapon_sheets.js (documentation/KH3_WEAPON_DATA.md)
    'kh3-materials': {
      file: 'kh3_materials.rs', row: 'Kh3MaterialRow', table: 'KH3_MATERIALS', key: 'id', key_prefix: 'KH3_MAT',
      doc: 'One of KH3\'s 60 synthesis materials: grade, prices, when it becomes buyable, which enemies drop it, what uses it.',
      columns: { id: { type: 'str', doc: 'MAT_ITEMnn' }, item_id: { type: 'u32', doc: '0x700 + nn' }, row: { type: 'str' }, rank: { type: 'str', doc: 'ETresItemRank (RANK03 shard .. RANK00 crystal / ores)' }, grade: { type: 'u32', doc: '1 shard, 2 stone, 3 gem, 4 crystal / ore' }, name: { type: 'str' }, help: { type: 'str' }, icon: { type: 'str' }, buy: { type: 'u32', doc: 'm_BuyPrice (once buyable)' }, sell: { type: 'u32', doc: 'm_SellPrice' }, can_sell: { type: 'bool' }, max_num: { type: 'u32', doc: 'carry cap (the Workshop stock holds up to 999)' }, shop_level: { type: 'u32' }, unlock_flag: { type: 'i32', doc: 'm_UnlockFlagIndex (-1 = never buyable)' }, buy_goal: { type: 'str', doc: 'the Collector\'s Goal that makes it buyable, or -' }, drops: { type: 'str[]', doc: 'enemy (name) variant: rate %' }, chests: { type: 'str[]', doc: 'treasure chests holding it (TresTreasureData*)' }, goal_rewards: { type: 'str[]', doc: 'Collector\'s Goals that give it (GET_ITEM)' }, made_by: { type: 'str[]', doc: 'recipes that produce it' }, milestones: { type: 'str[]', doc: 'Lucky Emblem milestone rewards (TresLuckyMarkMilestoneRewardData)' }, recipes: { type: 'str[]', doc: 'recipes that use it' }, forge_keyblades: { type: 'u32', doc: 'Keyblades whose Forge uses it' } },
    },
    // (weapons) kh3-material-drops: written by tools/kh3_weapon_sheets.js (documentation/KH3_WEAPON_DATA.md)
    'kh3-material-drops': {
      file: 'kh3_material_drops.rs', row: 'Kh3DropRow', table: 'KH3_DROPS', key: 'id', key_prefix: 'KH3_DROP',
      doc: 'One item slot of a KH3 enemy\'s base data table (m_DropItemID / m_DropItemRate): what it drops and how often.',
      columns: { id: { type: 'str', doc: '<table>.<row>.<slot>' }, table: { type: 'str', doc: 'DataTable/Enemy/Base table' }, variant: { type: 'str', doc: 'row name' }, enemy: { type: 'str', doc: 'enemy code' }, enemy_name: { type: 'str', doc: 'English (enemy dictionary) or -' }, slot: { type: 'u32', doc: '1..3' }, drop_id: { type: 'str', doc: 'ETresDropItemID' }, drop_value: { type: 'i32', doc: 'its value' }, material: { type: 'str', doc: 'MAT_ITEMnn or -' }, material_name: { type: 'str' }, rate: { type: 'f32', doc: 'percent per kill' } },
    },
    // (weapons) kh3-shop-stock: written by tools/kh3_weapon_sheets.js (documentation/KH3_WEAPON_DATA.md)
    'kh3-shop-stock': {
      file: 'kh3_shop_stock.rs', row: 'Kh3ShopStockRow', table: 'KH3_SHOP_STOCK', key: 'id', key_prefix: 'KH3_STOCK',
      doc: 'One item a KH3 shop can sell: Moogle shop level, Gummi shop level, M.O.G. Card, Collector\'s Goal unlock, price, sold-out rule.',
      columns: { id: { type: 'str', doc: 'm_KeyName' }, item_id: { type: 'u32' }, kind: { type: 'str', doc: 'item category' }, name: { type: 'str' }, buy: { type: 'u32' }, sell: { type: 'u32' }, can_sell: { type: 'bool' }, shop_level: { type: 'u32', doc: 'Moogle shop level it appears at (0 = not by level)' }, gumi_level: { type: 'u32', doc: 'Gummi shop level (0 = not there)' }, mog_card: { type: 'bool', doc: 'stocked once the M.O.G. Card is held' }, unlock_flag: { type: 'i32', doc: 'm_UnlockFlagIndex' }, unlock_goal: { type: 'str', doc: 'Collector\'s Goal that unlocks it' }, sold_out: { type: 'bool', doc: 'm_bCanbeSoldOut (hidden once held count >= max)' }, max_num: { type: 'u32' } },
    },
    // (weapons) kh3-weapon-sources: written by tools/kh3_weapon_sheets.js (documentation/KH3_WEAPON_DATA.md)
    'kh3-weapon-sources': {
      file: 'kh3_weapon_sources.rs', row: 'Kh3WeaponSourceRow', table: 'KH3_WEAPON_SOURCES', key: 'id', key_prefix: 'KH3_WSRC',
      doc: 'One way KH3 gives a weapon to the player: start, story event, chest, shop, synthesis, minigame, entitlement.',
      columns: { id: { type: 'str', doc: '<item>.<n>' }, item: { type: 'str', doc: 'weapon key' }, how: { type: 'str', doc: 'start | event | event+moogle | chest | shop | synthesis | minigame | entitlement | event (+entitlement)' }, where: { type: 'str', doc: 'world / story point / recipe' }, source: { type: 'str', doc: 'asset + export / bytecode offset, table row, or function address' } },
    },
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
    // (GF-STATS) each friend's own Strength / Magic / Defense per level, and the damage-taken reference curve (crates/kh-party/src/balance.rs; built by tools/party_balance_sheet.js)
    'party-balance-levels': {
      file: 'party_balance_levels.rs', row: 'BalanceLevelRow', table: 'PARTY_BALANCE_LEVELS',
      doc: 'A friend\'s own Strength / Magic / Defense before his weapon and armour at each level 1..99 (port design, the user 2026-11-01: independent of Sora), and the rows of chara `reference`: the Defense and HP a hit on any friend is measured against (build/sora_tmp/party/GOOFY/PLAN.md R8).',
      columns: { id: { type: 'str' }, chara: { type: 'str', doc: 'n_ex001 | n_ex002 | reference' }, level: { type: 'u32' }, strength: { type: 'i32' }, magic: { type: 'i32' }, defense: { type: 'i32' }, hp: { type: 'i32', doc: 'reference rows: the HP a hit\'s share is taken of; 0 on a friend\'s row' }, source: { type: 'str' } },
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
        interval: { type: 'f32' }, map_hit: { type: 'str' }, particle: { type: 'str' }, particle_loc: { type: 'f32[]' }, hit_effect: { type: 'str' }, hit_se: { type: 'str' }, hit_offset: { type: 'f32' }, fire_effect: { type: 'str' }, fire_se: { type: 'str' }, spawn: { type: 'str' }, speed: { type: 'f32' } /* (AUDIT-C) */ },
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
    // (weapons P2, 2026-11-03) Sora's 17 Keyblades, their Forge levels and weapon fx / sound / anim rows (tools/sora_keyblade_sheet.js)
    'sora-keyblades': {
      file: 'sora_keyblades.rs', row: 'SoraKeybladeRow', table: 'SORA_KEYBLADES',
      doc: 'One row of sheets/port/sora-keyblades: a Keyblade (kind keyblade), one of its Forge levels (level), or a weapon effect / sound / anim field of it (fx / sound / anim).',
      columns: {
        id: { type: 'str', doc: 'kb.<KEY>, kb.<KEY>.lv<n>, kb.<KEY>.fx|sound|anim.<name>' },
        kind: { type: 'str', doc: 'keyblade / level / fx / sound / anim' },
        keyblade: { type: 'str', doc: 'the item key WEP_KEYBLADE_SO_nn' },
        level: { type: 'u32', doc: 'Forge level 1..10 (level rows), else 0' },
        item_id: { type: 'u32', doc: 'item id 0x300 + index (keyblade rows)' },
        row: { type: 'str', doc: 'TresItemData I03nnn / Forge IW_n' },
        name: { type: 'str' },
        name_id: { type: 'str' },
        help_id: { type: 'str' },
        icon: { type: 'str' },
        forge_icon: { type: 'str' },
        mesh: { type: 'str' },
        blueprint: { type: 'str' },
        equip: { type: 'str' },
        projectile_set: { type: 'str' },
        strength: { type: 'i32', doc: 'keyblade: base m_AttackPlus; level: this level\'s own +Strength' },
        magic: { type: 'i32', doc: 'keyblade: base m_MagicPlus; level: this level\'s own +Magic' },
        abilities: { type: 'str[]', doc: 'm_AppendAbility (ETresAbilityKind game names)' },
        shotlocks: { type: 'str[]', doc: 'm_ShootflowAbilitys (ETresTextAbilityKind)' },
        forms: { type: 'str[]', doc: 'CHANGE_* formchange commands' },
        wear_forms: { type: 'str[]', doc: 'm_WearForms' },
        init_level: { type: 'u32', doc: 'the level it has when obtained (m_bInitAchieved)' },
        max_level: { type: 'u32' },
        init_achieved: { type: 'bool', doc: 'level rows: m_bInitAchieved' },
        ability: { type: 'str', doc: 'level rows: the added ability or -' },
        materials: { type: 'str[]', doc: 'level rows: MAT_ITEMnn' },
        counts: { type: 'i32[]', doc: 'level rows: material counts' },
        asset: { type: 'str', doc: 'fx / sound / anim rows: the asset path or -' },
        number: { type: 'f32', doc: 'fx rows: a time / scale value, or a flag 1 / 0; hit rows: m_fHitEffectRandomOffsetRadius' },
        // (P2 hitfx) the hit rows (one m_HitEffects entry of a weapon / the pawn component)
        group: { type: 'str', doc: 'hit rows: m_GrpName' },
        effects: { type: 'str[]', doc: 'hit rows: m_PawnHitEffectPerAttr[0..7] (- = none)' },
        direct: { type: 'str', doc: 'hit rows: m_DirectHitEffect' },
        map: { type: 'str', doc: 'hit rows: m_MapHitEffect' },
        change_map: { type: 'bool', doc: 'hit rows: m_bChangeMapHit' },
        sounds: { type: 'str[]', doc: 'hit rows: m_PawnHitSEAsset, m_PawnHitSEAssetPerAttr[0..7], m_MapHitSEAsset' },
        change_se_map: { type: 'bool', doc: 'hit rows: m_bChangeSEMapHit' },
        decal: { type: 'bool', doc: 'hit rows: m_bEnableDecal' },
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
