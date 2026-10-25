// Writes sheets/port/sora-bossfinish.json: KH3's boss-finish presentation (docs/sora-bossfinish-spec.md): the
// slow motion, the three camera cuts, the screen blur, the white-out, the sound and what follows, as the boss
// maps' level blueprints script it (128 packages; the reference graph is tt_01_camera_ms060, the first boss),
// plus the rows that are choices of the port (port.* / battle.*). Registers the sheet in
// tools/lib/sheet_defs.js once. Idempotent.
// Then: node tools/extract.js; node tools/preflight.js --summary; node tools/gen_port.js
// Evidence scripts: build/sora_tmp/er/er22/research/{kdump,survey}.js (bossdefeat_survey.json = all 127 graphs)
'use strict';
const fs = require('fs');
const path = require('path');
const root = path.resolve(__dirname, '..');

const A = 'confirmed'; // read from an asset (a level blueprint's bytecode, a class default object, the sound configuration)
const C = 'inferred'; // decompiled C only
const G = 'guess'; // not read: the port's reading
const LV = 'assets_converted/packages/KINGDOM HEARTS III/Content/Levels/';
const REF = LV + 'tt/tt_01/umap/tt_01_camera_ms060.umap ExecuteUbergraph (event tt_01_enemy.._REvt_BattleFinish)';
const SURVEY = 'build/sora_tmp/er/er22/research/bossdefeat_survey.json';
const PORT = 'PORT CHOICE (2026-10-13; not game data)';

const rows = [
  // ---- the slow motion: UTresGameBlueprintLibrary::SetGlobalGameSpeed(CH_EVENT, rate, interp)
  ['speed.channel', 2, 'ETresObjTypeProcChannel', REF + ': SetGlobalGameSpeed(self, 2, 0.05, 0.1)', 'CH_EVENT; every one of the 127 graphs uses channel 2', '0x14222C9D0', A, '-'],
  ['speed.rate', 0.05, 'x real time', REF + ': InNewTimeDilation 0.05', '112 of 127 graphs (0.08 on 15, 0.06 on 4, 0.03 / 0.13 / 0.3 on one each: ' + SURVEY + '); GameState vt+0x880 0x144119CE0 stores it as the channel\'s target', '0x144119CE0', A, 'the WHOLE world: the tick 0x14411A230 writes min(channels < 1) x max(channels > 1) into WorldSettings.SqEX_CustomTimeDilation (+0x634)'],
  ['speed.interp', 0.1, 's (real)', REF + ': InInterpTime 0.1', '112 of 127 (0.2 on 19); tick 0x14411A230: current += (target - current) / time_left x dt, time_left -= dt; its dt is the global double 0x14871E340 handed over by 0x144119710, taken as the application\'s undilated frame delta', '0x14411A230', C, 'linear'],
  ['speed.min', 0.01, 'x', 'GameState vt+0x880 0x144119CE0: a rate below 0.01 is stored as 0.01', '-', '0x144119CE0', C, '-'],
  ['speed.reset_interp', 0, 's', REF + ': ResetGlobalGameSpeed(self, 2, 0.0) when the white-out has finished', 'all 127 graphs; 0x140E62B00 = the same call with rate 1.0', '0x14222A250', A, 'at once'],
  ['cycle', 0.0166667, 's (real)', REF + ': TresWaitCycle(1) before each camera and before the white-out\'s delay', 'one tick of the level script; a 60 Hz frame is taken', '0x14223D9D0', C, 'at the frame the kill arrives the rate is still close to 1'],
  // ---- the three cuts: ATresCameraRotTarget "Battle_End", priority 210, on a TresNoteMovable put at a bone
  ['cut.count', 3, 'count', REF, '112 graphs have the delays 0.05 and 0.1 (three cuts); the camera numbers differ per boss', '-', A, '-'],
  ['cut.blend', 0, 's', REF + ': StartTresCamera(false, ViewTargetTransitionParams{BlendTime 0, ...})', 'every cut is a hard cut (112 graphs)', '-', A, '-'],
  ['cut.priority', 210, '-', REF + ': SetPriority(210)', '-', '-', A, 'above the field camera and the lock-on camera'],
  ['cut.move_time', 0.5, 's (game)', REF + ': SetBaseRotation(rot, 0.5), SetDistance(distance, 0.5)', '0x14060A640 / 0x1405EFB60 start the camera\'s interpolators with this time; the camera ticks on world time, so 0.5 s takes 10 s of real time at 0.05: a cut shows the first tenth of the move', '0x14060A640', C, '-'],
  ['cut.move_curve', 5, 'interpolator type', '0x14060A640 / 0x1405EFB60 pass type 5 to the interpolator (0x141194110 / 0x141194190)', 'type 5 = 1 - (1 - t)^2 (kh_camera::lockon::interp_curve, dispatch 0x1411A0E70 read for the camera work)', '0x14060A640', C, '-'],
  ['cut.bone', 't_camera', 'bone', REF + ': GetBoneTranslationByName(<pawn>.MyMesh, "t_camera", 0); the note actor is put there with the pawn\'s rotation', 'most graphs; some bosses name a bone of their own (kubi, mune)', '-', A, 'a host without that bone gives a point of the body'],
  ['cut.1.subject', 'enemy', 'enemy | player', REF, 'the defeated enemy the event carries', '-', A, '-'],
  ['cut.1.delay', 0, 's (game)', REF + ': after TresWaitCycle(1)', '-', '-', A, '-'],
  ['cut.1.distance', [200, 300], 'cm', REF + ': SetTresCameraRotTargetParam(200, ...), SetDistance(300, 0.5)', 'start -> end of the move', '0x14060B9C0', A, '-'],
  ['cut.1.rotation', [5, -30, 5], 'pitch yaw roll', REF + ': SetTresCameraRotTargetParam(200, R(5,-30,5), V(0,0,0), V(0,0,50))', 'm_Rotation (+0x828): added to (0, target actor yaw, 0) (camera spec 2.6: base rotation)', '0x14060B9C0', A, 'yaw relative to the subject\'s facing'],
  ['cut.1.rotation_end', [10, -100, 5], 'pitch yaw roll', REF + ': SetBaseRotation(R(10,-100,5), 0.5)', '-', '0x14060A640', A, '-'],
  ['cut.1.local_offset', [0, 0, 50], 'cm', REF + ': TargetLocalOffset V(0,0,50) (camera+0x684)', 'rotated by the target actor (camera spec 2.2)', '0x14060B9C0', A, 'TargetOffset (camera+0x678) is 0'],
  ['cut.1.collision', 0, 'bool', REF + ': EnableCollision(false) on the first camera', '-', '-', A, 'see port.camera_collision'],
  ['cut.2.subject', 'player', 'enemy | player', REF + ': the note actor is moved to GetLocalTresPlayerPawnBase().MyMesh t_camera with the player\'s rotation', '-', '-', A, '-'],
  ['cut.2.delay', 0.05, 's (game)', REF + ': TresWaitCycle(1) then TresDelay(0.05)', 'TresDelay 0x140E70A70 calls the stock Delay 0x144187DD0: world (dilated) time; 1 s of real time at 0.05', '0x140E70A70', A, '-'],
  ['cut.2.distance', [200, 300], 'cm', REF, '-', '-', A, '-'],
  ['cut.2.rotation', [-5, -25, 0], 'pitch yaw roll', REF + ': R(-5,-25,0)', '-', '-', A, '-'],
  ['cut.2.rotation_end', [-10, -100, 0], 'pitch yaw roll', REF + ': SetBaseRotation(R(-10,-100,0), 0.5)', '-', '-', A, '-'],
  ['cut.2.local_offset', [0, -50, 0], 'cm', REF + ': V(0,-50,0)', '-', '-', A, 'UE axes: -Y = to his left'],
  ['cut.2.collision', 1, 'bool', REF + ': no EnableCollision call on the second and third camera', 'the class default applies (not read)', '-', G, '-'],
  ['cut.3.subject', 'enemy', 'enemy | player', REF, '-', '-', A, '-'],
  ['cut.3.delay', 0.1, 's (game)', REF + ': TresWaitCycle(1) then TresDelay(0.1)', '2 s of real time at 0.05', '-', A, '-'],
  ['cut.3.distance', [600, 450], 'cm', REF + ': 600, SetDistance(450, 0.5)', 'the wide shot moves in', '-', A, '-'],
  ['cut.3.rotation', [-20, -20, 0], 'pitch yaw roll', REF + ': R(-20,-20,0)', '-', '-', A, '-'],
  ['cut.3.rotation_end', [-30, -80, 0], 'pitch yaw roll', REF + ': SetBaseRotation(R(-30,-80,0), 0.5)', '-', '-', A, '-'],
  ['cut.3.local_offset', [0, 0, 50], 'cm', REF + ': V(0,0,50)', '-', '-', A, '-'],
  ['cut.3.collision', 1, 'bool', REF, 'as cut.2.collision', '-', G, '-'],
  // ---- the shake: /Game/Camera/shake/no_speed_kokizami_1s
  ['shake.scale', 1, 'x', REF + ': ClientPlayCameraShake(no_speed_kokizami_1s_C, 1.0, ...) with the first camera', '102 play calls with scale 1, 25 with 3 (' + SURVEY + ')', '-', A, 'StopAllCameraShake first'],
  ['shake.real_time', 1, 'bool', 'assets_converted/packages/KINGDOM HEARTS III/Content/Camera/shake/no_speed_kokizami_1s.uasset Default__no_speed_kokizami_1s_C: m_bRealDeltaTime true', 'it does not slow with the world ("no_speed")', '-', A, '-'],
  ['shake.duration', 1, 's', 'same asset: OscillationDuration 1', '-', '-', A, '-'],
  ['shake.blend_in', 0, 's', 'same asset: OscillationBlendInTime 0', '-', '-', A, '-'],
  ['shake.blend_out', 3, 's', 'same asset: OscillationBlendOutTime 3', 'stock UCameraShake: the blend out begins when the remaining time falls below it, so the whole second is a fade', '-', A, '-'],
  ['shake.pitch', [3, 50], 'degrees, frequency', 'same asset: RotOscillation.Pitch Amplitude 3, Frequency 50', '-', '-', A, 'm_AmplitudeRandScaleMin 0.5 / Max 1 (a Square Enix addition; not evaluated by kh_camera::shake)'],
  ['shake.yaw', [1, 50], 'degrees, frequency', 'same asset: RotOscillation.Yaw Amplitude 1, Frequency 50', '-', '-', A, '-'],
  // ---- the screen blur: UTresSceneManager::SetScreenBlur
  ['blur.transition', 0.001, 's', REF + ': SetScreenBlur(0.001, params, CH_EVENT, priority 3, false)', 'all 127 graphs', '0x142A266D0', A, '-'],
  ['blur.intensity', 0.5, '-', REF + ': TresScreenBlurParams.fIntensity 0.5', 'all 127 graphs (plus per-boss extras)', '-', A, 'the blur\'s shader was not read'],
  ['blur.centre', [0.5, 0.5], 'screen', REF + ': vPos (0.5, 0.5)', '-', '-', A, '-'],
  ['blur.offset', [0.02, 0.02], 'screen', REF + ': vOffset (0.02, 0.02)', '-', '-', A, '-'],
  ['blur.range', 2, '-', REF + ': fRange 2', '-', '-', A, '-'],
  ['blur.hole', 0, '-', REF + ': fHoleSize 0', '-', '-', A, '-'],
  ['blur.attenuation_inside', 0.2, '-', REF + ': fAttenuationDistanceInside 0.2 (Outside 0)', '-', '-', A, '-'],
  ['blur.end_transition', 0.01, 's', REF + ': SetScreenBlurDefault(0.01, CH_EVENT, 3, true) after the hold', '-', '-', A, '-'],
  // ---- the white-out
  ['white.delay', 0.15, 's (game)', REF + ': TresWaitCycle(1) then Delay(0.15)', 'all 127 graphs; 3 s of real time at 0.05', '0x144A1C040', A, '-'],
  ['white.time', 0.08, 's (game)', REF + ': WaitBPTresStartWhiteOut(layer 0, InFadeTime 0.08, InApplyGameSpeed true, InGamePause false)', '103 of 127 (0.6 without the game speed on 15); 0x140E81E30 starts a fade-out to the colour constant at 0x14871D7E0 on layer 0 and waits for it', '0x140E81E30', A, 'that the game-speed flag stretches it to 0.08 / 0.05 = 1.6 s is the reading of its name; the fade\'s curve was not read (linear taken)'],
  ['white.colour', [1, 1, 1], 'rgb', 'the function\'s name (WhiteOut); the colour constant at 0x14871D7E0 was not dumped', '-', '0x140E81E30', G, '-'],
  ['after.hold', 1, 's', REF + ': TresDelay(1.0) after ResetGlobalGameSpeed, EndAllBPTresCamera and AllPrizeCollection', '112 graphs; under the white: the enemy is destroyed after it', '-', A, '-'],
  ['after.fade_in', 0.7, 's', LV + 'bt/bt_01/umap/bt_01_camera_ms010.umap: BPTresStartFadeIn(colour 0, layer 0, 0.7, true); CancelFade(0.7) in the reference graph', '95 graphs; the others go into the map\'s cutscene', '0x14221F260', A, '-'],
  ['after.prizes', 1, 'bool', REF + ': AllPrizeCollection(self, player) right after the cameras end', 'every prize on the field flies to the player (all 127 graphs)', '0x14221E440', A, '-'],
  // ---- sound
  ['sound.static_se', 22, 'ETresStaticSE', LV + 'bt/bt_01/umap/bt_01_camera_ms010.umap: SpawnSound2D(se00000_047) at the very start of the event', 'se00000_047 in 53 of the 128 packages, se00000_048 (static SE 23) in 46; bank SE/common/00000 sound 47: 7.9 s, category SE_system, UI sound, ignores the engine pause; build/sora_pack/audio/tables.json static_se id 22', '-', A, '2D, volume 1, pitch 1: not slowed'],
  ['sound.static_se_alt', 23, 'ETresStaticSE', REF + ': PlaySound2D(se00000_048, 1, 1)', '46 packages; 7.8 s, same category', '-', A, '-'],
  ['sound.mode', 4, 'ETresSoundModes', REF + ': TresSoundStatics.TresSoundMode_Start(4) = BOSS_DEAD; _End(4) after the hold', '0x141515D60: mode 4 sends the sound event STATE_MISSION_RESULT_START (0x141515C90: _END)', '0x141515D60', C, 'the event silences every SE and voice category (volume 0 in 0.05 - 2 s, layer 4) except SE_system; the end event stops them (0.5 s) and restores the volumes over 1 s (build/sora_pack/audio/tables.json events 6 / 7)'],
  ['sound.bgm_fade', 4, 's', REF + ': playing BGM slot SetSuspendFadeOutTime(4), SetStopFadeOutTime(4); slot "Event" SetSilent(), Play()', '111 of 127 (0.6 on 15)', '-', A, 'the battle music fades out over 4 s; what plays next is the map\'s business'],
  // ---- the rest of the script
  ['hud.hide', 0, 'bool', REF + ': TresStartCinematicMode(..., bHideCockpit false, ...)', 'the call does not hide the HUD (103 graphs: flags 0010000101011); whether the HUD hides for another reason was not read', '0x14222F940', A, '-'],
  ['input.lock', 1, 'bool', REF + ': TresStartCinematicMode(bDisableMove true, bDisableCameraMove true, bDisableIKinema true); TresSetDisableGamePause(true)', 'from the kill until TresEndCinematicMode after the hold', '-', A, '-'],
  // ---- choices of the port
  ['port.watchdog', 10, 's (real)', PORT, 'a sequence older than this is ended whatever its state (the KH3 timeline is 6.7 s)', '-', A, '-'],
  ['port.camera_collision', 1, 'bool', PORT, 'the port keeps every cut out of the level (one sphere sweep from the look point); KH3 switches the first camera\'s collision off', '-', A, '-'],
  ['port.probe_radius', 20, 'cm', PORT, 'the field camera\'s probe radius (kh_camera::shot)', '-', A, '-'],
  ['battle.rate', 0.3, 'x real time', PORT, 'THE LAST ENEMY OF AN ORDINARY FIGHT: KH3 HAS NO SUCH EFFECT (no SetGlobalGameSpeed in any ordinary encounter script: 143 packages call it, 128 boss finishes + 15 events; no native caller found). A host option, off by default', '-', A, '-'],
  ['battle.interp', 0.1, 's (real)', PORT, '-', '-', A, '-'],
  ['battle.hold', 0.6, 's (real)', PORT, 'seconds at battle.rate', '-', A, '-'],
  ['battle.out', 0.25, 's (real)', PORT, 'ramp back to 1', '-', A, '-'],
];

const columns = ['id', 'value', 'unit', 'source', 'evidence', 'ref_addr', 'ref_conf', 'note'];
const out = '{"sheet":"sora-bossfinish","group":"port","columns":' + JSON.stringify(columns) + ',"rows":[\n' + rows.map((r) => JSON.stringify(r)).join(',\n') + '\n]}\n';
fs.writeFileSync(path.join(root, 'sheets', 'port', 'sora-bossfinish.json'), out);

const defs = path.join(root, 'tools', 'lib', 'sheet_defs.js');
let s = fs.readFileSync(defs, 'utf8');
if (!s.includes("'sora-bossfinish'")) {
  const anchor = "  'sora-er-fx': {";
  const at = s.indexOf(anchor);
  if (at < 0) throw new Error('sheet_defs.js: anchor not found');
  const entry =
    "  // (boss finish) KH3's boss-finish presentation: slow motion, camera cuts, blur, white-out, sound. Written by tools/sora_bossfinish_sheet.js, generated into build/gen/rust/sora_bossfinish.rs (crates/kh-core combat/finish.rs)\n" +
    "  'sora-bossfinish': { group: 'port', file: 'sora-bossfinish.json', by: 'tools/sora_bossfinish_sheet.js', from: 'the boss maps\\x27 level blueprints (128 packages call TresStartBossDefeatEffect; reference graph tt_01_camera_ms060), the camera shake asset, the sound configuration and the natives they call (docs/sora-bossfinish-spec.md); rows port.* and battle.* are choices of the port, not game data', external: true, cols: [\n" +
    "    ['id', K, 'str', '-', 'speed.* = the global game speed; cut.* = the three camera cuts; shake.* / blur.* / white.* / after.* / sound.* = the rest of the script; port.* / battle.* = the port'], ['value', V, 'json', '-', 'a number, a list of numbers or a word; null = unchecked'],\n" +
    "    ['unit', V, 'str', '-', '-'], ['source', V, 'str', '-', 'where the value was read, or who chose it'],\n" +
    "    ['evidence', V, 'str', '-', 'how the game uses it', true], ['ref_addr', V, 'str', '-', 'the function that reads it; \"-\": data or a choice'],\n" +
    "    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = read from data, an immediate, or an authored choice; inferred = decompiled C only; guess = not read'],\n" +
    "    ['note', V, 'str', '-', '-']] },\n";
  s = s.slice(0, at) + entry + s.slice(at);
  fs.writeFileSync(defs, s);
  console.log('registered sora-bossfinish in tools/lib/sheet_defs.js');
}
console.log('sheets/port/sora-bossfinish.json: ' + rows.length + ' rows');
