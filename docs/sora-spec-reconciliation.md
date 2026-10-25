# Sora stage 1: reconciliation of the animation spec and the movement spec

Compared: `docs/sora-animation-spec.md` (animation, assets, pack) and `docs/sora-movement-spec.md` with
`sheets/port/sora-movement.json` (movement). The movement spec and its sheet were not edited. Each point was
decided from the code or the data named in it.

## A. Where the movement spec needs a correction or a sharper statement

| # | Movement spec says | What the code / data shows | Evidence |
|---|---|---|---|
| A1 | Sections 3.6 and 4.7, table rows "Run stop", "Walk stop", "Landing", "Idle": played with **root motion**; open question Q3 asks for the root-motion displacement of `RunEnd`, `WalkEnd` and the landings | These clips are played with the root-motion *mode bit* (mode 2), but **no root motion is extracted and there is none to extract**. Extraction needs the mode bit *and* the sequence's `bEnableRootMotion`; that property is false (class default, not serialized) on `0001`, `0004`, `0005`, `0006`, `0011`, `0012`, `0034`, `0120`, and in each of them the root bone `kh_sk` has exactly 1 translation key and 1 rotation key (constant). Their displacement is 0 cm and 0°. For these states "mode 2" only means "not looped". | gate: 0x143F33C00 (not-compiled; instance flag bit 1 and montage virtual +0x268), 0x143F44DB0 / 0x143F292D0 (same), 0x143F44E10 (sequence `bEnableRootMotion`), default false in the constructor 0x143F31D90 (verified). Data: `*.anim.json` properties; `build/sora_pack/manifest.json` → `root_motion.clips_without_root_motion` |
| A2 | Q3: root-motion displacement of `RunStartN` unknown | Known. All five samples are 8 frames / 0.2333 s and have `bEnableRootMotion` = true. Totals in actor space (forward, right; yaw + = right): `0007` (180, 0) 0°; `0007L` (0, −180) −90°; `0007R` (0, 180) +90°; `0007BL` (−180, 0) −180°; `0007BR` (−180, 0) +180°. Not uniform in time: `0007` is at 11.9, 26, 40.4, 56.6, 75, 104, 180 cm on frames 1–7. The blend parameter is `delta` (set once), and the blend space mixes the two neighbouring samples. | `manifest.json` → `root_motion.tracks` (per frame, UE and pack conventions); blend space `RunStartN` samples |
| A3 | Section 4.7: "bit 1 root motion (bit-1 meaning inferred)" | Confirmed, not inferred: the sequence-to-montage builder enables the montage's root-motion translation and rotation when `(mode & 0x12) == 2`, and bit 0 links the section to itself (loop). | 0x14056E920 (differs; the two tests are plain in the decompiled C) |
| A4 | Section 5.1: "whether the mesh feet sit exactly at the origin was not checked"; Q9 | The lowest sole vertex is 0.56 cm **below** the actor origin in the reference pose and 0.55 cm (left) / 0.40 cm (right) below it at frame 0 of `IdleN` (LOD 0 skinned on the CPU, no leg IK). The mesh component's relative location is (0, 0, 0): the constructor that creates `TresCharMesh0` never writes it and the blueprints do not override it. See the note below for what that means against the floor. | `manifest.json` → `feet`; constructor 0x14059F950 (verified) |

Note to A4: origin height above the floor = clearance (1.9–2.4 cm) − 2 cm (capsule bottom offset) =
−0.1…+0.4 cm, so the soles (0.4–0.56 cm below the origin) sit about 0.2–0.65 cm below the floor plane before
leg IK. That is within what the foot IK of the AnimBlueprint (`TresAnimGraphNode_TwoBoneIKMultiped`,
`CollTwoBoneIK` on `L_ashi1` / `R_ashi1`) would absorb; the IK was not analysed. A port without IK can
raise the model by about 0.5 cm, or ignore it.

## B. Where the animation spec was wrong or incomplete (fixed there)

| # | Animation spec said | Correct | Evidence |
|---|---|---|---|
| B1 | After a landing interrupted by stick input, "the queued Run starts in its loop, no `RunStart`" | The movement spec is right: no Run is queued during a jump or landing. "Can run" is true in the Jump state only in sub-step 3 (ledge fall with on-ground or the small-drop flag). On interruption the Jump state finishes, Idle begins, and the next request starts Run through `RunStartN`. | 0x14142CE00 (differs): Jump case tests sub-step == 3 and flag 0x20; 0x141440030 calls it first |
| B2 | "Which `Size` component is radius and which half height was not examined" | Radius = `Size.X` = 60, half height = `Size.Z` = 89, centre = `RelativeLocation` = 91 above the origin, as the movement spec states | 0x1405DF310 (differs): the CAPSULE1 branch reads Size +0 and +8 with a floor of 1.0 and writes radius and `2 × (halfHeight − radius)` |
| B3 | "Whether a jump can start during the landing was not read" | Movement spec section 4.1: accepted in sub-step 11 once `m_bEnableChange` is set (same notify, 0.099 s). Taken over, not re-read. | movement spec: 0x14143D8F0, 0x14142A3D0, 0x14142B070 |
| B4 | Turn call `0.1 × dt × 60`: "lerp fraction or step not determined" | Movement spec: a slerp of the actor yaw with that alpha (`dt × 6` in the air, `dt × 12` in the stop animation). Taken over, not re-read. | movement spec section 0, answer 4 |
| B5 | "How root motion and the movement component's velocity combine is stock-engine assumption" | Movement spec section 3.6: `m_bAddFullVelocityToRootMotion` = 1, `PhysWalking` adds the root-motion velocity to the move velocity; no input is issued during the run-start sub-step. Taken over, not re-read. | movement spec: 0x14142348D |

## C. Same function or value, described the same way

- Sub-step numbering: Run 0 loop / 1 start requested / 2 start playing / 3 stance transition / 4 stop /
  5 water flow; Jump 0 start / 1 start continues after launch / 2 rise loop / 3 fall entry / 4 falling /
  5–9 wall slide, dive / 10–11 landing.
- Walk below stick magnitude 0.8 (0x141440092); run → walk grace timer 0.2 s; turn-around at
  `|delta| ≥ 75°` (0x1413AF8E0) replaying `RunStartN`; stop animation when no request and horizontal
  acceleration² ≤ 1, `L` when `m_bReverseLeg`.
- Launch at the `TresJumpStart` notify, 0.0666 s into `0031` (handler 0x14139AB60); rise loop when the
  0.1 s start clip ends; fall loop at vertical velocity ≤ 0, deferred until the jump-duration phase is
  over; ledge → Jump sub-step 3; small-drop rule (≤ 40 cm, run loop kept up to 1.0 s).
- Landing: velocity is zeroed by the Jump state (pawn "stop movement", vfunc +0x16E8) on the tick the pawn
  is on ground, before the landing clip is chosen; `LightLand` for fall height ≤ 75 cm, `Land` above,
  `RollLand` at ≥ 800 cm with stick; control returns at the `TresEnableChange` notify (0.0991 s in `0034`
  and `0120`), else at the clip end (0.4 s).
- `IdleN` replayed at its end, `IdleBreakN` after more than 2 ends; play rate 1.0 everywhere (speed does
  not scale the animation); blend-space axes and samples.

## D. Still open in both

- Whether the blend from the previous pose uses a pose snapshot refreshed every frame or held from the play
  call, and the blend curve (animation spec, open question 1).
- Tick order of controller, state queue, animation and movement within a frame (movement spec Q1): it
  decides on which frame the launch and the root-motion displacement land.
