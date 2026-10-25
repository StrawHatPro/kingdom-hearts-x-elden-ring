# Sora animation: what the app does versus what the game does

For the implementer of `crates/kh-core` and `crates/kh-app`. Read against the code as it stood when this was
written: `crates/kh-core/src/anim.rs` (`AnimPlayer`, `AnimRequest`), `crates/kh-core/src/sora.rs` (step
order), `crates/kh-core/src/states.rs` (play calls, notify handlers), `crates/kh-app/src/pack.rs`
(`animate`, `Clip::apply`, `mix`), `crates/kh-app/src/player.rs` (fixed step + interpolation).

"Spec" = `docs/sora-animation-spec.md`; "notes" = files under `build/sora_tmp/`. Items are ordered by how
much they change what is seen. Data for every item is in `build/sora_pack/manifest.json` (pack tool 1.3.0)
unless a file is named.

## A. Changes to existing behaviour

| # | The app now | The game | Where specified |
|---|---|---|---|
| A1 | **Blend-in from a snapshot** taken at the play call: `shown = mix(snapshot, target, since_play / blend_in)` (`pack.rs` `animate`, `pack.snapshot`) | **Recursive saved pose**: each evaluation `S[n] = blend(montage pose, S[n−1], w_n)` with `w_n` the montage weight of that frame; `S` is kept in local space, per bone; when nothing plays on the slot `S[n] = S[n−1]`. The old pose's share after k frames is `Π(1 − w_i)`. `S[0]` = IdleN at t = 0 | spec §6, §8; notes `native_C_graph.md` §3.4; `manifest.runtime.saved_pose` |
| A2 | Blend weight `since_play / blend_in`, brought to the drawn instant between two fixed steps | `w_n = min(1, (dt_1 + … + dt_n) / blendIn)` evaluated **once per game update**; the first update after the play call already has weight `dt_1 / blendIn`. Because A1 is recursive, the result depends on the update rate — it cannot be sampled at arbitrary instants | spec §8; `native_E_blend.md` §1.2 |
| A3 | `time` starts advancing in the step of the play call (`advance_clock` adds `dt` right away; `since_play == 0` only marks the first advance for notifies) | On the **first update after the play call the position stays 0**; it advances by `dt × rate` from the second update. (The weight does advance on the first update.) | spec §8; `native_E_blend.md` §1.3 |
| A4 | Rotation between keys and between poses: `Quat::slerp` (`Clip::apply`, `mix`) | Keys: **normalised linear blend along the shorter arc**. Pose blends (slot, blend-space samples): accumulate `R = q0·w0; R += ±qi·wi` (sign from `dot(R, qi·wi)`), normalise — montage/first sample first, source last | spec §7, §8; `docs/formats/animsequence.md`; `native_E_blend.md` §2 |
| A5 | Scale between keys and between poses: linear `lerp` | Keys: geometric per component (`a·(b/a)^t` when `a·b > 0`, else linear). Pose blends: `exp(Σ w_i · ln s_i)`. Matters for the helper bones with animated scale | spec §7, §8 |
| A6 | Blend-space lean smoothing: time-weighted mean of the inputs of the last 0.4 s (`axis_history`) | **Triangular FIR with one unweighted entry per update** (`FFIRFilterTimeBased`, Linear, window 0.4 s): entry stamp = clock before this update + window; weight `1 − |clock − stamp| / window`; peak 0.4 s in the past, support 0.8 s; reset at play to one entry holding the play-time parameter (0). RunStartN: no filter | spec §8; `native_E_blend.md` §3.2; `manifest.logical_animations[].input_filter` |
| A7 | Blend-space parameter: `set_axis` takes the first value after a play unsmoothed (`axis_history.is_empty()`) | The play resets the filter with the play-time parameter **0**; a value set afterwards enters as a normal entry (weight `dt/window` on its first update). For RunStartN (no filter) the value set right after the play is used from the first update | `native_E_blend.md` §3.1–3.2 |
| A8 | One-shot end: `time` clamps at the length, `ended = true`, the last frame keeps being drawn (the request stays on the clip) | On the update that reaches the end the instance is stopped (blend-out 0); that frame shows the last frame; `IsAnimEnd` is true from that update; from the next update the slot shows the **saved pose** (= the last blended pose, bit-exact). Same picture, but it matters with A1: after the end nothing blends any more, and a blend-in that had not reached weight 1 stops where it was | spec §8; `native_E_blend.md` §1.3 |
| A9 | Notifies: a notify at `at` fires when `at > before && at <= before + dt`, or on the first advance when `at <= before`; only `jump_start` and `enable_change`, only for one-shots | Window passed to the stock rule `(Prev, Cur]`: first update `(0, 1e-4]`; later `(prev + 1e-4, cur + 1e-4]`; after the instance's first loop wrap `(prev, cur]`. Notifies at t = 0 carry a +1e-4 trigger offset and fire on the first update. `TresJumpStart` (0.0666445) fires on update 5 at 60 Hz (update 1 = the t = 0 frame); `TresEnableChange` (0.099) on update 7 | spec §9; `native_E_blend.md` §5; `items[].notifies` (full precision, with `trigger_time`) |
| A10 | `reverse_leg()` tests `time` against the window of the blend space's **first** sample | Notify states are tracked by Begin/End events from the per-update windows of A9: Begin on the first update whose shifted window overlaps the state, End on the first update where it is no longer listed. With `SQEX_AllAnimation` **every** sample's notifies fire (all three run samples have the same `TresReverseLeg` window 0.100371–0.333671, so the result is the same; implement it event-driven so the one-update lag at the end is reproduced) | spec §9; `native_E_blend.md` §5.3–5.4 |
| A11 | Loop wrap: `time %= length`; pose sampled at the wrapped time (no last→first interpolation: already right) | Same, with the remainder carried exactly. Keep it: **never interpolate last key → first key** | spec §7; `native_E_part4_seq.md` §0 |
| A12 | Root during RunStart: root bone held at its **first key** (`hold_root` → `t = 0`) | Root bone **locked to the mesh reference root transform** (rotation (−0.70710671, 0, 0, 0.70710683), scale (1, 0.99999994, 0.99999994) in UE; `manifest.skeleton.bones[0]`), for every sample with `bEnableRootMotion`, whenever the play mode lacks bit 0x10 — also in the pose of Turn / RunSway clips. In all other clips the root track value is used as is | spec §7; `native_E_part4_seq.md` §3 |
| A13 | Root-motion delta: per sample, displacement turned back by the yaw reached at `from`; samples blended by weight; `sample_indices` gives two samples | Same idea (weighted blend of all cached samples' deltas, weights clamped). The rotation delta is blended as quaternions with sign correction, not as yaw numbers (equal for pure yaw). The delta is taken over the position interval of that update (zero on the first update) and consumed by the movement tick of the **same** frame as `worldDelta / dt` | spec §5, §8; `native_E_blend.md` §3.6, §7 |
| A14 | Fixed 60 Hz steps for everything; the drawn pose is sampled between two steps (`sim.alpha`) | The game ticks animation once per rendered frame with the real dt; no sub-stepping, no clamp. With A1, A2, A6 and the leg-IK controller being per-update recurrences, **the pose must be computed once per simulation step and the two latest poses interpolated for drawing** (or drawn un-interpolated), not re-evaluated at the drawn instant | spec §5; `native_E_blend.md` §6 |
| A15 | Step order: controller → state queue → movement (root motion advanced there) → animation advance + notifies | Pose tick (weights, positions, notify dispatch, graph update) runs **inside the movement tick, before the move**, every frame — not only for root-motion clips. A notify's effect reaches the states on the next frame's state tick if the queue ticks before the movement component (working assumption; runtime request R1) | spec §5 |
| A16 | Bones without a track: the pack's reference pose (right) | Mesh reference pose — identical values. No change | spec §7 |

## B. Missing parts (new work)

| # | Part | What to build | Data | Where specified |
|---|---|---|---|---|
| B1 | **Leg IK** | `TresAnimNode_TwoBoneIKMultiped` after the saved pose: alpha interpolator driven by the `TresIK` notify states (linear to target over the given time, merge rule for equal targets, cap 0.99; 0 in run / air); per foot a 10 cm sphere sweep from 30 / 33 cm above to 120 cm below the ankle on the IK collision channel, hit cache (reuse once if moved < 2 cm / 10°, keep-highest rule), delta smoothing (13/s), hips drop through the damped controller (gain 140, damping 23.6643, dt ≤ 1/30, stored value × alpha), Maya two-bone solve with pole (−50, 50, 0) in shin space and the 160° knee limit, foot tilt to the hit normal (roll ±15°, pitch ±10°, yaw ±25°, slerp `13·dt·0.5`), blend of `center`, thighs, shins, feet by alpha. Needs a host sphere-sweep query that returns time, impact point and normal | `data/legik.json`; `manifest.runtime.leg_ik.windows_by_clip` | spec §10; `native_D_legik.md` §3–4 |
| B2 | **Face** | Two more slots on the same saved pose: `Face` owns the `G_eye` subtree, `Mouth` the `G_mouth` subtree (`manifest.runtime.saved_pose.face_subtrees`). Face clips are started by the body clips' `TresFaceAnim*` notifies (type → row table, `_EL` / `_LIP` lookup, SameCheck, blend-in −1 = the clip's `m_InterpolationTime`), all looping, each a montage with the same blend rules. Minimum: hold frame 0 of `0000EM` on both subtrees | `face/*.glb`; `manifest.runtime.face` | spec §12; `native_C_graph.md` §8 |
| B3 | **KineDriver** | After leg IK, on component-space transforms: run the 313 operators in order (stateless). Port `tools/lib/kd_eval.js` (158 lines; UE units and axes — convert at the boundary or port the formulas). Then recompute local transforms for skinning. The tracked `_spo` bones are overwritten by it. Verify with the tool's two checks (reference pose; baked tracks at key times) | `data/kinedriver.json` | spec §13; `native_F_kinedriver.md` §4, §7 |
| B4 | **Bonamik** | Port the reference evaluator; recipe in section B4 below | `data/bonamik.json`, `data/bonamik2.json` | spec §14; `native_H_bonamik2.md` |
| B5 | Look-at | Nothing: exact pass-through for the player without a look-at target | — | spec §11 |
| B6 | Physics asset | Nothing: it moves no bone in field play | `data/physics_asset.json` | spec §14 |

### B4. Bonamik: recipe

**Port `tools/lib/bonamik/*` to the `kh-anim` crate**, module for module (`m.js` float32 helpers, `core.js`,
`cone.js`, `chain.js`, `coll.js`, `kdi.js`, `reset.js`, `wind.js`, `hfps.js`, `bnm_eval.js` as the driver,
`notify.js`). The JS is a transcription of the exe in float32 with the exe's operation order: keep `f32`
arithmetic and the order of operations; do not "simplify" expressions. Functions are named by their exe
address (`fn_143b166d0` …) so that every piece can be traced to `native_H_bonamik2.md`. Branches that
throw "not transcribed" are dead for Sora's data: port them as `unreachable!` with the same message.

1. **Inputs per frame**: the component-space pose **after leg IK and after KineDriver** (UE axes and cm —
   convert at the boundary, the solver's constants are in those units), the mesh component's world
   transform, the frame dt. **Output**: the component-space pose with the 86 driven bones rewritten
   (`manifest.runtime.bonamik.driven_bones`).
2. **Setup** once, on the mesh reference pose, from `data/bonamik.json`. Check the built records against
   `data/bonamik2.json` → `runtime2` (particles, 164 links with rest lengths and the hard-length flag, 49
   cones with their four quaternions, 18 anchors, 37 chain roots, 57 collision objects in sorted order, 24
   self pairs, the KDI record): this is the first test to write.
3. **Frame driver** (`hfps.js`): the 1/30 s timer with overshoot carry; evaluation on timer expiry (first
   two ticks both evaluate); solver step `dtSim = 0.05 s` at ≥ 30 fps; prev / cur delta buffers against the
   animated local transform; displayed pose = animated local ∘ delta interpolated with `a(a + 1)/2`; an
   evaluation frame shows prev. The simulation is driven by the game step dt (60 Hz in the app): do not
   run it at the render rate, and do not interpolate its output a second time between fixed steps without
   knowing that this adds to the built-in one-evaluation lag (prefer drawing the latest step's pose for
   these bones, or interpolate the final local transforms like every other bone — decide once and note it).
4. **Start state**: reference pose + soft reset pending, no pre-roll (as the code does after setup). Offer
   `hard_reset_at_start` as an option (same end configuration, different first `70 evaluations).
5. **Jitter policy**: zeroed by default (the generator returns the constant that makes the jitter 0, as
   `zeroJitter` does in the evaluator); an option to feed a logged sequence of draws (three per dynamic
   particle per sub-step, in particle order). Do not seed a private stream and call it faithful.
6. **Host inputs through the trait**: world wind vector (default 0), ground query → optional (point,
   normal) (default none), time dilation (default 1). The test level supplies none.
7. **Notifies** (`notify.js`), from the notify begin / end events of the body clip (same event stream as
   the reverse-leg and IK windows): only clip `0033` matters. Begin: `setGroupParam` rows (WindLift on
   SLEEVE / JCT / HAIR joints; InnerConeStr 0.12 on the cone records owned by SLEEVE / HOOD / JCT joints)
   and `setCharaWind` (scale 1); from the second tick `setCharaWindScale(2)` (the clip's `Wind` curve);
   end: `restoreGroupParam` and `clearCharaWind`. Rows naming groups absent from the asset are ignored.
   **Undetermined part**: the restore reads default tables whose filling was not read — the port restores
   the post-setup values (asset defaults), as `notify.js` does; world wind and ground are not modelled.
8. **Tests to reproduce** (the numbers of `tools/lib/bonamik/validate.js` and of `tools/sora_pack.js
   --check`; all with the jitter zeroed, 1/30 s frames unless said):

| Test | Expected |
|---|---|
| reference pose held for 120 evaluations | max node move per evaluation < 0.001 cm from evaluation **35** on; settled nodes at most **2.97** cm (2.974) from the animated position |
| IdleN (`0001`) frame 0 held, soft start | from evaluation **70** on; at most **2.72** cm (2.722) |
| same with hard reset at start | settles from evaluation 40; final configuration within 4e-4 cm of the soft start |
| driven set | 86 weighted bones; the **188 non-driven bones bit-identical** to the input pose on every frame |
| determinism | two runs of the IdleN clip, 240 frames at 60 fps: identical particle states and poses |
| frame rate | with the same animated pose per evaluation index, **45, 60, 120 and 144 fps give bit-identical evaluation states to 30 fps** for 120 evaluations; at 20 fps the states differ from evaluation 15 (bucket change) |
| 60 fps frame pattern | evaluation on frames 0, 1, 3, 5, …; alphas 1, 0, 0.375, 0, 0.375, … |
| random stream at rest | seeded and zero-jitter runs give the same state hash |
| fall-loop notifies | on the `0033` frame-0 pose, 50 evaluations after applying the notify effects the driven bones differ from the un-notified run (12.7 cm in the JS evaluator); after restore + 150 evaluations they agree again within 2e-3 cm |
| cross-check against the JS | dump particle positions per evaluation from `bnm_eval.js` for the IdleN clip and compare: positions should agree to the last bits except where CRT float functions or `rsqrt` differ (`native_H` §11 "approximations") |

9. **Not covered by any of this**: comparison with the real game (runtime requests R11), level-script
   resets at spawn, per-map wind, the ground clamp in practice.

## C. Order of one frame, as the game does it

```
state queue tick (play calls, blend-space parameter)              [order against the next line: runtime request R1]
movement tick:
    pose tick with the frame dt:
        montage weights (linear), positions (first update: stays 0), root-motion delta
        blend-space filter, sample weights
        notifies of this update -> handlers (jump launch, enable-change, reverse-leg, IK windows, face plays)
    consume the root-motion delta -> velocity = worldDelta / dt -> move; apply root-motion rotation
pose evaluation:
    P = saved pose of the previous update
    body = slot blend(FullBody, P); eyes = slot blend(Face, P); mouth = slot blend(Mouth, P)
    saved pose = eyes on the G_eye subtree, mouth on the G_mouth subtree, body elsewhere
    leg IK (alpha from the pawn interpolator) on a copy -> component space
KineDriver on the component-space pose
Bonamik (30 Hz evaluation + interpolation) on the component-space pose
skin
```

## D. What stays uncertain until the runtime probe reports

The list of requests with objects and offsets is spec §15 (R1–R11). The ones that can change an
implementation decision: R1 (frame order), R2 (confirms A1), R3 (confirms A2, A3, A6), R4 / R5 (leg-IK
alpha timeline and solve), R7 (A12, A13), R11 (Bonamik: start state at spawn, random draws, wind, ground — and node states for validating the port).
