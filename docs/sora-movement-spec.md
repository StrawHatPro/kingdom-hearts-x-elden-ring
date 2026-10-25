# Sora on foot: ground movement and jump specification (stage 1) — revision 2

Revision 2 replaces revision 1 completely: every function on the path was re-read in the
disassembly (iced-x86), the frame order was determined statically, and several revision-1 statements
were wrong (list: `docs/sora-movement-spec-changes.md`, which also says what the Rust port must
change). Source: `base/KH3.exe`, the cooked packages under
`assets_converted/packages/KINGDOM HEARTS III/Content`, the converted animations, the ini files under
`assets/`. Research only. The parameters of section 9 are the sheet `sheets/port/sora-movement.json`
(written by `tools/sora_movement_sheet.js`).

Tags on statements and on whole subsections (a tag in a heading covers the subsection):

- **[I]** verified at instruction level (address given)
- **[C]** decompiled C only — used only where said so explicitly
- **[U]** stock UE 4.17 by identity: an engine function outside the game code, identified by shape,
  whose stock behaviour is assumed where it was not re-read
- **[A]** asset or ini value
- **[D]** derived by arithmetic from [I]/[A] values
- **[?]** not determined statically (section 10 says what to log)

Units: cm, s, degrees; X forward, Y right, Z up (left-handed); yaw 0 = +X, positive toward +Y.
"Slot n" = vtable index (offset / 8). Vtables: movement component `TresAICharMovementComponent`
0x145D073F0 (base `TresCharMovementComponent` 0x145D36078, stock engine class 0x1476228C8 for
comparison), pawn `TresPlayerPawnSora` 0x14696B930, state-request interface (pawn+0x450) 0x14685EB18,
`TresPlayerState_Idle/Run/Jump` 0x1469301E0 / 0x14693B450 / 0x14692CE20. Component offsets used:
Velocity +0x214, Acceleration +0x4A4, MovementMode +0x314, GravityScale +0x2F8, MaxStepHeight +0x2FC,
JumpZVelocity +0x304, MaxWalkSpeed +0x388, MaxAcceleration +0x39C, AnalogInputModifier +0x4E0,
CurrentFloor +0x4F8 (flags bit 0 blocking, bit 1 walkable, bit 2 line trace; FloorDist +4,
LineDist +8, hit +0x10).

---

## 0. Summary of the model

1. `TresCharMovementComponent` is a fork of stock `UCharacterMovementComponent` (same property
   names, same structure) with a list of Square Enix changes that matter: no capsule dimensions
   (all movement queries sweep the real root body), an always-on perch test with a fixed 1 cm radius
   plus two extra edge checks, a `StepUp` without size tests, velocity never recomputed from
   displacement, the pose tick moved into the movement tick, a three-phase jump gravity.
2. Frame order: controller → movement component (consume input, jump check, **animation clock +
   notifies + root motion**, move) → pawn tick → state `MainProc`. State writes act one frame later;
   the state sees the movement's result in the same frame.
3. Speeds are binary: walk 140 (stick < 0.8), run 680, sprint 980 after more than 1.0 s of running.
   Ground acceleration is `MaxWalkSpeed × 60`; stops and starts are animation-led.
4. Facing is a per-tick slerp by the state (`0.1·dt·60` run, `12·dt` stop, `6·dt` jump).
5. Jump: launch `Velocity.Z = 1480` five frames (60 Hz) after the press; gravity −4116 rising,
   −147 for the first sub-steps after the apex while the accumulated time is below 0.15 s, then
   −1470; released button → rise gravity −8232 from the first rise-loop tick. No coyote time, no
   buffer.
6. Movement is frame-rate dependent in the original (section 8): dt is the frame time (≤ 0.05 s),
   one physics sub-step per frame, linear per-tick blends, tick-quantised timers.

---

## 1. Architecture

### 1.1 Objects [I for classes and offsets, A for the blueprint]

| Object | Class | Where | Role |
|---|---|---|---|
| Pawn | `p_ex001_Pawn_C` → `p_ex001_PawnBase_C` → `TresPlayerPawnSora` → `TresPlayerPawnBase` → `TresAIPawnBase` → `TresCharPawnBase` → `TresPawnBase` → `Pawn` | `Blueprints/Player/p_ex001` | tunables; `ACharacter`-like jump latch (`bPressedJump` +0x5BC bit 0, second latch +0x5C0 bit 0, `JumpKeyHoldTime` +0x5C4, `JumpMaxHoldTime` +0x5C8 = 0); `ControlInputVector` +0x428; analog modifier +0x63C |
| Root | `TresRootComponent` (a `PrimitiveComponent`) | pawn+0x468 | `UpdatedComponent`; one capsule body (section 6) |
| Movement | `TresAICharMovementComponent` | pawn+0x578 | section 3–5. The AI subclass adds only: ctor `MaxAcceleration = 1e6` (0x14051240A), a `SetMovementMode` wrapper (slot 178 0x1405262A0), an avoidance flag |
| State queue | `TresStateQueueComponent` | pawn+0x5A0, default state IDLE | holds the current state; **its own tick is disabled** (1.2) |
| States | `TresPlayerState_Idle/Run/Jump` | | facing, input, speeds, animation, transitions |
| Request interface | sub-object pawn+0x450 | | `CanX` / `TryX` called by the controller |
| Controller | `TresPlayerControllerSora` | | polls stick and buttons |
| Mesh / anim | `TresSkeletalMeshComponent` pawn+0x580, anim instance `p_ex001_Anim_C` (`TresPlayerAnimInstance`) | | clips are montages on Slot nodes; `RootMotionMode` = montages only |

The locomotion-definition asset (`p_ex001_DefaultLocomotion_Land`, 300 / 980 / 200) is an AI state
(`AIMove`, used by the AI controller's MoveTo 0x1405241D0) and never runs for the player [I].

### 1.2 Frame order [I]

Tick functions (`FTickFunction` 0x58 bytes; fork `ETickingGroup`: PrePhysics = 3):

| Tick | Group | Enabled | Set at |
|---|---|---|---|
| controller actor tick | PrePhysics | yes (also when paused, but the Tres tick returns at once if paused: 0x14141E97E) | `APlayerController` ctor 0x144422829 |
| movement component tick | PrePhysics | yes; `bTickBeforeOwner` | `UMovementComponent` ctor 0x144009E23 / 0x144009E31 |
| pawn actor tick | PrePhysics | yes | `AActor::InitializeDefaults` 0x143EA0D80 |
| mesh component tick | PrePhysics | yes, always tick pose | pawn ctor 0x14059FCEB |
| state queue component tick | DuringPhysics | **never**: the pawn ctor calls queue vt+0x3E0(true) (0x14059FEB4 → 0x141514250: `MyDisableAutoMainProc` = true), BeginPlay 0x14150E340 then leaves the tick disabled | |

Prerequisites: controller → movement (`AController::AddPawnTickDependency` 0x144078C80, stock);
movement → pawn actor (0x14401DA60 `bTickBeforeOwner` rule, and again in
`ATresCharPawnBase::PostInitializeComponents` 0x1405B4A30 at 0x1405B4C10); movement → mesh
(0x1405B4BAB). No pawn ↔ mesh prerequisite (irrelevant: the mesh tick does not advance the
animation, see below).

Order of frame N:

```
C(N)  APlayerController::TickActor 0x1444429C0 → PlayerTick → ProcessPlayerInput (keys, axes; 2.x)
      → TresPlayerControllerBase::Tick 0x14141E950 → slot 631 0x1412875B0 → Sora input update 0x1413F1AB0
M(N)  TresCharMovementComponent::TickComponent 0x140598190 (slot 97):
        a. InputVector = ConsumeInputVector()          — what S(N−1) wrote
        b. pawn CheckJumpInput(dt) 0x1405A5B20         — sees the latch the notify set in M(N−1)
        c. Acceleration, AnalogInputModifier (3.2)
        d. 0x140595730: pose tick = pawn slot 365 0x1405AF030 → TresSkeletalMeshComponent::TickPose
           0x1405DA2B0 → UAnimInstance::UpdateAnimation 0x143F2EC40: montage clocks advance, root motion
           is extracted and consumed into the component (+0x940..), ALL notifies fire (notify timing mode
           OnPostUpdateAnimation: 0x140559854 → 0x143F2D1B0). Every frame: component+0xC18 bit 0x20 is set
           in the ctor (0x14057185E) and never cleared.
        e. force-volume preparation (slots 411, 414; zero in stage 1)
        f. PerformMovement (slot 276 0x14058CC60) — as a task on a worker thread (task 0x14057F840),
           the tick completes only after it and the post step 0x14058C770 → 0x140591690
S(N)  pawn actor tick: TresPlayerPawnBase::Tick 0x141448AA0 → 0x1405544D0 → ATresCharPawnBase::Tick
      0x1405BC470 → state queue MainProc(dt) (call at 0x1405BC51D): End / Begin / Tick of the state
      mesh tick: bones only (ShouldTickPose false: already ticked this frame)
```

Hand-offs:

| From → to | Latency |
|---|---|
| controller latch → state tick | same frame |
| state writes (`ControlInputVector`, `MaxWalkSpeed`, `MaxAcceleration`, walk mode, `GravityScale`, velocity zeroing) → movement | **next frame** |
| movement result (mode, velocity, position) → state | **same frame** |
| state starts a clip → first clock advance, its root motion, its notifies | next frame's M, step d [? R3: unless the play call ticks the pose itself] |
| notify → state flags (`m_bEnableChange`, launch handler) | fired in M(N), seen by S(N) |
| `TresJumpStart` notify → `DoJump` | next frame's M, step b |

dt [I]: world frame time, real elapsed time clamped to `MaxDeltaTime` = 0.05 s (`DefaultEngine.ini`;
read at 0x1445913F3), times dilations that are 1.0 in plain play; `FixupDeltaSeconds` 0x1445C3FE0
clamps to 0.0005..0.4. No fixed step; the same dt reaches C, M, the animation and S.

### 1.3 State queue [I]

`MainProc` 0x14150A7E0; queue flags (component+0x268): 1 finish, 2 restart, 4 loop again, 8 busy.

```
if flags & 8: return;  flags |= 8
do:
  flags &= ~4
  if NeedChange():                       // 0x14150E830: (flags & 3) || !cur || queue empty || cur != queue[0]
      if cur: cur.End()
      flags &= ~1
      if queue empty: PushState(default = IDLE)
      if flags & 2: flags &= ~2; cur = queue[0]
      else: recycle(old); if old == queue[0]: pop front (push default if now empty); cur = queue[0]
      if cur: cur.Begin()
  if cur && (flags & 7) == 0: cur.Tick(dt)      // a state begun in this call is ticked in this call
while flags & 4
flags &= ~8
```

`PushState(id)` 0x141513260: takes a pooled instance or creates one, **calls Reset**, appends.
`SetState(id)` 0x141514920: clear the queue, then `PushState` — the change happens at the next
`MainProc`. `DoFinish` (flags |= 1), `DoChain` (flags |= 5: end, begin and tick the next state in
this call). State fields: `m_nSubStep` +0x78; flags +0x84 bit 0 `m_bEnableChange`, bit 1
`m_bReverseLeg`. Base Reset clears sub-step and `m_bEnableChange`. Ids: IDLE 1, RUN 2, JUMP 3.

"On ground" = 0x1405AD850: `!(pawn+0xD80 & 1) && MovementMode ∈ {Walking, NavWalking}`.

---

## 2. Input [I]

### 2.1 Pad to axis values

- XInput layer `XInputInterface::SendControllerEvents` 0x142DFA100 (stock): thumb value =
  `short × 1/32767` (`× 1/32768` when ≤ 0); **no dead zone**; an event is sent only when the short
  changed. `TresGameViewportClient::InputAxis` 0x141523130 forwards the value unmodified.
- `UPlayerInput` is the stock class. `ProcessInputStack` 0x1445B21B0: per key
  `Value = MassageAxisInput(RawValue)` (0x1445B2524), pressed / released event counts swapped in.
- `MassageAxisInput` 0x1445AF120: with `dz` > 0: `v > 0 ? max(v − dz, 0)/(1 − dz) : −max(−v − dz, 0)/(1 − dz)`;
  then exponent (if ≠ 1), sensitivity, invert; no clamp. `DefaultInput.ini` lines 31–32 [A]:
  `Gamepad_LeftX` / `LeftY`: DeadZone 0.25, Sensitivity 1, Exponent 1, bInvert False.
  **Per-axis** dead zone (it bends directions near the axes).
- `GetInputAnalogStickState(0)` 0x1444333A0 returns the massaged `Gamepad_LeftX`, `Gamepad_LeftY`.

### 2.2 Keyboard

A Square Enix emulation inside `APlayerController` (slot 414 0x144441460, run from
`PreProcessInput` at the start of `ProcessInputStack`, so **keyboard input is one frame later than
pad input**). Defaults (0x140EDF630; user-rebindable in the game settings): W / S = +/− `LeftY`,
D / A = +/− `LeftX`, raw ±1.0 injected every frame while held (0x14442C190); LeftAlt halves the raw
value (→ 0.3333 after the dead zone → walk); opposite keys give 0; any held movement key overrides
the pad; diagonal magnitude 1.414 (not clamped); no ramp. Jump = SpaceBar (sub key BackSpace),
injected as the pad jump key (0x14442B8F0).

### 2.3 Stick to yaw (Sora update 0x1413F1AB0)

```
V = (X = LeftY, Y = LeftX, 0);  mag = |V|            // not clamped; stored controller+0x854
active = !(mag < 0.1)                                // 0x1413F206D
if active: V = normalize(V)
           rot = camera rotation (0x1405E6B80) with Pitch = 0 (0x1413F21FC; Roll kept)
           moveYaw = ToOrientationRotator(V · FRotationMatrix(rot)).Yaw      // controller+0x85C
```

Closed form with camera yaw Y and roll R:
`moveYaw = atan2(LeftY·sinY + LeftX·cosR·cosY, LeftY·cosY − LeftX·cosR·sinY)`; with R = 0:
**`moveYaw = cameraYaw + atan2(LeftX, LeftY)`** (stick right increases yaw; stick up is +LeftY).
The engine uses its polynomial `Atan2` (0x142CE9900) and sin/cos (≈ 1e-4° from libm).
The camera rotation is the `PlayerCameraManager` cached view of the **previous frame's** camera
update (the camera manager ticks in a later group, 0x1405E2D30); a "keep heading across a camera
cut" latch (camera manager+0x290C) does not arise with one follow camera.

### 2.4 Dispatch order in the update (addresses of the calls)

1. jump key just pressed (`WasInputKeyJustPressed`, true for one frame, 0x1413F1BD9): risk dodge
   (slot 61) and attack-context (slot 45) — stubs returning false — then **`JumpRequest(0)`**
   0x1413F280B (result ignored);
2. else if the key is down: glide (slot 67, stub) then **`JumpHeld()`** 0x1413F283F — not on the
   press frame;
3. then, if the stick is active: **`MoveInput(yaw, mag)`** 0x1413F2878, **`TryRun(yaw, mag)`**
   0x1413F288F.

Jump key: `i.GamePadDecideType` (int 0x1487D7370) default 0 (0x140EE0CA0) → 0x140EDD580 sets
jump / cancel = `Gamepad_FaceButton_Right` (**Xbox B**), attack / decide = `Gamepad_FaceButton_Bottom`.
No buffering: a refused press is gone.

### 2.5 Request interface

```
CanRun (slot 38 0x14142CE00):
  IDLE: Idle.timer(+0x9C) <= 0 && OnGround
  RUN : OnGround || Run.probe(2000)                 // 4.5
  JUMP: sub == 3 && (flag 0x20 || OnGround)
TryRun(yaw, mag) (slot 40 0x141440030):
  if !CanRun: return
  walk = mag < 0.8                                   // 0x141440092; restriction overrides are 0 (7.)
  IDLE: st = SetState(RUN); if !walk: st.sub = 1
  RUN : if !walk: if st.walk != 0 { if st.sub == 0: st.sub = 1 } else { if st.sub == 4: st.sub = 1 }
        else: walk = (st.sub == 0)                   // a walk request is honoured only in sub 0
  JUMP: st = SetState(RUN); walk = 0; st.animName = current animation    // replaces the Jump at the next MainProc
  st.yaw(+0x8C) = yaw; st.mag(+0x90) = mag; st.walk(+0x94) = walk; st.request(+0x88) = 1
MoveInput(yaw, mag) (slot 35 0x141440600): only when the current state is JUMP (any sub-step):
  Jump.yaw = yaw; Jump.request = 1; Jump.mag = mag
JumpHeld (slot 43 0x14143E600): if current is JUMP: Jump.held(+0x94) = 1
Jump gate (slot 41 0x14142A3D0):
  IDLE: Idle.timer <= 0 && OnGround;  RUN: OnGround
  JUMP: nothing queued && sub == 11                   // 0x14142A552; no m_bEnableChange test
JumpRequest (slot 42 0x14143D8F0): if gate: yaw = actor yaw (IDLE) / state yaw (RUN, JUMP);
  ClearQueue; RequestJump(yaw, type 0, bFall = false) → PushState(JUMP), fills yaw / type / power;
  clears the auto-held byte (+0xE9)
Fall(yaw) (slot 172 0x141444C00): RequestJump(yaw, 0, bFall = true) → Jump begins at sub 3
```

Consequences: a jump press in Idle on a frame with an active stick is lost (`TryRun` afterwards
does `SetState(RUN)`, clearing the queued Jump); on the frame of a jump from Run the `TryRun` latch
goes to the Run state that is about to end, and the new Jump state has no request (4.2).

---

## 3. Ground movement

### 3.1 Run state [I] (Tick 0x1413AED70, Reset 0x1413AD350, Begin 0x1413A58A0, End 0x1413AB9A0)

Fields: +0x88 request, +0x8C yaw, +0x90 mag, +0x94 walk, +0x98 grace, +0xA0 anim name, +0xB8 sprint
timer, +0xBC sprint flag, +0xD4 flags (bit 1 initial start, bit 2 probe cached). Reset zeroes all.
Sub-steps: 0 loop, 1 start requested, 2 start clip playing, 3 stance change (stub), 4 stop clip.

```
Tick(dt):
  if !OnGround: clear probe cache; if !probe(2000): Fall(yaw) → DoChain; return        // 4.5
  isRun = (grace > 0) ? true : (walk == 0)                 // value at tick start, used below
  switch sub (one dispatch on the value at tick start):
   0: if walk == 0:
         delta = wrap180(yaw − actorYaw)
         if |delta| >= 75:                                   // turn-around, 0x1413AF8E0
             StopMovement; Play(RunStart, root motion); blendX = delta
             sprintTimer = 0; sprintFlag = 0; ResetMaxWalkSpeed; sub = 2 → COMMON
         else: sprintTimer += dt
               if sprintTimer > MySprintDelay(1.0): sprintFlag = 1; SetMaxWalkSpeed(MySprintSpeed 980)   // every tick
      else: sprintTimer = 0; sprintFlag = 0; ResetMaxWalkSpeed
      loop anim = isRun ? RunLoop : WalkLoop; Play(loop) if its name differs
   1: delta as above; Play(RunStart, root motion); blendX = delta; flags |= 2; sub = 2
   2, 3: if clip ended: Play(loop by isRun); flags &= ~2; sub = 0
   4: Turn(yaw, dt × 12); if clip ended: DoChain (→ IDLE); return     // request latch not cleared here
  COMMON (sub 0..3 at tick start):
   if request == 0:
      if sub == 0 || sub == 3 || (sub == 2 && !(flags & 2)):
          if Acceleration.x² + Acceleration.y² <= 1.0:      // component Acceleration; 0x1413B0876
              StopMovement; Play(isRun ? RunEnd : WalkEnd, L if m_bReverseLeg else R); grace = 0; sub = 4; return
      if sub == 0 || sub == 3: Turn(yaw, 0.1 × dt × 60)     // still turns on the no-request tick before the stop
   else:
      SetWalkMode(!isRun)
      grace = (walk == 0) ? 0.2 : max(grace − dt, 0)
      if sub == 0 || sub == 3:
          blendX = walk ? 0 : clamp(wrap180(yaw − actorYaw), axis range of the playing asset)
          Turn(yaw, 0.1 × dt × 60)                          // 0x1413AF153, 0x1413B0982
          SetMaxAcceleration(GetMaxWalkSpeed() × 60)        // 0x1413B0C39
          SetMovementInputXY((cos yaw, sin yaw), 1.0)       // 0x1413B0C6F
   request = 0
End: SetWalkMode(false); ResetMaxWalkSpeed                  // no StopMovement
```

- `Turn(yaw, a)` = pawn slot 438 → 0x1405DDD90: actor rotation = slerp(current, (0, yaw, 0),
  clamp01(a)); with pitch = roll = 0: `actorYaw += wrap180(yaw − actorYaw) × clamp01(a)`.
- `StopMovement` = pawn slot 733 0x140530DD0: Velocity = 0 (all axes); does not touch Acceleration
  or the input vector.
- `SetMovementInputXY` = pawn slot 724 0x1405B9BD0: `ControlInputVector = (x, y, 0)` (assigned, not
  accumulated); pawn+0x63C = clamp01(scale).
- Speeds [I]: run 680 = `MaxWalkSpeed` (player pawn ctor 0x141423412; `ResetMaxWalkSpeed` restores
  the class default); sprint 980 (0x141423709) when `sprintTimer > 1.0` (0x141423713); walk =
  680 × `WalkSpeedRate` 0.205882 [A] = 140.

Timing with the frame order of 1.2: last stick frame N−1; in frame N the state has no request but
the component's Acceleration (from M(N)) is non-zero → turn only; M(N+1) has no input → one
braking tick (3.3); S(N+1) stops. A run request during the stop clip restarts at sub 1; a walk
request does not interrupt it.

Idle [I] (Tick 0x1413944D0, Begin 0x141386050): Begin returns without playing if not on ground;
plays `IdleN`. Tick: if not on ground → `Fall(actorYaw)` → DoChain; replays the idle on its end,
`IdleBreakN` on the third end; never touches speed, velocity or input. Leaving Idle is the
controller's `TryRun` / `JumpRequest`.

### 3.2 Movement tick head [I] (`TickComponent` 0x140598190)

```
InputVector = ConsumeInputVector()
pawn.CheckJumpInput(dt)                                    // 4.2
Acceleration = MaxAcceleration × (|v|² > 1 ? normalize(v) : v), v = InputVector with Z = 0   // slots 271, 272
AnalogInputModifier = 1.0; if |Acceleration|² > 0 && MaxAcceleration > 1e-8: = pawn+0x63C (always 1.0)
                      if walk mode (component+0xA3C bit 0): = WalkSpeedRate          // slot 177 0x14057D050
pose tick (1.2 d); PerformMovement
```

`PerformMovement` 0x14058CC60: scoped update; based movement (nothing on static ground);
root motion → velocity (slot 282, 3.6); **`ClearJumpInput`**; encroach check (slot 404: pushes the
body out of overlaps present at tick start; body read as [C]); `StartNewPhysics(dt, 0)`;
root-motion rotation applied to the actor; clear root motion. The first physics call after spawn is
skipped (`m_bDisableMoveOnSpawnFrame` = 1, 0x1414233FB). `StartNewPhysics` 0x140596E10 dispatches
Walking → `PhysWalking`, Falling → `PhysFalling`.

### 3.3 `CalcVelocity` [I] (slot 211 0x1405795F0; arithmetic identical to stock)

```
CalcVelocity(dt, Friction, bFluid = false, BrakingDecel):
  MaxSpeed = GetMaxSpeed() × AnalogInputModifier           // GetMaxSpeed: MaxWalkSpeed for Walking AND Falling
  bZeroAccel = Acceleration == 0;  bOverMax = |V|² > MaxSpeed² × 1.01
  if bZeroAccel || bOverMax:
      Old = V;  ApplyVelocityBraking(dt, Friction, BrakingDecel)
      if bOverMax && |V|² < MaxSpeed² && Acceleration · Old > 0: V = normalize(Old) × MaxSpeed
  else: V = V − (V − normalize(Acceleration) × |V|) × min(dt × Friction, 1)
  NewMax = (|V|² > MaxSpeed² × 1.01) ? |V| : MaxSpeed
  V += Acceleration × dt
  V = (NewMax < 1e-4) ? 0 : clampSize(V, NewMax)
ApplyVelocityBraking(dt, Friction, Decel)  (0x140577890, verdict same):
  Friction = max(0, Friction × max(0, BrakingFrictionFactor 2));  Old = V;  Rev = −Decel × normalize(V)
  remaining = dt
  while remaining >= 1e-6:
      step = (remaining > 1/33 && Friction != 0) ? min(1/33, remaining × 0.5) : remaining;  remaining −= step
      V += (−Friction × V + Rev) × step
      if V · Old <= 0: V = 0; return
  if |V|² <= 1e-4 || (Decel != 0 && |V|² <= 100): V = 0
```

Differences from stock: no root-motion early-out; a skip flag (+0xA3C bit 2, off); a root-motion
blend that is skipped for the player (`m_bAddFullVelocityToRootMotion` = 1); a direct-velocity tail
(off). Normalisations use `rsqrtss` + two Newton steps.

Ground call: `CalcVelocity(dt, GroundFriction 8, false, BrakingDecelerationWalking 2048)`.
One zero-input tick at 60 Hz from 680: `680 + (−16 × 680 − 2048)/60` = **464.5** [D].

### 3.4 `PhysWalking` [I] (slot 246 0x1405900B0)

```
MaintainHorizontalGroundVelocity();  Acceleration.Z = 0
CalcVelocity(dt, 8, false, 2048)                           // ONCE with the whole dt (0x1405902DF)
MoveVel = (V.x + RM.x, V.y + RM.y, RM.z)                   // RM = root-motion velocity (3.6); + force-volume terms (0)
remaining = dt
while remaining >= 1e-6 && Iterations < 4:
    Iterations++
    t = (remaining > 0.1 && Iterations < 4) ? min(remaining × 0.5, 0.1) : remaining;  t = max(t, 1e-6);  remaining −= t
    OldLoc = location;  OldFloor = CurrentFloor
    Delta = MoveVel × t;  bZero = all |Delta.i| <= 1e-4
    if bZero: remaining = 0
    else: MoveAlongFloor(MoveVel, t, &StepDown)
          if now Falling: StartNewPhysics(remaining + t × (1 − min(1, moved2D/|Delta|)), Iterations); return
    CurrentFloor = StepDown.bComputedFloor ? StepDown.Floor : FindFloor(location, bZero)      // 5.
    if CurrentFloor walkable: AdjustFloorHeight(); SetBase
    elif CurrentFloor.hit start-penetrating && remaining <= 0: resolve penetration upward (trace end = start + 2.4)
    if !CurrentFloor walkable && !start-penetrating:
        if OldFloor walkable && EdgeFloat(): return                                           // 5.5
        bMustJump = bJustTeleported || bZero || OldBase == null
        if (bMustJump || !bCheckedFall) && CheckFall(...): return                             // → StartFalling
        bCheckedFall = true
    // NO "Velocity = (location − OldLoc) / t"
    if location == OldLoc: break
MaintainHorizontalGroundVelocity()
```

For dt ≤ 0.1 (every frame, since dt ≤ 0.05): one `CalcVelocity`, one iteration with `t = dt`.
`CheckFall` 0x14057BC80 → `StartFalling` 0x140596D10: `SetMovementMode(Falling)`, then
`StartNewPhysics(remaining', …)` with `remaining' = remaining + t × (1 − min(1, moved2D/|Delta|))` —
0 when the walking move was unobstructed, so **gravity starts in the next tick**. No coyote time;
`bCanWalkOffLedges` = 1; `ShouldCatchAir` is constant false; stock `HandleWalkingOffLedge` is not
called. Velocity is never recomputed from the move: against a wall it keeps the commanded speed
(nothing in `SlideAlongSurface`, `HandleImpact`, `StepUp` writes Velocity).

`MoveAlongFloor` 0x14058A240 (stock but for the last clause):

```
Delta = (MoveVel.x, MoveVel.y, 0) × t
Ramp = ComputeGroundMovementDelta(Delta, CurrentFloor.hit, CurrentFloor.bLineTrace)
SafeMove(Ramp) → Hit
if Hit.startPenetrating: HandleImpact; SlideAlongSurface(Delta, 1, Hit.Normal); return
if Hit.blocking:
    Applied = Hit.Time
    if Hit.Time > 0 && Hit.Normal.Z > 1e-4 && IsWalkable(Hit):          // hit a walkable ramp
        Left = 1 − Applied;  SafeMove(ComputeGroundMovementDelta(Delta × Left, Hit, false))
        Applied = clamp(Applied + Hit.Time × Left, 0, 1)
    if Hit.blocking && !Hit.startPenetrating:
        if CanStepUp(Hit): if !StepUp((0,0,−1), Delta × (1 − Applied), Hit, &StepDown):
                               HandleImpact; SlideAlongSurface(Delta, 1 − Applied, Hit.Normal)
        elif component refuses step-up: HandleImpact; SlideAlongSurface(...)
ComputeGroundMovementDelta (0x14057DC90, stock): if 1e-4 < ImpactNormal.Z < 0.9999 && Normal.Z > 1e-4 && !lineTrace && IsWalkable:
    return (Delta.x, Delta.y, −(ImpactNormal · Delta)/ImpactNormal.Z)        // horizontal speed kept on slopes
```

`StepUp` (slot 216 **0x140597570**; `CanStepUp` slot 215 0x14057A3E0) — **uses no capsule size**:

```
if !CanStepUp(InHit) || !(InHit.ImpactPoint.Z > origin.Z): return false
fd = (walking on a walkable floor) ? max(CurrentFloor.FloorDist − 2.0, 0) : 0
Up = max(47 − fd, 0);  Down = 47 + 4.8;  FloorPointZ = origin.Z − fd
scoped move:
  move (0,0,Up);          if start-penetrating: revert, false
  move Delta → Hit;       if blocking: if start-penetrating: revert, false
                              HandleImpact; s = SlideAlongSurface(Delta, 1 − Hit.Time, Hit.Normal)
                              if Hit.Time == 0 && s == 0: revert, false
  move (0,0,−Down) → Hit; if start-penetrating: revert, false
  if Hit.blocking:
      dz = Hit.ImpactPoint.Z − FloorPointZ;  if dz > 47: revert, false
      if !IsWalkable(Hit): if Delta · Hit.ImpactNormal < 0: revert, false
                           if Hit.Location.Z > origin.Z(old): revert, false
      if dz > 0 && !CanStepUp(Hit): revert, false
      StepDown.Floor = FindFloor(location, &Hit)
      if Hit.Location.Z > oldZ && !StepDown.Floor.blocking && StepSideZ < 0.08: revert, false
      StepDown.bComputedFloor = true
return true
```

Compared with stock: no top-of-capsule reject, no edge-tolerance reject on the down sweep, the
`−2.0`, `FloorDist` instead of the distance getter. `SlideAlongSurface` 0x140596360: on ground only
`Normal.Z > 0 && !IsWalkable → flatten to 2D` (the stock `Normal.Z < −1e-4` branch is absent), then
the stock base. `TwoWallAdjust` 0x140598B60: base adjust, then on ground: `delta.z > 0`: walkable
and `ImpactNormal.Z > 1e-4` → scale the whole delta so `delta.z ≤ 47`, else `delta.z = 0`;
`delta.z < 0 && FloorDist < 1.9 && blocking → delta.z = 0`. `HandleImpact`: notifications only.
`AdjustFloorHeight` 0x1405744E0 (stock): if FloorDist (LineDist for a line floor) is outside
1.9..2.4: one sweep move of `2.15 − dist`. A floor 30 cm below is reached in that one move.

Moves [I]: `SafeMoveUpdatedComponent` 0x14401E730 (stock): move; if start-penetrating:
`adj = Normal × ((depth > 0 ? depth : 0.125) + |p.PenetrationPullbackDistance| [?R9])`, resolve,
move again. Every move hit comes back with stock pull-back
(`Time −= clamp(0.1/Dist, …) + 0.001`) and with **`Location` = actor origin at the hit**
(`TresRootComponent::MoveComponentImpl` 0x1405D27A0 rewrites it).

### 3.5 Rotation [I]

The turning rules with the instructions quoted, the two stop-and-lunge triggers and the stick motions
that fire them: `docs/sora-movement-spec-turning.md`.

`PhysicsRotation` 0x140590DA0 returns at once (both orient flags are 0: 0x140571355, 0x1414233CC);
`RotationRate` 1000 is unused. Turning is the state's slerp (3.1, 4.2).

### 3.6 Root motion [I]

Slot 282 0x1405969D0: when the pose tick delivered root motion: `RM = world root translation of
this tick / dt / blend weight` (+0xA24); added in full to the walking move velocity and to the
falling delta (`m_bAddFullVelocityToRootMotion` = 1, 0x14142348D); root-motion rotation is applied
to the actor after the physics. Only `RunStartN` (5 samples, 0.2333 s, 180 cm; per-frame track in
`build/sora_pack/manifest.json`) moves; `RunEnd`, `WalkEnd`, `Land`, `LightLand`, `IdleN` have no
root motion (see `docs/sora-spec-reconciliation.md` A1, A2).

---

## 4. Jump, fall, landing

### 4.1 Request and launch [I]

`RequestJump` (2.5) pushes JUMP with power from pawn slot 1347 0x141434010: type 0 →
`MyJumpPowerNormal` = 1480 [A] (stage 1: no high-jump ability).

Launch chain: `TresJumpStart` notify of `JumpNormalStart` at 0.0666 s [A] fires in the pose tick of
M(N) → handler 0x14139AB60 (only if sub == 0): pawn slot 530 0x1405ADF50 (`bPressedJump` = 1,
latch +0x5C0 = 1, `JumpKeyHoldTime` = 0), `JumpZVelocity = power`, sub = 1. Same M(N):
`ClearJumpInput` 0x1405A5D80 clears `bPressedJump` (hold time 0 ≥ max hold time 0). M(N+1):

```
CheckJumpInput (0x1405A5B20):
  if bPressedJump || latch:
      wasHeld = JumpKeyHoldTime > 0;  JumpKeyHoldTime += dt
      did = iface.vt+0x108() [constant true] && DoJump()
      if !wasHeld && did: latch = 0; OnJumped()
DoJump (slot 195 0x14057ECB0, verdict same):
  Velocity.Z = JumpZVelocity;  SetMovementMode(Falling);  SetJumpDurationFinish(false)
```

At 60 Hz [D]: press in C(0); S(0) begins the Jump state and plays the clip; the clock first advances
in M(1) and passes 0.0666 in M(4); `DoJump` in M(5) (which is a pure falling tick). The pawn is in
Walking mode on the ground for those five movement ticks.

### 4.2 Jump state [I] (Begin 0x1413868C0, Tick 0x141395450, End 0x14138D850, Reset 0x14138F740)

Fields: +0x88 request, +0x8C yaw, +0x90 mag, +0x94 held, +0xA0 type, +0xA4 power, +0xAC fall time,
+0xB0 fall height, +0xB4 last Z, +0xDC probe distance, +0xE0 probe ground Z / last actor Z,
+0xE8 deferred down loop, +0x114 flags (0x20 step-down, 0x1000 probe cached), +0x120 step-down timer.
Sub-steps: 0 start clip, 1 after launch, 2 rise loop, 3 fall entry, 4 falling, 10 / 11 landing.

```
Begin:
  if sub == 3: (4.5)
  SetMaxWalkSpeed(default 680);  SetMaxAcceleration(680 × 5.4 = 3672)        // 0x141386E5D
  sub 0: Play(JumpNormalStart)                                               // does not read the stick
Tick(dt):
  probe(2000) (cached on the first tick after a fall-entry Begin)
  req = request && !(sub is 10 or 11)
  switch sub:
   0: —
   1: if clip ended: Play(JumpNormalRiseLoop); sub = 2
      lastZ = actor Z
      if held == 0 && AnimTime > 0.1: GravityScale = MySmallJumpGravityScale     // never true for the 0.1 s clip
   2: if Velocity.Z > 0: if held == 0: GravityScale = MySmallJumpGravityScale (3.0)      // 0x141395EE5
      else: if IsJumpDurationFinish: Play(JumpNormalDownLoop) else deferred = 1
            sub = 4; lastZ = actor Z; GravityScale = class default (1.5)
   3: (4.5)
   4: if OnGround: landing (4.6)
      elif deferred && IsJumpDurationFinish: Play(JumpNormalDownLoop); deferred = 0
   10, 11: (4.6)
  TAIL (every sub-step): Turn(yaw, dt × 6)                                   // 0x141397886
        if req: SetMovementInputXY((cos yaw, sin yaw), 1.0)
  END:  request = 0; held = 0
        if sub == 4: fallTime += dt
                     fallHeight = (Velocity.Z >= 0) ? 0 : fallHeight + (lastZ − actorZ);  lastZ = actorZ
        probe cache cleared; groundZ(+0xE0) = actor Z
End: clear the pawn jump latch; ResetMaxWalkSpeed; GravityScale = class default     // MaxAcceleration stays 3672
```

- `held` is set by the controller only on frames where the key is down and was not just pressed,
  and cleared at the end of every tick.
- **Short hop**: the sub-1 test is dead (clip length 0x3DCCCCCD equals the compared constant, the
  test is strict). The hop starts on the first sub-2 tick with `Velocity.Z > 0` and the button not
  down; the 3.0 acts from the next movement tick. A tap and any release before sub 2 give the same
  hop.
- **Take-off dip** (jump from Run, press in frame N): M(N) accelerates normally; S(N): `Run::End`
  (sprint speed dropped), `Jump::Begin`, first Jump tick with `request == 0` → no input written;
  M(N+1): Acceleration = 0 → one braking tick in Walking mode (680 → 464.5 at 60 Hz); from S(N+1)
  the Jump state steers with 3672 cm/s² (+61.2 cm/s per 60 Hz tick, cap 680) until launch.
- Fall height is accumulated at the end of the tick while sub == 4 only: the landing tick's drop
  is not counted.

### 4.3 Gravity [I]

`GetGravityZ` 0x140582270 (same): `−980 × GravityScale × rate(+0xBF8 = 1.0)` (0 if gravity is
disabled, +0xA3C bit 1). −980 = `UPhysicsSettings.DefaultGravityZ` (ctor 0x1443FB130; `BaseEngine.ini`
1654); `AWorldSettings::GetGravityZ` 0x1445C4F70 falls through to it; none of the 3809 cooked maps
sets a gravity key and the Olympus maps contain no physics volume [A]. `GravityScale` 1.5 [A].

Phase hook, applied to the gravity vector once per falling sub-step, sees `Velocity.Z` from before
the step (slot 405 0x140577F30):

```
if !(finished flag +0xBFC & 1):
    if Velocity.Z > 0: return G × m_JumpUpGravityScale (2.8)
    if timer(+0xC00) < m_MaxJumpDurationTime (0.15):
        timer += step
        if timer < 0.15: return G × m_JumpDurationGravityScale (0.1)
        SetJumpDurationFinish(true)
return G
```

`SetJumpDurationFinish(true)` is also called by every real mode change (0x140594A9C), so only
`DoJump` opens the curve; a ledge fall has plain gravity.

| Phase | Gravity (cm/s²) |
|---|---|
| rise, button held | −4116 |
| rise, button released (from the movement tick after the first sub-2 state tick) | −8232 |
| hang: sub-steps starting with `Velocity.Z ≤ 0` while the accumulated time stays < 0.15 | −147 |
| fall, and any fall not started by `DoJump` | −1470 |

The step in which the apex is crossed runs entirely with the rise gravity (overshoot below zero up
to 4116 × dt). The state restores `GravityScale` in the same frame as that step, so the hang always
uses 1.5. Hang length in f32 accumulation [D]: 4 steps at 30 Hz, **8 at 60 Hz** (0.1333 s), 17 at
120 Hz. Jump height (continuous) 1480² / (2 × 4116) = 266 cm.

### 4.4 `PhysFalling` [I] (slot 222 0x14058D0C0)

```
FallAccel = clampSize2D((Acceleration.x, Acceleration.y, 0) × AirControl 0.95, MaxAcceleration)   // no boost
bAir = |FallAccel|² > 0
per sub-step t (same rule as 3.4; one step of the whole dt for dt <= 0.1):
    OldV = V
    if bAir: VNoAir = CalcVelocity with Acceleration = 0 on (V.x, V.y, 0), Z = OldV.z   (V restored)
    Acceleration = FallAccel; V.z = 0; CalcVelocity(t, FallingLateralFriction 0, false, BrakingDecelerationFalling 2048); V.z = OldV.z
    if !bAir: VNoAir = V
    G = phase hook((0, 0, GetGravityZ()), t)
    V = NewFallVelocity(V, G, t);  VNoAir = NewFallVelocity(VNoAir, G, t)
    AirControlAccel = (V − VNoAir) / t
    Adjusted = ((OldV + V) × 0.5 + RM) × t                 // midpoint; RM = root-motion velocity (0 unless a root-motion clip plays)
    SafeMove(Adjusted) → Hit;  sub = t × (1 − Hit.Time)
    if Hit.blocking:
        if IsValidLandingSpot(location, Hit): ProcessLanded(Hit, remaining + sub); return
        Adjusted = V × t;  HandleImpact
        if bAir: Adjusted = (VNoAir + LimitAirControl(t, AirControlAccel, Hit) × t) × t
        OldN = Hit.Normal;  OldIN = Hit.ImpactNormal
        Delta = ComputeSlideVector(Adjusted, 1 − Hit.Time, OldN, Hit)
        if sub > 1e-4 && Delta · Adjusted > 0:
            SafeMove(Delta) → Hit
            if Hit.blocking:
                last = sub;  sub = sub × (1 − Hit.Time)
                if IsValidLandingSpot: ProcessLanded(Hit, remaining + sub); return
                HandleImpact
                if bAir && Hit.Normal.Z > 0.001: Delta = ComputeSlideVector(VNoAir × last, 1, OldN, Hit)
                Pre = Delta;  TwoWallAdjust(Delta, Hit, OldN)
                if bAir: A = LimitAirControl(sub, AirControlAccel, Hit) × sub; if A · OldN > 0: Delta += A × sub
                if |Hit.Normal.Z| < 0.001 && |OldN.Z| < 0.001 && Adjusted has XY && Delta.xy · Adjusted.xy < 0:   // two vertical walls
                    Delta = (0, 0, Pre.Z);  V.x = V.y = 0
                    Delta = ComputeSlideVector(Delta, 1 − Hit.Time, Hit.Normal, Hit) + Hit.Normal × 0.01
                bDitch = OldIN.Z > 0 && Hit.ImpactNormal.Z > 0 && |Delta.Z| <= 1e-4 && Hit.ImpactNormal · OldIN < 0
                SafeMove(Delta) → Hit
                if Hit.Time == 0: if IsValidLandingSpot: ProcessLanded(Hit, 0); return
                                  SafeMove(SafeNormal2D(OldN + Hit.ImpactNormal), fallback (OldN.y, −OldN.x, 0))
                if bDitch: ProcessLanded(Hit, 0); return
                if IsValidLandingSpot: ProcessLanded(Hit, 0); return
                if Hit.Time == 0: unstick (ResolvePenetration with Delta, then with its 2D part); land only if a valid spot results; return
    if V.x² + V.y² <= 0.001: V.x = V.y = 0
NewFallVelocity (0x14058BCA0): R = V + G × t; limit = 4000 (OverrideMaxFallingSpeed 0 → volume TerminalVelocity);
    if R · dir(G) > limit: clamp that component to limit
```

Differences from stock (compared with the stock function in the exe, 0x143FF5540): **no
`Velocity = Delta / time` after either deflection** (velocity is only what `CalcVelocity` and gravity
make it — a ceiling hit does not zero `Velocity.Z`); no `ShouldCheckForValidLandingSpot` block; the
two-wall block; the `Time == 0` unstick; no random "ditch" velocity; `HandleSlopeBoosting` returns
`(0, 0, ZLimit)` instead of zero; external-velocity terms and a "no landing while moving up" flag
(+0xC18 bit 3) that are off for Sora; an alternative implementation behind cvar
`p.TresTestFallingMovement` (default 0).

Air numbers [D]: input acceleration 3672 × 0.95 = 3488 cm/s²; horizontal cap 680 (a sprint jump at
980 is braked: over-max branch, 2048 cm/s² plus the acceleration term, never below 680 while
pushing forward); no input: 2048 cm/s² braking, friction 0 (so no direction blending in the air).

### 4.5 Falling without a jump; the step-down [I]

Idle / Run tick, not on ground (and, for Run, probe failed): `Fall(yaw)` → Jump at sub 3 in the same
`MainProc` call. Movement side: `PhysWalking` already switched to Falling (plain gravity).

```
Jump::Begin (sub 3):
  m = stick magnitude; if m >= 0.1: mag = m; request = 1                    // 0x141386A52
  probe(2000)
  if current anim ∈ {RunLoopN/W/B, LightLandR/RB/L/LB} && request && probeDist <= 40: flags |= 0x20   // step-down
  (if a floor is found here: sub stays 3 in stage 1 — the land-pose path never applies)
Tick sub 3:
  if !request: flags &= ~0x20
  if probeDist > 40: flags &= ~0x20
  if flags & 0x20: keep the run / walk loop playing; timer += dt; if timer > 1.0: flags &= ~0x20 (acts next tick); → TAIL
  else: Play(JumpNormalDownLoop); sub = 4; lastZ = actor Z; continue with the sub-4 code
```

While the step-down flag is set `CanRun` is true, so the controller's next `TryRun` does
`SetState(RUN)`: the Jump is replaced by a fresh Run (sub 0, no start clip, latch set). If the pawn
is back in Walking mode the run simply continues (no landing animation); if still airborne and the
probe fails, Run calls `Fall` again — a Run ↔ Jump(3) alternation per frame until the ground is
reached or more than 40 cm below. Note that most small drops never leave Walking at all (5.4).

Ground probe [I] (0x14063A650; no stock counterpart): sweep a **sphere of radius 50** from
`origin + (0,0,55)` straight down `range`; channel Pawn, blocking WorldStatic / WorldDynamic /
TresGimmick only, ignoring the pawn. For each blocking hit: start-penetrating → `dist = 0`, false;
surface less steep than 45° → `dist = ActorZ − ImpactPoint.Z`, ground Z = ImpactPoint.Z, true;
steeper → remember that distance, continue. Nothing → `dist = range`, false. `dist` is ≈ +0.15 when
standing (floor distance − 2), negative just after leaving an edge while the sphere rests on the
lip, 0 when geometry higher than 5 cm above the feet is within 50 cm.
Run probe (slot 113 0x1413A3D20): `hit && dist < 2.0`, cached per tick (a cached read returns
`dist < 2.0` without the hit flag). Jump probe (slot 114 0x141383880): once per tick, stores
distance and (if hit) ground Z.

### 4.6 Landing [I]

Movement: `IsValidLandingSpot` 0x140589960: blocking; not start-penetrating → `IsWalkable(Hit)`,
start-penetrating → `Normal.Z ≥ 1e-4`; `FindFloor(location, falling distances, &Hit)` walkable
(including the perch stage, 5.3); a flag (component+0x308 bit 0) that is 0. No lower-hemisphere and
no edge-tolerance test. `ProcessLanded` 0x140591DC0 → `SetPostLandedPhysics` 0x140594DA0 →
`SetMovementMode(Walking)`: `Velocity.Z = 0`, XY kept, `FindFloor` with the walking distances,
`AdjustFloorHeight` (to 2.15), then `StartNewPhysics(remaining)` walks the rest of the tick.

State, sub 4, first tick on ground (the same frame as the landing):

```
StopMovement                                               // Velocity = 0, all axes
light = !(fallHeight > 75)                                 // 0x14139628B
Play(light ? LightLand : (request && fallHeight >= 800 ? RollLand : Land));  m_bEnableChange = 0;  sub = 10
next tick, sub 10: sub = 11
    if actorZ > groundZ && actorZ − groundZ <= 75: SetActorLocation(z = groundZ, sweep)     // 0x141397358..0x141397380
    then the sub-11 code in the same tick:
sub 11: if request && m_bEnableChange: StopMovement; DoChain            // → IDLE; the next TryRun starts RUN (with RunStart)
        if clip ended: StopMovement; DoChain
```

- Stick: control returns at the `TresEnableChange` notify — 0.0991 s into `Land` / `LightLand`
  (length 0.4 s), 0.4325 s into `RollLand` (0.7667 s) [A].
- Jump: the gate accepts state JUMP as soon as `sub == 11`, without the notify. Touchdown in frame
  T (state sets sub 10); a press in frame T+1 (controller still sees sub 10) is refused and lost;
  from frame T+2 a press starts a new jump.
- The landing snap puts the actor origin on the probe's ground point (lowers it by
  `FloorDist − 2` ≈ 0.15 cm on flat ground).

### 4.7 Animation per phase [I for the logic, A for clips]

`PlayAnim(name, flags)`: bit 0 loop, bit 1 root-motion mode. Names are AnimSet keys
(`p_ex001_AnimSet`), `N` = normal stance.

| Phase | Name | Clip `rt/body/` | Mode | Length |
|---|---|---|---|---|
| Idle | `IdleN` (third end → `IdleBreakN`) | 0001 (0004) | 2 | 3.0 s |
| Walk / run loop | `WalkLoopN` / `RunLoopN` (BlendSpace1D, axis = clamped yaw delta) | 0002 / 0003* | 1 | |
| Run start, turn-around | `RunStartN` (BlendSpace1D, axis = signed yaw delta −180..180) | 0007* | 2 | 0.2333 s |
| Run / walk stop | `RunEndR/L` / `WalkEndR/L` | 0011, 0012 / 0005, 0006 | 2 | 0.5 / 0.5667 s |
| Jump start / rise / fall | `JumpNormalStart` / `RiseLoop` / `DownLoop` | 0031 / 0032 / 0033 | 0 / 1 / 1 | 0.1 s (notify 0.0666) |
| Landing | `Land` / `LightLand` / `RollLand` | 0034 / 0120 / 0108 | 2 | 0.4 / 0.4 / 0.7667 s |

---

## 5. Floor and edges [I]

### 5.1 `FindFloor` (slot 259 0x14057FA80)

```
adj = walking ? 2.4001 : −2.4
Sweep = Line = max(2.4, MaxStepHeight 47 + adj)             // 49.4001 walking, 44.6 falling
ComputeFloorDist(location, Line, Sweep, Out, radius 0, DownwardSweepResult)    // bAlwaysCheckFloor = 1: every time
if Out.blocking && !Out.lineTrace && ShouldComputePerchResult(Out.hit):
    MaxPerch = max(2.4, adj + 47 + FallingPerchAdditionalHeight 10) (+ max(PerchAdditionalHeight 0, 0) when walking)   // 59.4001 / 54.6
    if ComputePerchResult(radius 1.0, Out.hit, MaxPerch, Perch):
        if (2.15 − Out.FloorDist) + Perch.FloorDist >= MaxPerch: Out.FloorDist = 2.15
        if !Out.walkable: Out = line-trace floor from Perch.hit (dist = min(Perch.FloorDist, Perch.LineDist))
    elif Out.walkable: if EdgeFallCheck(Out): Out.walkable = false          // 5.4
    else: Out.walkable = false
```

### 5.2 `ComputeFloorDist` (slot 264 0x14057D100)

Main path (radius argument 0 — the pawn's capsule-size virtuals pawn vt+0xD00 / +0xD08 return 0):

```
(if a vertical DownwardSweepResult is given: use it with dist = location.Z − hit.Location.Z; return if walkable)
Hits = ComponentSweepMulti(root body, location → location − (0,0,Sweep))    // 0x143FC9090: the real capsule, no shrink, no pull-back
Hit = first blocking hit;  FloorDist = Sweep × Hit.Time;  Hit.Location = location − (0,0,FloorDist)
Out = sweep floor(Hit, FloorDist); Out.blocking = blocking && !startPenetrating
if valid blocking && IsWalkable(Hit) && FloorDist <= Sweep: Out.walkable = true; return
if !Out.blocking && !Hit.startPenetrating: Out.FloorDist = Sweep; return
line trace from location (the actor origin) down Line:
    if hit && Time > 0: d = Time × Line; Out.blocking = true
        if d <= Line && IsWalkable(hit): Out = line floor (normals / component from the line; Time, Location, ImpactPoint from the sweep; LineDist = d; walkable); return
Out.walkable = false; Out.FloorDist = Sweep
```

`IsWalkable` (0x140589A60, stock): blocking, not start-penetrating, `ImpactNormal.Z ≥ 0.656059`
(49°) — only `ImpactNormal` is read. `ImpactNormal` for a capsule sweep is the hit shape's face /
triangle normal [U; what PhysX reports exactly on an edge is ?R4].

Perch path (radius 1.0, from `ComputePerchResult` 0x14057E0C0: start = the floor hit's origin
location, `Sweep = MaxPerch`, `Line = max(0, MaxPerch − InHitAboveBase)`,
`InHitAboveBase = max(0, ImpactPoint.Z − Location.Z)`): capsule radius 1.0, half height 13.6,
centre 15 cm above the start, swept down `Sweep + 1.4`; if start-penetrating or the impact is
≥ 0.85 cm from the axis: second sweep radius 0.8499, half height 2.4 (down `Sweep + 12.6`);
`FloorDist = Time × TraceDist − Shrink`; its line trace starts 15 cm above the start. Success =
walkable and `InHitAboveBase + FloorDist ≤ MaxPerch`.

### 5.3 What the perch stage means

`ShouldComputePerchResult` (0x140596260) is true whenever the floor contact is more than 1.0 cm
(2D) from the capsule axis — on every slope and every edge (the stock "threshold ≤ 0.15 → never"
test is gone). With the 60 cm hemisphere a contact d cm from the axis lies
`2 + 60 − sqrt(3600 − d²)` above the origin. So an edge supports Sora only while walkable ground
exists straight below the contact-side probe within 59.4 cm (walking) / 54.6 cm (when testing a
landing) of the edge height.

### 5.4 Edge checks added by Square Enix

`EdgeFallCheck` (slot 260 0x1405887B0; true = drop the floor), called after a failed perch on a
walkable floor:

```
fd = max(FloorDist, 0)
part 1 (m_bNeedFallTravelCheck = 1): D = location.xy − ImpactPoint.xy
  if |D| <= 25: dir = normalize(D); P = location.xy + dir × max(24 − |D|, 0); z0 = location.z − min(fd, 2.15)
      sphere r 25 multi-sweep (P, z0 + 37.5) → (P, z0 − 5.0), ignoring the pawn and the floor component
      if some non-penetrating hit has dot(dir, normalize(hit.ImpactPoint.xy − P)) >= 0: return false
part 2: if dist2D(location, ImpactPoint) > 47 + (walking ? 10 : 0): return true
  L = MaxPerch + min(fd, 2.15) − max(ImpactPoint.Z − location.Z, 0)
  line from the origin down L, up to 4 segments: walkable hit → return false; else step back 0.5 and slide along the face
return true
```

`EdgeFloat` (slot 409 0x14057BD10; `m_bCheckEdgeFloat` = 1), in `PhysWalking` when the floor was
walkable before the sub-step and is not now: slide `(0, 0, −3.225)` along the floor hit normal; if
that move ends on a valid landing spot and `FindFloor` there is walkable: adjust height and stay
Walking; else revert.

Resulting behaviour at a kerb of height h: walking off it, the main sweep first meets the kerb edge;
for h ≤ 59.4 the perch probe finds the lower ground, the floor stays walkable and
`AdjustFloorHeight` rolls the capsule down around the edge (Z = −(60 − sqrt(3600 − d²)) for axis
offset d) without ever leaving Walking; for higher edges the edge checks run and Sora starts falling
about 1 cm after the axis passes the edge. Walking up: `StepUp` (3.4) accepts a landing whose
impact point is at most 47 cm above `origin − max(FloorDist − 2, 0)`.

---

## 6. Collision: what the host must supply [I]

Body: `TresRootComponent` builds one capsule from `CollisionShapesSrc[0]` (0x1405DF310): radius
`Size.X` = 60, half height `Size.Z` = 89 (cylinder length 58), centre `RelativeLocation` = (0, 0, 91)
[A]: bottom at origin + 2, top at origin + 180. The mesh soles are 0.4–0.56 cm below the origin
(reconciliation A4). While walking the origin is `FloorDist − 2` (−0.1..+0.4 cm) above the floor.

Collision profile: `TresPlayerPawn` (player pawn ctor 0x14142339F; `DefaultEngine.ini` 1397 [A]):
object type Pawn; **blocks** WorldStatic, WorldDynamic, PhysicsBody, Vehicle, Destructible,
TresEquip, TresProjectile, TresGimmick, TresSound, TresBlockDetect; overlaps triggers, water
current, attack collision; ignores Pawn, Visibility, Camera and the enemy / special pawn channels —
other pawns never block the capsule (the encroach push-out is separate).

| Query | Shape and path | Used by |
|---|---|---|
| move sweep | the capsule (r 60, hh 89) from the current place by a delta; first blocking hit: time (then stock pull-back), impact point, normal, impact normal, start-penetrating + depth | all moves (3.4, 4.4), `AdjustFloorHeight`, landing snap |
| floor sweep | the same capsule straight down 49.4001 / 44.6; no pull-back | 5.2 |
| floor line | from the actor origin straight down the same distance | 5.2 |
| perch capsule | radius 1.0 / half height 13.6 (then 0.8499 / 2.4), from 15 cm above a given point down `Sweep + 1.4` (`+ 12.6`) | 5.2 |
| perch line | from 15 cm above a given point down `Line + 15` | 5.2 |
| edge sphere | sphere r 25, all hits, from `z0 + 37.5` to `z0 − 5`, ignoring the floor component | 5.4 |
| edge line | lines from the origin, up to 4 segments | 5.4 |
| ground probe | sphere r 50 from origin + 55 down 2000, first blocking hits in order, with impact normal | 4.5 |

---

## 7. Flags that are constant in plain field play [I]

All verified against the constructors and their writers: restrict-jump / restrict-run / command
flags (pawn+0xE60 bits 9, 11, 15, 16, 17) = 0 (set only by script `BP_SetRestrict…`, volumes, HUD
command menu); floating (pawn+0xD80 bit 0) = 0; battle mode (pawn+0x630 bit 1) = 0 without a
battle; damage lock, guarding, bad status, land pose (pawn+0x6E8, written only by a notify no
stage-1 clip has), one-shot resume (pawn+0x1460) = 0; air-jump available (pawn+0xE48 bit 0) = 1 but
unused without the ability. Movement component: +0xC18 = 0x20; +0x68C = 0x06 (edge float + fall
travel check; async-off bit 3 clear; cvar bit 0 clear); +0xBDC = 0; +0x458 bit 2 set, bits 0–1
clear; `bAlwaysCheckFloor`, `bCanWalkOffLedges`, `bMaintainHorizontalGroundVelocity` = 1;
`bUseFlatBaseForFloorChecks` = 0.

Other transitions out of Idle / Run / Jump (stubs): swim, floating, cover, stun / burn, rail slide
(forward probe returns false without rail actors), slope slide, hopping, climbing, gimmick attach,
dangle (ledge grab: only on geometry flagged `SQEX_bEnableDangle`), glide, dive fall
(`m_bEnableDiveFall` off), wall slide, water flow, battle stance, force walk; controller requests:
attack, magic, guard, dodge, air slide, double jump (ability 0x11), and the rest of the interface
(list with addresses: revision 1, kept at `build/sora_tmp/mv/sora-movement-spec.rev1.md` section 7).

---

## 8. Frame-rate dependence of the original [I / D]

The game has no fixed step: dt = frame time ≤ 0.05 s. For dt ≤ 0.1 there is exactly one physics
sub-step per frame (limits `MaxSimulationTimeStep` 0.1, `MaxSimulationIterations` 4, shared between
a fall and the walk after its landing).

Uses dt linearly per tick (so the result depends on the frame rate):

- turn alphas `clamp01(0.1·dt·60)`, `12·dt`, `6·dt`: a fraction of the *remaining* angle per tick
  (at 60 Hz 10 %, at 30 Hz 20 %, at 120 Hz 5 % per tick — not the same curve);
- ground direction blend `min(dt × 8, 1)`, `Velocity += Acceleration × dt` then clamp;
  `MaxAcceleration = MaxWalkSpeed × 60`: full speed in one tick only when dt ≥ 1/60;
- braking: explicit Euler with step `min(1/33, remaining/2)` when remaining > 1/33 (1/30 s = two
  steps of 1/60);
- gravity phase chosen once per sub-step; hang 4 / 8 / 17 steps at 30 / 60 / 120 Hz; apex overshoot
  up to 4116 × dt;
- timers accumulated per tick and compared: sprint (> 1.0), run grace (0.2), step-down (> 1.0),
  animation clocks and notify crossing (launch at the first tick with clock ≥ 0.0666).

One-tick events whose size scales with dt: the take-off dip, the braking tick before a stop, the
lost-press frame after touchdown, the frame of delay between a state write and the movement.

Per-tick constants not scaled by dt: floor snap to 2.15 (outside 1.9..2.4); step-up 47 / +4.8 /
−2.0 / 0.08; edge float 3.225; zero-delta 1e-4 cm per tick; braking stop thresholds (|V|² ≤ 1e-4,
≤ 100 with deceleration); over-speed factor 1.01; penetration pull-out 0.125 (+ cvar); two-wall
nudge 0.01; landing snap; the 5-frame launch latency is in frames of clip time (0.0666 s) plus one
frame.

For a 1:1 port run the core once per rendered frame with the frame's dt (≤ 0.05), or at a fixed
1/60 as the reference rate; do not sub-step the core differently from the game.

---

## 9. Parameters

The sheet `sheets/port/sora-movement.json` (111 rows; `node tools/q.js sora-movement`) is the
table: id, value, unit, source (instruction address or asset property), evidence, function,
confidence. Main values: gravity −980 × 1.5; jump scales 2.8 / 0.15 s / 0.1; small-jump scale 3.0;
jump power 1480; terminal 4000; run 680, sprint 980 after > 1.0 s, walk rate 0.205882; stick
thresholds 0.1 / 0.8, dead zone 0.25, run grace 0.2; ground acceleration × 60, air × 5.4 (3672),
air control 0.95; friction 8, braking factor 2, braking 2048 / 2048; turn 0.1·60 / 12 / 6;
turn-around 75°; floor angle 49°, step 47, floor distance 1.9 / 2.15 / 2.4, sweeps 49.4001 / 44.6,
perch radius 1.0, max perch 59.4001 / 54.6, edge sphere 25, edge float 3.225; capsule 60 / 89 / 91;
probe sphere 50 from +55 over 2000, near-ground 2.0, slope 45°; step-down 40 cm / 1.0 s; landing
75 / 800 / snap 75, control return 0.0991 / 0.4325 s; `MaxDeltaTime` 0.05.

---

## 10. Confidence and what is left

Verified at instruction level in this revision: everything tagged [I] — the frame order and its
prerequisites; the input path from XInput to the yaw; the request interface, state queue and the
three states (control flow re-derived from the code, not from the C); `TickComponent`,
`PerformMovement`, `StartNewPhysics`, `CalcVelocity`, `PhysWalking`, `MoveAlongFloor`,
`ComputeGroundMovementDelta`, `StepUp`, `CanStepUp`, `SlideAlongSurface`, `TwoWallAdjust`,
`AdjustFloorHeight`, `CheckFall`, `StartFalling`, `FindFloor`, `ComputeFloorDist`, the perch
functions, the two edge checks, `IsWalkable`, `IsValidLandingSpot`, `PhysFalling`,
`GetFallingLateralAcceleration`, `LimitAirControl`, `NewFallVelocity`, the gravity hook,
`ProcessLanded`, `SetPostLandedPhysics`, `SetMovementMode`, `OnMovementModeChanged`, the ground
probe, the gravity chain, the collision profile, the constructors.

Still decompiled C: the body of the encroach check (slot 404 0x14057AE30), `ResolvePenetrationImpl`
0x14059E000 (stock structure), the inside of blocks that are off for Sora.

Stock-on-identity [U]: PhysX sweep results (hit ordering, `ImpactNormal` on edges), the task-graph
tie-break between pawn and mesh tick (irrelevant), `UInputSettings` loading of the axis config.

Not determinable statically — runtime probe items (fields and events to log are listed as R1–R12
in `docs/sora-movement-spec-changes.md` section F):

| # | Question |
|---|---|
| R1 | confirm the frame order and the worker-thread move |
| R2 | regression traces of the stage-1 scenarios (velocity, mode, state, sub-step per frame) |
| R3 | whether a state's play call advances the pose in the same frame (shifts launch 5 → 4 frames and the first root-motion frame) |
| R4 | `ImpactNormal` reported for capsule sweeps on box edges and mesh edges (decides walkability of edge contacts) |
| R5, R6 | perch / edge-check / step-up traces on real kerbs (30, 45, 50, 58, 62 cm) |
| R7 | ground probe values and the landing snap on real geometry |
| R8 | the flags of section 7 and the external-velocity inputs, expected constant / zero |
| R9 | `p.PenetrationPullbackDistance` and the other console variables |
| R10 | the montage time multiplier in the short-hop test (expected 1.0) |
| R11 | the input layer in the user's device setup (DirectInput / Steam Input paths were not read) |
| R12 | the camera rotation the controller reads (previous frame; latch clear) |
