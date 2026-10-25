# Sora on foot: turning — addendum to the movement spec (revision 2)

Question asked: the port "stops and lunges" on direction changes with an analog stick; is the
turning of revision 2 a misreading? Everything that decides facing and travel direction on the
ground was re-read in the disassembly for this addendum (the instructions are quoted). **Result:
revision 2 was right on every turning rule, and the port implements each of them as the game's code
does.** The stop-and-lunge is in the game's code, and it has **two** triggers, not one; section 4
works out which stick motions fire them. What remains is to confirm on the real game that it shows
the same thing for the same stick motion (section 6) — if it does not, the difference is upstream of
these rules (the stick values that reach them), not in them.

Addresses are in `base/KH3.exe`; `rsi` = the Run state object, `[rsi+68h]` = the pawn, `xmm11` = dt
(`1413aed93 movaps xmm11,xmm1`), `xmm8` = 0, `xmm9` = 1.0, `xmm10` = 60.0.
State fields: +0x78 sub-step, +0x88 request, +0x8C target yaw, +0x90 magnitude, +0x94 walk, +0x98 grace.

---

## 1. Travel direction vs facing (item 1)

**Travel follows the stick yaw at once; only the facing eases.** The movement input is built from the
target yaw, not from the pawn's forward vector and not from an intermediate.

Run tick `0x1413AED70`, requested tick, sub-step 0 or 3:

```
1413b09ee  movss xmm0,[rsi+8Ch]            ; target yaw (written by TryRun from controller+0x85C)
1413b0a02  movss [rsp+44h],xmm0            ; rotator at [rsp+40h] = (Pitch 0, Yaw target, Roll 0)
1413b0a08  mov dword ptr [rsp+40h],0
1413b0a10  mov dword ptr [rsp+48h],0
...
1413b0bf6  movsd xmm0,[rsp+40h]            ; the same rotator copied to [rsp+50h] = turn target
1413b0bfc  mulss xmm7,xmm11                ; xmm7 = 0.1 (1413af153 movss xmm6,[0.1]; 1413af15b movaps xmm7,xmm6) × dt
1413b0c0e  mulss xmm7,xmm10                ; × 60
1413b0c16  call qword ptr [rax+0DB0h]      ; pawn slot 438 Turn(target rotator, alpha)            (section 2)
1413b0c2f  call qword ptr [rbx+1580h]      ; GetMaxWalkSpeed
1413b0c39  mulss xmm0,xmm10                ; × 60
1413b0c41  call qword ptr [rbx+1640h]      ; SetMaxAcceleration
1413b0c4f  lea rcx,[rsp+40h]               ; the TARGET rotator again
1413b0c57  call 0000000142D93270h          ; FRotator::Vector()
1413b0c6b  movaps xmm2,xmm9                ; scale 1.0
1413b0c6f  call qword ptr [rbx+16A0h]      ; pawn slot 724 SetMovementInputXY(vector, 1.0, bForce)
```

So `ControlInputVector = (cos targetYaw, sin targetYaw, 0)`. In the movement component this becomes
`Acceleration = MaxWalkSpeed × 60 × that vector`, and `CalcVelocity` (spec 3.3) turns the velocity
toward it: `V = V − (V − dir·|V|)·min(8·dt, 1)`, `V += Acceleration·dt`, clamp to the speed. At 60 Hz
a 60° change of the input direction turns the velocity by 34° in the first tick and leaves 11°, 5°, 2°, 1°
after the second to fifth: **the path bends almost at once and the body turns after it** (time constant of
the facing: 9.5 ticks, 0.16 s). For changes below 75° that is the whole turning model: no arc
driven by the facing, a mild crab-walk for a few frames hidden by the run loop's lean blend
(`RunLoopN` axis = yaw delta clamped to ±10, 0.4 s filter).

Per sub-step and state (all [I]):

| Where | Travel direction | Facing |
|---|---|---|
| Run sub 0 (loop), sub 3 | target yaw (above) | `Turn(target, 0.1·dt·60)` |
| Run sub 1 → 2 (`RunStartN` playing) | none from input: no `SetMovementInputXY` (the common part tests the sub-step `1413b0c1c mov eax,[rsi+78h]; test eax,eax; je …; cmp eax,3; jne 1413B0C75`); displacement = the clip's root motion | no `Turn` (same test at `1413b0a18..1413b0a1f`); yaw = the clip's root-motion rotation, applied by `PerformMovement` |
| Run sub 4 (stop clip) | none (velocity was zeroed) | `Turn(target, dt·12)` (`1413b03e3 mulss xmm11,[12.0]` … `1413b040c call [rax+0DB0h]`) |
| Run, no request, before the stop | none (input not written) | `Turn(target, 0.1·dt·60)` (`1413b096d`..`1413b0999`) |
| Walking (walk flag set, sub 0) | target yaw, same code | same `Turn`; blend axis 0 |
| Idle | none | `Turn(own yaw, dt·9)`: no-op |
| Jump (every sub-step) | target yaw when requested (`0x141397BC8`, same `FRotator::Vector` of state+0x8C) | `Turn(target, dt·6)` (`0x141397886`) |

Port: `states.rs` `run_tick` 536–547 (`set_movement_input(s, r.yaw)`, `turn(s, r.yaw, 0.1·dt·60)`),
`set_movement_input` 413–416, `cmc.rs` `calc_velocity`. **Faithful.**

---

## 2. The facing update (item 2)

`Turn` = pawn slot 438 `0x1405DDD90` (verdict `same`):

```
1405ddd99  mov rax,[rcx+188h]              ; RootComponent
1405dddc0  movups xmm0,[rax+2A0h]          ; current world rotation quaternion          → [rsp+50h]
1405dde0f  call 0000000142D8BE00h          ; FRotator(target).Quaternion()               → [rsp+60h]
1405dde14  comiss xmm8,xmm6                ; alpha vs 0
1405dde20  jb short …                      ; alpha < 0 → 0
1405dde26  minss xmm6,[1.0]                ; alpha = min(alpha, 1)
1405dde40  call 0000000142D90CA0h          ; FQuat::Slerp(current, target, alpha)
1405ddebd  rsqrtps …                       ; normalise
1405ddf11  call 0000000142D8DB60h          ; FQuat::Rotator()
1405ddf32  call 0000000143EAD160h          ; AActor::SetActorRotation(rotator)
```

- Endpoints: the actor's current rotation → `(0, targetYaw, 0)`. Alpha clamped to 0..1. Applied to
  the actor directly, every state tick it is called. With pitch = roll = 0 it is
  `yaw += wrap180(target − yaw) × clamp01(alpha)`.
- **No other limit**: no maximum degrees per tick, no different alpha for a large delta, for
  sprinting or for walking. The only other alpha values in the function are for modes that are off
  in plain play (forced move `1413af237`, style momentum `1413b07ef`).
- Nothing else rotates the pawn on the ground:
  - `PhysicsRotation` `0x140590DA0` is still a no-op: `bUseControllerDesiredRotation` (component
    +0x458 bit 0) is cleared by the component ctor (`140571355 and dword ptr [rdi+458h],0FFFFFFFEh`),
    `bOrientRotationToMovement` (bit 1) by the player pawn ctor
    (`1414233cc and dword ptr [rbx+458h],0FFFFFFFDh`); no setter in the game code. `RotationRate`
    1000 is unused.
  - `APawn::FaceRotation` does nothing: `bUseControllerRotationPitch/Yaw/Roll` (pawn+0x3E8 bits 0–2)
    are cleared in the `APawn` ctor (`1443d0cf8 and dword ptr [rdi+3E8h],0FFFFFFF8h`) and again in
    the `ATresPawnBase` ctor (`1405c0645`, same instruction).
  - Root-motion rotation (only `RunStartN`) is applied in `PerformMovement`.

Port: `turn` (`states.rs` 408–410). **Faithful.**

---

## 3. The stop-and-lunge triggers (item 3)

### 3.1 Trigger A: the turn-around (≥ 75°) — complete condition as coded

Only in the switch case for sub-step 0, only when the walk flag is clear:

```
1413af7a8  cmp byte ptr [rsi+94h],0        ; walk flag
1413af7b3  jne 00000001413AFEFBh           ; walking → no turn-around test at all
1413af7b9  mov rbx,[rcx+188h]              ; pawn RootComponent
1413af7c0  movss xmm6,[rsi+8Ch]            ; target yaw
1413af813  movsd xmm0,[rbx+2E0h]           ; component rotator (pitch, yaw) — the ACTOR's facing
1413af84d  shufps xmm0,xmm0,55h            ; yaw
1413af85b  subss xmm6,xmm0                 ; target − facing
1413af885..1413af8d2                       ; FRotator::NormalizeAxis: wrap to (−180, 180]
1413af8d9  andps xmm0,[7FFFFFFFh]          ; absolute value
1413af8e0  comiss xmm0,[75.0]
1413af8e7  jb 00000001413AFA26h            ; below 75 → normal running (sprint timer etc.)
1413af8f4  call qword ptr [rax+16E8h]      ; StopMovement: Velocity = 0
1413af922  call qword ptr [rbx+1C80h]      ; RunStart animation name
1413af95f  mov dword ptr [rsp+20h],2       ; mode 2 (root motion, not looped)
1413af967  call qword ptr [rax+18h]        ; PlayAnim
1413af9a0  movaps xmm1,xmm6                ; blend axis = the signed delta (not clamped)
1413af9a8  call qword ptr [rdx+120h]
1413af9b2  mov [rsi+0B8h],r14d             ; sprint timer = 0
1413af9b9  mov byte ptr [rsi+0BCh],0       ; sprint flag = 0
1413af9c3  call qword ptr [rax+1578h]      ; ResetMaxWalkSpeed
1413af9c9  mov dword ptr [rsi+78h],2       ; sub-step 2
```

- The two yaws: **this frame's target yaw** (stick + camera, from the controller) against the
  **actor's facing** (after last tick's `Turn`). Not the velocity direction, not last frame's target.
- `|delta| ≥ 75` (75 itself fires). Evaluated every tick in sub 0.
- **No other condition**: no speed test, no stick-magnitude test beyond "run, not walk", no timer,
  no persistence counter or hysteresis, no suppression during the 0.2 s grace, while sprinting or
  right after a `RunStartN`; it does not distinguish a stick change from a camera change.
- What it does: velocity to zero; `RunStartN` with the blend axis = delta (the mix of the two
  neighbouring samples: 0° / ±90° / ±180°, each 180 cm of root motion and its yaw in 0.2333 s);
  for the 14 frames of the clip no input is written and `Turn` is not called; then the loop and
  normal running resume. Counting the frame order (spec 1.2): velocity is zero for two movement
  ticks (the tick after the stop, and the first clip update, whose position stays 0), then the
  clip's displacement: 12, 26, 40, 57, 75, 104, **180** cm at the 30 Hz keys — the last thirtieth
  of a second alone covers 76 cm. That profile is the "stop and lunge".

### 3.2 Trigger B: walk → run re-crossing (stick magnitude dips below 0.8)

`TryRun` `0x141440030`, current state RUN (`rbx` = the Run state, `bpl` = requested walk,
`sil` = 0, `r14d` = 1):

```
141440092  comiss xmm6,[0.8]               ; xmm6 = stick magnitude after the dead zone
141440099  setb sil                        ; walk = magnitude < 0.8
…
141440313  test bpl,bpl
141440316  jne short 0000000141440347h     ; requested walk → below
141440318  mov eax,[rbx+78h]               ; sub-step
14144031b  cmp [rbx+94h],sil               ; walk flag currently stored in the state
141440322  je short 0000000141440335h      ; was already running → (only: sub 4 → 1)
141440324  test eax,eax
141440326  jne 0000000141440431h
14144032c  mov [rbx+78h],r14d              ; WAS WALKING, now run, sub-step 0  →  sub-step 1
…
141440347  cmp [rbx+78h],esi               ; requested walk: honoured only in sub-step 0
…
141440441  mov [rbx+94h],bpl               ; store the walk flag
```

Sub-step 1 (`0x1413B007D`..`0x1413B025A`) plays `RunStartN` unconditionally (axis = current delta,
mode 2) and goes to sub-step 2 — there is no test of the grace timer or of anything else on that
path. So **one frame with massaged magnitude below 0.8 while in the run loop, followed by a frame at
or above 0.8, replays the run-start clip**: 14 frames without input or turning, the component's own
velocity braking toward zero while the clip's 180 cm are added. (The 0.2 s grace only keeps the run
animation and run speed during the dip; it does not prevent this.) No `StopMovement` here, so it is
a surge rather than a stop.

Port: turn-around `run_tick` 466–474; `try_run` (walk flag / sub-step rules) and sub 1 at 489–494.
**Faithful to the code on both triggers.**

---

## 4. What stick motion avoids the triggers under the true rules (items 3, 4)

Upstream of the state nothing smooths or rate-limits (re-checked): the controller normalises the
massaged stick vector and converts it to a yaw every frame (`0x1413F1AB0`), `TryRun` stores that
yaw unchanged (`141440431 movss [rbx+8Ch],xmm7`), the Run tick reads it. The only shaping is the
per-axis dead zone (0.25, rescaled; `0x1445AF120`) — which warps directions and magnitudes:

- direction: a stick within 14.5° of an axis at full deflection reads exactly on the axis
  (`asin 0.25`); between 14.5° and 75.5° of physical angle the yaw sweeps the full 0..90° — the
  yaw moves about 1.5 times faster than the thumb there. It is continuous (no jumps), but yes, a
  stick 10° off an axis reads as on-axis: that snapping is the game's.
- magnitude: on an axis "run" needs a raw deflection ≥ 0.85; on a diagonal ≥ 0.953 of a circular
  gate (`m/√2 ≥ 0.25 + 0.6/√2`). An Xbox pad through XInput reports more than 1.0 on diagonals
  (the two axes are independent), which is where the margin comes from.

Trigger A, simulated with the exact rules at 60 Hz (`build/sora_tmp/mv/turnsim.js`: stick swept at
constant angular speed along the rim, dead zone, `delta` against the eased facing):

| Stick sweep | Fires unless the sweep takes at least |
|---|---|
| 45° | never fires |
| 90° | 7 frames (0.12 s) on a unit-radius rim; 5 frames with raw radius 1.1 |
| 135° | 15 frames (0.25 s) |
| 180° (around the rim) | 27 frames (0.45 s) |
| any step ≥ 75° (keyboard 90° switch, flick through the centre to a new direction) | always fires |

Rule of thumb: the facing lags a steadily turning target by 10 × (degrees per tick), so the
turn-around fires when the massaged yaw moves faster than about **450°/s for long enough** — a
quick thumb flick of a quarter turn does it, a deliberate quarter turn does not. A keyboard player
gets a `RunStartN` on every 90° key change (sample `0007L` / `0007R`: a pivot step of 180 cm), by
the same instructions — which is what the L / R / BL / BR samples of that blend space are for.

Trigger B fires whenever the massaged magnitude dips below 0.8 for a frame while running: cutting a
corner during a flick (raw magnitude 0.9 at 45° → massaged 0.73), relaxing the thumb, or **any host
stick processing that shrinks diagonals** (a circular clamp to 1.0 leaves only 0.862 of massaged
magnitude on the rim at 45°; a live-zone or radial dead zone below that makes every diagonal pass a
walk frame).

---

## 5. Port does / game does

| Rule | Game (address) | Port | Verdict |
|---|---|---|---|
| input vector from the target yaw, scale 1.0 | `1413b0c57`, `1413b0c6f` | `set_movement_input(s, r.yaw)` | faithful |
| travel direction via `CalcVelocity` (friction 8, accel = speed × 60) | `0x1405795F0`, `1413b0c39` | `cmc.rs` `calc_velocity`, `run_tick` 545 | faithful |
| facing: slerp actor → target, alpha `clamp01(0.1·dt·60)`, applied directly | `0x1405DDD90`, `1413b0bfc..1413b0c16` | `turn` 408–410, `run_tick` 544 | faithful |
| facing still eased on the no-request tick before the stop | `1413b0949..1413b0999` | `run_tick` 532–535 | faithful |
| stop clip: alpha `dt·12` | `1413b03e3`, `1413b040c` | `run_tick` 505 | faithful |
| no turn and no input during `RunStartN` (sub 1, 2) | `1413b0a18`, `1413b0c1c` | `run_tick` 539 (`if r.sub == 0`) | faithful |
| root-motion yaw of `RunStartN` turns the actor | `PerformMovement` `0x14058CF36..` | `sora.rs` 189–195 | faithful (not re-audited beyond the call) |
| turn-around: sub 0, not walk, `abs(wrap180(target − facing)) ≥ 75`, no other condition | `1413af7a8..1413af8e7` | `run_tick` 466–467 | faithful |
| turn-around effect: StopMovement, `RunStartN` axis = delta, sprint reset, speed reset, sub 2 | `1413af8f4..1413af9c9` | `run_tick` 468–474 | faithful (the port does not clear a sprint *flag*; it has none — no effect) |
| walk → run in the loop replays `RunStartN` (sub 1), no grace test | `141440318..14144032c`, `0x1413B007D` | `try_run`, `run_tick` 489–494 | faithful |
| walk request honoured only in sub 0; walk skips the turn-around test | `141440347`, `1413af7b3` | `try_run`, `run_tick` 466 / 481 | faithful |
| no `PhysicsRotation`, no `FaceRotation`, no rate limit | section 2 | none in the port | faithful |
| no smoothing of the stick yaw before the state | `141440431` | none | faithful |
| per-axis dead zone 0.25 with rescale, no clamp | `0x1445AF120` | `dead_zone` 160–167 | faithful |
| Jump: travel = target yaw, facing `dt·6` | `0x141397BC8`, `0x141397886` | `jump_tick` | faithful (checked in revision 2) |

Nothing in revision 2 of the spec or in the sheet needs a correction for turning. (One wording
point: the common part of the Run tick tests the sub-step *after* the switch — `mov eax,[rsi+78h]` at
`1413b09f6` / `1413b0c1c` — so on the tick the clip ends and sub-step 2 becomes 0, the turn and the
input are issued in that same tick; the port does exactly that.)

---

## 6. What to check, since the code says the port is right

The user's observation "the real game is smoother" and this reading can both be true only if the
*inputs* to these rules differ between the game and the port. In order of likelihood:

1. **The stick values the port's host delivers.** The game reads XInput shorts (`× 1/32767`), whose
   diagonals exceed 1.0. If the host's pad backend delivers a circularly normalised or otherwise
   reshaped stick (Bevy takes its values from gilrs; on Windows that is not necessarily raw XInput),
   trigger B fires on ordinary diagonal passes and trigger A earlier than on the real pad. Log the
   raw stick the core receives while rolling the stick around the rim: the radius at 45° should be
   about 1.1 or more on an Xbox pad; if it is ≤ 1.0, read the pad through XInput directly.
   Check in the port's trace: frames with massaged magnitude < 0.8 (walk flag) just before each
   `RunStartN`.
2. **Frame rate.** The rules are per tick; the comparison must be at the same tick rate (60 Hz).
3. **Camera yaw.** The target yaw is `previous-frame camera yaw + stick angle`. If the port's
   camera turns faster than the game's while running sideways, `delta` grows from the camera side.
   (A camera turning at 100°/s adds a steady lag of only 17°, so this matters only together with a
   stick flick.)

Runtime probe requests (the probe already records per frame: actor yaw, controller moveYaw, stick,
velocity, state, sub-step, animation, camera yaw — add the raw stick shorts if not there):

| # | Record on the real game | Expect if this document is right |
|---|---|---|
| T1 | a fast quarter-turn flick (≤ 5 frames) while running, 10 times | Run sub-step 2 and `RunStartN` on the frame `abs(wrap180(moveYaw − actorYaw)) ≥ 75` first holds; velocity 0 for 2 frames, then the clip's displacement |
| T2 | a deliberate quarter turn over ≥ 10 frames | no `RunStartN`; velocity direction within 5° of moveYaw 3 frames after it settles; actor yaw closing 10 % of the gap per frame |
| T3 | rolling the stick around the rim | massaged magnitude (controller+0x854) stays ≥ 0.8; raw radius at 45° > 1.0; no walk frames |
| T4 | letting the stick dip to about 70 % and back while running straight | walk flag (Run+0x94) for the dip, then sub-step 1 → `RunStartN` |
| T5 | keyboard: W held, then D instead | `RunStartN` with axis 90 on the first frame with the new yaw |
| T6 | position per frame through one `RunStartN` | 180 cm in 14 frames with the non-uniform profile (last 2 frames ≈ 76 cm), or less if the clip is cut — this settles whether the lunge itself is as large in the game |

If T1 shows the game *not* entering sub-step 2 where `abs(delta) ≥ 75` holds, the misreading is in
which yaw the game compares (log state+0x8C and the root component's yaw at `0x1413AF8E0`); nothing
in the instructions above leaves room for that.
