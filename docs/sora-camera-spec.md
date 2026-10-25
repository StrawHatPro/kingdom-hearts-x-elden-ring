# Sora's field camera: specification (stage 1)

Source: `base/KH3.exe` (decompiled C under `re/decomp`, disassembly read with iced-x86), the cooked packages
under `assets_converted/packages/KINGDOM HEARTS III/Content`, the game's ini files under `assets/`. Research
only: no game code or data was changed. The parameter table of section 9 is also the sheet
`sheets/port/sora-camera.json`. Companion documents: `docs/sora-movement-spec.md` (same conventions),
`docs/sora-port-plan.md`. Working notes: `build/sora_tmp/CAMERA_STATE.md`, `build/sora_tmp/cam/*_notes.md`.

Evidence tags on every item:

- **[I]** read at instruction level in the exe (address given)
- **[C]** decompiled C only (address given); the progress `verdict` of the function is quoted where it matters
  (`same` = the differential test proved the C identical to the original, so it counts as [I])
- **[A]** asset or ini value (path + property given)
- **[U]** stock UE 4.17 behaviour, function identified in the exe by shape/slot, body not re-read line by line
- **[?]** not determined

Units: cm, seconds, degrees; UE axes X forward, Y right, Z up (left-handed); yaw 0 = +X, positive yaw turns
toward +Y; **pitch positive = the camera looks up** (so the camera sits below the look point). Addresses are
virtual addresses. "Slot n" is a vtable index (byte offset / 8). "this+0x…" is an offset in the
`TresCameraNormal` actor unless another object is named.

Vtables: `TresCameraBase` 0x145D43A68, `TresCameraTarget` 0x145D6D888, `TresCameraDistanceTarget` 0x145D4F0B8,
`TresCameraRotTarget` 0x145D5C0F8, `TresCameraNormal` 0x145D5CA28 (slots 0..310),
`TresCameraManager` 0x145D4C3F8 (stock `APlayerCameraManager` 0x1477841D0).

---

## 0. The answers that matter most

1. **One actor, one function.** The field camera is an actor of class `BP_TresCameraNormal_ex001_C` (native
   `TresCameraNormal`), named by Sora's load asset and spawned by the controller when it takes the pawn. Its
   whole behaviour is `FUN_1406033C0(this, dt)`, run once per rendered frame from the camera manager's update,
   **after** every actor tick group of the frame.
2. **The controller steers by last frame's camera.** `moveYaw` uses the yaw of `CameraCache.POV.Rotation` as
   the previous frame's camera update left it (pitch zeroed, roll not zeroed). The camera in turn sees this
   frame's Sora.
3. **Orbit model.** Look point = the bone socket `Zup_Cam` + 48 cm = **150.34 cm above Sora's origin** (constant in
   every stage-1 animation), followed with an exponential lag of 8 /s capped at 100 cm, the same on all axes
   (no dead band, no jump rule). Camera = look point − forward × distance. Distance **400 cm**, shrinking
   linearly to 200 cm as the pitch goes from 0 to +50 (looking up). Pitch limits **−70 .. +50**, start pitch
   **−10**, start yaw = Sora's yaw. FOV **100° horizontal**, 16:9 constrained, near plane 3. Nothing depends on
   run / sprint speed.
4. **Right stick**: engine dead zone 0.25, then the camera's own 0.3 (so nothing below 0.475 of raw
   deflection), linear, **120 °/s × sensitivity**; shipped sensitivity 50 → ×1.2424 = **149.1 °/s** on both
   axes, no acceleration. It turns a target rotation; the shown rotation follows it at 8 /s.
5. **Automatic follow exists** (shipped option: on): 1.5 s after the last stick input, while Sora moves and is
   not Idle, the target rotation is pulled toward "behind Sora, pitch −10" at `(speed/800) × power` per
   second, `power` ramping 0 → 1.2 over 1 s (≈ 1.02 /s at run speed); yaw only while he is not running toward
   the camera. While running, slopes steeper than 15° add their angle to the pitch (16-frame mean).
6. **R3 recentres**: the target rotation jumps to behind Sora / pitch −10 and the shown rotation eases to it
   (8 /s); no snap.
7. **Collision**: two sphere sweeps on the Camera channel — the look point (radius 21) and the camera (radius
   20) from the look point outward; a hit puts the camera at the hit at once, the way back out is the distance
   smoothing at 4 /s. A third mechanism turns the camera around obstacles while Sora moves. No character fade
   by distance exists in the camera code.

---

## 1. Architecture

### 1.1 Objects

| object | class | what |
|---|---|---|
| camera manager | `TresCameraManager` (`APlayerCameraManager`), controller+0x478 and +0x830 | keeps a stack of camera actors (`m_CameraArray` +0x2838, pending adds +0x2848 / deletes +0x2858); the top is the view target; blends between cameras; fills `CameraCache` |
| field camera | `BP_TresCameraNormal_ex001_C` → `TresCameraNormal` → `TresCameraRotTarget` → `TresCameraDistanceTarget` → `TresCameraTarget` → `TresCameraBase` → `Actor`; manager `m_CameraNormal1st` (+0x28D8) | the orbit camera of section 2; its root component is a `CameraComponent` |
| lock-on camera | `TresCameraNormalLoc` (`m_CameraLocClass`, this+0x410) | child camera used while locked on; not stage 1 |
| camera config | `TresCameraConfig`, manager+0x28D0 | two config bools, `m_bNoCollision` (mask 2) and `m_bAutoMoveRotationYawOnly` (mask 1): both false — the constructor 0x1405E4F50 clears them [I] and no ini under `assets/` has a `TresCameraConfig` section [A] |

### 1.2 Which asset, and its values

`Blueprints/Player/p_ex001/p_ex001_PawnBase` CDO `m_LoadAsset` → `/Game/Load/Player/TresPlayerPawnLoad_ex001`,
whose `PlayerCameraNormal` = `/Game/Blueprints/Camera/Setting/p_ex001/BP_TresCameraNormal_ex001` [A]. (The same
load asset names the wall, diving, rail-slide … cameras and the `PlayerStateEventCamera` classes.)

Creation [C, `same-partial`]: `ATresPlayerControllerLowerBase::SetPawn` 0x1414473D0 → `FUN_1405ED7F0(ctrl,
pawn)`: if `m_CameraNormal1st` is null and the pawn is a player pawn, the class is `pawn->m_LoadAsset(+0x1448)->
PlayerCameraNormal(+0x58)` (`FUN_141434870`; native `ATresCameraNormal` if it is not a `TresCameraNormal`); the
actor is spawned with the controller as owner, attached to the pawn (slot 241, 0x1405D8440) and started with no
blend (`StartTresCamera`, slot 242, 0x14060BA20; section 2.9).

Values of the blueprint's class default object (everything it does not list is the native constructor's value;
full table in section 9) [A]:

| property | value | note |
|---|---|---|
| `m_TargetInfo.m_TargetOffset` | (0, 0, 48) | added to the socket position |
| `m_AnalogStickPlayX` / `Y` | 0.3 / 0.3 | camera dead zone (native 0.1) |
| `m_NearDistance` | 200 | distance at the upper pitch limit (native 0 = off) |
| `m_RotationSpeedMaxDistance` / `MinDistance` / `MaxRate` | 2500 / 500 / 0.45 | only copied to the lock-on camera (`FUN_14060A930`); **not read by the field camera** |
| `m_LockonAutoRotationLowDist` / `HighDist` | 300 / 3000 | lock-on only |
| CameraComponent `FieldOfView` | 100 | native 90 |
| CameraComponent `bUsePawnControlRotation` | true | no effect: the owner is not a pawn [U] |

The blueprints under `Blueprints/Camera/Setting/pc_cam_variation` (14, all named after boss fights) and the
few levels that reference the normal camera by name (bt_01, bt_02, ew_02, rg_11..22 …) replace it as the
"second" normal camera (`m_CameraNormal2nd`, +0x28E0, `BP_SetTresCameraNormalDefault` 0x141D29CA0) for those
fights; ordinary exploration uses the blueprint above [A for the references; the level scripts were not read].

### 1.3 Who calls whom

```
UWorld::Tick 0x1441BA050
  tick groups 0..10            (Early, LevelScript, PrePrePhysics, PrePhysics, Start/During/End/PostPhysics, 3 SQEX groups)
      controller TickActor 0x1444429C0 -> ... -> Sora input 0x1413F1AB0   reads FUN_1405E6B80 = last frame's camera
      pawn states, movement component                                         (movement spec 1.2)
      camera actor Tick (slot 134, 0x14060CE60)                               timed parameter tweens only (slot 268 chain)
  timers, tickables
  for each player controller: UpdateCameraManager (vt+0xE18) (dt x time rate of object type 9 = camera)
      -> manager slot 248 override 0x1405F55A0
           FUN_1405FBD10   release of the frozen input view point (1.4)
           FUN_1405FB910   camera stack: adds / deletes / new view target
           stock UpdateCamera 0x144443BA0 -> DoUpdateCamera (slot 266 override 0x1405E9BE0)
               -> stock UpdateViewTarget (slot 271, 0x144444630) -> view target CalcCamera (AActor slot 179)
                    TresCameraNormal 0x1406023B0 -> FUN_1406033C0(this, dt)      <- section 2
                                                 -> TresCameraBase 0x1405C8FD0 -> AActor::CalcCamera 0x143E90790
                                                    (POV from the CameraComponent; rotPrev; snap bit cleared)
               blend with a pending view target, roll offset, FillCameraCache 0x144430D10
           FUN_1405FBFF0   see-through fades
  tick groups 11, 12
```

Evidence: the camera-side chain [C + the virtual slots read from the vtables]; `UWorld::Tick` [C, `differs`,
shape of stock 4.17, the vt+0xE18 call site at 0x1441BAF30 confirmed by a call scan]. No tick-group write was
found in the constructors of the controller, the camera or the manager, so their groups are the default
(PrePhysics) [?: inferred]; whatever they are, the camera update comes after all of groups 0..10. The camera
`dt` is the world delta times the "object type 9" time rate (slow-motion effects scale the camera separately);
inside `DoUpdateCamera` a manager flag (manager+0x2A34 bit 8) can replace it by a fixed global step [C].

### 1.4 What `cameraYaw` the controller reads

The Sora input update 0x1413F1AB0 calls `FUN_1405E6B80(ctrl+0x830, &loc, &rot)` at 0x1413F2144 [I];
`FUN_1405E6B80` (`same`) = `BP_GetInputCameraViewPoint`: normally stock `APlayerController::GetPlayerViewPoint`
0x144434C00 = `CameraCache.POV.Location / Rotation`. The cache was written by `FillCameraCache` at the end of
the **previous** frame's camera update (1.3), so:

- **movement uses the previous frame's camera yaw**; the camera then follows with this frame's Sora;
- it is the final POV: with only the field camera active it equals the field camera's component rotation of
  the previous frame; a blend between two cameras, the manager's roll offset and (stock modifier path, [U])
  camera shake are in it too;
- pitch is zeroed (`mov [rbp-40h], r14d` at 0x1413F21FC [I]); **roll is not** — with roll 0:
  `moveYaw = cameraYaw + atan2(LeftX, LeftY)`;
- a "frozen input view point" (manager+0x290C bit 0, rotation +0x28F4) replaces it for a while after a camera
  cut when the new camera asked for it (`BP_EnableKeepChangeRotation`); never with the field camera alone.

### 1.5 FOV and projection

The POV is the CameraComponent's: **FieldOfView 100** (blueprint [A]), `bConstrainAspectRatio` = 1 (set by the
`TresCameraBase` constructor at 0x1405C043D [I]) with `AspectRatio` 1.777778 (engine default [C]): 100°
horizontal on a 16:9 frame (≈ 67.7° vertical; bars at other window shapes), near plane 3.0
(`UCameraComponent` constructor 0x143FA3C16 [I]; `NearClipPlane=3.0` in DefaultEngine.ini [A]). The manager
does not override the FOV. FOV can be tweened per camera (`SetFieldOfView`, 0x141D05C20 → `FUN_1405D6F40`),
but no caller is in the Idle / Run / normal Jump path: **the FOV does not change with speed** [C; callers
listed in `build/sora_tmp/cam/arch_notes.md`].

---

## 2. The follow model

One function does all of it: `FUN_1406033C0(this, dt)` (2186 bytes, verdict `not-compiled`; every step below
was read in the disassembly, `build/sora_tmp/cam/normal_update.dis`). It is called from the camera's
`CalcCamera` (section 1.3) once per rendered frame with the frame's `dt`.

### 2.1 State (the fields a port needs)

| field | offset | type | initial | meaning |
|---|---|---|---|---|
| `rotTarget` | this+0x898 | FRotator (pitch, yaw, roll) | start rotation (2.9) | the rotation the camera is asked to have: the stick adds to it, the automatic rotations pull it |
| `rotCur` | CameraComponent `RelativeRotation` (component+0x2FC) | FRotator | = rotTarget | the rotation actually shown; follows `rotTarget` (2.4) |
| `rotPrev` | this+0x46C | FRotator | = rotCur | `rotCur` as it was at the end of the previous frame's update (written in `CalcCamera`, 0x1405C8FD0) |
| `stickDelta` | this+0x8A4 | FRotator | 0 | this frame's rotation from the right stick |
| `lookSmooth` | this+0x8B0 | FVector | target point | smoothed look point (2.3) |
| `m_LookPosition` | this+0x660 | FVector | target point | look point after its collision correction (= `lookSmooth` when nothing is hit) |
| `distSmooth` | this+0x8BC | float | 400 | smoothed distance |
| `m_CurrentDistance` | this+0x428 | float | 100 (ctor), then actual | distance actually used; after collision it is the real camera-to-look distance |
| `autoPower` | this+0x8DC | float | 0 | ramp of the automatic follow (2.6) |
| `stickWait` | this+0x8E4 | float | 0 | seconds until the automatic follow may act again after stick input |
| `stillWait` | this+0x8E8 | float | 0 | seconds until the wall-avoiding rotation may act after standing still (2.7) |
| `resetWait` | this+0x8EC | float | 0 | set to 0.5 by a camera reset (2.8); while > 0 the fast rotation rate is used and 2.7 is off |
| `slopeRing` | this+0x98C (16 floats), index this+0x988, mean this+0x9CC | | 0 | samples of the slope pitch (2.6) |
| flags | this+0x8F0 (byte) | | 0xDC | 0x02 stick active this frame · 0x04 wall-avoiding rotation allowed · 0x08 distance restarts from `m_CurrentDistance` · 0x10 rotation smoothing on · 0x40 reset allowed · 0x80 `m_bMoveRotation` (automatic follow allowed); 0x01 and 0x20 belong to lock-on and wall-run |
| snap bit | this+0x468 bit 0 | | set at start | "first update / snap": every smoothed value jumps to its target; cleared at the end of `CalcCamera` |

Constructor defaults [I, verdict `same`]: `ATresCameraBase` 0x1405C02C0, `ATresCameraTarget` 0x1406161D0,
`ATresCameraDistanceTarget` 0x1405E1DE0, `ATresCameraRotTarget` 0x1405FD1A0, `ATresCameraNormal` 0x1405FC450.

### 2.2 Order of one update [I 0x1406033C0]

`dt` is the frame time. `target` = the followed actor (`m_TargetInfo.m_TargetActor`, this+0x690; slot 245).
Nothing happens if there is no player controller (this+0x3F8) or no target.

1. **Stick** — slot 301 (0x140607ED0): read the stick, `rotTarget += stickDelta`, clamp, flag 0x02 (2.5).
2. **Automatic follow** — slot 300 (0x140600630 → `FUN_1405FFA60`): may pull `rotTarget` toward "behind the
   player"; returns `followed` (bit 0) (2.6).
3. **Rotation smoothing** — `FUN_1405D8170(this, &rotTarget, dt, flag 0x10, fast)` with
   `fast = stick active || stickWait > 0 || resetWait > 0` [I 0x14060353A–0x140603556]: `rotCur` moves toward
   `rotTarget` (2.4).
4. `rot = rotCur` (copied now; later steps that turn the camera write the component again).
5. **Distance** — `want = slot 290()` (2.3.3); `distSmooth = lerpStep(flag 0x08 ? m_CurrentDistance : distSmooth,
   want, rate this+0x448 = 4.0, dt)` [I 0x14060359F–0x1406035E0]; snap bit → `distSmooth = want`;
   `m_CurrentDistance = distSmooth`.
6. **Target point** — `rawTarget = slot 303()` (2.3.1).
7. **Look point** — `lookSmooth = lerpStepVec(lookSmooth, rawTarget, m_fPositionLerpPer, dt)` then the lag clamp
   `m_fPositionLerpLimit` (2.3.2); snap bit → `lookSmooth = rawTarget`.
8. Look-point collision `FUN_1406054D0` (section 3) may move `lookSmooth`; `m_LookPosition = lookSmooth`.
9. `camPos = m_LookPosition − Forward(rot) × m_CurrentDistance` (`FUN_1405C90A0`: `look + RotationMatrix(rot) ×
   (−distance, 0, 0)`).
10. **Camera collision** (only if this+0x6B4 ≠ 0) — slot 287 (0x1406223B0) `d = f(&rot, &camPos, &look)`; when
    `d > 0`: the component is turned to the returned `rot` and `m_CurrentDistance = d` (section 3).
11. The component gets location `camPos` and its (normalised) rotation; `camPos` and the rotation are read back
    from the component; `lookPt = camPos + Forward(rotation) × m_CurrentDistance` (`FUN_1405CE7F0`).
12. `away = FUN_140608440(this, −0.8)` (2.6, "not running toward the camera").
    `doAvoid = followed`; if `|m_LookPosition − rawTarget|² > 1.0` and the stick is not active and `away` and
    `stickWait <= 0` → `doAvoid = true` [I 0x140603A5A–0x140603A7C].
    If `resetWait > 0`: `resetWait −= dt` (floored at 0), `doAvoid = false`.
    If flag 0x04 is clear, or step 8 corrected the look point: `doAvoid = false`.
13. **Wall-avoiding rotation** — `FUN_140603FF0(this, &camPos, &lookPt, &camPos, doAvoid, away, dt)` (2.7).
14. Water surface — `FUN_1406048D0` (keeps the camera above a water plane; only where the level has water;
    not stage 1) [C, `no-verdict`].
15. **Final collision** — `FUN_14061CB40(this, &camPos, &camPos, &lookPt, 1, 1, 1, 1, …)`; slot 308 when it
    changed something (section 3).
16. `m_CurrentDistance = |lookPt − camPos|`; the component is moved to `camPos`.

Back in `CalcCamera` (0x1405C8FD0): the view is taken from the component (location, rotation, FOV), `rotPrev`
= the component's rotation, the snap bit is cleared.

The two smoothing primitives [I] (both loop in slices of at most 1/60 s, so a long frame is cut into 60 Hz
steps plus a remainder; constants 0.016666668 at 0x145D314C4):

```
lerpStep(cur, target, rate, dt):                 // FUN_1405D0850 (same) ; vector form FUN_1405D0B30
    while dt > 0:
        step = min(dt, 1/60);  dt -= 1/60
        cur += (target - cur) * clamp(step * rate, 0, 1)
    return cur

rotLerpStep(cur, target, rate, dt):              // FUN_1405D0650, per component (pitch, yaw, roll)
    same loop with   cur += normalizeAxis(target - cur) * clamp(step * rate, 0, 1)   // shortest way round
```

`normalizeAxis` wraps to (−180, 180]. (The 360/180 vector constants at 0x1487C9DB0 / 0x1487C9DA0 are filled at
start-up, so their values are [U], not read.)

### 2.3 Target point, look point, distance

#### 2.3.1 Target point (what the camera looks at)

Slot 303 (0x140605150) → slot 284 (0x140621B90) → `FUN_140624260(&m_TargetInfo, out, attached)` [C `differs` /
`not-compiled`; structure confirmed by the data below]:

```
rawTarget = worldPosition(socket "Zup_Cam" of the target's skeletal mesh)      // FUN_140621E00 / FUN_140622200
          + m_TargetInfo.m_TargetLocalOffset rotated by the socket's transform   // (0,0,0) for Sora
          + m_TargetInfo.m_TargetOffset                                          // (0,0,48) for Sora [A]
          + RotationMatrix(rotCur) × m_CameraLocalOffset                         // (0,0,0) for Sora
```

- The socket name is the constructor default `FName("Zup_Cam")` at this+0x698 [I 0x1406161D0, `same`]. In
  Sora's skeleton (`Character/pc/p_ex001/mdl/p_ex001_0_Skeleton`) the socket `Zup_Cam` is on bone `t_camera`
  (parent `trans`, parent `kh_sk`), relative rotation (pitch 90, yaw 90, roll 0), no relative location [A].
- `t_camera` has the reference translation (0, −102.338, 0) under the root's −90° X rotation = **102.338 cm
  above the mesh origin**, and it is **not animated**: every stage-1 clip (`build/sora_pack/anims/*.glb`, 29
  clips: idle, walk, run, run start/stop, turn, jump, fall, land) has exactly one translation key and one
  rotation key for both `trans` and `t_camera` [A, `build/sora_tmp/cam/animcam.js`]. The mesh component sits
  at the actor origin (animation spec 1), and the actor origin is at foot level (port plan 2).
- So for stage 1: **`rawTarget = actorOrigin + (0, 0, 150.338)`**, independent of the animation pose, the
  actor's yaw and the capsule. (If the lookup fails — no such socket or bone — the code falls back to the
  actor's root component location [C 0x140621E00].)
- The world↔local conversions around it (`FUN_14062C980`, `FUN_14062C870`) are the identity unless the camera
  component is attached to a parent (slot 275, 0x140624870: `RootComponent->AttachParent != null`); it is not
  attached in field play [C].
- `m_NearTargetOffsetZ` (this+0x908) and `m_NearTargetOffsetZDistance` (this+0x90C) are 0 for Sora (constructor
  default, not overridden in the blueprint), so the "raise the target when the camera is close" branch of slot
  303 never runs [I 0x1405FC450 defaults; C for the branch].

There is **no vertical dead band and no separate rule for jumps**: the target point is the bone, and the only
smoothing is 2.3.2, the same on all three axes.

#### 2.3.2 Look point (position lag)

`FUN_1405D0B30(this, out, cur, target, per, dt, limit)` [I]:

```
lookSmooth = lerpStepVec(lookSmooth, rawTarget, m_fPositionLerpPer = 8.0, dt)
if m_fPositionLerpLimit >= 0 and |lookSmooth - rawTarget| > m_fPositionLerpLimit (= 100):
    lookSmooth = rawTarget + normalize(lookSmooth - rawTarget) * m_fPositionLerpLimit
```

At 60 Hz the look point closes 13.33 % of its distance to the target per frame; in steady motion at speed `v`
it trails by `v × (1/(60 × 0.1333) − 1/60) ≈ v × 0.1083 s` — 74 cm at 680 cm/s — and never by more than 100 cm
(reached above ≈ 923 cm/s, i.e. while sprinting at 980 and during the fast part of a jump). The clamp is a 3D
sphere, not per axis.

#### 2.3.3 Distance

Slot 290 (0x140606390) [I]:

```
d = m_Distance (this+0x800, 400)                         // slot 304; "non-battle" blend skipped: m_NonBattleDistance = 0
p = rotCur.pitch                                         // after step 3 of this frame
if p > 0 and m_NearDistance (200) > 0:                   // looking up: pull in
    t = clamp((m_LimitPitchMax - min(m_LimitPitchMax, p)) / m_LimitPitchMax, 0, 1)      // m_LimitPitchMax = 50
    d = m_NearDistance + (d - m_NearDistance) * t
d = d * distanceScale (this+0x804, 1.0) + distanceOffset (this+0x6A8, 0)
if d <= 0: d = 0.1
```

So 400 cm for every pitch ≤ 0, shrinking linearly to 200 cm at pitch +50 (the upper limit). The smoothed
distance follows with rate 4.0 /s (`distSmooth`, step 5). Because flag 0x08 is set, the smoothing restarts each
frame from `m_CurrentDistance`, which after a collision is the shortened real distance: **a collision pulls
the camera in at once and it eases back out at 4.0 /s** (6.67 % of the remaining gap per 60 Hz frame).

(The wall-run branch — `FUN_140608790`: player state id 8 and this camera is the manager's active normal
camera — replaces the pitch rule by a blend toward `m_WallDistance` 700; not stage 1.)

Speed has **no** influence on distance: nothing in slot 290 reads the velocity. (FOV: section 1.5.)

### 2.4 Rotation smoothing (`rotTarget` → `rotCur`)

`FUN_1405D8170(this, &rotTarget, dt, apply, fast)` (`same`) + `FUN_1405D0650` [I]:

```
rate = not fast ? m_fRotationLerpPer (this+0x454 = 8)
     : (inputSource (this+0x460) == 1 ? this+0x45C (30) : this+0x458 (8))
component rotation = rotTarget
if apply (flag 0x10, default set):
    from   = snap bit ? rotTarget : rotPrev                  // rotPrev = the rotation shown last frame, after collision
    rotCur = rotLerpStep(from, rotTarget, rate, dt)          // 2.2
    component rotation = rotCur
```

`inputSource` is written by the stick read each frame: 2 = the right-stick key states were fed by a gamepad,
1 = by another device, 0 = none (2.5). With a gamepad every rate is 8 /s: each 1/60 s slice moves 13.33 % of
the remaining angle, whether or not the stick is held — a 99 % settle takes ≈ 0.54 s. (A quaternion-slerp
variant behind this+0x469 bit 0x10 is not the default.)

So the stick is not applied to the view directly: it moves `rotTarget` at a constant angular speed and the view
trails it by `speed × (1/(60 × 0.1333) − 1/60) ≈ speed × 0.108 s` — about 16° at 149 °/s — and coasts to it
after release.

### 2.5 Right stick

Slot 301 (0x140607ED0) [I] → slot 259 (0x1405CD290, gate and sensitivity) → slot 258 (0x1405CD3F0, read) →
slot 266 (0x1405CF0F0, delta):

```
(rx, ry) = Gamepad_RightX, Gamepad_RightY after the engine's axis config                 // GetInputAnalogStickState 0x1444333A0
          DefaultInput.ini: DeadZone 0.25, Sensitivity 1, Exponent 1, no invert [A]:
          e = sign(r) * max(|r| - 0.25, 0) / 0.75            per axis (stock MassageAxisInput 0x1445AF120 [U])
zero if: camera input disabled (this+0x469 bit 0x02 clear), manager+0x2A34 bit 0x80 clear,
         or the controller ignores look input (controller vt+0x8A0)
ry = -ry                                                     // right stick only [I 0x1405CD525]
X = sign(rx) * max((|rx| - m_AnalogStickPlayX) / (1 - m_AnalogStickPlayX), 0)            // 0.3 for Sora; per axis, linear
Y = sign(ry) * max((|ry| - m_AnalogStickPlayY) / (1 - m_AnalogStickPlayY), 0)
if config byte0 & 0x04: X = -X          // "camera horizontal: reverse" (shipped: off)
if config byte0 & 0x08: Y = -Y          // "camera vertical: reverse"   (shipped: off)
if |manager roll offset (+0x2A3C)| > 90: X = -X, Y = -Y      // upside-down camera
(sx, sy) = sensitivity(config byte5), sensitivity(config byte6)                          // FUN_1405EC530
          v = (byte - 1) / 99;   s = v <= 0.5 ? 0.5 + 0.75 * 2v : 1.25 + 1.15 * (2v - 1)     // 1..100 -> 0.5 .. 2.4
          (inputSource == 1, a non-gamepad device: fixed 1.25 instead; a manager override +0x2A38 >= 0 replaces both)
stickDelta.yaw   = X * sx * m_RotationSpeedX (120) * dt
stickDelta.pitch = Y * sy * m_RotationSpeedY (120) * dt
flag 0x02 = (stickDelta.yaw != 0 or stickDelta.pitch != 0)
rotTarget += stickDelta
rotTarget.pitch = clampAngle(rotTarget.pitch, min(base.pitch, m_LimitPitchMin = -70), max(base.pitch, m_LimitPitchMax = 50))
(yaw limit only if m_LimitYawMax > 0; it is 0)      rotTarget = normalize(rotTarget)
```

"config" is the 32-byte user configuration block at `(GameInstance + 0x870) + 0x2E8`.

- **Dead zones stack**: output starts at raw deflection 0.25 + 0.3 × 0.75 = **0.475** and is full at 1.0; both
  are per axis (a cross, not a circle).
- **Speed**: linear in the rescaled deflection, no curve, no acceleration, no smoothing of the input itself.
  Shipped sensitivity bytes are 50 / 50 (configuration constructor 0x140E84340, `same`) → scale 0.5 + 0.75 ×
  98/99 = **1.2424** → **149.09 °/s** at full deflection, yaw and pitch alike.
- **Signs**: stick right → yaw increases (view turns right). Stick up → after the sign flip `Y` is negative →
  pitch decreases: the camera rises and looks down. This assumes the raw `Gamepad_RightY` is positive for "up"
  (the game uses its own input plugin, XInput is disabled in the ini); section 6 asks the probe to settle it.
- **Pitch clamp** is a hard clamp (stock `FMath::ClampAngle` 0x142D74AD0) to [−70, 50]; the range is widened to
  contain the base pitch (it already does). No soft zone.
- The stick flag also reloads `stickWait` to 1.5 s every frame it is set (2.6), so the automatic follow starts
  1.5 s after the stick is released.
- Mouse: no separate path was found in the camera; a non-gamepad source of the right-stick keys gives
  `inputSource` 1 (fixed ×1.25, fast rate 30). Not researched further.

### 2.6 Automatic follow ("move rotation")

`FUN_1405FFA60(this, &rotTarget, base, dt, stickActive, 1, −0.8)` (3015 bytes, `same-partial`; read in the
disassembly, `build/sora_tmp/cam/ffa60.dis`). `base` = the base rotation (slot 299, 0x1405EBDF0 [I]):

```
base = m_Rotation (this+0x828: pitch -10, yaw 0, roll 0)  +  (0, targetActorYaw, 0)
```

(`targetActorYaw` is the yaw of the actor's root component — Sora's facing; it is written to this+0x6FC on
every call. With `m_bWorldBaseRotation` (this+0x8F1 bit 0, default off) the actor yaw is not added.)

```
if stickActive:  stickWait = m_MoveRotationWaitTime (1.5)                       [I 0x1405FFAB4]
else if stickWait > 0:  stickWait -= dt;  if stickWait <= 0: stickWait = 0, autoPower = 0

allowed = autoCameraOption (2.6.1)  and  flag 0x80 (m_bMoveRotation)  and  stickWait <= 0
if not allowed or stickActive or no target:  return 0

v = velocity of the target (2.6.2);   if player state id == 1 (Idle):  v = 0
speed2 = |v.xy| ;  speed3 = |v|
if speed3 <= 1.0:   autoPower = 0;  return 0                                    [I 0x1405FFEF3]

autoPower = min(autoPower + m_AutoTurningPower * dt, m_AutoTurningPower)        // 1.2 per second up to 1.2
rate = (speed2 / m_TargetMaxSpeed) * autoPower                                  // m_TargetMaxSpeed = 800; not clamped

facing = dot( normalize(camForward.x, camForward.y, actorForward.z), actorForward )     // camForward from rotCur

want = base                                                                     // (pitch, yaw, roll)
if v.z > -800:
    if player state id is 2 (Run) or 10 (slope slide):   want.pitch += slopePitch(...)    (2.6.3)
else:                                                                           // long fall
    p   = max(v.z * 0.05, -50)                                                  // deg/s, negative
    lim = min(-50, base.pitch)
    if rotTarget.pitch >= lim:   rotTarget.pitch += p * dt
    want.pitch = rotTarget.pitch
want.roll = rotTarget.roll
if facing < -0.8:                      want.yaw = rotTarget.yaw                 // running toward the camera: yaw is left alone
else if Jump state, sub-step 7 or 8:   want.yaw = rotTarget.yaw                 // dive fall (not stage 1)

rotTarget = rotLerpStep(rotTarget, want, rate, dt)
rotTarget.pitch = clampAngle(rotTarget.pitch, min(base.pitch, m_LimitPitchMin), max(base.pitch, m_LimitPitchMax))
(yaw limit: m_LimitYawMax = 0 -> none)      rotTarget = normalize(rotTarget)
return 1          // "followed"
```

Reading it as behaviour:

- It **exists**, and it turns both yaw and pitch: toward "behind Sora" (his facing yaw) and toward the base
  pitch −10 (plus the slope term while running).
- **Delay**: 1.5 s after the last frame with right-stick input (`stickWait`), then a ramp: `autoPower` grows
  from 0 by 1.2 per second for 1 s. It is reset to 0 whenever the delay runs out, and whenever Sora stops.
- **Rate**: `rotTarget` closes the angular gap exponentially with rate `(speed/800) × autoPower` per second:
  at run speed 680 and full power 1.02 /s (1.7 % of the gap per 60 Hz frame), at sprint 980 1.47 /s, walking
  140 0.21 /s. On top of that `rotCur` follows `rotTarget` (2.4).
- **Conditions**: the automatic-camera option is on; Sora is moving (measured speed > 1 cm/s) and not in the
  Idle state; no stick input for 1.5 s. Yaw is only pulled while Sora is not running toward the camera
  (`facing >= −0.8`, i.e. the angle between the camera's heading and his facing is below 143.13°); pitch is
  pulled in either case.
- In the air (Jump state) the yaw follow continues with the horizontal speed; the pitch goes back to −10
  unless the fall is faster than 800 cm/s, where the camera tilts down at up to 50°/s, never below −50°.

##### 2.6.1 The option bit

`FUN_1406086B0` [I, `same`]: `byte[(GameInstance + 0x870) + 0x2E8] bit 1`, the user configuration block
(`FUN_140EA8620` = GameInstance+0x870). The same byte holds the reverse bits of the camera stick (2.5).
The block is created by the configuration constructor 0x140E84340 (`same`) with byte 0 = 0xC3: **bit 1 is set
in a fresh configuration, the automatic camera is on by default**. (That this bit is the options-menu item
"automatic field camera" is inferred from what the code does with it; the menu texts were not read.)

##### 2.6.2 The velocity it uses

`FUN_140607BE0` [I, `same`] → pawn slot 453 (vt+0xE28, 0x1405AB330): a pointer to **`MyMovement + 0x9A0`**, a
vector that the movement component fills once per tick in 0x140591690 [C `differs`; the stores at
0x140591D2E–0x140591D67 read in the disassembly]: `(actor location now − actor location at the previous call)
/ dt`. It is the **measured displacement velocity**, not `Velocity`: it has a Z part on slopes and stairs, it
is 0 against a wall, and it includes root-motion displacement (run start). The camera update runs after the tick groups (1.3), so it reads the value of this frame's movement tick.

##### 2.6.3 Slope pitch (automatic pitch on slopes)

Slot 302 (0x140606B10) `slopePitch(pawnPos, camPos, v)` [I for every constant and comparison]:

```
toPawn = normalize2D(pawnPos - camPos);   move = normalize2D(v)
d = v            if dot2D(toPawn, move) > -0.7        // moving away from / across the camera
d = -v           otherwise                            // moving toward the camera: mirrored
a = 0            if |d| < 0.0001 on every axis
    else  a = atan2(d.z, |d.xy|) in degrees
          hi = max(60 - base.pitch, 60)               // 70 for base pitch -10
          if a > hi: a = hi
          else if a < -50: a = -50
          else if -15 < a < 15: a = 0                 // dead zone: slopes under 15 degrees do nothing
ring[(index + 1) mod 16] = a                          // one sample per call
return FUN_140605BD0(this, |normalize(d).z| < 0.9)    // see below
```

`FUN_140605BD0` (`same-partial`) [C]: with its flag set it first looks at the 7 samples before the newest; if
fewer than 7 of them "agree" with the newest (both 0, or both above 15 and not larger than it, or both below
−15 and not smaller) it returns the previous mean (this+0x9CC) unchanged; otherwise (and always when the flag
is clear) it returns and stores the **mean of all 16 samples**. So the slope pitch reacts only after the slope
has been steady for 7 frames and then changes over 16 frames — **both counts are in frames, not seconds**.

The result is added to the base pitch: running up a 20° slope away from the camera asks for pitch −10 + 20 =
+10 (camera lower, looking up the slope); running down asks for −30; running up toward the camera is mirrored
(−30). The pull is the slow follow rate above.

### 2.7 Wall-avoiding rotation

`FUN_140603FF0(this, &camPos, &lookPt, &camPos, doAvoid, away, dt)` (2266 bytes, `differs`; constants and
branches read in the disassembly, `build/sora_tmp/cam/ff0.dis`):

```
v = velocity of the target (2.6.2);  Idle state -> v = 0
speed2 = |v.xy|
if speed2 < 10 and |v.z| < 1:      stillWait = 0.3;  doAvoid = false          // standing: re-arm the delay
else if stillWait > 0:             stillWait -= dt;  doAvoid = false
else:  alongAxis = |dot2D(normalize2D(v), Forward(0, rotCur.yaw, 0))| > 0.99   // moving straight along the view axis
if the camera config has m_bNoCollision, or this+0x704 bit 0x10, or the automatic-camera option is off,
   or not doAvoid:  return false

found = FUN_14060F490(this, &newCamPos, &lookPt, &camPos, 2.0)                // path probe, below
if not found: return false

want = Rotation(m_LookPosition - newCamPos)                                    // FVector::Rotation
if not alongAxis:  rotTarget.yaw = want.yaw
if |want.pitch| < |base.pitch|  or  |v.z| >= 50  or  (rotTarget.pitch < want.pitch and want.pitch <= 0):
    rotTarget.pitch = want.pitch
clamp rotTarget.pitch as in 2.6; yaw limit
rate = 10 * clamp(speed2 / m_TargetMaxSpeed, 0, 1)                             // 8.5 /s at 680
rotCur = rotLerpStep(rotCur, rotTarget, rate, dt)
if not away or speed2 < 200:
    rotTarget = rotLerpStep(from rotCur toward rotTarget, 4.0, dt)             // i.e. rotTarget = rotCur + (rotTarget - rotCur) * a
component rotation = rotCur;  camPos = m_LookPosition - Forward(rotCur) * m_CurrentDistance
lookPt = camPos + Forward(rotCur) * m_CurrentDistance
return true
```

`FUN_14060F490` (3133 bytes, `differs`) [C; constants I]: sweeps a sphere of radius `2 × probeRadius` (section
3) from the look point to the camera position; where it is blocked it slides along the obstacle (direction
projected on the hit plane, 0.1 cm off the surface) for the rest of the original length, at most 10 times, and
returns the end of that path when at least one slide happened. In open space it finds nothing and 2.7 does
nothing. It needs the level's collision; a port without camera collision can stub 2.7 as "return false"
knowingly.

### 2.8 Reset (recentre)

Slot 244 (0x140609C40, `same-partial`) `Reset(snap)` [C]: only if flag 0x40 is set (default):
`rotTarget = base` (behind Sora, pitch −10, 2.6). With `snap` the component is turned at once, `rotPrev` is
set and the snap bit is raised (the next update jumps look point and distance too); without it
`resetWait = 0.5`, so for half a second `rotCur` runs after `rotTarget` with the fast rate (2.4) and 2.7 is off.

The button: action `ActionCameraReset` = `Gamepad_RightThumbstick` (R3) in `DefaultInput.ini` [A], bound in
`ATresPlayerControllerBase` 0x1414160F0 to the handler 0x14140ECC0 [C], which calls slot 244 of the view target
with `snap = false` when camera input is enabled. So **R3 does not snap**: `rotTarget` jumps to behind Sora and
the view eases to it through 2.4 (99 % in ≈ 0.54 s); the distance and the look point are not touched. Whether
the handler fires on press or release was not decoded [?]. `ResetCamera` (0x141D05A10) is the same slot for
Blueprints, with a snap argument.

### 2.9 Start state

`StartTresCamera` = slot 242 (0x14060BA20, `no-verdict`) [C]: `rotTarget = base` (slot 299: Sora's yaw, pitch
−10, roll 0), the component is turned to it, `m_LookPosition = lookSmooth = target point` (slot 284).
The base part (`FUN_1405D9CC0`) registers the camera with the manager; the snap bit (this+0x468 bit 0, set by the
base constructor) makes the first update jump `distSmooth` to the wanted distance and `lookSmooth` to the target
point. So the camera **starts directly behind Sora: yaw = his yaw, pitch −10, distance 400, no blend**.
For a port: initial `rotTarget = rotCur = rotPrev = (−10, soraYaw, 0)`, `lookSmooth` = target point, distance 400,
all timers 0, `autoPower` 0, slope ring all 0.

---

## 3. Collision

All of it is switched off by `TresCameraConfig.m_bNoCollision` (false) or this+0x704 bit 0x10 (`EnableCollision
(false)`, slot 282). Every query is a **sphere on trace channel 4 (`ECC_Camera`), simple collision**
(`bTraceComplex` false). Geometry decides through its collision profile's response to `Camera`: Block = a
camera wall; Overlap = an object that is faded instead (3.4); Ignore. The `TresPlayerPawn` profile ignores the
channel [A, `assets/KINGDOM HEARTS III/Config/DefaultEngine.ini`]. Engine calls, identified by argument shape
[U]: `FUN_143FDC0C0` SweepMultiByChannel, `FUN_143FDC540` SweepSingleByChannel, `FUN_143FD5D30`
OverlapMultiByChannel.

**Probe radius** `r = FUN_1405CCFD0(this)` [I] = manager+0x2A40 (current camera-shake amplitude, 0 without
shake) + (manager+0x2A44 > 0 ? manager+0x2A44 (`BP_SetOverrideCameraSize`) : this+0x464 = **20.0**).

### 3.1 Step 8: the look point — `FUN_1406054D0` [constants and flags I; geometry C, `same-partial`]

Runs only with flag 0x08 set and this+0x6AC bit 0 clear (defaults).

```
(water-current surface probe: sphere r+1 from look+(0,0,100) down to look against channel 21 only; levels with water currents)
r1 = r + 1
(radius, halfHeight) = target actor's collision cylinder (actor vt+0x548)
base = target actor root location + up * max(r1, halfHeight)
sweep sphere(r1) from base to lookSmooth, channel 4,
      response: everything ignored except WorldStatic, WorldDynamic and TresGimmick (18) = Block     // FUN_1405E86D0
first blocking hit: lookSmooth = hit location (if the sweep starts inside: pushed out along the normal by the
                    penetration depth, then swept again);  return "corrected"
```

The correction is written into `lookSmooth` itself, so after the obstacle the look point returns through the
normal position lag (2.3.2). "Corrected" also switches 2.7 off for the frame. The values `vt+0x548` returns for
Sora were not read [?]; they only matter when something stands between his body and the look point above his
head (a low ceiling).

### 3.2 Step 10: slot 287 does not run

this+0x6B4 is not a collision switch but the "outside wall check type" (0 = none; constructor value 0; set
only through `SetOutsideWallCheckType` 0x141D5A090 by state-event, rail-slide, airplane … cameras). It is 0 for
the field camera, so the 7801-byte function 0x1406223B0 is never called from `FUN_1406033C0` [I for the gate;
C for the callers].

### 3.3 Step 15: the camera — `FUN_14061CB40(this, out, cam, look, 1, 1, 1, 1, null)` [main path I]

```
ignored: the target actor (this+0x718 bit 8, default set), the actors / components of the ignore lists
         (this+0x6D8 / +0x6E8, `BP_AddCollisionIgnoredActor`), every component that already blocks a sphere(r)
         placed at the look point (overlap query)
response: everything Block; TresWaterCurrent (21) ignored if the look point sphere overlaps blocking geometry (slot 267)
sweep sphere(r) from the look point to camPos, channel 4:
      first blocking hit -> camPos = hit location (the sphere centre at the hit);  changed
body-collision push-out, at most 4 rounds: for every overlapping (non-blocking) `TresBodyCollPrimitive` that asks
      for it, move camPos out by its penetration vector; if anything moved: sweep again from the look point,
      and re-aim the camera at the look point
see-through pass (3.4)
return changed
```

Then (step 16) `m_CurrentDistance = |lookPt − camPos|` and the component moves to `camPos`. There is no margin
beyond the sphere: the camera centre stays 20 cm from the wall, against a near plane of 3.

- **Pull-in is immediate** (the same frame, any frame rate); there is no pull-in speed.
- **Push-out** is the distance smoothing of step 5: next frame the wanted distance is approached from the
  collided `m_CurrentDistance` with rate this+0x448 = **4.0 /s** (set by the `TresCameraNormal` constructor at
  0x1405FC69C [I]; the base class value 8.0 is overwritten), and the result is swept again. While the obstacle
  is still there the camera therefore presses against it each frame and is put back; once it is clear it
  recovers 6.67 % of the gap per 60 Hz frame.
- The camera is not rotated by this pass (except the re-aim after a body push-out). Turning around obstacles is
  2.7, which only acts while Sora moves and the automatic camera is on.
- `m_WallDistance` (700) is not collision: it is the distance used during wall-run. `EnableInterpCollision`
  (this+0x469 bit 4) only affects blends between two cameras.

### 3.4 Near plane and fades

- Near clip plane 3.0, never adapted by the collision code.
- **See-through fade of obstructing objects** (`FUN_1405D87F0`, called from 3.3) [C]: a sphere of radius 1.0 at
  the camera and swept from the look point to the camera, channel 4; every result that *overlaps* (profile
  response Overlap, e.g. the "…OverlapCamera" profiles) and is a character pawn, static / skeletal mesh actor,
  gimmick, destructible, projectile or weapon is registered with the manager (list at manager+0x28A0), which
  tweens its "CameraFade" material parameter to the actor's `m_CameraFadeAlpha` (actor+0x3C0) over
  `|Δalpha| × 0.3 s` and back to 1 when it is no longer hit (`FUN_1405FBFF0`). Player pawns are excluded unless
  the controller says otherwise (controller vt+0x8A0).
- **No fade of Sora by camera distance exists in the camera code.** If the game fades him at close range it is
  done in his materials (`MaterialExpressionSQEXCameraFadeOutput`) or on the pawn side [?].

A stage-1 port implements 3.1–3.3 against its own `World` (sphere sweeps; "Camera channel" = whatever the test
level calls solid) or stubs them (no pull-in), and stubs 3.4.

---

## 4. Frame-rate dependence

The camera runs once per rendered frame with that frame's `dt` (times the camera time rate); it has no fixed
step of its own. What that does to each part:

| part | how it depends on the frame rate |
|---|---|
| look-point lag, distance smoothing, rotation smoothing, automatic follow, wall-avoiding lerps | `lerpStep` / `rotLerpStep`: slices of 1/60 s with factor `clamp(slice × rate, 0, 1)`, the last slice shorter. Identical to a 60 Hz simulation when `dt` is a multiple of 1/60; at higher rates each frame is one short first-order step (`1 − rate × dt` per frame instead of `0.8667^(60 dt)`), so the lag is slightly different (at 120 Hz the per-1/60 s retention is 0.8711 instead of 0.8667). A fixed-step port at 1/60 reproduces the 60 Hz game exactly. |
| stick rotation | `stick × speed × dt`: frame-rate independent. The "stick active" flag is false on a frame with `dt = 0`. |
| timers `stickWait` 1.5 s, `stillWait` 0.3 s, `resetWait` 0.5 s, `autoPower` ramp 1.2 /s | plain `dt` arithmetic: independent, resolution one frame |
| fall tilt | `deg/s × dt`: independent |
| **slope pitch** | **per frame**: one sample per update into a 16-entry ring, a 7-sample agreement test, a 16-sample mean. At 60 Hz the mean spans 0.267 s; at another rate it spans 16 frames of that rate. |
| collision | stateless per frame; pull-in in the same frame; at most 4 body push-out rounds and at most 10 slides of the path probe per frame |
| the yaw the controller reads | one frame old (1.4): the staleness in seconds is one frame time |
| the velocity the camera reads | the displacement of the movement component's last tick divided by its `dt` (2.6.2) |
| the quaternion variant of the rotation smoothing (not default) | `clamp(rate × dt, 0, 1)` without slicing |
| see-through fades | `dt`-driven tweens (0.3 s per unit alpha) |

In the port's terms (fixed 1/60 step, port plan 3): step the camera once per fixed step, after Sora's step, with
`dt = 1/60`; feed Sora's next step the camera yaw produced by the previous camera step.

---

## 5. What else can change the field camera (not stage 1)

Everything below is outside stage 1. Each line: what it is, what triggers it, where it enters.

| # | what | trigger | entry point | stage-1 stub |
|---|---|---|---|---|
| 1 | **Lock-on**: a child camera `TresCameraNormalLoc` (class this+0x410, ctor 0x1405FC7E0) takes over; the `Lockon*` properties, `m_RotationSpeedMax/MinDistance`, `MaxRate` | player lock-on | manager `m_LockonCamera` +0x28B8, `m_LockonTarget` +0x28C0; start `FUN_1405F0990`, end `FUN_1405F0140`; slot 253 (vt+0x7E8) "is lock-on" returns false on the plain camera | slot 253 = false |
| 2 | **Battle / non-battle** distance and base rotation (`m_NonBattleDistance`, `m_NonBattleRotation`, 1.0 s blend this+0x9D4..0x9DC) | battle state | slot 290, `FUN_1406068F0`, Tick 0x14060CE60 | inert for Sora: `m_NonBattleDistance` = 0 |
| 3 | **Wall-run** (player state id 8): base yaw ±40 / pitch 0, pitch max 60, distance toward `m_WallDistance` 700; **rail slide**: base pitch 5; **slope slide** (state 10) uses the slope pitch like Run | those player states | `FUN_140608790`, `FUN_1406068F0`, slot 290, `FUN_1405FFA60` | "not in those states" |
| 4 | **Dive mode of the Jump state**: distance 30, pitch −85, FOV 120, target offset Z −35 over 0.5 s (controller+0x924..0x930) | not determined (dive fall, Jump sub-steps 7–9?) | Jump state slot 28 0x1413A0420 | none |
| 5 | **Long fall tilt** (part of 2.6): pitch down while falling faster than 800 cm/s | automatic camera on | `FUN_1405FFA60` | in the model already |
| 6 | **Camera position effects** (offsets driven by attack animations) | `TresAnimNotifyState_CameraPosEffect` | manager `m_CameraPositionEffects` +0x2A48; `FUN_1405F1760`, `FUN_1405EB030`, `FUN_1405EB190` | none |
| 7 | **Camera shake** (`TresCameraShake`, stock modifier list; "damage beat direction"); also widens the probe radius (manager+0x2A40) and reaches the movement yaw (1.4) | hits, damage, effects | `BP_EnableCameraDamageBeatDirection` 0x141D28000; stock `PlayCameraShake` | none; radius term 0 |
| 8 | **State-event cameras** (`Blueprints/Camera/p_ex001_StateEvent/*`: attractions, form changes, links, NPC talk …) | player states / events push them on the stack | load asset `PlayerStateEventCamera`; `StartTresCamera` slot 242 → manager `FUN_1405E5AC0` | none |
| 9 | **Any other camera on the stack** (Fix, FixTarget, Path, Diving, Airplane, FirstPerson, HoldCamera selfie, ShooterMode, AthleticFlow …) with a blend (`m_AddTransitionParams` / `m_DelTransitionParams`, ease-out exp 2, "look rotation distance" blend `FUN_1405F6190` / `FUN_1405F6490`) | situation commands, attractions, gimmicks, level scripts | manager `FUN_1405FB910`, `DoUpdateCamera` 0x1405E9BE0 | the field camera is the only camera |
| 10 | **Camera path routes** | level actors `TresCameraPathRouteActor` near the pawn | manager Tick 0x1405F5260 → `FUN_1405FB040` | none |
| 11 | **Level camera volumes / actors**: `TresCameraWall` (+`WallLoc`), `TresCameraHe02Wall`, `SQEX_CameraInterest`, `SQEX_CameraUpVector`, `CameraBlockingVolume` | level placement | `BP_EnableTresCameraWall` 0x141D28240, `BP_EnableTresCameraHe02Wall` 0x141D281B0 | none |
| 12 | **Boss-fight variations** of the normal camera (`pc_cam_variation`) as second normal camera | level / boss scripts | `BP_SetTresCameraNormalDefault` 0x141D29CA0 → manager+0x28E0 | none |
| 13 | **Blueprint setters on the camera** (distance, base rotation, pitch limits, aim rotation, target offsets, lerp coefficients, enable move rotation / world base rotation / controller input / collision, controller stick), most with timed tweens (17 interpolators at this+0x484.., advanced in Tick by the slot 268 chain) | level scripts, events | `exec*` natives 0x141D03250..0x141D2B270, 0x141D57F30..0x141D59410 | defaults stay |
| 14 | **Roll offset** (also flips the stick beyond ±90°), **hold frames** (manager+0x2A36), **fixed dt** (manager+0x2A34 bit 8), cancel interpolation | scripts | `SetTresCameraRollOffset` 0x141D2B1F0; `DoUpdateCamera` | 0 / off |
| 15 | **Frozen input view point** after a camera cut (1.4) | `BP_EnableKeepChangeRotation` on the new camera | `FUN_1405F83A0`, `FUN_1405FBD10` | off |
| 16 | **Time rate** of object type 9 (camera) | slow-motion effects | `UWorld::Tick` | 1.0 |
| 17 | **Post process, depth of field, motion blur, lens effects, see-through fades** | per-camera execs 0x141D03700..0x141D04110; `FUN_1405FBFF0` | — | none |
| 18 | **Cutscene / event cameras** (`TresCameraAnim`, Matinee / sequencer, `MBCameraData` tracks): a non-Tres view target goes through the stock path | events | stock `SetViewTarget` | none |
| 19 | **Water**: camera kept above the surface, look point probe against water currents | levels with water | `FUN_1406048D0`, `FUN_14062D1F0` | no water |
| 20 | **User options**: automatic camera on/off, horizontal / vertical reverse, two sensitivities (2.5, 2.6.1) | options menu | config block | shipped defaults |

---

## 6. Confidence and open questions

### 6.1 Per statement

| statement | level |
|---|---|
| Constructor defaults of the five classes (distance 400, pitch −10, limits −70 / 50, speeds 120, lerp rates 8 / 8 / 30, distance rate 4, position limit 100, `m_TargetMaxSpeed` 800, `m_AutoTurningPower` 1.2, wait 1.5, flags 0xDC, socket name) | [I] (constructors have verdict `same`; the `TresCameraNormal` stores re-read in the disassembly) |
| Blueprint values (offset 48, play 0.3, near distance 200, FOV 100) and the load-asset reference | [A] |
| That the spawned camera really is this blueprint class, and that nothing changes its values at run time in exploration | [C] for the spawn path; the absence of level-script changes is **inferred** — probe |
| Order of the update (2.2), the snap bit, the "fast" condition, the `doAvoid` conditions | [I] |
| Smoothing primitives (1/60 slices, clamp, lag limit as a 3D clamp) | [I] (`FUN_1405D0850` `same`; `FUN_1405D0B30`, `FUN_1405D0650` read in the disassembly); the 360 / 180 wrap constants are run-time globals → [U] |
| Target point = socket `Zup_Cam` + offsets | [C] for the lookup code (`FUN_140624260` `not-compiled`, `FUN_140621E00` `same-partial`, `FUN_140622200` `no-verdict`); [A] for socket, bone and "not animated in the 29 stage-1 clips". The resulting 150.34 cm is **derived** — probe (`rawTarget − actor location`) |
| Distance rule (2.3.3) | [I] |
| Stick path (2.5): gates, sign flip, dead zone rescale, reverse bits, sensitivity map, delta, clamp | [I]; engine axis config [A] + stock `MassageAxisInput` [U]; shipped config defaults [I] (`same` constructor); the option names of the bits are **inferred** from what the code does with them (the menu texts were not read) |
| Raw sign of `Gamepad_RightY` | **not determined** — probe |
| Rotation smoothing (2.4) | [I] (`FUN_1405D8170` `same`) |
| Automatic follow (2.6): conditions, timer, ramp, rate, facing test, fall tilt, clamp | [I] |
| Velocity source `MyMovement+0x9A0` = displacement / dt | pointer [I]; the meaning from [C] of 0x140591690 (`differs`) with the stores read in the disassembly; when in the frame it is written relative to the move is **inferred** |
| Slope pitch (2.6.3) constants and comparisons | [I]; the agreement / mean function `FUN_140605BD0` [C] (`same-partial`) |
| Wall-avoiding rotation (2.7): gates, timers, constants, the two lerps | [I]; the path probe `FUN_14060F490` [C] with its constants [I] |
| Reset (2.8) and its binding to R3 | slot 244 [C + key instructions I]; the action mapping [A]; the handler 0x14140ECC0 [C]; press vs release **not determined** |
| Start state (2.9) | [C] |
| Collision 3.1 | flags and constants [I], geometry [C] |
| Collision 3.3 main path, channel 4, radius 20, no margin, slot 287 never called | [I] |
| See-through fade 3.4 | [C] |
| "No distance fade of Sora in the camera code" | negative result of reading the camera classes — **inferred**; a material-side fade is possible |
| Update chain and "camera after the tick groups" (1.3) | [C] (`UWorld::Tick` `differs`) + one call site [I]; tick groups of controller / camera **inferred** |
| Controller reads last frame's `CameraCache` (1.4); pitch zeroed, roll kept | [I] for the call and the zeroing; `FUN_1405E6B80` `same`; "last frame" follows from the order above |
| FOV 100, constrained 16:9, near 3 (1.5) | [A] + [I]; "no manager override" and "no FOV change with speed" are negative results [C] |

### 6.2 What only the running game can settle

1. Sign of the raw right stick Y (does "up" raise or lower the view with the shipped setting).
2. The live values of the camera's parameters in a normal field map (is the class the blueprint, is anything
   overwritten: `m_Distance`, `m_Rotation`, limits, lerp rates, play, flags 0xDC, `m_NearDistance`).
3. `rawTarget` relative to the actor: 150.34 cm above the origin in all stage-1 states (idle, run, jump apex,
   landing)? This also settles whether the mesh / socket is smoothed separately from the capsule.
4. The update order and the one-frame-old yaw (hooks in section 7).
5. The velocity vector the camera uses while running on a slope, against a wall, and during run start (root
   motion): `MyMovement+0x9A0` vs `Velocity`.
6. The automatic follow in numbers: `stickWait` pinned at 1.5 while the stick is held, `autoPower` ramp, the
   rate, the −0.8 facing threshold; whether the option bit is really set in a fresh save.
7. Collision behaviour against real geometry: radius 20, immediate pull-in, recovery at 4 /s, when 2.7 fires.
8. Whether Sora fades when the camera is close (visual check), and the values of his collision cylinder used in 3.1.
9. The tick groups of controller, pawn, camera actor and manager.
10. `inputSource` (this+0x460) with a gamepad (expected 2) and with mouse / keyboard.

---

## 7. What the runtime probe should log

Per rendered frame, with a frame counter and the frame's `dt`. Pointers: controller → `+0x830` manager →
`+0x28D8` camera (`m_CameraNormal1st`) → `+0x188` root component (= `+0x3E0` CameraComponent).

**Camera actor (`TresCameraNormal`)**

| offset | type | what |
|---|---|---|
| +0x898 | 3 × f32 | `rotTarget` (pitch, yaw, roll) |
| +0x8A4 | 3 × f32 | `stickDelta` of the frame |
| +0x46C | 3 × f32 | `rotPrev` |
| +0x8B0 | 3 × f32 | `lookSmooth` |
| +0x660 | 3 × f32 | `m_LookPosition` |
| +0x8BC, +0x428 | f32, f32 | `distSmooth`, `m_CurrentDistance` |
| +0x8DC, +0x8E4, +0x8E8, +0x8EC | f32 × 4 | `autoPower`, `stickWait`, `stillWait`, `resetWait` |
| +0x8F0, +0x8F1 | u8, u8 | flags (expect 0xDC with bit 0x02 toggling), `m_bWorldBaseRotation` |
| +0x468, +0x469, +0x460 | u8 × 3 | snap bit etc., input / slerp bits, `inputSource` |
| +0x6F8 | 3 × f32 | base rotation offsets (+0x6FC = Sora's yaw as the camera saw it) |
| +0x988, +0x98C (16 × f32), +0x9CC | i32, f32 × 16, f32 | slope ring index, samples, mean |
| +0x8C0 | 3 × f32 | target location delta of the camera Tick |
| once per session: +0x800, +0x804, +0x6A8, +0x828 (3), +0x880, +0x884, +0x888, +0x890, +0x894, +0x8D8, +0x8E0, +0x900, +0x904, +0x908, +0x90C, +0x430, +0x434, +0x438, +0x43C, +0x448, +0x44C, +0x450, +0x454, +0x458, +0x45C, +0x464, +0x66C (3), +0x678 (3), +0x684 (3), +0x690 (ptr), +0x698 (FName), +0x6B0, +0x6B4 (i32), +0x704 (u8), +0x718 (u8), +0x844 (i32) | f32 unless noted | every parameter of section 9, to confirm the live values |

**Root / camera component** (camera+0x188): +0x2B0 3 × f32 world location (final camera position); +0x2FC
3 × f32 `RelativeRotation` = `rotCur`; +0x418 f32 FieldOfView; +0x42C f32 near clip.

**Manager (`TresCameraManager`)**: +0x7B0 f32 `CameraCache.TimeStamp`; +0x7C0 3 × f32 POV location; +0x7CC
3 × f32 POV rotation; +0x7D8 f32 FOV; +0x1710 ptr view target; +0x1ED0 ptr pending view target; +0x2838 ptr +
i32 `m_CameraArray`; +0x28B8 ptr lock-on camera; +0x28D0 ptr config (its +0x40 u8); +0x28E0 ptr second normal
camera; +0x290C u8 input freeze; +0x2A34 u8, +0x2A36 i8, +0x2A38 f32, +0x2A3C f32, +0x2A40 f32, +0x2A44 f32.

**Controller (`TresPlayerControllerSora`)**: +0x848 / +0x84C f32 left stick Y / X; +0x858 3 × f32 move rotator
(yaw at +0x85C); +0x421 u8 (look input ignored); the `FKeyState` of `Gamepad_RightX` / `Gamepad_RightY` in its
PlayerInput (value and flags), or the raw pad right stick.

**Pawn**: root component world location (+0x2B0) and rotation; `MyMovement`: +0x9A0 3 × f32 (the velocity the
camera uses) next to `Velocity`; state id (`FUN_1405ABE20(pawn+0x450)`) and sub-step.

**Config**: 9 bytes at `GameInstance + 0x870 + 0x2E8` (byte 0 bits, bytes 5 and 6).

**Entry hooks** (frame-ordered log of first entry per frame): 0x1413F1AB0 (Sora input), 0x140598190 (movement
component), 0x14060CE60 (camera Tick), 0x1405F55A0 (manager update), 0x1406033C0 (follow update), 0x144430D10
(FillCameraCache). In 0x1413F1AB0 also the rotation returned by 0x1405E6B80.

**Checks the log should pass** if this document is right:

- `|component location − m_LookPosition| == m_CurrentDistance` every frame;
- the rotation returned to the input function == `CameraCache.POV.Rotation` of the previous frame;
- `stickDelta.yaw == X × 1.2424 × 120 × dt` at the shipped sensitivity, 0 below 0.475 raw deflection;
- `rotCur[n] = rotPrev + 0.1333 × (rotTarget − rotPrev)` per 1/60 s;
- `lookSmooth` trails the target by ≈ 74 cm at 680 cm/s and never by more than 100;
- `stickWait` = 1.5 while the stick is held, then counts down; `autoPower` rises by 1.2 /s only after it reached 0;
- `rawTarget − actor location` = (0, 0, 150.34) (compute from `lookSmooth` when standing still);
- this+0x70C never changes (slot 287 not called).

---

## 8. Functions

| address | what | verdict | read as |
|---|---|---|---|
| 0x1406033C0 | field camera update (section 2.2) | not-compiled | I |
| 0x1406023B0 / 0x1405C8FD0 | `CalcCamera` of `TresCameraNormal` / `TresCameraBase` (slot 179) | same / no-verdict | C |
| 0x140607ED0 | slot 301: stick → `rotTarget` | differs | I |
| 0x1405CD290 / 0x1405CD3F0 / 0x1405CF0F0 | slots 259 / 258 / 266: stick gate + sensitivity / read + dead zone + reverse / delta | — | I |
| 0x1405EC530 | sensitivity map | — | I |
| 0x140600630 → 0x1405FFA60 | slot 300 → automatic follow | same / same-partial | I |
| 0x1405EBDF0 | slot 299: base rotation (+ actor yaw) | differs | I |
| 0x1406068F0 | base rotation source (`m_Rotation`, non-battle, rail, wall-run) | differs | C |
| 0x140606B10 / 0x140605BD0 | slot 302: slope pitch / ring agreement + mean | differs / same-partial | I / C |
| 0x140607BE0, 0x1405AB330 | target velocity (pawn slot 453 → `MyMovement+0x9A0`) | same / — | I |
| 0x140591690 | movement component: displacement velocity | differs | C (+ stores I) |
| 0x1406086B0 | automatic-camera option bit | same | I |
| 0x140608440 | "not running toward the camera" (dot ≥ −0.8) | same | I |
| 0x1405D8170 / 0x1405D0650 | rotation smoothing / rotator lerp | same / differs | I |
| 0x1405D0850 / 0x1405D0B30 | float lerp / vector lerp + lag clamp | same / differs | I |
| 0x1405C90A0 / 0x1405CE7F0 | point + rotation × distance | differs | C |
| 0x140606390 | slot 290: wanted distance | differs | I |
| 0x140605150 / 0x140621B90 / 0x140624260 | slot 303 / slot 284 / target point | differs / differs / not-compiled | C |
| 0x140621E00 / 0x140622200 / 0x140621FF0 | socket position / socket-or-bone lookup / socket matrix | same-partial / no-verdict / same-partial | C |
| 0x140608970 / 0x1405D0DB0 / 0x1406088D0 | slot 260 pitch clamp / clamp angle / yaw limit | differs | I / I / C |
| 0x140603FF0 / 0x14060F490 | wall-avoiding rotation / path probe | differs | I / C |
| 0x1406054D0 / 0x1405E86D0 | look-point collision / its sweep | same-partial / — | I + C |
| 0x14061CB40 | camera sweep | — | I (main path) |
| 0x1405CCFD0 | probe radius | no-verdict | I |
| 0x1405D87F0 / 0x1405FBFF0 | see-through registration / fade update | — | C |
| 0x1406223B0 | slot 287: outside-wall check (not used by the field camera) | differs | C |
| 0x1406048D0 | water surface | no-verdict | C |
| 0x140609C40 | slot 244: reset | same-partial | C (+ I) |
| 0x14140ECC0, 0x1414160F0 | reset input handler, its binding | — | C |
| 0x14060BA20 / 0x1405D9CC0 | slot 242 `StartTresCamera` / base part | no-verdict | C |
| 0x14060CE60, 0x14060ECA0 … 0x1405DD180 | camera Tick, timed tweens (slot 268 chain) | no-verdict, same … differs | C |
| 0x140608790 | wall-run test (state id 8) | same-partial | C |
| 0x1405ED7F0, 0x141434870, 0x1414473D0 | camera creation at `SetPawn` | same-partial | C |
| 0x1405F55A0, 0x1405E9BE0, 0x1405FB910 | manager update, `DoUpdateCamera`, camera stack | — / differs / — | C |
| 0x1405E6B80 | input camera view point (what the controller reads) | same | I |
| 0x1405E4F50, 0x140E84340 | `TresCameraConfig` ctor, user configuration ctor | — / same | I |
| 0x1405C02C0, 0x1406161D0, 0x1405E1DE0, 0x1405FD1A0, 0x1405FC450, 0x1405E2970 | constructors (Base, Target, DistanceTarget, RotTarget, Normal, Manager) | same | I |

---

## 9. Parameters

Sheet `sheets/port/sora-camera.json` (81 rows; `node build/sora_tmp/cam/mksheet.js` wrote it and this table). `confirmed` = value and
use read at instruction level or in a class default object / ini; `inferred` = decompiled C, stock engine
knowledge or derived; `guess` = not determined (value empty).

| id | value | unit | where the value is | conf. | note |
|---|---|---|---|---|---|
| `target.socket` | Zup_Cam | - | ATresCameraTarget ctor 0x1406161D0 (same): FName at this+0x698 | confirmed | socket on bone t_camera (parent trans), assets_converted/packages/KINGDOM HEARTS III/Content/Character/pc/p_ex001/mdl/p_ex001_0_Skeleton.uasset.json |
| `target.socket_height` | 102.338 | cm | Character/pc/p_ex001/mdl/p_ex001_0_Skeleton referenceSkeleton bone t_camera translation (0,-102.338,0) under root rotation -90 deg X | inferred | height above the actor origin (foot level); derived, to be confirmed by the probe |
| `target.offset_z` | 48 | cm | Blueprints/Camera/Setting/p_ex001/BP_TresCameraNormal_ex001 export Default__BP_TresCameraNormal_ex001_C m_TargetInfo.m_TargetOffset (0,0,48) | confirmed | target point = actor origin + 150.338 for stage 1 |
| `target.local_offset` | 0 | cm | ATresCameraTarget ctor 0x1406161D0: m_TargetLocalOffset = zero vector | confirmed | all three axes |
| `target.camera_local_offset` | 0 | cm | ATresCameraNormal ctor 0x1405FC450: m_CameraLocalOffset = zero vector | confirmed | all three axes |
| `look.lerp_rate` | 8 | 1/s | ATresCameraBase ctor 0x1405C02C0: m_fPositionLerpPer (this+0x450) | confirmed | same on X, Y and Z |
| `look.lag_limit` | 100 | cm | ATresCameraBase ctor 0x1405C02C0: m_fPositionLerpLimit (this+0x44C) | confirmed | negative would disable the clamp |
| `lerp.slice` | 0.016666668 | s | constant 0x145D314C4 | confirmed | 1/60 |
| `distance.base` | 400 | cm | ATresCameraNormal ctor 0x1405FC450, mov dword [rbx+800h] at 0x1405FC491 (m_Distance) | confirmed | the same in and out of battle (m_NonBattleDistance = 0) |
| `distance.near` | 200 | cm | Blueprints/Camera/Setting/p_ex001/BP_TresCameraNormal_ex001 m_NearDistance | confirmed | native default 0 = off |
| `distance.scale` | 1 | - | ATresCameraDistanceTarget ctor 0x1405E1DE0: this+0x804 | confirmed | changed by SetDistance tweens only |
| `distance.offset` | 0 | cm | m_TargetInfo +0x30 (this+0x6A8), ATresCameraTarget ctor | confirmed | set by FUN_14061AB60 only when m_TargetInfo flag +0x34 bit 0 is set (it is not) |
| `distance.min` | 0.1 | cm | constant 0x145C87D00 at 0x1406064FE | confirmed | - |
| `distance.lerp_rate` | 4 | 1/s | ATresCameraNormal ctor, mov dword [rbx+448h],40800000h at 0x1405FC69C | confirmed | the push-out speed after a collision; the base class value 8.0 is overwritten |
| `distance.current_initial` | 100 | cm | ATresCameraBase ctor 0x1405C02C0: m_CurrentDistance (this+0x428) | confirmed | - |
| `distance.wall_run` | 700 | cm | ATresCameraNormal ctor 0x1405FC49B (m_WallDistance) | confirmed | not stage 1 |
| `rotation.base_pitch` | -10 | deg | ATresCameraNormal ctor, mov dword [rbx+828h],0C1200000h at 0x1405FC487 (m_Rotation.Pitch) | confirmed | start pitch, reset pitch, the pitch the automatic follow returns to |
| `rotation.base_yaw_offset` | 0 | deg | ATresCameraRotTarget ctor 0x1405FD1A0 (m_Rotation.Yaw = 0) | confirmed | start yaw = Sora yaw |
| `rotation.pitch_min` | -70 | deg | ATresCameraNormal ctor 0x1405FC450 (m_LimitPitchMin, this+0x880) | confirmed | pitch positive = looking up |
| `rotation.pitch_max` | 50 | deg | ATresCameraNormal ctor 0x1405FC450 (m_LimitPitchMax, this+0x884) | confirmed | 60 during wall-run |
| `rotation.yaw_limit` | 0 | deg | ATresCameraNormal ctor 0x1405FC450 (m_LimitYawMax, this+0x888) | confirmed | 0 = no limit |
| `rotation.lerp_rate` | 8 | 1/s | ATresCameraBase ctor 0x1405C02C0: m_fRotationLerpPer (this+0x454) | confirmed | rotCur follows rotTarget |
| `rotation.lerp_rate_fast_gamepad` | 8 | 1/s | ATresCameraBase ctor 0x1405C02C0: this+0x458 | confirmed | fast = stick active or stickWait > 0 or resetWait > 0 |
| `rotation.lerp_rate_fast_other` | 30 | 1/s | ATresCameraNormal ctor, mov dword [rbx+45Ch],41F00000h at 0x1405FC6A6 | confirmed | non-gamepad source of the right-stick keys |
| `rotation.smoothing_enabled` | true | - | ATresCameraNormal ctor: flags this+0x8F0 = 0xDC, bit 0x10 | confirmed | - |
| `stick.engine_dead_zone` | 0.25 | - | assets/KINGDOM HEARTS III/Config/DefaultInput.ini AxisConfig Gamepad_RightX / Gamepad_RightY DeadZone | inferred | Sensitivity 1, Exponent 1, bInvert False |
| `stick.camera_dead_zone_x` | 0.3 | - | Blueprints/Camera/Setting/p_ex001/BP_TresCameraNormal_ex001 m_AnalogStickPlayX | confirmed | native default 0.1; raw threshold 0.25 + 0.3 x 0.75 = 0.475 |
| `stick.camera_dead_zone_y` | 0.3 | - | Blueprints/Camera/Setting/p_ex001/BP_TresCameraNormal_ex001 m_AnalogStickPlayY | confirmed | native default 0.1 |
| `stick.yaw_speed` | 120 | deg/s | ATresCameraBase ctor 0x1405C02C0: m_RotationSpeedX (this+0x430) | confirmed | per unit of rescaled stick, before the sensitivity scale |
| `stick.pitch_speed` | 120 | deg/s | ATresCameraBase ctor 0x1405C02C0: m_RotationSpeedY (this+0x434) | confirmed | - |
| `stick.sensitivity_x_default` | 50 | - | user configuration ctor 0x140E84340 (same): byte 5 of the block at GameInstance+0x870+0x2E8 | confirmed | range 1..100 -> 0.5..2.4 |
| `stick.sensitivity_y_default` | 50 | - | user configuration ctor 0x140E84340 (same): byte 6 | confirmed | - |
| `stick.sensitivity_scale_default` | 1.2424 | - | derived: 0.5 + 0.75 x 2 x 49/99 | inferred | derived from the two rows above |
| `stick.scale_non_gamepad` | 1.25 | - | constant 0x145D4A764 | confirmed | replaces the sensitivity scale |
| `stick.y_sign_flip` | true | - | xorps at 0x1405CD525 | confirmed | assumes raw Gamepad_RightY is positive for up: not determined, probe |
| `stick.reverse_x_default` | false | - | user configuration ctor 0x140E84340: byte 0 = 0xC3, bit 0x04 clear | confirmed | option name inferred |
| `stick.reverse_y_default` | false | - | user configuration ctor 0x140E84340: byte 0 = 0xC3, bit 0x08 clear | confirmed | option name inferred |
| `stick.response_exponent` | 1 | - | slot 258 / 266: no power, no time term | confirmed | - |
| `auto.option_default` | true | - | user configuration ctor 0x140E84340: byte 0 = 0xC3, bit 0x02 set | confirmed | option name (automatic field camera) inferred |
| `auto.enabled_flag` | true | - | ATresCameraNormal ctor: flags 0xDC, bit 0x80 (m_bMoveRotation) | confirmed | - |
| `auto.wait_after_stick` | 1.5 | s | ATresCameraNormal ctor 0x1405FC614 (m_MoveRotationWaitTime, this+0x8E0) | confirmed | - |
| `auto.power_max` | 1.2 | - | ATresCameraNormal ctor 0x1405FC609 (m_AutoTurningPower, this+0x8D8) | confirmed | ramp 0 -> 1.2 in 1 s; reset when the wait ends and when speed <= 1 |
| `auto.speed_reference` | 800 | cm/s | ATresCameraNormal ctor 0x1405FC5FF (m_TargetMaxSpeed, this+0x890) | confirmed | 1.02 /s at 680, 1.47 /s at 980 |
| `auto.min_speed` | 1 | cm/s | constant 1.0 at 0x1405FFEF3 | confirmed | Idle state forces the velocity to 0 |
| `auto.facing_threshold` | -0.8 | - | constant 0x145D6C2C4 passed at 0x140603503 | confirmed | angle below 143.13 deg |
| `auto.fall_speed_threshold` | -800 | cm/s | constant 0x145D6C2D4 at 0x1406001F5 | confirmed | - |
| `auto.fall_tilt_gain` | 0.05 | deg/cm | constant 0x145C9C4BC at 0x1406001FE | confirmed | - |
| `auto.fall_tilt_floor` | -50 | deg | constant 0x145CF35C0 at 0x14060020E | confirmed | - |
| `slope.dead_zone` | 15 | deg | constants 0x145D6C2C8 / 0x145C9C510 at 0x140606E1B | confirmed | Run and slope-slide states only |
| `slope.min` | -50 | deg | constant 0x145CF35C0 at 0x140606E09 | confirmed | - |
| `slope.max` | 70 | deg | max(60 - base pitch, 60) at 0x140606DEC | confirmed | - |
| `slope.toward_camera_dot` | -0.7 | - | constant 0x145D6C2C0 at 0x140606D2F | confirmed | - |
| `slope.ring_size` | 16 | frames | cmp eax,10h at 0x140606E38 | confirmed | per frame, not per second |
| `slope.agree_count` | 7 | frames | FUN_140605BD0 (same-partial) | inferred | decompiled C only |
| `avoid.still_wait` | 0.3 | s | mov dword [rbx+8E8h],3E99999Ah at 0x14060422A | confirmed | - |
| `avoid.still_speed` | 10 | cm/s | constant 0x145C87D24 at 0x1406041E3 | confirmed | - |
| `avoid.rate_max` | 10 | 1/s | constant 0x145C87D24 | confirmed | rotCur toward rotTarget |
| `avoid.return_rate` | 4 | 1/s | constant 0x145C9C4FC at 0x1406045F8 | confirmed | - |
| `avoid.slow_speed` | 200 | cm/s | constant 0x145C9C544 at 0x140604589 | confirmed | - |
| `avoid.axis_dot` | 0.99 | - | constant 0x145D0260C at 0x1406043B4 | confirmed | - |
| `avoid.probe_radius_factor` | 2 | - | constant 0x145C9C4F4 at 0x140604417 | confirmed | 40 cm |
| `avoid.max_slides` | 10 | - | cmp esi,0Ah at 0x14060FBC8 | confirmed | - |
| `avoid.look_moved_sq` | 1 | cm^2 | constant 1.0 at 0x140603A5A | confirmed | - |
| `reset.wait` | 0.5 | s | mov dword [..+8ECh],3F000000h in slot 244 (0x140609D43) | confirmed | R3 (ActionCameraReset = Gamepad_RightThumbstick), non-snap |
| `reset.allowed` | true | - | ATresCameraNormal ctor: flags 0xDC, bit 0x40 | confirmed | - |
| `collision.radius` | 20 | cm | ATresCameraBase ctor, this+0x464 = 0x41A00000 (0x1405C04CA) | confirmed | sphere |
| `collision.channel` | 4 | - | argument of the sweeps at 0x14061D83D, 0x14061D3FC | confirmed | - |
| `collision.look_radius_add` | 1 | cm | constant at 0x1406055A0 | confirmed | - |
| `collision.margin` | 0 | cm | ATresCameraTarget ctor: this+0x6B0 = 0 | confirmed | - |
| `collision.pull_in_speed` | - | cm/s | FUN_14061CB40 | confirmed | deliberately none |
| `collision.outside_wall_check` | 0 | - | ATresCameraTarget ctor: this+0x6B4 = 0 | confirmed | - |
| `collision.body_push_rounds` | 4 | - | loop bound in FUN_14061CB40 | inferred | decompiled C only |
| `collision.no_collision_config` | false | - | UTresCameraConfig ctor 0x1405E4F50 clears bits 0 and 1 of +0x40; no ini section | confirmed | - |
| `fade.see_through_time` | 0.3 | s | ATresCameraManager ctor 0x1405E2970: +0x2A2C = 0x3E99999A | inferred | decompiled C only; not a fade of Sora |
| `fade.character_near` | ? | - | not found in the camera classes | guess | open: visual check in the running game |
| `fov.horizontal` | 100 | deg | Blueprints/Camera/Setting/p_ex001/BP_TresCameraNormal_ex001 export CameraComponent FieldOfView | confirmed | native default 90 |
| `fov.aspect_ratio` | 1.777778 | - | UCameraComponent ctor 0x143FA3B70 | inferred | vertical FOV about 67.7 deg |
| `fov.speed_dependence` | false | - | callers of FUN_1405D6F40 (SetFieldOfView) | inferred | negative result |
| `clip.near` | 3 | cm | UCameraComponent ctor, m_NearClipPlane at 0x143FA3C16; DefaultEngine.ini NearClipPlane=3.0 | confirmed | - |
| `order.controller_reads_previous_frame` | true | - | FUN_1405E6B80 called at 0x1413F2144 | inferred | order from UWorld::Tick 0x1441BA050 (decompiled C) |
| `order.camera_after_tick_groups` | true | - | UWorld::Tick 0x1441BA050: UpdateCameraManager (vt+0xE18) at 0x1441BAF30 after tick groups 0..10 | inferred | tick groups of controller / camera not measured |
