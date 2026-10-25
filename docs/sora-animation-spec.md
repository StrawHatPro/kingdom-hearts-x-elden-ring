# Sora (p_ex001): assets and animation selection for stage 1

Scope: what the player character Sora needs to stand, walk, run and jump outside the game — his assets, how the
game picks and plays his animations, and the local asset pack `build/sora_pack/` built by `tools/sora_pack.js`.
The pack is converted from the user's own copy and stays on this machine (doctrine `d12`); nothing is distributed.

Evidence classes used below:

- **data** — read from a decoded package under `assets_converted/packages/` (asset path given).
- **exe** — read from decompiled C in `re/decomp/` (address given; "verdict" is `impl/progress.verdict`,
  where known). Decompiled C is evidence, not source.
- **exe (delegated)** — read from the same decompiled C (and disassembly) by two separate code-reading
  passes whose full write-ups, with one address per constant, are `build/sora_tmp/native_A.md` (play API,
  Idle, notifies) and `build/sora_tmp/native_B.md` (Run, Jump). Statements marked **✓** below were re-read
  in the decompiled C by the author of this document; statements marked **(A)** / **(B)** rest on those
  write-ups alone.

Verdicts (`impl/progress.verdict`) of the functions relied on:

| Function | Address | Verdict |
|---|---|---|
| name selectors: idle / loop / end / start / jump / land | 0x141433F80 / 0x141434F80 / 0x141434EC0 / 0x141435030 / 0x141434150 / 0x141434330 | same / same-partial / same / same / same-partial / same-partial |
| Idle begin / tick | 0x141386050 / 0x1413944D0 | same-partial / no-verdict |
| Run request / begin / tick / end | 0x141440030 / 0x1413A58A0 / 0x1413AED70 / 0x1413AB9A0 | differs / same-partial / same-partial / no-verdict |
| Jump begin / tick / end / set-sub-step / JumpStart handler | 0x1413868C0 / 0x141395450 / 0x14138D850 / 0x1413A0BF0 / 0x14139AB60 | same-partial / same-partial / same-partial / differs / no-verdict |
| pawn play-by-name wrapper; anim instance play-by-name; sequence to montage; montage play; blend-space play | 0x1405A0E50; 0x14056ECE0; 0x14056E920; 0x14056F470; 0x14056F260 | differs (all five) |
| name lookup | 0x14056DE80 | no-verdict |
| montage instance advance (root motion) | 0x143F33C00 | not-compiled |
| is-anim-end | 0x14056BFD0 | same |
| montage / segment "has root motion" | 0x143F44DB0 / 0x143F292D0 | same / same |
| sequence `bEnableRootMotion` getter; `m_InterpolationTime` getters | 0x143F44E10; 0x143F2D270, 0x143F08C80 | never compiled and compared (8–9 byte getters) |
| constructors `UAnimSequenceBase`, `UAnimSequence`, `UBlendSpaceBase` | 0x143F5A030, 0x143F31D90, 0x143F5A5C0 | same (verified), all three |
| constructor creating `TresCharMesh0`; `TresPlayerPawnBase`; `TresPlayerPawnSora` | 0x14059F950; 0x141423210; 0x1413E7580 | same (verified); differs; no-verdict |

"differs", "same-partial" and "no-verdict" mean the decompiled C did not (fully) pass the differential
test. The constants and call arguments quoted here are plain to see in it, but the control flow around them
carries that caveat.

## 1. Sora's assets

All paths are under `/Game` = `KINGDOM HEARTS III/Content`.

| What | Asset | Evidence (data) |
|---|---|---|
| Pawn blueprint | `Blueprints/Player/p_ex001/p_ex001_Pawn` (`p_ex001_Pawn_C`) → parent `p_ex001_PawnBase_C` → native `TresPlayerPawnSora` | class exports' `superIndex`; `audit/blueprint_class_bases.json` |
| Skeletal mesh (one mesh: body, face, hair, clothes) | `Character/pc/p_ex001/mdl/p_ex001_0` | `SkeletalMesh` of component template `TresCharMesh0` (`TresSkeletalMeshComponent`) in `p_ex001_Pawn` |
| Skeleton | `Character/pc/p_ex001/mdl/p_ex001_0_Skeleton` (274 bones, root `kh_sk`) | `Skeleton` property of the mesh; `TargetSkeleton` of the AnimBlueprint |
| Physics asset | `Character/pc/p_ex001/mdl/p_ex001_0_Physics` | import of the mesh package (`PhysicsAsset`) |
| Secondary motion data (not packed) | `p_ex001_Bonamik` (`SQEX_BonamikAsset`: hair/cloth), `p_ex001_0_kdi` (`SQEX_KineDriverData`: helper bones) | asset user data exports of the mesh package |
| AnimBlueprint | `Character/pc/p_ex001/anm/p_ex001_Anim` (`p_ex001_Anim_C`), parent native class `TresPlayerAnimInstance` (: `TresAnimInstance`) | `AnimClass` of `TresCharMesh0` |
| Animation set | `Character/pc/p_ex001/anm/p_ex001_AnimSet` (`TresAnimSet`, 428 rows) | `MyAnimSets` in the class default object of `p_ex001_Anim_C` |
| Face animation set (not packed) | `Character/pc/p_ex001/anm/p_ex001_FaceAnimSet` | `m_SubAnimSets` of the same object |
| Data tables | `Blueprints/Player/p_ex001/p_ex001_DataTableSet` (`TresChrDataTableSet`: base params, level-up, attack data, voice) | `m_pDataTableSet` of the pawn class default object |
| Load asset | `Load/Player/TresPlayerPawnLoad_ex001` (`TresPlayerPawnLoadAsset`) holds cameras and effects only — no mesh or animation | `m_LoadAsset`; decoded export |

Mesh material slots (LOD 0 has 12 sections, one per slot; 46,709 vertices, 67,732 triangles; 4 LODs exist):

| Slot | Material instance (`mdl/mat0/`) | Root material | Base colour used by the pack |
|---|---|---|---|
| `p_ex001_H_mat0`, `p_ex001_ksk_mat0` | `p_ex001_hair_Inst`, `p_ex001_hair_Inst_Inst` | `SQEX_KHHairBaseMat` | no texture: mean of `HairColor` and `HairPointEndColor`; normal `p_ex001_H_0n00` |
| `p_ex001_M_mat0` | `p_ex001_M_mat0_Inst` | `SQEX_Mouth` | shader texture `Mouth_M_0d00` |
| `p_ex001_F_mat0` | `p_ex001_F_mat0_Inst` | `pc_skinbasemat` (via `_OnTear_Inst`) | `p_ex001_F_0d00`, normal `p_ex001_F_0n00` |
| `p_ex001_E_mat0` | `p_ex001_E_mat0_Inst` | `SQEX_WhiteEyeMaterial` | shader texture `EyeWhiteBaseColor` |
| `p_ex001_E_mat1` | `p_ex001_Eyelens_Inst` | `SQEX_IrisTexEyeMaterial` | procedural iris: flat mean of `Color` and `Color_Dark` |
| `p_ex001_E_mat2` | `p_ex001_E_mat1_Inst` | `SQEX_EyeLens` (translucent highlight) | exported fully transparent |
| `p_ex001_E_mat3` | `p_ex001_E_mat3_Inst` | `SQEX_BaseEyelashes` | `EyelashesColor` with the alpha of `p_ex001_E_0d01` |
| `p_ex001_mat0`, `_mat1`, `_mat2` | `p_ex001_mat0_Inst` … | `p_ex001_Basemat` | `FormChangeTex0` = `p_ex001_0d00` / `0d01` / `0d02`; normals `0n00` / `0n01` / `0n02` |
| `p_ex001_mat3` | `p_ex001_mat3_Inst` | `pc_skinbasemat` | `p_ex001_0d00`, normal `p_ex001_0n00` |

The alpha channel of the `*d*` textures is not opacity (it feeds other shader inputs, e.g.
`LightAttenuationChannel`), so the pack drops it. Every parameter of every instance is in
`manifest.json` → `materials`.

### Placement in the pawn (data: `p_ex001_PawnBase`)

- `TresCharMesh0`: attached to the root component `TresRoot`; `RelativeRotation` yaw 270° (= −90°);
  `RelativeLocation` is not serialized in either blueprint, so it is the native default, and that is
  **(0, 0, 0)** ✓: the constructor that creates `TresCharMesh0` (0x14059F950) sets flags, attaches it to the
  root component and never writes its `RelativeLocation` (component +0x2F0); neither do the
  `TresPlayerPawnBase` and `TresPlayerPawnSora` constructors. Consistent with the mesh (lowest vertex 0.6 cm
  below the mesh origin) and with the capsule below.
- `TresRoot` (`TresRootComponent`) `CollisionShapesSrc[0]`: `ETresCollision::CAPSULE1`, `Size` (60, 0, 89),
  `RelativeLocation` (0, 0, 91). The shape builder 0x1405DF310 (differs) ✓ makes a capsule with radius =
  max(Size.X, 1), half height = max(Size.Z, 1), centre = RelativeLocation: **radius 60 cm, half height
  89 cm, centre 91 cm above the actor origin** — bottom 2 cm above the origin, top at 180 cm. So the actor
  origin is at the feet, not at the capsule centre as in stock UE. (Settled by
  `docs/sora-movement-spec.md` section 5.1; re-read here.)
- Feet: the lowest sole vertex is **0.56 cm below the actor origin** in the reference pose, and 0.55 cm
  (left) / 0.40 cm (right) below it at frame 0 of `IdleN` (CPU-skinned LOD 0, no leg IK;
  `manifest.json` → `feet`). The soles are therefore about 2.5 cm below the capsule bottom, i.e. level with
  the floor when the capsule floats at its 1.9–2.4 cm walking clearance (movement spec).
- In mesh space Sora faces UE +Y; the −90° yaw turns that to actor +X.
- Movement component overrides and jump powers are listed in `manifest.json` → `identification`
  (`GravityScale` 1.5, `WalkSpeedRate` 0.205882, `m_JumpUpGravityScale` 2.8, `m_MaxJumpDurationTime` 0.15,
  `MyJumpPowerNormal` 1480, …); they belong to the movement spec.

## 2. How animations are chosen and played

### 2.1 Structure

**There is no state machine in the AnimBlueprint** (data: `p_ex001_Anim`, class default object, 34 anim
nodes). The graph is: slot `FullBody` → cached pose → leg IK (`TresAnimGraphNode_TwoBoneIKMultiped`,
`CollTwoBoneIK` on `L_ashi1`/`R_ashi1`) → look-at (`TresAnimGraphNode_ExLookAt`, `ModifyBone` on `kubi`,
`atama`) → face/mouth layers (`Face`, `Mouth`, `Face2`, `Mouth2` slots filtered at `G_eye`/`G_mouth`) →
a `TwoWayBlend` with slot `FullBody2` (alpha `blendBodyWeight`) → root. `m_CacheStartPoseAnimName` = `IdleN`.

Animations are played **by name** from native state objects:

- `TresAnimSet.Anims` = array of `TresAnimAssetUnit { AnimName: FName, AnimData: AnimationAsset }`.
  `AnimData` is an `AnimSequence` or a `BlendSpace1D`. Dump of all 428 rows: `build/sora_tmp/animset_main.tsv`.
- The pawn has a queue of state objects (`TresStateQueueComponent`, default state `TSID_STATE_IDLE`):
  `TresStateBase` → `TresCharState_X` → `TresPlayerState_X`. State vtable slot 96 = begin, 97 = tick, 98 = end
  (exe: vtables compared with `build/sora_tmp/vtdiff.js`).
- Each state calls a helper at `state+0x40`, slot `+0x18`: play(name, slot, blendIn, mode, effectGroup,
  playRate, rootScale). Script-visible twin: `TresStateBase::BP_AnimPlay(InAnimName, InBlendInTime,
  InBlendOutTime, InLoop, InRootTrans, InRootRot, InPlayRate)`.
- The **name** is chosen by virtual functions of the pawn. `TresPlayerPawnSora` (vtable 0x14696B930) uses
  the `TresPlayerPawnBase` implementations (exe, read in full):

| Pawn vtable slot | Function | Returns |
|---|---|---|
| 909 (+0x1C68) | 0x141433F80 | `DiveIdle` when pawn vfunc +0x1C30 is true; else `IdleN`, or with both flags set `IdleB` / `IdleLockon` (third flag) |
| 910 (+0x1C70) | 0x141434F80 | walk flag ? `WalkLoop` : `RunLoop`, suffix `N` / `W` / `B` |
| 911 (+0x1C78) | 0x141434EC0 | `WalkEnd` / `RunEnd` + `L` or `R` (flag) + `` / `W` / `B` |
| 912 (+0x1C80) | 0x141435030 (verified, same) | `RunStartN` / `RunStartW` / `RunStartB` |
| 913 (+0x1C88) | 0x141434150 (same-partial) | jump kind 0 normal, 1 high, 2 super, 3 double flight, 4 super flight; phase 0 `…Start`, 1 `…RiseLoop`, other `JumpNormalDownLoop`; `B` suffix flag |
| 915 (+0x1C98) | 0x141434330 (same-partial) | by `ETresLandAnimPose`: 1 `Land`, 2 `LightLand`, 3 `LightLandR`, 4 `LightLandL`, `B` suffix flag; 0 → no name |

  Suffixes: `N` = normal, `W` = second flag only, `B` = both flags (battle). The callers pass pawn vfunc
  +0x1330 and battle-state queries; (B) +0x1330 = weapon out (equipment component). For a peaceful stage-1
  scene the `N` names apply. For Sora `…B` jump/land names map to the same assets as the plain ones (data).

### 2.2 The play call

`length = play(name, slot, blendIn, mode, effectGroup, playRate, rootScale)` — state interface slot +0x18 →
pawn (0x1405A0E50) → mesh component → `TresAnimInstance` 0x14056ECE0.

- **slot**: 0 = `AnimPlayUseSlotNodeName` (default `FullBody`) (A).
- **mode** ✓ (0x14056E920): bit 0 = loop (the montage section is linked to itself); the montage's
  root-motion translation and rotation are enabled when `(mode & 0x12) == 2`, i.e. bit 1 = root motion,
  bit 0x10 = suppress. Looping is decided by this argument only, never by the asset.
- **blendIn** ✓: seconds; a negative value → the asset's virtual +0x260 = `m_InterpolationTime`
  (0x143F2D270, 0x143F08C80). Its class default is **0.0** ✓ (constructors 0x143F5A030, 0x143F5A5C0), so an
  asset that does not serialize it blends in instantly. Blend-out is 0.
- **playRate**: every stage-1 call passes 1.0 ✓. **rootScale**: every call passes (1, 1) ✓ (0x14871E368).
- A sequence is played as a transient one-segment montage ✓ (0x14056E920); play length = sequence length /
  |RateScale|. Starting it stops every montage of the slot group with blend time 0 (A, 0x14056F470), and a
  by-name play always restarts (A). A blend space is played by the same kind of montage instance
  (0x14056F260, A); the parameter given to "set blend-space parameter" is copied each tick without extra
  smoothing at that level (A) — the smoothing is the blend space's own `InterpolationParam`.
- Name lookup (A, 0x14056DE80), each array last to first: `m_ReplaceMatineeAnimSets`,
  `m_AppendAnimSetList`, `m_ReplaceAnimSets2`, `m_ReplaceAnimSets`, `m_WorldAppendAnimSetList`,
  `MyAnimSets`. Missing name → `m_DefaultAnimName`.
- **Is-anim-end** (0x14056BFD0; A): ended unless the slot's montage instance is playing with a blend target
  > 0. A one-shot ends exactly at its last frame; a loop never ends.
- **Root motion** ✓: extracted only when the instance has the root-motion bit **and** the montage "has root
  motion" (0x143F33C00, the +0x268 call; 0x143F44DB0 → 0x143F292D0 → the sequence's `bEnableRootMotion`,
  0x143F44E10). `bEnableRootMotion` defaults to false ✓ (0x143F31D90) and is serialized as true only on
  `0007*` (RunStart), `0074*` (Turn) and `0101` / `0102` (RunSway) (data). So of the stage-1 animations
  **only `RunStartN` moves the actor by root motion**; every other one is in place even when played with
  mode 2. `RootMotionMode` is the engine default `RootMotionFromMontagesOnly` (A).
- **Blend behaviour**: settled in sections 6 and 8. The previous montage is cut at once; the new one's
  weight rises linearly with elapsed time; the slot blends it against the **saved pose of the previous
  frame** (recursive: `S[n] = blend(montage, S[n−1], w_n)`), with sign-corrected nlerp for rotation, linear
  translation and log-space scale. It is not a cross-fade of two running animations and not a blend from a
  snapshot taken at the play call.

### 2.3 What plays when (stage 1)

Sequences are under `Character/pc/p_ex001/anm/rt/body/`; 30 frames/s. Mode and blend-in are the arguments
of the play call ✓; "blend-in" shows the resolved value (argument −1 → `m_InterpolationTime`, default 0).
The same numbers are in `manifest.json` → `logical_animations[].play`.

| Situation | Name | Asset(s) | Length (s) | Mode | Blend-in (s) | Root motion applied |
|---|---|---|---|---|---|---|
| Idle | `IdleN` | `0001` | 3.0 | 2 (one-shot, replayed at end) | 0.067 | no |
| Idle fidget | `IdleBreakN` | `0004` | 9.833 | 2 | 0.07 | no |
| Walk | `WalkLoopN` | `0002` | 1.067 | 1 (loop) | 0.2 | no |
| Stop from walk | `WalkEndR` / `WalkEndL` | `0005` / `0006` | 0.567 | 2 | 0.13 | no |
| Start running, sharp turn | `RunStartN` | BlendSpace1D, X −180…180: −180 `0007BL`, −90 `0007L`, 0 `0007`, 90 `0007R`, 180 `0007BR` | 0.233 | 2 | 0.067 | **yes**: 1.8 m towards the new direction, and the turn itself |
| Run | `RunLoopN` | BlendSpace1D, X −10…10: −10 `0003L`, 0 `0003`, 10 `0003R`; sample-weight interpolation 0.4 s | 0.533 | 1 | 0.2 | no |
| Stop from run | `RunEndR` / `RunEndL` | `0011` / `0012` | 0.5 | 2 | 0.13 | no |
| Jump take-off | `JumpNormalStart` | `0031` | 0.1 | 0 | 0 | no |
| Rising | `JumpNormalRiseLoop` | `0032` | 1.1 | 1 | 0 | no |
| Falling | `JumpNormalDownLoop` | `0033` | 0.667 | 1 | 0.67 | no |
| Landing, fall ≤ 75 cm | `LightLand` | `0120` | 0.4 | 2 | 0 | no |
| Landing, fall > 75 cm | `Land` | `0034` | 0.4 | 2 | 0 | no |
| not needed for stage 1 | `LightLandR` / `LightLandL`, `TurnN`, `RunSwayR/L` | `0109` / `0110`, `0074*`, `0101` / `0102` | | | | see below |

Loops (`0001`, `0002`, `0003*`) are in place: the root bone has one key and the hips (`center`) only bob.
**Walking and running displacement comes from the movement component; play rate is not scaled by speed.**
For stage 1 the weapon is not drawn, so the `N` names (and the un-suffixed End / Jump / Land names) apply:
the selectors get "weapon out" from pawn vfunc +0x1330 and "battle" from +0x1748 (B).

**Idle** — `TresPlayerState_Idle` (id 1), vtable 0x1469301E0: begin 0x141386050 ✓, tick 0x1413944D0 (A).

- Begin ✓: if the pawn holds a land pose (vfunc +0x1788; set by a `TresAnimNotify_LandAnimPose`, cleared by
  every play-by-name) it plays that landing name (−1, mode 2) and waits in sub-step 1; otherwise it plays
  the idle name (−1, mode 2) unless that idle is already the current animation.
- Tick (A): the idle animation is replayed each time it ends; after it has ended 3 times `IdleBreakN` plays
  once, then idle again (a cycle counter, no timer). Every tick the yaw is interpolated towards the desired
  yaw (`dt × 9`). There is no turn-in-place animation in Idle.
- Idle has no transition to Run of its own (A): Run is pushed by the input path, next paragraph.

**Run** — `TresPlayerState_Run` (id 2), vtable 0x14693B450: begin 0x1413A58A0 (B), tick 0x1413AED70 ✓
(play calls, constants and sub-steps re-read), end 0x1413AB9A0 (no animation, B).

- Request (0x141440030 ✓ for the parts quoted), every frame with stick input, gated by "can run"
  (0x14142CE00, differs ✓): stores the target yaw (state +0x8C), the magnitude and
  `bWalk = magnitude < 0.8`. When the current state is Idle it pushes Run — with sub-step 1 for a run
  request, sub-step 0 for a walk request. While the Jump state is current, "can run" is true **only in
  Jump sub-step 3** (ledge fall entry, with on-ground or the small-drop flag); there Run is queued with
  sub-step 0 (loop, no `RunStart`). During a normal jump, fall or landing no Run is queued.

| Sub-step | Animation | Play ✓ | Leaves when ✓ |
|---|---|---|---|
| 1 start | `RunStartN` | −1, mode 2; then blend-space X = `delta`, once | same tick → 2 |
| 2 | (start playing) | | is-anim-end → loop name played (−1, mode 1) → 0 |
| 0 loop | `RunLoopN` or `WalkLoopN` | −1, mode 1, only when the name differs from the current one | below |
| 4 end | `RunEnd` / `WalkEnd` + `R` / `L` | −1, mode 2 | is-anim-end → state finishes → Idle (B); a run request → 1 (B) |

- `delta` ✓ = target yaw − actor yaw, wrapped to (−180, 180] degrees. Negative = target on the left (B;
  consistent with the data: sample −90 is `0007L`, whose root moves to the character's left).
- **Sharp turn** ✓: in the loop while running, `|delta| ≥ 75` → pawn vfunc +0x16E8 (stop movement),
  `RunStartN` played again (−1, mode 2) with X = `delta` → sub-step 2. This is also the 180° turn; the Run
  state has no other turn.
- **`RunLoop` X** ✓: each tick with a request, the blend-space range is read from the asset and
  X = `delta` clamped to it (−10…10) — the remaining yaw error in degrees, not a turn rate; 0 while
  walking (B). In the same tick the actor is turned towards the target yaw (pawn vfunc +0xDB0 with
  `0.1 × dt × 60`; lerp fraction or step not determined — movement spec).
- **Walk ↔ run** ✓: the run loop is kept while the timer at state +0x98 is > 0; it is reloaded to 0.2 s
  (0x3E4CCCCD) on every run request and counts down on walk requests, so run → walk is delayed by 0.2 s.
  Walk → run goes through `RunStart` (B). The loop change is a plain play with the default blend (0.2 s).
- **Play rate** ✓: the only set-play-rate call is in forced-walk mode (`MyForceWalkSpeed × 0.007142857`);
  otherwise 1.0.
- **Stop** ✓: on a tick without a request, when the movement component's horizontal acceleration² ≤ 1:
  pawn vfunc +0x16E8, then the End name from selector 911 with walk = "not running" and
  **left = bit 1 of state +0x84 = `m_bReverseLeg`**, played (−1, mode 2) → sub-step 4. `m_bReverseLeg` is
  set and cleared by the `TresReverseLeg` notify state of the loop animation (B; data: window
  0.100–0.333 s in `0003*`, 0.200–0.733 s in `0002`). So: stop inside the window → `…EndL`, outside →
  `…EndR`. An End animation always plays (no minimum run time, B); Idle follows when it ends.
- **Ledge** (B): not on ground and floor further than 2 cm → Jump state pushed at sub-step 3.
- `TurnN` (B): its selector 0x141435690 is called only from cinematic and AI states — not used for the
  player's own locomotion. `RunSway*` (B): played by a separate state `TresPlayerState_Swaying`
  (0x1413CC7B0; blend 0.1, mode 2), entered with a target object from outside the Run tick; its trigger was
  not determined. Stage 1 omits both.

**Jump / fall / land** — `TresPlayerState_Jump` (id 3), vtable 0x14692CE20: begin 0x1413868C0 (B), tick
0x141395450 ✓ (sub-steps 0–4, 10, 11, play calls and thresholds re-read), end 0x14138D850 (no animation).

| Sub-step | Animation | Play ✓ | Leaves when ✓ |
|---|---|---|---|
| 0 | `JumpNormalStart` | −1, mode 0 (0x1413A0BF0) | `TresJumpStart` notify → handler 0x14139AB60 → 1 |
| 1 | (take-off continues) | | is-anim-end → `JumpNormalRiseLoop` (−1, mode 1) → 2 |
| 2 | `JumpNormalRiseLoop` | | vertical velocity ≤ 0 → `JumpNormalDownLoop` (−1, mode 1) → 4; the play is deferred while pawn vfunc +0x1A10 (jump-duration phase over) is false |
| 3 | fall entry | `JumpNormalDownLoop` (mode 1) | → 4. With stick held and floor ≤ 40 cm below, the run loop is kept for up to 1.0 s (B) |
| 4 | `JumpNormalDownLoop` | | on ground → landing |
| 10 → 11 | landing animation | −1, mode 2 | below |

- **The launch impulse waits for the notify** ✓: the handler acts only in sub-step 0; it calls pawn vfuncs
  +0x1090 and +0x1A28 with the jump power stored on the state (+0xA4) and sets sub-step 1. The notify sits
  at 0.0666 s in `0031` (data). Counting the update that follows the play call as update 1 (position 0,
  section 8), the notify fires on update 5 at 60 fps and update 3 at 30 fps (section 9); the rise loop
  starts when the 0.1 s clip ends. (Scripted jumps with the state's flag bit 0 skip the wait, B.)
- **Rise → fall** ✓: exactly "vertical velocity ≤ 0" (pawn vfunc +0x1568). While rising with the button
  not held, another gravity scale is set (pawn vfunc +0x1650) — movement spec.
- **Landing** ✓: on the tick the pawn is on ground: pawn vfunc +0x16E8; then by accumulated fall height
  (state +0xB0, summed only while falling): **≤ 75 cm → `LightLand`** (pose 2), **> 75 cm → `Land`**
  (pose 1), and > 75 cm with stick held and ≥ 800 cm → `RollLand` (not packed). Played (−1, mode 2),
  `m_bEnableChange` cleared, sub-step 10. With stick held and the "small drop" flag 0x20, no landing
  animation is played at all.
- **Interrupting the landing** ✓ (sub-step 11): if the stick is held and `m_bEnableChange` is set — by the
  `TresEnableChange` notify, at **0.099 s** in `0034` and `0120` (data) — the pawn's movement is stopped and
  the Jump state finishes at once. No Run is queued at that point (see the request gate above), so Idle
  begins and the next run request starts Run **through `RunStartN`** (or `WalkLoopN` for a walk request).
  Otherwise the landing plays to its end (0.4 s) and Idle follows. A new jump is accepted from the same
  notify on (movement spec section 4.1: "can jump" 0x14142B070 — not re-read here).
  *Correction:* an earlier version of this document said the queued Run starts in its loop after a
  landing; that path exists only for the small-drop case (Jump sub-step 3).
- `LightLandR` / `LightLandL` ✓ are never chosen by the landing code above (it passes only poses 1 and 2).
  They need pose 3 / 4 stored on the pawn by a `TresAnimNotify_LandAnimPose`; none of the packed animations
  carries that notify (data). Stage 1 omits them.
- Walking off a ledge (B): Run pushes Jump at sub-step 3 → falling loop directly. `JumpNormalDownStart` has
  no row in Sora's set. Wall slide (`FallSlide`), dive fall and diving are other sub-steps, excluded.

### 2.4 `RunStart` and the movement component

`RunStartN` is played with mode 2 and its five sequences have `bEnableRootMotion` = true, so the montage
extracts the root bone's motion and hands it to the character. Per sample (data; per-frame values in
`manifest.json` → `root_motion.tracks`, actor space, UE cm, yaw positive = to the right):

| Sample (X) | Clip | Total translation (forward, right) | Total yaw | Frames / time |
|---|---|---|---|---|
| 0 | `0007` | (180, 0) | 0° | 8 / 0.2333 s |
| −90 | `0007L` | (0, −180) | −90° | 8 / 0.2333 s |
| 90 | `0007R` | (0, 180) | 90° | 8 / 0.2333 s |
| −180 | `0007BL` | (−180, 0) | −180° | 8 / 0.2333 s |
| 180 | `0007BR` | (−180, 0) | 180° | 8 / 0.2333 s |

The motion is not uniform: `0007` covers 11.9, 26, 40.4, 56.6, 75, 104, 180 cm at frames 1–7 (the last
frame alone 76 cm); the turning samples reach their final yaw at frame 6. The blend space mixes the two
neighbouring samples by `delta` (fixed for the whole clip), so the extracted motion is the weighted mix of
their tracks.

How it reaches the capsule (movement spec section 3.6, not re-read here): the movement component has
`m_bAddFullVelocityToRootMotion` = 1, so `PhysWalking` adds the root-motion velocity to the move velocity;
during Run sub-step 2 the state issues no movement input and no turning, and the sharp-turn case calls
"stop movement" (pawn vfunc +0x16E8) right before the play ✓ — so during `RunStart` the displacement and
the yaw are the animation's. When it ends the run loop takes over and velocity is the movement
component's again.

**No other stage-1 clip has a root track.** The root bone `kh_sk` has exactly one translation key and one
rotation key (constant, zero displacement, zero yaw) in `0001`, `0004` (idle), `0002` (walk), `0003`, `0003L`,
`0003R` (run), `0005`, `0006` (WalkEnd), `0011`, `0012` (RunEnd), `0031`, `0032`, `0033` (jump), `0034`
(Land), `0120` (LightLand), `0109`, `0110` (`manifest.json` → `root_motion.clips_without_root_motion`), and
`bEnableRootMotion` is false on all of them. They are played with the root-motion mode bit, but nothing is
extracted: the stop and landing clips move only the body over a fixed root (hips travel a few cm).

For a reimplementation: during `RunStart`, drive the capsule from the weighted blend of the samples' per-frame
root deltas (section 8), and in the pose **lock `kh_sk` to the mesh reference root transform** (section 7) —
not to the track's first key. Resume normal movement afterwards.

## 3. The pack: `build/sora_pack/`

Built by `node tools/sora_pack.js` (about 20 s; `--check` validates the files on disk without writing;
`--no-normals` leaves the normal maps out of the materials). Texture decoding: `tools/lib/bcdec.js`
(BC1, BC3, BC4, BC5, BC7, BGRA8, G8 → RGBA; PNG writer). Reference evaluators: `tools/lib/kd_eval.js` (KineDriver),
`tools/lib/bonamik/` (Bonamik). 57 files, 30 MB (pack tool 1.4.0).

| File | Content |
|---|---|
| `sora.glb` | LOD 0 mesh, 12 primitives, one skin with **194 joints**, 12 materials referencing `textures/*.png` by URI |
| `sora_all.glb` | the same plus all 29 animations (animation name = sequence asset name, e.g. `0003L`) |
| `anims/<sequence>.glb` | skeleton nodes (same names, same order, same reference pose) + one animation |
| `textures/*.png` | 13 files: base colour (RGB, sRGB), normal maps (RGB, linear), tinted eyelash mask (RGBA) |
| `face/<sequence>.glb` | the six face clips the stage-1 body clips start (`0000EM`, `0006EM`, `0021EM`, `0050M`, `0051M`, `0052M`), same skeleton |
| `data/animgraph.json` | the 34 anim-graph nodes with every property and bound input (section 6) |
| `data/legik.json` | leg-IK node definitions and native defaults (section 10) |
| `data/kinedriver.json` | the 313 KineDriver operators and per-bone write list (section 13) |
| `data/bonamik.json` | Bonamik asset data: scene, groups, particles, links, cones, chains, notify effects (section 14) |
| `data/bonamik2.json` | the same plus the runtime records after setup (particles, links, cones with quaternions, anchors, chain roots, collision objects) |
| `data/physics_asset.json` | physics asset bodies and constraints (not used for the player's pose) |
| `manifest.json` | format `kh3-sora-pack/1`, see below |
| `validation.json` | validator, independent-check and KineDriver-check results |

The data files are copies of `audit/sora_port/*.json` (extracted by the research passes; each carries its
schema).

`manifest.json`:

- `identification` — the asset paths of section 1, mesh component transform, capsule, movement overrides;
- `conventions` — the exact transform applied (below);
- `skin` — the joint list of the skin (joint index → bone index and name) and the bones that are not
  joints. The skeleton has 274 bones; only the 194 that carry a vertex weight at LOD 0 are skin joints (a
  loader limit of 256 joints per skin), the other 80 remain ordinary nodes of the same hierarchy, with the
  same names, and are still animation targets. `JOINTS_0` indexes the joint list;
- `root_motion` — per-frame track of the root bone for every clip that has one (pack and UE conventions),
  and the list of clips whose root track is constant; `feet` — sole height relative to the actor origin;
- `placement` — how to put the model under the actor;
- `skeleton.bones[]` — index, name, parent, reference pose in pack units and the original UE values;
- `materials[]` — per slot: instance, parent chain, every texture/scalar/vector parameter, what was simplified;
- `logical_animations[]` — per animation-set name: asset, class, `play` (mode, loop and root-motion bits, blend-in argument and resolved seconds, whether root motion is extracted, which function plays it), use, and for blend spaces the axis
  range, interpolation times and samples (value → file);
- `runtime` — the constants of sections 5–14 in machine-readable form: frame order, the saved-pose rule and
  blend formulas, the bone index lists of the `G_eye` / `G_mouth` subtrees, leg-IK alpha rule and per-clip
  `TresIK` windows, face rows / type table / per-clip face plays, KineDriver summary and check result, the Bonamik facts (order, 30 Hz rule and interpolation, start
  state, gravity, counts, driven bone list, dynamic particle bones, per-clip notify effects, check result);
- `face_animations`, `data_files` — what the `face/` and `data/` files are;
- `logical_animations[]` for blend spaces also carry `grid_samples`, `sample_weight_rule`, `input_filter`
  (type, window, algorithm), `notify_trigger_mode`;
- `items[]` — every file: source asset and package, sha256 of the cooked `.uasset` / `.uexp` / `.ubulk`, sha256
  and size of the packed file; for animations duration (the float32 value exactly), `NumFrames`, frame rate,
  loop, track count, tracked bone indices, key counts, sequence properties with defaults filled in,
  root-bone track summary, and **every notify** with full-precision time, trigger offsets, duration,
  class and the notify object's serialized properties.

The same items, one row each, are in `sheets/port/sora-assets.json` (`node tools/q.js sora-assets`).

### Conventions

UE: centimetres, +Z up, left-handed; the mesh faces +Y in its own space. Pack: metres, +Y up, right-handed;
**the character faces +Z, left hand on +X, feet at the origin.** The conversion is baked into every vertex,
bone and key; no file has a conversion node, and the root bone `kh_sk` is a scene root.

| Quantity | UE → pack |
|---|---|
| position | (x, y, z) cm → (x, z, y) × 0.01 m |
| direction (normal, tangent xyz) | (x, y, z) → (x, z, y) |
| tangent w | negated |
| rotation quaternion | (x, y, z, w) → (−x, −z, −y, w) |
| scale | (x, y, z) → (x, z, y) |
| triangle | UE order (a, b, c); counter-clockwise is front |
| UV | unchanged |

To undo: apply the same rules with × 100. To place: put the scene at the actor origin (feet), rotated
about +Y so that pack +Z points along the actor's facing; no vertical offset; the capsule centre is 0.91 m
above the origin.

Animations: key times in seconds, values as decoded by `tools/anim_convert.js` (see
`docs/formats/animsequence.md`), `LINEAR` samplers. Rotations are renormalised and kept on one hemisphere
between neighbouring keys. Bones without a track (173 of 274 in the locomotion set: hair, face, sleeves,
skirt, some `_spo` helpers) keep the reference pose. Root tracks are left on `kh_sk` — nothing is extracted
or zeroed; the consumer decides (section 2.2).

### Validation (run of this session)

- glTF-Validator 2.0.0-dev.3.10 on all 37 `.glb`: 0 errors, 0 warnings (infos only: `NODE_EMPTY`,
  `URI_GLB`, `UNUSED_OBJECT`, `UNUSED_MESH_TANGENT`).
- Independent read of `sora.glb` (194-joint skin): weights sum to 1 within 5.2e-8; no joint index out of range; inverse bind
  × bind = identity within 5.9e-8; skinning at the bind pose reproduces the vertices within 1.3e-7 m; 194
  bones carry weights; bounds x ±0.558, y −0.006…1.666, z −0.219…0.253 m; nose 0.11 m in front (+Z) of the
  head bone; left hand at x = +0.41; 66,957 triangles counter-clockwise against their normals, 775 not.
- Animations: every quaternion unit within 1e-5, no non-finite value, times start at 0 and increase.
- Textures: each decoded top mip was compared with the decoded next mip (mean absolute difference 0.2–1.4
  of 255, no outlier block); the BC7 files exercise modes 0, 1, 3–7 and all 64 two-subset shapes.
- Software render of `sora_all.glb` (idle front view, run side view; `build/sora_tmp/render.js`) shows a
  correctly textured, correctly posed Sora; the run render is byte-identical before and after the
  joint-list reduction.
- `node tools/extract.js` and `node tools/preflight.js --summary`: 0 errors; `port/sora-assets` 57 rows, 0
  unchecked cells, 0 dangling refs.
- Bonamik evaluator checks (section 14) pass.
- KineDriver model check (section 13): reference pose and baked tracks of all 29 body clips pass.

## 4. Confidence, corrections and open questions

Part I (sections 1–3): asset identification, animation-set and blend-space contents, name selectors, play
call arguments, Run and Jump sub-steps and thresholds are data or exe re-read by the author (✓), or rest on
the delegated passes ((A) / (B)) as marked.

**Corrections made to Part I after the Part II research:**

1. The blend source is the previous frame's saved pose (recursive), not a snapshot taken at the play call.
   The earlier recommendation "blend linearly from a snapshot" is withdrawn (sections 6, 8).
2. The first update after a play call shows t = 0: the position starts moving on the second update, the
   weight on the first (section 8).
3. A one-shot does not "hold" inside the montage: it is stopped on the update that reaches its end and the
   slot then shows the saved pose (which equals the last blended pose).
4. Rotation keys and pose blends use sign-corrected nlerp, scale uses log-space blending (sections 7, 8).
5. RunLoopN's input smoothing is a triangular FIR filter with one sample per update, not a time-weighted
   average (section 8).
6. Notify windows are shifted by 1e-4 until the first loop wrap, and `SQEX_AllAnimation` fires the
   notifies of every blend-space sample (section 9).
7. During RunStart the root bone in the pose is locked to the mesh reference root (section 7).
8. Leg IK, face animation, KineDriver and Bonamik are part of the displayed pose (sections 10, 12–14);
   Part I's "not reproduced" list is superseded.
9. The `_spo` scale tracks are baked KineDriver output, not artefacts (section 13).

**Open (details and probe requests in section 15 and the notes' "not determined" lists):**

1. Tick order of state queue, movement component and mesh within a frame (R1).
2. Bonamik (section 14): the start state in the real game if a level script issues a reset / pre-roll; who
   fills the default tables the group-parameter notify restores from; world wind per map; the ground query;
   the random stream. The solver itself is specified and has a reference evaluator.
3. The active look-at algorithm and its target selection (not needed without targets).
4. The game-instance factor on the leg-IK alpha; whether `TresChangeIKTypeVolume` volumes exist in maps of
   interest (they switch to the other IK branch).
5. `GlobalAnimRateScale` / update-rate optimisation on the player mesh (assumed defaults); the ref-pose
   override virtual of the mesh component (assumed null).
6. Exact operand order of two quaternion products in the leg solve (decompiled C only); steady-state of
   the hips controller derived, not measured.
7. KineDriver: `TargetRotate`, `ComputeSpaceBases` and the two-source `SourceRotate` rest on code reading;
   bit-exact float behaviour not compared.
8. Several functions used as evidence have verdict differs / same-partial / no-verdict / not-compiled
   (tables in the notes).
9. **Materials are approximations** (flat hair and iris colours, no eye highlight, no subsurface / AO /
   detail layers); normal maps wired with inverted green and negated tangent w, not checked visually.
10. Reconciliation with `docs/sora-movement-spec.md`: `docs/sora-spec-reconciliation.md`. What the current
    app does differently from the game: `docs/sora-animation-spec-changes.md`.

# Part II — reproducing the pose 1:1

Sections 5–14 specify what the game does to turn the clips into the displayed pose. They summarise five
code-reading passes; the full pseudocode, every constant with its address, the address → role → verdict
tables and the "not determined" lists are in the notes named at the head of each section (all under
`build/sora_tmp/`). Evidence marks on each statement:

- **[I]** read at instruction level (disassembly); **[C]** decompiled C only; **[A]** asset data;
- **[U]** stock UE 4.17 behaviour, the function's identity confirmed in the exe (the public 4.17 source
  semantics apply);
- **[?]** not determined.

Where Part I (sections 1–4) said something different, Part I has been corrected; the corrections are
listed in section 4.

## 5. Frame order

Notes: `native_C_graph.md` §9, `native_E_blend.md` §6–7.

1. **The pose is ticked from inside the movement component's tick, with the real frame dt, before the
   movement is performed** [C]: `UTresCharMovementComponent::TickComponent` 0x140598190 → 0x140595730 calls
   `mesh->TickPose(dt, bNeedsValidRootMotion = true)` whenever the pawn plays root motion **or** the native
   flag movement component +0xC18 bit 0x20 is set, and the constructor sets that flag [I 0x14057185E].
   That one call runs, immediately and on the game thread: montage weight update, montage position advance
   (root motion extracted), notify dispatch (the `SQEX_NotifyTriggerTiming` of `UTresAnimInstance` is
   `OnPostUpdateAnimation` [I 0x140559854]), `NativeUpdateAnimation`, the graph **update**. The mesh's own
   tick then only evaluates.
2. Root motion extracted in that tick is consumed in the same movement tick (`ConsumeRootMotion`
   0x144053230), converted to world space and turned into a velocity `worldDelta / dt` for `PhysWalking`;
   root-motion rotation is applied to the actor after the move [C]. No translation-scale factor on this path.
3. Graph **evaluation** (worker or game thread): slots blended against last frame's saved pose → saved pose
   written → leg IK → look-at → local pose → component-space transforms (section 6).
4. `TG_PostPhysics`: the KBD component runs KineDriver and Bonamik on the component-space transforms
   (sections 13, 14). Their results never feed back into the saved pose.
5. There is **no fixed-rate sub-stepping and no dt clamp** in the animation path [C]: everything advances
   with the variable frame dt (× `GlobalAnimRateScale`, default 1; update-rate optimisation default off —
   both assumed at engine defaults [?]). Bonamik is the exception (30 Hz, section 14).
6. Consequence for game logic: a play call or blend-space parameter set by a state lands in the same
   frame's pose if the state queue ticks before the movement component (the movement spec's working
   order); the effect of a notify fired in frame N is seen by the states in frame N + 1. The state-queue
   vs movement tick order itself is still open (movement spec Q1) [?].

## 6. The anim graph at runtime

Notes: `native_C_graph.md` §1–4, 6, 7; data: `data/animgraph.json` in the pack (all 34 nodes, every
serialized property, bound inputs).

- `LinkID` = index into the class's node property list with Root = 0 [C 0x143F459B0]. The blueprint has
  **no event-graph logic**; every bound variable is a native property of `TresCharAnimInstance` [A].
- Tree (`#n` = LinkID):

```
Root#0 <- C2L#17 <- ModifyBone atama#24 <- ModifyBone kubi#23 <- ExLookAt#12 <- L2C#18
       <- BlendListByBool#13 (IKinemaSW)
            true : C2L#33 <- TwoBoneIKMultiped#31 (Alpha <- m_LegIKAlpha) <- L2C#32 <- UseCachedPose#16 "SavedPose2910"
            false: C2L#19 <- CollTwoBoneIK L#6 <- CollTwoBoneIK R#5 <- L2C#4 <- UseCachedPose#15
SaveCachedPose#14 "SavedPose2910" <- TresSaveCachedPose#3 <- LayeredBoneBlend#9
       base            : TwoWayBlend#20 (blendBodyWeight) : A = Slot FullBody#1 <- TresUseCachedPose#2 ; B = Slot FullBody2#21 <- TresUse#22
       layer G_eye     : LayeredBoneBlend#29 : base Slot Face#8 <- TresUse#7 ; layer Slot Face2#26 <- TresUse#25
       layer G_mouth   : LayeredBoneBlend#30 : base Slot Mouth#11 <- TresUse#10 ; layer Slot Mouth2#28 <- TresUse#27
```

- Values in stage 1 (no target, weapon away) [C ctor 0x140572C50 / native update 0x14058B450, A]:
  `IKinemaSW` true (constructor; nothing found writing it for the player except a level volume class) →
  the `TwoBoneIKMultiped` branch at weight 1, **the two `CollTwoBoneIK` nodes never run**;
  `blendBodyWeight` 0 → `FullBody` only; `blendFaceWeight` / `blendMouthWeight` 0 → `Face2` / `Mouth2` not
  evaluated; `m_FaceAnimBlendRate` 1 → both face layers fully applied; `LookAtRate` 0, `blendKubiWeight` /
  `blendAtamaWeight` 0 → look-at and both `ModifyBone` are exact pass-throughs.
- **The Square Enix cached pose is recursive** [I 0x14053F220, 0x14053F310, 0x14056CA70, 0x14056D2F0,
  0x14056D240]: `TresAnimNode_SaveCachedPose::Evaluate` stores its input pose in the proxy's `MySavedPose`
  on **every** evaluation, unconditionally; `TresAnimNode_UseCachedPose::Evaluate` returns whatever is stored.
  All six readers run before the single writer within one evaluation, so every slot's source pose is the
  **previous frame's** saved pose. The node flags `m_bAlwaysRefferPrevPose` / `m_bSetRefferedFlag` are
  never read. The pose is stored in **local space**, bit-exactly, per mesh bone.
- Per-frame recurrence (`S` = saved pose, `w` = montage weight of the slot this frame):

```
P = S[n-1]                                  S[0] = IdleN (0001) at t = 0, reference pose for untracked bones
Body = SlotBlend(FullBody, P); Eye = SlotBlend(Face, P); Lip = SlotBlend(Mouth, P)
S[n][b] = Eye[b] for the G_eye subtree (30 bones), Lip[b] for the G_mouth subtree (26), Body[b] otherwise
shown pose = ModifyBone(ExLookAt(LegIK(S[n])))   -> then KineDriver, Bonamik.   Never fed back.
SlotBlend(slot, P): slot weight <= 1e-5 -> P exactly;  else blend of the montage pose (weight w) and P (weight 1 - w), section 8
```

  So during a blend-in the old pose's share after k frames is `Π(1 − w_i)`, not `1 − w_k`: the blend
  converges faster than the nominal blend-in time and depends on the frame rate. After a one-shot ends the
  slot returns `P` unchanged: the last blended pose is held bit-exactly. The saved pose is taken **after**
  the face layers and **before** leg IK and look-at.
- First frame [C 0x14056BD30]: `MySavedPose` is initialised lazily from `m_CacheStartPoseAnimName` =
  `IdleN` evaluated at t = 0.
- Stock nodes: `FAnimNode_Slot`, `LayeredBoneBlend`, `TwoWayBlend`, `BlendListByBool`, the space
  conversions and the stock cached pose are stock 4.17 [U]; `LayeredBoneBlend`'s per-bone blend and
  `ModifyBone`'s inner function were not re-read (bypassed or trivial with Sora's values) [?].
- No post-process AnimBlueprint (the mesh package imports none) [A].

## 7. Pose extraction from a sequence

Notes: `native_E_part4_seq.md`; per-sequence facts in `manifest.json` → `items[].sequence_properties`,
`key_counts`.

- Stock `UAnimSequence::GetBonePose` 0x143F41360 [C]. **No looping argument anywhere: a looping clip never
  interpolates from its last key to its first**; the wrap is done on the play position before evaluation.
- Key lookup [C, decoders 0x143F12340…]: `p = time / SequenceLength`; block without frame table:
  `KeyPos = p × (NumKeys − 1)`, alpha = fraction (0 when `Interpolation` is Step or the Square Enix
  `m_DisableInterpolation` is set — false on every packed clip [A]); `p ≤ 0` → first key, `p ≥ 1` → last key.
- Between two keys (`docs/formats/animsequence.md`, exe): translation linear; rotation **normalised linear
  blend along the shorter arc** (not slerp); scale **geometric per component** (`a × (b/a)^t` when `a × b > 0`).
  The pack's glb samplers are `LINEAR`; a 1:1 implementation must apply these rules itself.
- Bones without a track get the **mesh** reference pose (bit-identical to the Skeleton asset's for Sora).
  All 274 bones have `TranslationRetargetingMode = Animation`: no retargeting effect [A].
- Root bone `kh_sk` in the pose: when the sequence has `bEnableRootMotion` and it is played without mode
  bit 0x10 (and `RootMotionMode` ≠ none — Sora: `RootMotionFromMontagesOnly`), the root is **locked to the
  mesh reference root transform** (`RootMotionRootLock` = RefPose, the default) — independent of mode
  bit 0x02. Otherwise the root track value is used. For exactness: the clips' root rotation key is
  (−0.70708334, 0, 0, 0.70713019) (16-bit quantisation), the locked value (−0.70710671, 0, 0, 0.70710683)
  with scale (1, 0.99999994, 0.99999994).
- Root-motion delta [I 0x143F3DC90]: Square Enix rotates the root translations by
  `Inverse(InitialRootRot × FRotator(0, 0, −90))` before the stock computation; for Sora that rotation is
  the identity up to quantisation (6.6e-5 rad), so the delta is the root-track translation difference in
  mesh space, as tabulated in `manifest.json` → `root_motion.tracks`.
- Sequence `RateScale` is 1 on every packed clip.

## 8. Blending and filters

Notes: `native_E_blend.md` §1–3.

**Play call** [C 0x14056F470 / 0x14056F260 / 0x143F4D3F0]: stops every montage of the slot group with blend
0, creates the instance with position 0 and weight 0. It does not advance, weight or evaluate anything.

**Weight** [C 0x143F522B0 same; `FAlphaBlend::Update` stock]: blend option of every by-name play is
**Linear**. `weight = min(1, (dt_1 + … + dt_n) / blendIn)` after the n-th update following the play — the
real dt of the first update counts. `blendIn = 0` → 1 on the first update.

**Position** [I 0x143F33CE6, C 0x143F33C00]: on the **first update after the play call the position stays
0** (flag b3: that update's dt is replaced by 0 for the position only). From the second update on
`position += dt × playRate`. So the t = 0 frame costs one frame of animation time.

**Loop wrap**: the remainder is carried exactly. **End of a one-shot**: on the update where the position
reaches `SequenceLength` (clamped) the instance is stopped with blend-out 0 in that same update; that
frame still shows the last frame at the current weight; `IsAnimEnd` is true from that moment; on the next
update the instance has weight 0 and the slot shows its source (the saved pose, which holds the last
blended pose — section 6). **A new play while one is alive**: the old instance contributes nothing to the
first frame evaluated after the call; there is never a cross-fade between two montages.

**Slot combine** [C 0x143F4EFF0, 0x143EF0160; U structure]: with the montage pose `M` (weight `w`) and the
source `P` (weight `1 − w`), per bone, in this order (montage first, source last):

```
rotation    R = q_M × w ;  R += (dot(R, q_P × (1−w)) >= 0 ? +1 : −1) × q_P × (1−w) ;  normalise      (sign-corrected nlerp, stock)
translation T = t_M × w + t_P × (1−w)                                                                   (stock)
scale       S = exp( ln(s_M) × w + ln(s_P) × (1−w) )  per component                                    (Square Enix: log space; stock is linear)
w >= 1 − 1e-5: the source is not evaluated and the result is M × w (all ten floats), rotation renormalised
```

**Blend space** [C 0x143F53D60, 0x143F79E00; I 0x143F425B0]:

- Sample weights: stock `UBlendSpace1D` grid interpolation (`GridNum` 4): RunLoopN
  `w(0003L) = clamp(−x/10, 0, 1)`, `w(0003R) = clamp(x/10, 0, 1)`, `w(0003) = 1 − |x|/10`; RunStartN linear
  between neighbours at −180 / −90 / 0 / 90 / 180. Stock per-sample weight smoothing is off
  (`TargetWeightInterpolationSpeedPerSec` 0).
- Sample poses are blended with the same formula as the slot (weights descending, log-space scale).
- All samples share one normalised position; equal lengths → always the same time, no drift.
- **Input filter of RunLoopN** [I]: `FFIRFilterTimeBased`, type **Linear** (constructor default,
  0x143F5A68B), window 0.4 s, **one unweighted entry per update**:

```
Update(input, dt): clock += dt
    drop entries with clock − stamp > window
    add { input, stamp = (clock − dt) + window }
    out = Σ w·input / Σ w   over entries with w = 1 − |clock − stamp| / window > 0
Reset at play (value = play-time parameter 0): one entry { value, stamp = clock }
```

  In terms of the age `a` of an input (time since the update that added it began): triangular weight
  peaking at `a = window`, zero at `a = 0` and `a = 2 × window`. The newest input has weight `dt/window`;
  the play-time value has weight `1 − elapsed/window`. The result depends on the frame rate (one sample per
  update, not dt-weighted). On the first update after play the output is the play-time parameter.
- RunStartN has no filter: the parameter is used unfiltered on the update after it is set.
- "Set blend-space parameter" writes a target that the next animation update consumes (through the
  filter); that update's evaluation uses it.
- `m_InterpolationTime` of a blend space is only the default blend-in time of the play call.
- Blend-space root motion (RunStartN): weighted blend of **every** cached sample's delta (weights clamped
  0…1; a sample without `bEnableRootMotion` contributes identity with its weight), then per-axis play scale.
  In the pose each sample's root is locked as in section 7.

## 9. Notify timing

Notes: `native_E_blend.md` §5; all notifies of all packed clips with full-precision times and the notify
objects' properties: `manifest.json` → `items[].notifies`.

- Inclusion rule [I 0x143F654D0, stock]: moving forward from `Prev` to `Cur`, a notify fires iff
  `Trigger ≤ Cur && EndTrigger > Prev` (`Trigger` = time + `TriggerTimeOffset`; for a state `EndTrigger` =
  its end). Instant notify: `(Prev, Cur]`.
- **Square Enix window shift** [I 0x143F3441F]: with `off` = 1e-4 (0 after the instance's first loop wrap):
  first update after play: window `(0, 1e-4]`; later updates: `(prev + 1e-4, cur + 1e-4]`; after the
  first wrap: `(prev, cur]`. Assets store `TriggerTimeOffset` = +1e-4 on instant notifies placed at t = 0,
  so **a t = 0 notify fires on the first update after the play call**; states starting at 0 are active on
  that update too.
- An instant notify at `T > 1e-4` fires on the first update whose position `p ≥ T − 1e-4`. With constant
  dt, update 1 = the t = 0 frame: `TresJumpStart` (0.0666445 s) fires on **update 5 at 60 fps, update 3 at
  30 fps**; `TresEnableChange` (0.099 s) on update 7 / 4. The end of a one-shot is not shifted and is
  sensitive to float rounding of the accumulated position.
- Notify states: active on an update iff `Start ≤ Cur' && End > Prev'`; Begin on the first such update,
  End dispatched on the first update where it is no longer listed. End calls are issued before Begin
  calls. `TresReverseLeg` in the run loop is 0.100371 … 0.333671 s [A].
- **Blend spaces with `SQEX_AllAnimation`** (RunLoopN, RunStartN) [C 0x143F79E00]: on every update the
  notifies of **every sample sequence** fire, regardless of weight (also samples with weight 0).
  Consequences: the three run samples' `TresReverseLeg` states coincide (same times); `0003`'s mouth
  notifies and `0007`'s `TresIK` window fire whatever the lean / turn angle.
- Blend-space and montage notifies pass only while the slot's weight is > 1e-5; instant notifies also pass
  the `TriggerWeightThreshold` (1e-5) against the instance weight.
- Dispatch happens inside the pose tick (section 5), i.e. inside the movement tick.

## 10. Leg IK

Notes: `native_D_legik.md` (full algorithm §4, enable logic §3, floor relation §6); data: `data/legik.json`,
`manifest.json` → `runtime.leg_ik` (per-clip `TresIK` windows).

**Which node**: `TresAnimNode_TwoBoneIKMultiped` (#31) [C ctor 0x140570C60, I 0x140570E87]. The two
`CollTwoBoneIK` nodes and the mesh component's `m_CheckFootSteps` (footstep effects only) are not part of it.

**Alpha** [C 0x14058B450, 0x1405BA6B0, 0x1405B4260, 0x1405B3010; I stock `TickBlendWeight` 0x143F0AA50]:
`m_LegIKAlpha` = a per-pawn linear interpolator (current, target, remaining time). `TresAnimNotifyState_IK`
begin sets target = `m_TargetAlpha` (default 1) over `m_StartInterpolationTime`; its end sets target = 0
(`MySkelCtrl_IkInitValue`) over `m_EndInterpolationTime`. Two states with the same target merge (no
end/begin pair when one clip's window follows another's) [C 0x143F2DC50]. The node uses
`min(alpha, 0.99)` [I]. Resulting values: idle, idle break, stop and landing clips 1.0; walk loop 0.9; run
loop, rise and fall loops 0 (no window → the node is an exact pass-through); RunStart 1 until 0.167 s then
to 0 over 0.1 s (the centre sample's window fires for every turn angle, section 9); jump start to 0 over
0.02 s. Windows with times: `runtime.leg_ik.windows_by_clip`. A global factor from the game instance
(vt+0x428) multiplies the alpha [?] (assumed 1 in field play).

**Algorithm** (per evaluation, when alpha > 1e-5):

1. Per foot (`L_ashi1`, `R_ashi1`): a **sphere sweep, radius 10 cm**, from 30 cm (left) / 33 cm (right)
   above the ankle to 120 cm below it, along the component's up axis; collision channel 22
   `TresIKCollision`, simple collision, owner ignored; first blocking hit [I parameters 0x14063ACB0]. A
   hit is cached and reused once when the component moved < 2 cm and turned < 10° (so sweeps happen every
   other frame while standing); a new hit lower than the cached one within 6 cm / 4 cm is rejected
   ("keep highest").
2. `delta = sweepOffsetAtHit + (0, 0, ankleHeightInComponentSpace − radius)`; on flat ground
   `delta.z = floorZ − componentOriginZ` for both feet whatever the pose. `delta` is smoothed with
   `VInterpTo(…, FootSmoothingVelocity 13)` when it jumps by more than `13 × dt`, clamped to the leg length.
   A foot without a hit copies the other foot's smoothed Z.
3. Hips (`center`): target drop = the largest downward foot move (a foot that must go up never raises the
   hips), limited by the sweep distance; driven through a critically damped controller (gain 140, damping
   23.6643, dt ≤ 1/30) [I]; the stored value is multiplied by the alpha each frame, so the steady state is
   0.916 × target at 60 fps and 0.961 × at 30 fps for alpha 0.99 (derived, not measured).
4. Each leg is re-solved with the **Maya `ik2Bsolver`** routine (bone lengths preserved, pole vector from
   `JointTargetLocation` (−50, 50, 0) in the shin's bone space, no stretching, internal knee angle limited
   to 160°) [C 0x140551F90, 0x140551010].
5. Foot rotation: rotation from up to the hit normal, as a rotator clamped to roll ±15°, pitch ±10°,
   yaw ±25°, approached with slerp weight `13 × dt × 0.5` per frame, applied on top of the animated foot
   rotation.
6. The resulting component-space transforms of `center`, thigh, shin, foot of both legs are blended into
   the pose with the alpha (stock `LocalBlendCSBoneTransforms`) [U].

Dead for Sora (two legs): hips rotation, `ExHitps*`, the non-Maya solver.

**Where the feet end up** (`native_D_legik.md` §6): with IK at full weight the animation's z = 0 plane is
glued to the traced floor: the term the IK absorbs is the capsule float (origin − floor = −0.1 … +0.4 cm),
**not** the sole offset. The lowest sole vertices therefore sit 0.40–0.56 cm **below** the collision floor
in idle, in the game too; nothing absorbs that. Without IK (run, air) the soles are 0.0–0.66 cm below the
floor. On a slope each ankle keeps its animated height above the surface point below it (+ `10 × (1/cosθ −
1)` cm), the downhill foot reaches through the hips drop, the uphill foot through knee bend, and the feet
tilt within the clamps.

## 11. Look-at

Notes: `native_C_graph.md` §5. `TresAnimNode_ExLookAt` (#12) and the two `ModifyBone` nodes are **exact
pass-throughs for the player without a look-at target**: `LookAtRate` (pawn +0x788) is 0, so the node's
alpha is 0 and its evaluate is skipped [C 0x140536060]. The active algorithm
(`EvaluateBoneTransforms` 0x1405384E0), what sets the automatic look-at target and its allow/deny
conditions were not read [?] — needed only when NPCs or lock-on targets exist.

## 12. Face

Notes: `native_C_graph.md` §8; data: `manifest.json` → `runtime.face`, `face_animations`; clips: `face/*.glb`.

- Face animation comes only from `TresAnimNotify_FaceAnim*` notifies of the body clips (the automatic face
  play is off for Sora) [C ctor 0x140559710, A]. Each plays a montage on slot `Face` (G_eye subtree: eyes,
  lids, brows) and/or `Mouth` (G_mouth subtree), by type or name, through the same play functions as the
  body: previous montage of the slot cut, blend-in from the saved pose, recursive (section 6). Tracks of
  bones outside the two subtrees in a face clip are discarded.
- Type → row [C 0x14056D3E0]: NORMAL_AUTO `NO_0`, NORMAL_AUTO_ED01 `NO_21`, MOUTH_OPEN_01/02/03 `NO_50` /
  `NO_51` / `NO_52`; lookup tries `name`, `name_EL`, then `name_EYE` / `name_LIP` [C 0x14056E2B0], so
  `NO_0` and `NO_21` play their `_EL` clip on both slots and `NO_50…52` play on `Mouth` only.
- Rows → clips [A]: `NO_0_EL` `0000EM` (5.967 s, blend-in 0.13), `NO_6_EL` `0006EM` (9.833 s, 0.13),
  `NO_21_EL` `0021EM` (3.0 s, 0), `NO_50/51/52_Lip` `0050M` / `0051M` / `0052M` (two-frame mouth poses, 0).
  All are played looping. `SameCheck`: a row already playing on the slot is not restarted.
- What plays: idle, walk, run sides, stops, run start: `0000EM` on both slots; the run centre sample adds a
  breathing mouth (`0050M` / `0051M` alternating at 0.089, 0.191, 0.308, 0.433 s, blend-in 0.1); the rise
  loop switches eyes to `0021EM` at 0.374 s; fall loop `0021EM`; landings `0021EM` then back to `0000EM`.
  Times and properties per clip: `runtime.face.plays_by_clip`.
- Default pose: there is no reference-pose phase in practice; an implementation that does not animate the
  face should hold **frame 0 of `0000EM`** on the two subtrees. No native blink timer was found (not
  searched exhaustively) [?].

## 13. KineDriver (helper bones)

Notes: `native_F_kinedriver.md` (operator math §4, loop §5, recipe §7); data: `data/kinedriver.json`;
reference evaluator: `tools/lib/kd_eval.js`.

- A **stateless** post-process of the final animation pose: 313 operators run once per frame, in asset
  order, in place on the **component-space** transforms, after the anim graph (leg IK included) [C 0x143ABFAD0, 0x143AC5AD0]; the KBD component runs it before Bonamik, every frame, with the real dt [I, `native_H_bonamik2.md` app. D]. It writes **102 of the 274 bones** (71 skinned): all 43
  `*_spo` correctives, hair roots' scale, skirt roots, sleeve chains, hood, shoes, cuffs, `jip`.
- Operator kinds in Sora's asset: `SourceRotate` (25; measures an animated bone against a base bone →
  angle / bend / roll scalars and quaternions), `ConnectEZParamLink` (110; curve) and
  `ConnectEZParamLinkLinear` (38; scale/offset/clamp) mapping one scalar to another, `TargetTranslate` (16),
  `TargetScale` (29), `TargetBendSTRoll` (26), `TargetBendRoll` (20), `TargetRotate` (2) building a helper
  bone's component-space value, `ComputeSpaceBases` (47) recomputing a child from its animated local
  transform.
- Inputs are only real animated bones (`koshi, hara1, mune, kubi, atama`, thighs, shins, feet, shoulders,
  arms, hands). **The `_spo` tracks in the clips are baked copies of the driver's own output** (the
  runtime overwrites them); the puzzling `L_momo_spo` scale 0.01 is the driver's value.
- Verification without the game, kept alive in `tools/sora_pack.js` (run at every build and `--check`;
  results in `validation.json` → `kine_driver`):
  1. on the reference pose the driver reproduces the reference transform of 99 of the 102 written bones
     within 1.1e-5 cm / 2.6e-5° / 3.1e-5 scale; the other three (`R_susoB1…3`) differ by exactly 1.000°
     because the asset's reference pose of `R_susoB1` is not the mirror of the left one (asset fact);
  2. at every key time of all 29 packed body clips the driver's output equals the baked tracks of the
     22–23 tracked helper bones within 8e-3 cm / 1.5e-2° / 4.1e-4 scale (0.11° on `0074`, whose channels
     are key-reduced to different key times).
  Covered by this ground truth: space conversions, single-source `SourceRotate`, both Connect kinds,
  `TargetTranslate`, `TargetScale`, `TargetBendSTRoll`, `TargetBendRoll`. Code reading only:
  `TargetRotate` (2 operators), `ComputeSpaceBases`, the two-source `SourceRotate` (head/neck) [C].
- Between keys the baked tracks interpolate linearly while the driver is non-linear (differences up to
  ~1 cm / 2°): the runtime value is the driver's, so the driver must be run; the baked tracks alone are not
  1:1.
- For bit-level agreement use float32 and UE's polynomial `Atan2` / `SinCos` (in the evaluator).

## 14. Bonamik (hair, hood, sleeves, coat, necklace) and the physics asset

Notes: `native_H_bonamik2.md` (second pass: the complete solver, master text §0–15 and appendices A–G with the
per-function transcriptions; it corrects the first pass, list in its §1) and `native_G_bonamik.md` (first
pass: data model, objects, notify classes). Data: `data/bonamik.json` (asset data), `data/bonamik2.json`
(the runtime records after setup). Reference evaluator: `tools/lib/bonamik/` (`bnm_eval.js`; `notify.js` for
the notify effects; `validate.js`).

**What it is.** A particle / chain simulation run by the KBD component in `TG_PostPhysics`, in place on the
component-space pose. 252 particles: 57 dynamic (the simulated nodes), 195 kinematic (anchors and colliders
riding bones); 164 links, 49 cones, 18 anchor constraints, 37 chain roots (the asset's 49 is an array
reservation), 57 dynamic-side collision objects, 24 self-collision pairs, one KDI record [C setup
0x143B07550; the counts rebuilt from the transcribed setup equal the asset's own totals]. It may write 86
bones (`manifest.runtime.bonamik.driven_bones`); the other 188 are bit-identical to the incoming pose.

**Order** [I, `native_H` app. D]: every frame the KBD component runs the KineDriver sub-task first (real
dt), then Bonamik; both after the anim graph. Bonamik modifies the mesh's current read buffer of
component-space transforms.

**30 Hz evaluation and interpolation** [I 0x143B5B990, 0x143B590D0]:

- A timer of 1/30 s decides the frames on which the solver is evaluated (overshoot carried). The first two
  ticks both evaluate. At frame rates ≥ 30 fps every evaluation uses the same solver step: frame-rate
  bucket 30 → step 1/30, `dtSim = step × 1.5 = 0.05 s` [I 0x143B0EFC0]. Below 30 fps every frame evaluates
  and after 15 evaluations the bucket (20 / 30 / 40 / 50 / 60 table) changes the step.
- Per driven bone two buffers hold the **delta of the simulated local transform against the animated local
  transform** (prev, cur). A frame shows the animated local transform composed with the delta interpolated
  from prev to cur with the eased alpha `a(a + 1)/2`, `a = 1 − remaining/interval`. An evaluation frame
  shows prev (a = 0): **the displayed pose lags the simulation by one evaluation**. At 60 fps the pattern
  is E(1) E(0) –(0.375) E(0) –(0.375) …
- Group and bone weight ramps also run every frame with the raw dt [I].

**Per evaluation** (`CoreUpdate` 0x143B0EFC0: glue [C], time step and reset parts [I]):

1. Time step; global weight; group weights and bone weights [C 0x143B10010, 0x143B11700].
2. Frame transforms, teleport check, reset [I, app. E]. A world jump ≥ 800 cm (`TeleportDistance`) of the
   root bone's delta position is a **soft** reset: the nodes are re-attached rigidly to their chain roots
   with zero local force; only a hard reset snaps the nodes to the bones with zero velocity.
3. PreStep and LocalForce [I, app. E 7]: kinematic particles follow their bones; the 27 collider particles
   on simulated joints follow the particle that drives the joint (Attach) [I, app. C 4]; cone records are
   refreshed from the animated bones where `m_EnableConeAnimation` is set; the local force from the
   component's motion is computed.
4. Step 0x143B0CB60 (call order and arguments [I], glue [C]); elements all [I]:
   - Integrate 0x143B123B0 with gravity **(0, 0, −981) cm/s²** (asset vector with Y/Z swapped × 100
     [C 0x143B61CF0]) and Aero 0x143B13000 (two variants; variant A carries the random jitter);
   - anchors, sphere snap, lateral links, velocity update (app. G 8–11);
   - ChainLink 0x143B192A0 (hard length on 20 ParentChild links: hair, hood, `jip`) and **Cone**
     0x143B166D0 (app. A; bit-checked against the exe's instructions by emulation): circular limit when the
     two limit angles are equal (35 cones), otherwise two independent plane clamps (14 cones) — not an
     ellipse — plus the inner spring towards the cone's target frame;
   - collisions (app. C): particle vs sphere / capsule / plane colliders of the groups listed in
     `m_ColId`, capsule objects on receiver links, the 24 self-collision pairs, ground clamp when the owner
     reports a floor;
   - the KDI link (app. F): one record that ties `jip1` to a simulated coat node.
5. Write-back (app. B): bone translation = node position; bone rotation = the shortest arc turning local +X
   to the direction of the child node — **no twist term and no smoothing** on Sora's path; a dynamic tip
   copies its parent's rotation; then JointTwist 0x143AF3F70; then the blend with the animation by weight.

**Start state** (app. E 5 [I]; search for callers by the author [C, A]): after setup the nodes sit at the
mesh **reference pose** with a soft reset pending and no pre-roll. The native callers of a hard reset +
pre-roll are the anim notify `Bonamik_ResetAndPreRoll` (0x140548A20; in none of Sora's stage-1 clips),
Matinee tracks (`TresInterpTrack*` 0x14117EB30, 0x141177A90, 0x14117C9D0, 0x14117ADC0), the photo hologram
actor (0x140F2CFA0) and the blueprint library function `TresBonamikResetAndPreRoll` (0x14222E2E0, every KBD
component of the world); no player pawn or state function calls them, and among the decoded packages under
`Content/Blueprints` only a boss state event and an enemy behaviour use the library function. Level scripts
(`Content/Levels`, `Content/Maps`) were **not** searched [?]. In the evaluator a soft start and a hard reset
end in the same configuration (3.9e-4 cm apart); they differ only during the first `70 evaluations.

**Notify states** [A, C]. Most Bonamik notifies of the stage-1 clips name groups that do not exist in
Sora's asset and change nothing. Only **`0033` (fall loop)** has an effect (`manifest.runtime.bonamik.clip_notifies`):

- `TresAnimNotifyState_Bonamik_CharaWind` [I begin 0x140562220; C 0x143B05F10]: direction (−0.3, −0.2, 0.8)
  normalised, model space (rotated by the core's root-bone delta rotation), base strength 600, wave
  amplitude 4, frequency 1/0.03; scale 1 on all groups at begin, then the value of the clip's float curve
  `Wind` (constant 2) from the second tick; cleared at end. It enters Integrate as
  `dir × (sin((2π·simTime + i·0.25)·freq)·amp + base) × scale` through the Aero term.
- `TresAnimNotifyState_Bonamik_ChangeGroupParams` [C 0x143B064B0 set / 0x143B018B0 restore, both
  no-verdict]: for every joint of the named groups, immediately and absolutely: `WindLift` 6 on SLEEVE and
  JCT, 2 on HAIR (joint +0x270); `InnerConeStr` 0.12 on SLEEVE, HOOD, JCT — **written to the cone record
  the joint owns** (`joint+0x2A8 → cone+0x28`; a joint without a cone is skipped). At the end the same rows
  are restored from default tables (per joint `core+0x37B0`, per cone `core+0x37A0`, indexed by the cone's
  own index), not from the values before the begin.
  Open [?]: who fills the two default tables (taken to be the values after setup — the asset defaults);
  the other branch of the set/restore functions (`inst+0x33E8 ≠ 0`, variable-fps data layout; 0 after setup
  for Sora). `tools/lib/bonamik/notify.js` implements this as determined; with it the fall-loop pose moves
  the driven bones by up to 12.7 cm after 50 evaluations in the evaluator.

**Random stream** [I, app. G 3]: a thread-local xorshift128 shared with about 900 other game functions,
consumed on Sora's path only by Integrate (3 draws per dynamic particle per sub-step) and effective only
in Aero variant A when the relative air flow is not zero. The Bonamik update runs on task-graph worker
threads, so the stream position is scheduler dependent: it cannot be reproduced from outside. At rest the
simulation is independent of it (verified). A port must replay logged draws or zero the jitter.

**World wind and ground** [?]: the per-map wind manager (0x143B57640 and its callees) is decompiled-C only
and needs map data; the owner's ground query (which object answers it, whether the movement component's
floor is used) was not settled. Both are inputs the host must supply or leave at "none".

**Verification without the game** (`tools/lib/bonamik/validate.js`; a subset runs in `tools/sora_pack.js`
at every build and `--check`, results in `validation.json` → `bonamik`):

| Check | Result |
|---|---|
| Rest stability, reference pose | node motion < 0.001 cm per evaluation from evaluation **35**; settled nodes at most **2.97** cm (mean 0.50) from the animated position (sleeve tips sagging under gravity against the arm colliders) |
| Rest stability, IdleN frame 0 (soft start) | from evaluation **70**; at most **2.72** cm (mean 0.51) |
| Driven set | 86 weighted bones; **188 non-driven bones bit-identical** to the input on every frame |
| Random stream at rest | zero-jitter and seeded runs give identical states |
| Determinism | two runs of the IdleN clip: bit-identical states |
| Frame rate | 45 / 60 / 120 / 144 fps give bit-identical evaluation states to 30 fps (same pose per evaluation index); 20 fps differs from evaluation 15, as specified |
| Setup consistency | record counts equal the asset's totals; with gravity and collisions off the reference pose is an exact fixed point for hair, hood, sleeves, necklace |
| Cone | solve and setup bit-checked against the exe's instructions (`build/sora_tmp/bnm/cone_emu/`) |

Faithfulness of the evaluator (`native_H` §11): instruction-level, float32 in the exe's operation order, for
every solver element named above; decompiled-C level for the `CoreUpdate` / `Step` glue, group and bone
weights, particle construction, the initial orientation block, the chain-root loop and the collision list
builders; approximations: `rsqrtss` / `rcpps` and CRT float functions in the last bit, a single fixed-seed
random stream, a stub owner (no world wind, optional ground plane, time dilation 1). Code paths that
Sora's data never takes throw "not transcribed". **Nothing was compared with a runtime capture.**

**What 1:1 can mean**: given the same sequence of (animated pose, component world transform, dt, notify
events, random draws) and the same start state, the same node positions up to float rounding.

**Physics asset** `p_ex001_0_Physics` (11 bodies, 9 constraints): moves no bone for the player in field
play — no rigid-body node in the graph, nothing enables simulation, Bonamik takes its colliders from its
own asset, body collision uses blueprint primitives [A, C]. (No whole-executable proof that nothing ever
ragdolls the player.)

## 15. Runtime observation requests

What only a run of the real game can settle, and exactly what to log. Offsets are hexadecimal, from the
start of the named object; types are float unless noted. `P` = the player pawn (`TresPlayerPawnSora`),
`M` = its mesh component (`P+0x580`, `TresSkeletalMeshComponent`), `A` = the anim instance (`M+0xB48`),
`X` = its proxy (`A+0x368`), `C` = the movement component, `K` = the KBD component (`M+0x1108`).

| # | Question | Log |
|---|---|---|
| R1 | Frame order: state queue vs movement vs mesh vs notify dispatch (decides the frame of launch, of a play landing in the pose, of notify effects) | a frame counter and timestamps at entry of 0x141515790 (state queue tick), 0x140598190 (movement tick), 0x14406FE00 (mesh tick), 0x143F2EC40 (`UpdateAnimation`), 0x143F2DC50 (notify trigger), 0x14056ECE0 (play by name), 0x143F4D960 (set blend-space parameter) |
| R2 | The saved-pose recurrence (section 6) | `X+0x4D8` (`MySavedPose` data pointer, 274 × 48 bytes: quat, translation, scale each padded to 16) for bones `center`, `L_momo`, `kubi`, and `M`'s final local transforms of the same bones, every frame across idle → walk (blend-in 0.2 s); `X+0x528` (flags, int32) and `X+0x530` (initial-pose sequence pointer) right after spawn |
| R3 | Montage weights and positions | for each instance in `A+0x98` (array pointer, count at `A+0xA0`): `+0x110` position, `+0x114` play rate, `+0x138` weight, `+0x140` desired weight, `+0xA0` playing (byte), `+0x17C` notify offset, `+0x184` flags (int32), `+0x198` / `+0x1A4` blend-space parameter current / target, `+0x1E8` sample cache, `+0x1F8` filter X (`+0` last output, `+0x28` clock, entries at `*(+8)`); the `FullBody` slot node `+0x50` source weight, `+0x54` slot weight, `+0x58` total — once per frame after `UpdateAnimation`, with dt |
| R4 | Leg IK alpha timeline | `A+0x50C` `m_LegIKAlpha`, `A+0x510` `m_LegIKOffset`, `A+0x578` bit 0 `IKinemaSW`; `P+0x710` / `+0x714` / `+0x718` leg interpolator current / target / remaining, `P+0x6FC` bit 0, `P+0x700`; the float returned by the game instance's vt+0x428 — through idle → run → stop → jump → land |
| R5 | Leg IK solve | Multiped node `N` (node #31 inside `A`): `N+0x58` actual alpha, `N+0xD0` dt, `N+0x130` / `+0x134` hips controller value / velocity, `N+0x110` hips delta; per leg `(N+0x178)[i]` smoothed delta (3 floats), `(N+0x198)[i]` cache counter (int32), `(N+0x158)[i]` cached hit (time `+4`, impact point `+0x18`, impact normal `+0x30`), `(N+0x1A8)[i]` foot quaternion; arguments and result of the sweep 0x14063ACB0; component-space transforms of `center`, `koshi`, `L/R_momo`, `L/R_sune`, `L/R_ashi1`, `L/R_ashi2` before the node (cached pose) and after the graph; actor Z, `C+0x4FC` floor distance, `C+0x520` floor impact Z. Scenarios: idle on flat ground ≥ 2 s, walk, idle → run → stop, jump and land, idle on ~15° and ~30° slopes, across a step |
| R6 | Sole height statement (section 10) | world Z of `L_ashi2` / `R_ashi2` (or the lowest sole vertex) and floor Z while standing |
| R7 | Root handling | while RunStartN plays: local `kh_sk` after the slot node (expect the mesh reference root), per-frame extracted delta at `C+0x940` (transform), `C+0x944` blend weight, `C+0xA24` root velocity; while RunLoopN plays: local `kh_sk` rotation (expect the track key (−0.7070833, 0, 0, 0.7071302)); `P+0x630` bit 0 (`m_bRootTransCalcRootSpace`) |
| R8 | Pose extraction assumptions | `M` vt+0xAB0 result (ref-pose override: null?); `X+0x3E0…0x3E2` (bytes), `X+0x230` (int32, expect 3); sequence time passed to 0x143F41360 around a loop wrap of IdleN |
| R9 | Look-at and face | `A+0x520` `LookAtRate`, `A+0x548` face blend rate, `A+0x54C…0x55C` blend weights, `P+0x788`; controller `+0x954` / `+0x994` look-at targets near an NPC; the `Face` / `Mouth` slot weights and `A+0x488` (last played per slot) during idle, run, a jump |
| R10 | KineDriver | at 0x143ABFAD0 (`rcx` = operator set, `rdx` = pose array, `r8` = local array): calls per frame and caller; dump the 48-byte transforms of bones 4, 12, 13, 14, 175, 176, 52, 53, 242, 262, 10 (outputs) and 2, 5, 6, 146, 147, 37, 40 (inputs) at entry and return — feed the entry dump to `tools/lib/kd_eval.js` and compare; `*(uint8*)(rcx+0x30)` (expect 0), `*(int32*)(rcx+8)` (expect 313) |
| R11 | Bonamik | `K+0x42C` dt, `K+0x498` (byte), `K+0x4A0` eval dt, `K+0x4A4` remaining, `K+0x4A8` interval, `K+0x4AC`; global int32 at 0x14874E21C; `core = *(K+0x468)[0] + 0x10`: `+0x3824` bucket (int32), `+0x382C`, `+0x3830` step, `+0x3708`, `+0x3714` sim time, `+0x36F8` weight, `+0x33F4` reset (byte); particles `*(*(core+0x3778))`, stride 0x330: `+0x00` position, `+0x0C` previous position, `+0x20` quaternion, `+0x40` velocity for the 57 dynamic particles; chain-root count `scene+0x750` (expect 37 or 49); component-space transforms of the 86 driven bones at entry of 0x143B62BC0 and after 0x143B56CB0; `M+0x9B0` / `+0x9B4` buffer indices; values returned by the random function 0x142CFF460 inside 0x143B123B0; call counts per frame of 0x143B0CB60 (step), 0x143B01210 (reset; `dl` = hard), 0x143B06EF0 (pre-roll), 0x140E6FBE0 on the first ticks after spawn; `core+0x33F4…0x33F6`, `+0x33EA`, `+0x33EC`; the prev/cur buffer `*(K+0x4B0)` (0x60 per bone); the return of 0x143B57640 (world wind) and of the owner's ground query in 0x143B0CB60; cone table `*(scene+0x30)` (stride 0xA0, `+0x28` / `+0x2C`) of a sleeve cone before, during and after clip `0033`; the TLS random state at entry of 0x143B123B0 with the thread id. Full list: `native_H_bonamik2.md` §15 |

Each notes file ends with its own longer probe list (`native_C_graph.md` §13, `native_D_legik.md` §11,
`native_E_blend.md` §11, `native_E_part4_seq.md` §10, `native_F_kinedriver.md` §12, `native_G_bonamik.md` §8).
