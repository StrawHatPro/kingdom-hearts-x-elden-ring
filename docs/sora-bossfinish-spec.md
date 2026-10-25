# The boss finish: what Kingdom Hearts III shows when the last blow lands on a boss

Written 2026-10-13 for the Sora port (the user: "can we also have the slo-mo effect and camera when a boss is defeated?"
and "the sfx and vfx associated with the slo-mo boss kill").

Evidence marks: **[A]** read from an asset (a level blueprint's bytecode, a class default object, the sound configuration);
**[C]** decompiled C only; **[I]** instruction level; **[U]** stock UE 4.17 on a confirmed identity; **[P]** a choice of the
port, not game behaviour; **[?]** not established.

Sheet: `sheets/port/sora-bossfinish.json` (67 rows; `tools/sora_bossfinish_sheet.js`). Code: `crates/kh-core/src/combat/finish.rs`,
`crates/kh-core/src/host/time.rs` (`GameSpeed`), `crates/kh-camera/src/finish.rs`, `crates/kh-app/src/finish.rs`,
`crates/kh-er/src/finish.rs`. Research aids (kept): `build/sora_tmp/er/er22/research/` — `kdump.js` (prints a level
blueprint's bytecode as lines), `survey.js` → `bossdefeat_survey.json` (every graph that calls the defeat effect),
`defeat_files.txt`, `gamespeed_files.txt`.

---

## 0. Summary

- The boss finish is **not native code and not a property of the enemy**. Each boss map's **level blueprint** runs the
  same script when the boss's spawn group is wiped out. 128 cooked packages contain it. [A]
- The script: a 2D sound; the **whole world at 0.05** (a linear 0.1 s ramp on real time); a radial screen blur; KH3's
  BOSS_DEAD sound mode (every other sound effect and voice silenced); no movement and no camera stick; **three hard
  camera cuts** — the boss, Sora, a wide shot of the boss — each a slowly turning camera 2 – 6 m from its subject, with a
  small real-time shake; then a **white-out** during which the speed returns to normal at once, every prize flies to the
  player, the boss is removed under the white, and the picture fades back in (or the map's cutscene starts). [A]
- Real-time line of the standard script at 60 fps: kill 0 s · cut 2 at ≈ 0.36 s · cut 3 at ≈ 1.36 s · white starts ≈ 2.36 s
  · full white and normal speed ≈ 3.96 s · control back ≈ 4.97 s · picture back ≈ 5.67 s. [derived from A + C]
- **No particle effect belongs to it**: 123 of the 127 graphs spawn none; the visuals are the blur, the white and the
  cameras. The hit burst is the killing attack's ordinary hit effect; the dissolve is the enemy's own. [A]
- **Ordinary fights have no slow motion** at their end: no encounter script calls the game-speed function, and no native
  caller was found. [A / ?]

## 1. Where it lives

| what | where | evidence |
|---|---|---|
| the trigger | `ATresLevelScriptActor::OnWipeOut(GroupName, Enemy, KilledBy)` (a blueprint event, thunk 0x1426DD740; raised through `RaiseWipeOutEvent`, exec 0x142702350 → vt+0x7B0) in the map's `*_enemy_*` level blueprint; for the boss's group it sends `RemoteEventPassActor("<map>_REvt_BattleFinish", Enemy)` | [A] `bt_01_enemy_01.umap`, `ew_26_enemy_01.umap` |
| the script | the receiving event in the map's `*_camera_ms###` level blueprint (some maps: in the enemy blueprint itself) | [A] 128 packages call `TresStartBossDefeatEffect`; 127 graphs surveyed |
| a native boss flag | none: `TresEnemyPawnBase` has no boss / finish property (reflection searched for Boss, Finish, LastHit, Slow, Dead); `TresStartBossDefeatEffect` (exec 0x142235370 → 0x140E76C50) only broadcasts a delegate of the game singleton (+0x4F0) that the BGM emotion manager and the vehicle pawns listen to | [C] |
| who raises the wipe-out, and at which point of the enemy's Die state | **not found** (the event is called through a vtable slot; its native caller was not located). The script reads the enemy's bone and destroys the actor itself at the end, so the enemy still exists when the event arrives: it comes with the kill, not after the death animation | [?] / [A] |

So "is this kill a finish" is the **map's** decision: the wipe-out of the group the level designer named. A boss with
adds finishes when the group is empty; a boss with several phases finishes when its last group does. In the port this is
`Targets::finish_kind(target)`, asked by the core right after a hit answered `killed` (default: never).

## 2. The script (reference graph `tt_01_camera_ms060`, the first boss) [A]

In execution order (the bytecode's flow stack resolved by hand; `TresWaitCycle(1)` = one tick):

```
on the event (Enemy):
  PlaySound2D(se00000_048, volume 1, pitch 1)          (other maps: SpawnSound2D(se00000_047))
  TresStartBossDefeatEffect()
  TresSetDisableGamePause(true)
  playing BGM slot: SetSuspendFadeOutTime(4), SetStopFadeOutTime(4);  BGM slot "Event": SetSilent(), Play()
  five flows start together:
   a  SetGlobalGameSpeed(CH_EVENT = 2, 0.05, 0.1)
      SceneManager.SetScreenBlur(0.001, {fIntensity 0.5, vPos (0.5, 0.5), vOffset (0.02, 0.02), fRange 2, fHoleSize 0,
                                         fAttenuationDistanceInside 0.2, ...}, CH_EVENT, priority 3, false)
   b  TresSoundMode_Start(4)                             (4 = ETresSoundModes::BOSS_DEAD)
      TresStartCinematicMode(bDisableMove, bDisableCameraMove, bDisableIKinema, bShowPlayerAuraEffect, bChangeTPViewMode;
                             NOT bHidePlayer, NOT bHideCockpit, NOT bPauseAI)
      TresCinematicPauseAI(friend AI, other AI, other cinematic)
   c  WaitCycle(1):  a TresNoteMovable is spawned at Enemy.MyMesh bone "t_camera" with the enemy's rotation
      camera 1 = BP_SpawnTresCameraRotTarget("Battle_End"), target = the note, SetPriority(210),
                 SetTresCameraRotTargetParam(200, R(5, -30, 5), V(0,0,0), V(0,0,50)), EnableCollision(false),
                 StartTresCamera(blend 0), SetBaseRotation(R(10, -100, 5), 0.5), SetDistance(300, 0.5)
      StopAllCameraShake();  ClientPlayCameraShake(no_speed_kokizami_1s, 1.0)
   d  WaitCycle(1), TresDelay(0.05):  the note is moved to the PLAYER's bone "t_camera" with his rotation
      camera 2: (200, R(-5, -25, 0), V0, V(0, -50, 0)) -> SetBaseRotation(R(-10, -100, 0), 0.5), SetDistance(300, 0.5)
   e  WaitCycle(1), TresDelay(0.1):   the note back on the enemy's bone
      camera 3: (600, R(-20, -20, 0), V0, V(0, 0, 50)) -> SetBaseRotation(R(-30, -80, 0), 0.5), SetDistance(450, 0.5)
   f  WaitCycle(1), Delay(0.15):
      WaitBPTresStartWhiteOut(layer 0, 0.08, InApplyGameSpeed true, InGamePause false)      (waits for the fade)
      ResetGlobalGameSpeed(CH_EVENT, 0.0)
      WaitCycle(1):  EndAllBPTresCamera(blend 0);  AllPrizeCollection(player)
      TresDelay(1.0)
      Enemy.K2_DestroyActor(), the note destroyed; SetActorStopOff(...); DestroyAllProjectile();
      TresCinematicPlayerStateClear(...); friends' states cleared
      SetScreenBlurDefault(0.01, CH_EVENT, 3, true);  CleanupForCinematic();  TresSoundMode_End(4)
      TresSetDisableGamePause(false);  RemoteEvent("..._REvt_End")       -> the map's cutscene, or
      (95 graphs) BPTresStartFadeIn(colour 0, layer 0, 0.7, true) and TresEndCinematicMode
```

Across the 127 graphs (`bossdefeat_survey.json`): game speed `(2, 0.05, 0.1)` in 112, `(2, 0.08, 0.2)` in 15, others in
7; the delays 0.05 / 0.1 (three cuts) in 112; `Delay(0.15)` before the white-out in all; white-out `(0, 0.08, true)` in
103, `(0, 0.6, false)` in 15; blur intensity 0.5 in all; sound mode 4 in all; BGM fade 4 s in 111; shake
`no_speed_kokizami_1s` × 1 in 102 play calls, × 3 in 25. **The camera numbers are per boss** (each designer framed their
boss; large bosses use 1200 – 2000 cm and bones of their own): the sheet carries the first boss's.

## 3. The pieces

### 3.1 The slow motion [C for the natives, A for the numbers]

`SetGlobalGameSpeed(channel, rate, interp)` 0x14222C9D0 → 0x140E66E40 → `GameState` vt+0x880 **0x144119CE0**: stores, for
the channel, target rate `max(rate, 0.01)` and interpolation time `max(interp, 0)` (table at GameState+0x46C, 16 channels
× {current, target, time left}). The game state's tick **0x14411A230** (vt+0x818):

```
for each channel: if current != target:
    if time_left > 0:  current += (target - current) / time_left * dt  (clamped at target);  time_left -= dt
    else:              current = target
rate = min(1, every channel's current) * max(1, every channel's current)        (|rate - 1| < 0.0001 -> 1)
WorldSettings.SqEX_CustomTimeDilation (+0x634) = rate
```

Its `dt` comes from 0x144119710, which passes the global double at 0x14871E340 for this function and a dilated delta for
the per-object-type one next to it: taken as the application's undilated frame delta, so **the ramp is linear in real
time** [C; the global's identity is inferred]. The effective world dilation is the product of four WorldSettings floats
(0x1441191D0), so **everything on world time slows alike: Sora, the enemies, the cameras' interpolators, the level
script's `Delay` nodes, effects**. This is a different table from the per-object-type dilation (`GameState` vt+0x8A8,
`host/time.rs` `TimeTable`) that the formchange and the shotlock aim use: nothing in the boss script touches that one.

`TresDelay` (0x140E70A70) calls the same stock `Delay` (0x144187DD0) as the plain node: both count **dilated** time [C].
Hence the real-time line: with `G(t)` = game seconds after `t` real seconds (`t - 4.75 t²` up to 0.1 s, then
`0.0525 + 0.05 (t - 0.1)`) and the delays starting one tick (1/60 s) after the kill: cut 2 at G = 0.05 → **0.36 s**; cut 3
→ **1.36 s**; the white-out's start → **2.36 s**. In the game these shift by up to a frame's worth of the ramp with the
frame rate (the ramp is where most of the first 0.05 game-seconds pass); the port integrates the ramp exactly.

### 3.2 The white-out, what follows [A; the fade's clock ?]

`WaitBPTresStartWhiteOut(layer 0, 0.08, InApplyGameSpeed = true, InGamePause = false)` → 0x140E7DD60 → 0x140E81E30: a
fade-out to the colour constant at 0x14871D7E0 on fade layer 0, and a latent wait for it. That `InApplyGameSpeed` makes
the 0.08 s run on the slowed clock (0.08 / 0.05 = **1.6 s** real) is the reading of the parameter's name; the fade's own
tick and curve were not read (linear taken) [?]. When it is white: speed 1 **at once** (`ResetGlobalGameSpeed(2, 0)`);
one tick later the cameras end (blend 0: a cut, unseen under the white) and `AllPrizeCollection(player)` (0x14221E440 →
0x140E469B0, not read past its name: every prize on the field goes to the player); **1.0 s** under the white; then the
enemy is destroyed, the blur, the sound mode and the input lock end, and either the map's cutscene starts or the white
fades in over **0.7 s**. The EXP / bonus panel belongs to what the map starts next; it is not part of this script.

### 3.3 The cameras [A for the numbers; C for the class]

Each cut is an `ATresCameraRotTarget` (the field camera's parent class, camera spec 2) whose target actor is the note:

```
look point = note position + TargetLocalOffset (camera+0x684) rotated by the note's rotation      (camera spec 2.2)
rotation   = m_Rotation (camera+0x828) + (0, note yaw, 0)                                         (camera spec 2.6)
position   = look point - forward(rotation) x distance (camera+0x800)
```

`SetTresCameraRotTargetParam` 0x14060B9C0 writes distance, rotation and the two offsets; `SetBaseRotation(rot, t)`
0x14060A640 and `SetDistance(d, t)` 0x1405EFB60 start the camera's interpolators with **type 5** (`1 - (1 - x)²`, the
dispatch read for the camera work) [C]. The interpolators tick on world time, so a 0.5 s move takes 10 s of real time: a
cut shows only its beginning — cut 1 (0.05 game-s, x = 0.1) covers 19 % of its move (200 → 219 cm, yaw −30° → −43°),
cut 3 runs through the white-out (0.13 game-s) and covers 45 % (600 → 532 cm). Yaw is relative to the subject's facing:
0 = behind it looking the way it faces. Blend time 0: hard cuts. Priority 210 (above the field and lock-on cameras, which
keep running underneath; the lock-on is not cleared by the script). `bDisableCameraMove`: the stick does nothing. The
note is *put* at the bone, not attached: a cut's look point stays where the bone was when the cut began.

The shake `/Game/Camera/shake/no_speed_kokizami_1s` [A]: `m_bRealDeltaTime` true (it is not slowed — the "no_speed" of
its name), duration 1 s, blend in 0, blend out 3 s (so it fades from its first frame), rotation only: pitch 3° and yaw 1°
at frequency 50, amplitude × random 0.5 – 1 (a Square Enix addition; `kh_camera::shake` does not evaluate it).

### 3.4 Screen effects [A for the request; the shader ?]

- **Radial blur**: `UTresSceneManager::SetScreenBlur` (exec 0x142A266D0) with the parameters of section 2, on from the kill
  (transition 0.001 s) until the clean-up (back to the default in 0.01 s). The post-process material behind it was not
  read: what "intensity 0.5, range 2, offset 0.02" look like is known only by their names.
- **White-out**: section 3.2. No other screen effect is in the script: no desaturation, no `ChangeSceneColor`, no
  vignette, no shatter (those words were searched for in the graphs).

### 3.5 Sound [A]

| what | id | when | slowed? |
|---|---|---|---|
| the finish sound | `/Game/Sound/SE/common/00000/se00000_047` (53 packages) or `_048` (46): bank `SE/common/00000` sounds 47 / 48, 7.9 / 7.8 s, category `SE_system`, UI sound, ignores the engine pause, stop fade 4 s. Both are **static SEs 22 / 23** (`ETresStaticSE`) and were already in the audio pack | the first node of the script (2D, volume 1, pitch 1) | no: nothing pitches `SE_system` |
| the sound mode | `TresSoundMode_Start(4)` → 0x141515D60 sends the sound-system event `STATE_MISSION_RESULT_START` (the pack's event 6): volume 0 on layer 4 for every SE and voice category **except `SE_system`** (fades 0.05 – 2 s), battle voices stopped. `_End(4)` → `STATE_MISSION_RESULT_END` (event 7): those categories stopped with a 0.5 s fade, volumes back over 1 s | with the game speed; ended after the hold | — |
| the music | the playing BGM slot's fade-out times set to 4 s, then the "Event" slot plays silence: the battle music fades out over 4 s | at the kill | no |
| a voice line | none in the script | | |
| the slow-motion sound state | **not used**: `STATE_SLOW_START` (pitch 0.3 on the SE categories) belongs to sound mode 2, the shotlock's; the boss finish silences instead of pitching | | |

A few bosses play a sound of their own instead (`se_kg02_XIIIdead` ×8, `se_g_ew_lich_slow02` ×5 ...).

### 3.6 HUD and input [A]

`TresStartCinematicMode` is called with `bHideCockpit` **false**: the script does not hide the HUD (whether cinematic mode
hides it some other way was not read [?]). `bDisableMove`, `bDisableCameraMove`, `TresSetDisableGamePause(true)`: Sora
cannot be steered, the camera cannot be turned and the pause menu cannot be opened from the kill until the clean-up. His
running animation simply plays on at 0.05 (`bContinueMotion` false, no state clear at the start).

## 4. The port

- **Core** (`kh-core`): `Targets::finish_kind(target) -> FinishKind { None, Battle, Boss }` (defaulted: None);
  `Ctx::deliver_hit` raises `EventKind::BossFinish { target, kind }` after a hit that answered `killed`;
  `host::time::GameSpeed` = the channel table and tick of 3.1; `combat::finish::FinishSequence` = the script as a
  real-time state machine. A host steps it with its REAL step and gets a `FinishFrame`: `rate` (it multiplies the `dt` of
  everything it steps, Sora's included, by it; `table()` gives the same rate per object type for the seam of
  host/time.rs), `shot` (the cut's interpolated distance / rotation / offset), `blur`, `white`, `hud_hidden`,
  `input_locked`, and `cues` (sound, sound state, BGM fade, shake, cameras end, collect prizes, input back, end).
  `abort()` puts everything back at once; a watchdog (`port.watchdog`, 10 s) ends a sequence that overstays. The script is
  stepped in slices of 1/240 s with the ramp integrated exactly, so its real-time line is the same at any frame rate.
  `FinishTuning` (default = the sheet) is what a host may change. 7 + 1 unit tests.
- **Camera** (`kh-camera/src/finish.rs`): `shot_view` (3.3's three lines, plus an optional sphere sweep from the look
  point), `FinishCamera` (keeps each cut's look point; reports hard cuts), `shake_def`. 3 unit tests.
- **[P] `FinishKind::Battle`**: a short slow-down (0.3 for 0.6 s) on the last enemy of an ordinary fight, with the
  slow-motion sound state. KH3 has nothing like it (section 6); both hosts leave it off.
- **[P] collision**: KH3 switches the first cut's collision off; the port sweeps every cut (`port.camera_collision`).

## 5. The hosts

**Test app** (`dummy-boss N [0/1]`): section "The scripted run" of `build/sora_tmp/er/IMPL_ER22_STATE.md`.
Not drawn: the radial blur (the app has no full-screen pass). Not played: the music fade (no music).

**Elden Ring** (`crates/kh-er/src/finish.rs`, key `finish.kh`, default **0**; untested in the game): the same state note.

## 6. Ordinary kills

Searched: every level blueprint and every blueprint under `Content/Blueprints` for `SetGlobalGameSpeed` — 143 packages,
128 of them the boss finishes above, the other 15 scripted events (`fz_0x_gameflow`, three `BP_TresPlayerStateEvent*`,
some cameras). No encounter script slows the game when its last enemy dies [A]. Native callers of the game-speed slot
were not enumerated (it is a virtual call), and no property or function named after a last-hit / battle-end slow-down
exists in reflection [?]. The only other slow motions found are the per-object-type ones already ported (formchange 0.1,
shotlock aim 0.4 / 0.1, the menu's slow option) and attraction / link finishers (`m_Param_FinishSlow*`, out of scope).

## 7. What only the running game can settle

1. Whether the HUD is visible during the slow motion.
2. The fade's clock and curve (1.6 s linear is the reading), and the white colour constant (0x14871D7E0, 4 floats).
3. What the screen blur looks like (strength and falloff of "0.5 / range 2").
4. Which native code raises the wipe-out, and how many frames after the killing hit.
5. The frame-exact start of cut 2 (0.2 – 0.5 s after the kill depending on tick order and frame rate; the port: 0.36 s).
