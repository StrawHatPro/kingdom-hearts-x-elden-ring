# Sora: the game-over sequence (specification, 2026-10-12)

User decision: "instead of having a 'You Died' screen it should be KH3's game over screen with Sora in a dark
void with his heart out". This document says what KH3 does, where each piece is in the data, what is built and
what is left. Evidence marks: **[A]** read from an asset or the executable's data, **[C]** decompiled C only
(not checked at instruction level), **[D]** inferred from names / labels, **[L]** LOCAL choice of the port.
Nothing here was verified at instruction level or in the running game; section 6 lists what only the game settles.

## 1. The pieces

| Piece | Where | Evidence |
|---|---|---|
| Die state `TresPlayerState_Die` (59): thrown from the causer, blow clips, then waits | ported: `crates/kh-core/src/states/die.rs` (combat strand E 9.8) | [C] |
| Game-over state `TresPlayerState_GameOver` (class id; `m_Camera: TresCameraAnim`) | vtable slots 95 `0x14138F390` (init), 96 `0x141384580` (begin), 97 `0x141390B50` (main), 98 `0x14138C460` (end) | [C] |
| Game-over body clip | `M_0270` (game over), `M_0271` (mission failed); pack: `build/sora_pack/anims/0270.glb`, `0271.glb` | [C] names in `0x141390B50`; [A] pack |
| Heart effect | `TresPlayerPawnLoad_ex001.m_PlayerGameOverHeartEffect` = `/Game/Effects/system/system_com/par/vs_so_dth_ui00_2` | [A] sheet `sora-effects` row `load.player.GameOverHeartEffect` |
| Light effect | `m_PlayerGameOverLightEffect` = `.../vs_so_dth_ui00`; mission failed: `vs_so_misn_ui00` | [A] same sheet |
| Common (non-player-specific) heart / light | `TresGameModeLoad.m_CmnGameOverHeartEffect` = `vs_com_dth_ui00`, `m_CmnGameOverLightEffect` = `vs_com_dth_ui01`, both at the player + (0, 0, 120) cm; mission failed light at (50, 0, 150) | [A] sheet rows `load.gamemode.GameOver*` |
| Camera | `m_PlayerGameOverCameraAnim` = `/Game/Camera/sora/hanyo/game_over_01` (mission failed: `game_over_mission_fail`); the die camera `/Game/Camera/aqua/hanyo/death_camera01` | [A] |
| Screen look | `TresGameModeLoadAsset.m_GameOverKHS` / `m_GameOverKHSBP` (a `TresKHShaderController`: the dark void is a shader-controller preset over the scene, not a level) | [A] property names only: the preset's values were NOT read |
| Loop sound | `TresGameInstance.m_GameOverSE` (audio component); static SE 21 (`se00000_046`, loop) or 34 (`se00000_068`): sheet `sora-sounds` rows `code.game_over.*` | [A] |
| Menu | clip `gameOverMenu` of the UI movie `Pause` (`UI/Scaleform/13_pause/Pause`), class `UTresUIP_GameOver` (`m_Button[5]`, `m_HintText`), owned by `UTresUIP_Pause.m_GameOver` | [A] movie in `build/sora_pack/ui/movies/Pause.json`; [C] |
| Menu texts | `kh3_menu.gameover.title_0001` "MENU", `menu_0001` "Continue", `0002` "Retry", `0003` "Prepare & Retry", `0004` "Load", `0005` "Game Help", `0006` "Exit", `0007` "Give up"; `kh3_menu.savepoint.menu_0005` "Title Screen" | [A] `build/sora_pack/ui/strings/en/kh3_menu.json` |
| Hints | DataTables `UI/Data/GameOverHintData` (`TresUIData.m_pGameOverHintTable`; per map / enemy), chosen by `TresGameOverHintSelector` | [A] exists; NOT decoded |
| Entry points | `UTresGameBlueprintLibrary::BP_TresStartGameOver(WorldContext, bPlayDeadSE, InGameOverKind)` (`exec` `0x1422244C0` -> `0x140E4B430` -> `0x140EBA930`), `BP_TresSetGameOverMenuType`, `BP_TresStartGameOverLoopSE(InKind)`, `WaitBPTresStartGameOverWhiteOut`; enums `ETresGameOverKind` (4 values), `ETresGameOverMenuType` (4 values) | [C] |

Everything the 2D part needs IS ALREADY IN THE UI PACK (the Pause movie carries `gameOverMenu`, the strings table
`kh3_menu` is packed): `tools/sora_ui_pack.js` needed no change. The 3D part's clips (0270 / 0271) and effects
(`vs_so_dth_ui00`, `vs_so_dth_ui00_2`, `vs_com_dth_ui00`, `vs_com_dth_ui01`, `vs_so_misn_ui00`) are in the Sora
pack and the effect pack already.

## 2. The game-over state (3D) [C]

`0x141384580` (begin): on the pawn: bad statuses cleared (`+0x440` component, `-1`), a row of virtual calls that
stop what he was doing (vt+0x1728, 0x16E8, 0x17A0 ...), movement stopped, collision / hit off (vt+0x1108,
0x1138 with 0), the equipment component's vt+0x3C8(0) (the weapon is hidden), his rotation set to the constant
at `0x149B5C7A8` (zero rotator), scale 1.0 (vt+0xDC8), the controller's vt+0x17B8(1) (input off); sub-step 0.

`0x141390B50` (main):
- sub-step 0: bad statuses cleared again; the body clip `M_0270` (or `M_0271` when the state's byte +0x88 is 1 =
  mission failed) is played through the state's animation interface (+0x40, slot +0x18) with start -1.0, mode
  3, rate 1.0; when it reports a length, slot +0x1F0(0). Flag +0x89 bit 1 set. The effect of
  `0x14138E5C0(state, kind, bit 0 of +0x89, &location)` (the heart / light particle system of section 1, by
  kind) is attached to the pawn's root component at the location / rotation constants. The camera anim of the
  load asset (+0xE8 game over, +0x120 mission failed) is created through `0x141382200`, bound to the pawn,
  priority 1000, started with a blend in of type 3 over 2.0 s (`0x1405D9D40`). Without a camera anim asset a
  `TresCameraRotTarget` is spawned instead: distance 200 cm, yaw 180 (it looks at his front), blend 2.0 s.
  Sub-step 1.
- sub-step 1: if flag bit 1: pawn vt+0xC90 and vt+0xC98(8) (the face / mouth state), bit cleared; sub-step 2.
- sub-step 2: nothing (the UI has the flow).

`0x14138C460` (end): the camera anim is stopped with a 2.0 s blend out; four pawn calls restore what begin
switched off (vt+0x1578, 0x1628, 0x1658, 0x1728(1), 0x16D8(0)).

So "Sora in a dark void with his heart out" = the pawn itself playing clip 0270 in place, the level darkened by
the game-over KHS preset, the heart effect `vs_so_dth_ui00_2` and the light `vs_so_dth_ui00` attached to him, the
camera anim `game_over_01`.

## 3. The menu (2D) [C] + [A]

`0x1417035E0(uip, menu_type)` opens it: for the five button slots the id from the table at `0x146B37470`
(5 x int32 per type; 13 = no button, the slot is hidden), the text from the key table at `0x1486F3D50` (filled by
`0x140161560`: ids 0 Continue, 1 Load, 2 Retry, 3 Prepare & Retry, 4 Game Help, 5 Exit, 6-9 the Gummi Ship's,
10 Title Screen, 11 Give up, 12 Prepare & Retry), the hint text into `m_HintText`, the clip's show label, the
focus on the first button. The table, read from the executable:

| type | buttons |
|---|---|
| 0 | Continue, Load, Title Screen |
| 1 | Continue, Game Help, Load, Title Screen |
| 2 | Retry, Prepare & Retry, Continue, Load |
| 3 | Retry, Game Help, Prepare & Retry, Continue, Load |
| 4 | Retry, Load, Exit |
| 5 | Retry, Game Help, Load, Exit |
| 6 | Retry, Continue, Load |
| 7 | Retry, Game Help, Continue, Load |
| 8 | Retry, Load |
| 9, 10 | Gummi Ship |
| 11 | Retry, Prepare & Retry, Give up |

The clip (stage 1920 x 1080): `gameOverMenu` (labels off 1, in 5, loop 15, out 19) with `txTitle` ("MENU", size
70, at the lower left), `titleLine`, `base`, `btnSet.coBtn1..5` (class `CoBtnGOver`: labels normal / disable /
select / disableSelect / decide; `cursor` with off / loop / decide; `btnName.coName.textField`; 50 px apart from
y 801) and `hintText.coText.textField`.

Which type a plain field death uses was not traced (`BP_TresSetGameOverMenuType` is called from blueprints);
type 0 (Continue / Load / Title Screen) is taken [D]: it is the one without Retry.

## 4. What is built

- `crates/kh-ui/src/gameover.rs`: `GameOverScreen` (the clip driven as section 3: texts, hidden slots, cursor
  states, up / down with wrap, decide with the button's and the cursor's `decide` label, the choice 16 frames
  later), `MENU_TYPES`, `GameOverButton`. Buttons a host cannot carry out are shown `disable` and refuse.
- `crates/kh-host/src/hud.rs`: `Hud::gameover_on`; when his state is Die for `GAMEOVER_MENU_DELAY` (180 stage
  frames) the menu opens (type 0), the cockpit is taken off, the pad drives it (D-pad / left stick up and down,
  A), `Hud::take_gameover_choice()` hands the button to the host. Layer "GameOver" of `kh_host::hud_layers`.
- Test app (`kh-app/src/gfx_hud/mod.rs`): on by default (`KH_GAMEOVER=0` switches it off); Load and Title Screen
  are disabled; Continue = he starts again at the level's start, gauges full, status / slots / pouch kept.
  Run: `khgame --level 1 --script "stop 0.5; dummy-auto 1 1 500 0.8; stop 3; dummy-auto 1 1 500 0; stop 3.5; tap down; stop 0.5; tap a; stop 0.5; tap up; stop 0.3; tap a; stop 2"`
  (pictures: `build/sora_tmp/er/hud/go/sheet.png`). Pad: up / down, A.
- Elden Ring (`crates/kh-er/src/hud.rs`, key `gameover.kh = 1` with `hud.kh = 1`; OFF by default): Sora's death
  holds the Tarnished alive (HP topped up as in play) and unhittable (`gameover_holds` in the HP write and the
  invulnerability line of sora.rs); the menu opens after 3 s; any choice = Elden Ring's own death (HP 0) and
  respawn at the last grace. Its YOU DIED text then still shows unless `gameover.banner = 0` works (a write of
  -1 to `FrontEndViewValues::full_screen_message_request_id` for 15 s: UNTESTED). Deaths the game causes itself
  (a fall, deep water) cannot be held: they go straight to Elden Ring's death.

## 5. LOCAL choices (to be replaced when traced)

- The delay from death to the menu: 180 frames (3 s) [L]. The game's own order (Die -> the game instance's flow
  -> GAMEOVER state -> white-out / fade -> menu) and its times were not traced.
- The cursor wraps; cursor / decide sounds are the menus' static SE 0 / 1, the open sound static SE 2 [L].
- No hint line (the hint tables are not decoded).
- Menu type 0 for every death [D].

## 6. NOT built: the 3D part (specified above for a later pass)

1. State `TresPlayerState_GameOver` in `kh-core` (one file + registry row): begin / main / end of section 2;
   entered by the host's flow after Die (a `Sora::game_over(kind)` request), clip 0270.
2. Effects through the existing effect seam: `vs_so_dth_ui00_2` (heart) and `vs_so_dth_ui00` (light) attached to
   his root; both are in the effect pack (rows of sheet `sora-effects`).
3. The dark void: the values of the game-over `TresKHShaderController` preset (`m_GameOverKHS`) have to be read
   from its blueprint; in the test app a KHS preset of `kh-look`, in Elden Ring a full-screen darkening of the
   scene target in our list before Sora (the renderer has no such pass yet) or the game's own fade plates
   (`CSFade.fade_plates`, IMPL_ER4).
4. The camera anim `game_over_01` (the camera spec's CameraAnim player; in Elden Ring through the KH camera of
   `kh-er/src/cam.rs`), or the fallback of the code: 200 cm in front of him, yaw 180, 2 s blend.
5. The loop SE (static SE 21 / 34) through the audio seam, stopped at the choice.
6. What only the game can settle: the order and times between Die, the white-out and the menu; which menu type a
   field death gets; whether the cockpit leaves by its `out` labels or at once; the hint selection.

---

# SECOND PASS (2026-10-13): the sequence completed. THIS PART SUPERSEDES sections 3 (table), 5 and 6 above.

Evidence marks as above. Nothing was verified in the running game or at instruction level. Hand-over with the API:
`build/sora_tmp/er/IMPL_GAMEOVER_UI.md`.

## 7. The sequence, from HP 0 [C unless marked]

1. HP 0: the Die path `0x14143A190` pushes `TresPlayerState_Die` (blow clips, thrown from the causer; the death camera
   `death_camera01`, blend 2.0 s) and IN THE SAME CALL starts the game mode's game over: `0x140EBA4D0(record,
   bPlayDeadSE = 1, kind 0)`. The game mode's state becomes 3; its tick is `0x140EAD210`, sub-step at `+0x214`:
2. step 0: BGM call `0x1458221C0(2.0)`; the loop SE (static SE 21 `se00000_046`; kind != 0: 34 `se00000_068`), kept
   at `+0x318`; kind 0 (normal): the game state's time dilation target 0.01 reached in 0.5 s (`0x144119CE0`) and the
   HUD's fade OUT over 1.0 s in the colour at `0x14871D850` = FFFFFFFF [A] = WHITE; kind 2: 0.1 in 1.0 s, fade 2.0 s
   white; other kinds (1 = mission failed): fade 1.0 s in `0x14871D854` = FF000000 [A] = black. The fade is the HUD's
   (it covers the scene and the cockpit).
3. step 1 (fade complete): pause, the pawn stopped. Step 2: one frame. Step 3: THE WORLD IS HIDDEN: every actor of
   the level (static and skeletal mesh actors, lights, sky light, fog, the KHS controller, volumes, enemies ...) is
   hidden or removed. THE DARK VOID IS THE EMPTY WORLD, not a shader preset.
4. step 4: the pawn's state interface `+0x500(kind, flag)` = `TresPlayerState_GameOver` (section 2: clip `M_0270`
   looped in place, rotation zero, weapon hidden, the effect, the camera anim blended in over 2.0 s with blend
   function 3 = stock ease out); a `KHS_Blueprint_C` (`TresGameModeLoad.m_GameOverKHS` [A]) is spawned at (50000,
   50000, 50000): the character materials' collections are the blueprint's defaults; timer 1.0 s.
5. step 5 (1.0 s later): the menu opens (`0x1415A2830`), the HUD fades IN over 0.5 s. The clip `gameOverMenu` plays
   `in` (10 frames of alpha) and the focus goes to the first button (default cursor = first entry).
6. step 6: waits for the result (`+0x310`); then fade OUT 0.5 s (black; white for result 2 with `+0x8D6`), the loop
   SE stops (`0x140EA4E90`); step 100: after the fade the result is carried out: 2 = retry (`vt+0x3C8(2)`), 3 =
   `0x140EB4260` (save data copied in: load), 4 = the snapshot of items / status restored, then `vt+0x3C8(1)`, 6 =
   `vt+0x3C0`, 7 = the title map, 1 / default = `vt+0x3C8(1)` (continue). Which button gives which code beyond the
   delegates' 1 / 4 / 2 / 6 was NOT traced [?]; in KH3 as played [D]: Retry = the fight again from its start,
   Prepare & Retry = the menu to change equipment / abilities / items first, Continue = back to the last checkpoint
   (the room's entry), Load = the load screen, Title Screen = the title.

## 8. Fade colour: the game's WHITE against the user's "fade to black"

A normal death whites out (step 2; the blueprint node is even called `WaitBPTresStartGameOverWhiteOut`). The user
asked for "fade to black when Sora dies into this" (2026-10-12): `GameOverFade::black()` is the port's default
(the game's times with a black plate); `GameOverFade::game(kind)` is the game's own. TO BE PUT TO THE USER.

## 9. The void scene [A] + [C]

- Clip: `0270` (5 s, 151 frames, looped): he lies on his back across the picture, head toward +Y, drifting slightly;
  `0271` (3 s) is the failed mission's. Notifies of 0270 [A]: `TresFaceAnim` type NO_1 looped (row `NO_1_EL` =
  face/0001EM: eyes closed); Bonamik speed rate 0.3, mass 0.05 and LFCoeff 0.1 on HAIR / ACS / SLEEVE / HOOD / JCT /
  jip, a world wind (0, 2, 8) x 1200 (wave 15, period 0.1 s) on every group and (5, 0, 2) x 600 on `jip`, weights 0.3
  (HOOD / JCT / jip) / 0.1 / 0.2 / 0.3, ground collision and collision groups on: hair and clothes float in an
  upward wind at a third of the speed; they do not hang.
- Camera [A]: `game_over_01` = ONE key: position (393.37, 6.91, 81.76), rotation pitch -7.63 yaw -178.91, FOV 60;
  no drift of its own. `game_over_mission_fail`: (518.56, 120.98, 22.02), pitch -0.36 yaw -167.09, FOV 50.
- Lighting [A]: the light effect `vs_so_dth_ui00` has NO drawn emitter: three `ParticleModuleLight` particles,
  looping: `light` at (0, 0, 120) colour (0.6, 0.9, 1) x 5 radius 350 exponent 2; `light_face` at (0, 35, 100) white
  x 10 radius 105 exponent 2; `OnePart_lp` at (100, 180, -30) colour (0.4, 0.8, 1) x 2 radius 700 exponent 4. No
  other light exists in the void (the level's are hidden): no ground, no shadow.
- HEART OR LIGHT (`0x14138E5C0`): mission failed -> `vs_so_misn_ui00`; otherwise the HEART effect
  `vs_so_dth_ui00_2` when the flag is set, else the LIGHT effect. The flag = the killing blow's damage-info byte
  +0x40 == 1, a copy of the attacker's `TresChrBaseParam` byte +0x7D; what that byte means was NOT identified [?]
  ([D]: a kind of attacker that takes hearts). So "Sora with his heart out" is ONE of two looks and the reference
  capture (killed by the Tornado Titan) is the other: no heart. The heart effect loops from the scene's start: heart
  meshes `kmd_heart_06` / `kmd_heart_05` (red, additive) at (0, 0, 120), rising red sparks, a red light (24, 3, 5) x
  3 radius 285 and a face light.
- Origin of the camera anim and the effect: decompiled C says the pawn / its root component (capsule centre). With
  that origin he lies 97 cm too low against the capture; with his FEET as the origin he is where the capture has him.
  The port uses the feet (`ROOT_ABOVE_FEET = 0`): FITTED, not traced [?].

## 10. The menu [A] + [C], and the port against the reference capture

- Menu types: 16 rows at `0x146B37470` [A]: 0 Continue / Load / Title Screen; 1 + Game Help; 2 Retry / Prepare &
  Retry / Continue / Load; 3 + Game Help; 4 Retry / Load / Exit; 5 + Help; 6 Retry / Continue / Load; 7 + Help; 8
  Retry / Load; 9, 10 Gummi Ship; 11 Retry / Prepare & Retry / Give up; 12 + Help; 13 Retry / Prepare & Retry (id
  12) / Load; 14 Retry / P&R (12) / Continue / Load; 15 + Help.
- Chooser (`0x1415A2830` and the four functions it calls) [C]: Gummi Ship: 9 + n; game-mode byte `+0x8CC` in 2..5:
  4 (+ help); no retry flag (`+0x8D5`, or kind 2): 0 (+ help) (8 or 13 outside a normal map); retry flag: byte
  `+0x867E` == 15: 11 (+ help); < 7 or > 17: 2 (+ help); else 6 (+ help) or 14 (+ help). "+ help" = the game has
  Game Help pages for the fight. The reference is type 2.
- Hint panel: the dark panel with the heart emblem is part of the clip (`base`) and is ALWAYS there; the text comes
  from `TresGameOverHintSelector` over the DataTable `UI/Data/GameOverHintData` [A]: rows with
  `HintNameLocSpaceKey` (`kh3_system.gameoverhint.<id>`, 209 texts: `general_*`, `enemy_*` ...), `HintType`
  (CommonHint / TopPriority ...), `Priority`, `WorldCode`, `MapName`, `StartGameFlag` / `EndGameFlag`, `EnemyInfo`,
  `SpecialType`; blueprints add optional hints (`TresUI_AddOptionalGameOverHint`). The selector itself was NOT
  traced. The text field is a `TextFieldAlign` set to Center / Center (a block of left-aligned lines centred in the
  940 x 129 field); `<br>` is a line break.
- The orb at the selected entry is the LIGHT CURSOR of the movie `Cursor` (`UTresUIP_Cursor` `0x14175AE50`): at the
  top right corner of the entry's anchor rectangle, wandering in a 360-frame loop, sliding to a new entry in 0.3 s.
- MEASURED against the capture at 1280 x 720 (`build/sora_tmp/er/gameover/img.js bbox`; capture / port): title
  x 111..326 y 453..489 / 111..325, 453..488; Retry 190..248, 527..543 / 190..247, 527..543; Prepare & Retry, Continue,
  Load: identical boxes; hint line 1 x 610..1096 y 561..573 / 610..1097, 561..573; line 2 identical; orb centre
  (355.6, 526.9) / (354.3, 526.5) at the matching phase of its loop. Everything of the 2D layer is within 1 - 2 px.
- NOT matching: COLOURS. The capture (a console capture with a copyright line) is lifted against the movie's own
  values: title 104 against the movie's #4C4C4C = 76, panel 23 / 13, emblem 36 / 25, the red bar a flat (169, 8, 16)
  plateau against the texture's ramp peaking at 140, the gold line less saturated, the orb whiter. The greys fit one
  linear-light gain of about 1.9; the red does not: the capture's transfer could not be derived from the data, so the
  port draws the movie's values.
- Sora (frame 605 of the run, camera blend over): capture box x 483..970 y 216..437, centre (714, 307); port
  x 494..947 y 195..421, centre (703, 281): about 25 px higher, about 7 % narrower (the capture wears the larger
  KH2-style shoes), the legs less bent at that moment of the loop. Lighting: same direction (lit from above and
  the face side, dark underside); brightness set by the LOCAL exposure 1.6.

## 11. What is built (second pass)

`kh_ui::gameover` (menu, variants, hint, light cursor), `kh_host::gameover` (the sequence's times and plain data),
`Hud::open_gameover / gameover_view / take_gameover_result / end_gameover / gameover_hint`, kh-look point lights,
the test app's void (`kh-app/src/gameover_scene.rs`: a separate posed copy rendered into a picture under the menu)
and its sequence at every death.

## 12. LOCAL / open

Plate default black (section 8); the frames between the full plate and the void (3); the choice taking effect 16
frames after decide; cursor wrap; menu sounds; exposure 1.6; origin fitted (section 9); the blend exponent (2).
NOT built: the heart effect's drawn emitters; the time dilation; Bonamik weights / second wind / collision
switches; a `sheets/port` sheet for the sequence's numbers; the hint selector; Game Help entry; the camp menu
opened directly for Prepare & Retry (the test app's Start path gives the Pause screen while "in battle").
Only the game can settle: the meaning of the heart flag, the result codes per button, the eye adaptation in the
void, whether the menu lies over or under the plate, the origin of the camera anim.
