# Sora movement: what the Rust port does vs what the game does (revision 2 of the spec)

For the implementer of `crates/kh-core` / `crates/kh-app`. Each item: what the code does now (file,
function, line numbers as of this pass) — what the game does — evidence (addresses in `base/KH3.exe`).
Everything here was read at instruction level (iced disassembly) unless an item says otherwise;
the full statements are in `docs/sora-movement-spec.md` (section numbers given as "spec §").
Items are ordered by how much they change the feel. "Already right" lists close each part so it is
known what was checked.

Legend: **C** = controller update, **M** = movement component tick, **S** = pawn tick → state
`MainProc`. Frame N runs `C(N) → M(N) → S(N)`.

---

## A. Frame order (spec §1.2) — `sora.rs`, `cmc.rs` `tick`, `anim.rs`

| # | Now | Game | Evidence |
|---|---|---|---|
| A1 | `Sora::step` (`sora.rs` 147–171): controller → `main_proc` → root motion + `cmc.tick` → `anim.advance` + notifies | controller → movement pre-part (consume input vector, `CheckJumpInput`, acceleration, analog modifier) → **pose tick: animation clock, root-motion extraction, all notifies** → `PerformMovement` → **state `MainProc` last** (called from the pawn's actor tick; the state queue's own tick is disabled). Split `Cmc::tick` into a pre-part and a perform part; move `main_proc` to the end | prerequisites: 0x144078C80 (controller → movement), 0x14401DA60 + 0x1405B4C10 (movement → pawn actor), 0x1405B4BAB (movement → mesh); `MyDisableAutoMainProc` = true 0x14059FEB4 / 0x141514250 / 0x14150E340; `MainProc` call 0x1405BC51D; pose tick in 0x140595730 because component+0xC18 bit 0x20 is set in the ctor (0x14057185E) and never cleared |
| A2 | the movement tick consumes the input vector written in the same step | it consumes what the state wrote in the **previous** frame. Same one-frame delay for `MaxWalkSpeed`, `MaxAcceleration`, walk mode, `GravityScale`, and for the first clock advance / root motion of a clip the state has just started | follows from A1 |
| A3 | the state sees a mode change (landing, falling) in the next step | **same frame**: `M(N)` lands, `S(N)` sees on-ground, zeroes the velocity and starts the landing clip | follows from A1 |
| A4 | `cmc.rs` 192–194 / 203–204: jump = `pressed_jump && on_ground`, cleared after | the notify (in the pose tick of `M(N)`, **after** that tick's `CheckJumpInput`) sets `bPressedJump`, a second latch (pawn+0x5C0 bit 0) and `JumpKeyHoldTime = 0`. `ClearJumpInput` in the same `PerformMovement` clears `bPressedJump` (max hold time 0). `M(N+1)`: `CheckJumpInput`: `if bPressedJump or latch: wasHeld = holdTime > 0; holdTime += dt; did = DoJump(); if !wasHeld and did: latch = 0`. Model the latch and the hold time; drop the `on_ground()` gate (the only gate is the interface test, constant true) | 0x1405ADF50, 0x1405A5B20, 0x1405A5D80 (called at 0x14058CE88, before `StartNewPhysics`), 0x1405A8030 |
| A5 | launch at 60 Hz in step 4 | press frame 0 → `S(0)` begins Jump and plays the clip → clock first advances in `M(1)` → reaches 0.0667 ≥ 0.0666 in `M(4)` → `DoJump` in `M(5)`: **5 frames**. (Unverified: whether the play call ticks the pose at once — runtime item R3) | spec §4.2 |
| A6 | `anim.rs` `advance` (236): no notifies when `take_root_motion` already advanced; `sora.rs` 158 gates root motion on `on_ground` | one pose tick per frame does clock + root motion + notifies, in every movement mode; whether root motion moves the pawn is decided inside the physics (walking adds it; falling adds it too, spec §4.4) | 0x1405DA2B0 → 0x143F2EC40 → 0x143F2D1B0 (notify mode `OnPostUpdateAnimation`, 0x140559854) |
| A7 | fixed step, any length | the game's dt is the variable frame time, clamped to **0.05 s** (`MaxDeltaTime`), the same value for C, M, animation and S in a frame. Never pass more than 0.05; 1/60 is the reference | 0x144590F00 (read of GameEngine+0xE28 at 0x1445913F3), `DefaultEngine.ini` |

Consequences to expect after A1–A4 (all are the game's behaviour, do not tune away):

- Last stick input in frame N: `S(N)` (no request) still sees non-zero `Acceleration` → no stop, but
  turns; `M(N+1)` has zero input → one braking tick (680 → 464.5 at 60 Hz); `S(N+1)` fires the stop
  (velocity 0, `RunEnd`).
- Jump from a run, press in frame N: `M(N)` still accelerates; `S(N)` ends Run, begins Jump, first
  Jump tick has no request → writes no input; `M(N+1)` brakes once (680 → 464.5); from `S(N+1)` on
  the Jump state steers (3672 cm/s², +61.2 cm/s per 60 Hz tick, cap 680) until `DoJump` in `M(N+5)`.
  **The dip is real** (spec §4.2).
- Apex: `M(N)` ends with `Velocity.Z ≤ 0`, `S(N)` restores `GravityScale` in the same frame, so the
  hang starts in `M(N+1)` with 1.5. The short-hop 3.0 written in `S(k)` first acts in `M(k+1)`.
- Landing: horizontal velocity is kept for the rest of the landing tick (the remainder is walked),
  then zeroed by `S` in the same frame.

Already right: controller before state; `MainProc` begins and ticks a new state in one call;
one clock advance per step; `ClearJumpInput` clearing at once; no controller update while paused.

---

## B. Floor, edges, step-up (spec §3.4, §5) — `cmc.rs`

| # | Now | Game | Evidence |
|---|---|---|---|
| B1 | no perching (`find_floor` 523–530; header line 37) | **perching is active**: after every floor query whose result is a valid blocking sweep hit (not line trace) and whose contact is more than **1.0 cm** (2D) from the capsule axis, run the perch test: `MaxPerch = max(2.4, adj + 47 + 10)` (+0 on ground) = **59.4001 walking / 54.6 falling**; `ComputePerchResult`: from the hit's origin location, capsule radius 1.0 probe (B2) with `SweepDist = MaxPerch`, `LineDist = max(0, MaxPerch − InHitAboveBase)`, `InHitAboveBase = max(0, ImpactPoint.Z − Location.Z)`; success needs walkable and `InHitAboveBase + PerchFloorDist ≤ MaxPerch`. Success: keep the floor (if `2.15 − FloorDist + PerchFloorDist ≥ MaxPerch`: `FloorDist = 2.15`; if the floor was not walkable take the perch hit as a line-trace floor). Failure on a walkable floor: B3 decides; failure otherwise: not walkable | `FindFloor` 0x14057FA80 (0x14057FD5B–0x14057FEC7), `ShouldComputePerchResult` 0x140596260, `GetValidPerchRadius` 0x1405861F0, `ComputePerchResult` 0x14057E0C0 |
| B2 | `compute_floor_dist` has only the body sweep | add the `SweepRadius > 0` branch (reached only from the perch test): `HH = max(0, r)` → `max(HH, 15)`; start = location + (0,0,15); `Shrink = (15 − 1) × 0.1 = 1.4`; capsule (r 1.0, half height 13.6) swept down `Sweep + 1.4`; if start-penetrating or the impact is ≥ 0.85 cm from the axis: second sweep r 0.8499, half height 2.4, `Shrink = 12.6`; `FloorDist = max(−2.4, Time × TraceDist − Shrink)`; walkable if `IsWalkable && FloorDist ≤ Sweep`. Its line trace starts 15 cm above the location, length `Line + 15`, result `Time × (Line + 15) − 15`. Needs a host query "sweep a capsule of given radius / half height" | `ComputeFloorDist` 0x14057D100 (0x14057D335–0x14057D82E, 0x14057DAC4–0x14057DBA9) |
| B3 | — | **slot 260** (after a failed perch on a walkable floor; true = drop the floor): part 1 (player flag set): if the contact is within 25 cm (2D): push point `P = loc.xy + dir × max(24 − d, 0)` (`dir` = contact → pawn), sphere r 25 multi-sweep from `z0 + 37.5` to `z0 − 5.0` (`z0 = loc.z − min(FloorDist, 2.15)`), ignoring the pawn and the floor component; any non-penetrating hit whose 2D offset from `P` has `dot(dir, ·) ≥ 0` keeps the floor. Part 2: if the contact is farther than 57 (walking) / 47 (falling) cm → drop; else a line from the origin down `MaxPerch + min(fd, 2.15) − max(ImpactZ − loc.z, 0)`, up to 4 segments sliding along unwalkable faces (step back 0.5): a walkable hit keeps the floor; otherwise drop | 0x1405887B0 |
| B4 | `phys_walking` 348–362: lost floor → `CheckFall` | before `CheckFall`, if the floor before this sub-step was walkable: **edge float** (slot 409): slide `(0,0,−3.225)` along the current (unwalkable) floor hit normal; if the move ends on a valid landing spot and `FindFloor` there is walkable: adjust height, stay Walking, end `PhysWalking`; else revert. `must_jump = bJustTeleported or zero_delta or old base == null` | 0x14059094D–0x140590961, 0x14057BD10 (`m_bCheckEdgeFloat` = 1: ctor 0x1405718FE) |
| B5 | `step_up` 429–456, 505–508: stock tests with radius 60 / half height 89 | the fork's `StepUp` (slot 216 **0x140597570**, not 0x1405768B0) uses **no capsule size**: reject unless `InHit.ImpactPoint.Z > origin.Z`; when walking on a walkable floor `fd = max(FloorDist − 2.0, 0)`, `up = max(47 − fd, 0)`, `down = 47 + 4.8`, `floorPointZ = origin.Z − fd`; sweeps: up (reject if start-penetrating), forward by the unapplied delta (blocked → `HandleImpact`, `SlideAlongSurface`; reject if no progress), down 51.8; reject if `ImpactPoint.Z − floorPointZ > 47`; if the down hit is not walkable: reject if `Delta · ImpactNormal < 0` or `Hit.Location.Z > origin.Z`; reject if `deltaZ > 0 && !CanStepUp(hit)`; then `FindFloor` at the new place (with the hit), reject if `Hit.Location.Z > oldZ && !floor.blocking && stepSideZ < 0.08`. No top-of-capsule test, no edge-tolerance test on the down sweep. `Hit.Location` is the actor origin at the hit (B9) | 0x140597570 (0x140597612..0x140597AB7), `IsWithinEdgeTolerance` 0x140589BA0 with literal radius 0 at 0x1405976E0 |
| B6 | `slide_along_surface` 858–867 has the stock `normal.z < −1e-4` branch | the fork has only: on ground, `Normal.Z > 0 && !IsWalkable(hit)` → flatten to 2D | 0x140596360 |
| B7 | `two_wall_adjust` 924–932: stock "maintain horizontal velocity" rebuild | after the base adjust, on ground: `if delta.z > 0: if IsWalkable(hit) && ImpactNormal.Z > 1e-4 { if delta.z > 47: delta *= 47 / delta.z } else delta.z = 0`; `elif delta.z < 0 && FloorDist < 1.9 && floor.blocking: delta.z = 0` | 0x140598B60 |
| B8 | `compute_floor_dist` 541: start-penetrating sweep stored as `blocking_hit: true`; 559–563: line result replaces the hit | `SetFromSweep`: `bBlockingHit = valid blocking (not start-penetrating)`, hit kept; `SetFromLineTrace`: keeps the sweep hit's Time, Location, ImpactPoint, TraceStart/End and takes normals / component from the line, `bLineTrace = 1` | 0x143FFD9C0, 0x143FFD880 |
| B9 | — | every move and floor hit has `Location` = **actor origin at the hit** (`TresRootComponent::MoveComponentImpl` rewrites it: start + delta × time; floor sweep: location − (0,0,Sweep × Time)); pull-back on moves is stock (`Time −= clamp(0.1/Dist…)+0.001`), none on the floor sweep | 0x1405D27A0, 0x14057D100 |
| B10 | `safe_move` 838–839, `phys_walking` 343–346 | penetration adjustment = `Normal × ((depth > 0 ? depth : 0.125) + |p.PenetrationPullbackDistance|)` (pullback value not read, stock 0.125 — runtime item R9), no maximum clamp | 0x144016CA0 |
| B11 | `tick` 183–207 | (a) the first physics call after spawn is skipped (`m_bDisableMoveOnSpawnFrame`); (b) root-motion **rotation** is applied to the actor after physics, inside `PerformMovement`; (c) encroach push-out before physics (only if the host can report overlaps; body of the function read as C only) | 0x1414233FB / 0x140596E37; 0x14058CF36..0x14058D03D; 0x14057AE30 |
| B12 | `phys_walking` 317 | move velocity = `(V.x + rm.x, V.y + rm.y, rm.z)` (+ external terms, 0); root-motion velocity = world root translation of this tick / dt / blend weight | 0x1405902EC..0x14059033F, 0x1405969D0 |
| B13 | `calc_velocity` 301 | result is exactly 0 when `NewMax < 1e-4`; normalisations are `rsqrtss` + two Newton steps (only for bit-exactness) | 0x1405795F0 |
| B14 | `within_edge_tolerance` 983: lower bound 0.1501 | `max(R − 0.15, 1e-4)` | 0x140589BA0 |

Answers to the implementer's questions:

- **Which shape each query uses**: movement sweeps and the main floor sweep = the real body
  (capsule r 60, half height 89, centre origin + 91), unshrunk, from the current place down
  49.4001 (walking) / 44.6 (falling); the floor line = from the actor origin (2 cm below the capsule
  bottom) straight down the same distance; perch probe = capsule r 1.0 (B2); edge check = sphere
  r 25 + lines (B3); state ground probe = sphere r 50 (D1).
- **"Rides up ledges of 48–58 cm"**: not from the floor code — a downward sweep beside a higher kerb
  moves the hemisphere away from the edge. With B5 a ledge is climbed iff the step-up lands with
  `ImpactPoint.Z − floorPointZ ≤ 47`. Re-test `steps_up_47_cm_but_not_48` after B1–B7.
- **"Sinks round an edge"**: yes, but governed by the perch test: stepping off a kerb of height
  ≤ 59.4 cm the capsule keeps the edge as floor and rolls down around it (Z follows
  `−(60 − sqrt(3600 − d²))`) without leaving Walking; higher edges: B3, then B4, then Falling about
  1 cm after the axis passes the edge. The port-plan note "a drop of up to 49.4 cm never leaves
  Walking" becomes "up to 59.4 cm, through the perch probe".
- **Sub-stepping**: for dt ≤ 0.1 (30/60/120 fps) exactly one `CalcVelocity(dt)` and one loop
  iteration of the whole dt; velocity never recomputed from displacement (walking or falling).
- **Floor lost in a walking tick**: the walking move is made in full, then Falling with the unused
  time fraction only (0 when unobstructed) — gravity starts in the next tick.

Already right (`cmc.rs`): `calc_velocity` once per tick before the loop and its arithmetic;
`apply_velocity_braking`; no velocity recompute in walking; `simulation_time_step`; zero-delta
test; `move_along_floor` structure; `ground_movement_delta`; fall remaining-time formula;
`adjust_floor_height` (1.9 / 2.4 / 2.15, one move); `find_floor` distances; main floor sweep and
line; `is_walkable`; `gravity_z`, `jump_phase_gravity` (strict `<`, sees the old `Velocity.Z`);
`new_fall_velocity`; base `slide_along_surface`, `compute_slide_vector`; `limit_air_control`;
`process_landed` / `set_mode` (`Velocity.Z = 0`, floor found, height adjusted, remainder walked with
the same iteration counter); `SetJumpDurationFinish(true)` only on a real mode change; `do_jump`.

---

## C. Falling (spec §4.4) — `cmc.rs` `phys_falling`

| # | Now | Game | Evidence |
|---|---|---|---|
| C1 | 717–720, 747–749: `velocity = delta / sub_remaining` after each deflection | **never**: after an air impact `Velocity` stays what `CalcVelocity` + `NewFallVelocity` produced; only the position slides (so a ceiling hit does not zero `Velocity.Z`; it decays under gravity) | one `divss` in 0x14058D0C0 (0x14058D5F4); stock copy 0x143FF5540 has three |
| C2 | 700–708: `ShouldCheckForValidLandingSpot` block | does not exist (0x14058D817 goes straight to `HandleImpact`); delete, `should_check_landing_spot` unused | 0x14058D0C0; slot 267 0x140596200 has no caller |
| C3 | — | fork block after `TwoWallAdjust` and the air-control term: if the new hit is a valid blocking hit, `|Hit.Normal.Z| < 0.001`, `|OldNormal.Z| < 0.001`, the adjusted delta has XY and `Delta.xy · Adjusted.xy < 0`: `Delta = (0,0,PreAdjust.Z)`, `Velocity.X = Velocity.Y = 0`, `Delta = ComputeSlideVector(Delta, 1 − Time, Hit.Normal) + Hit.Normal × 0.01` | 0x14058DC80–0x14058DDB1 |
| C4 | 753–765: `Time == 0` → land | order after the third move: if `Time == 0`: valid landing spot → land(0); else side-step move (`SafeNormal2D(OldNormal + ImpactNormal)`, fallback `(OldN.y, −OldN.x, 0)` when nearly zero); ditch → land(0); valid landing spot → land(0); if `Time` is still 0: unstick sequence (`ResolvePenetration` with the delta, then with its 2D part) and **return without landing** unless a landing spot is found | 0x14058DE7D–0x14058E4D4 |
| C5 | `handle_slope_boosting` 1014–1017: zero | in the "was heading down, would deflect up" case: `Result = (0, 0, ZLimit)` | 0x1405877E0 |
| C6 | `is_valid_landing_spot` 797 | add the last test "flag component+0x308 bit 0 → false" as a constant false parameter (cleared in the ctor; setter slot 175 has no caller in stage-1 code) | 0x140589960 |

Frame-rate facts for the fall (spec §8): one sub-step per tick for dt ≤ 0.1; the phase (rise / hang /
fall) is chosen once per sub-step from `Velocity.Z` at its start; the hang lasts while the f32
accumulated time is strictly below 0.15: **4 steps at 30 Hz, 8 at 60 Hz, 17 at 120 Hz**.

---

## D. States and probe (spec §3.1, §4) — `states.rs`, `world.rs`

| # | Now | Game | Evidence |
|---|---|---|---|
| D1 | `ground_distance` 188–195: line from the capsule centre; `World` has only `sweep_capsule` / `line_trace` | **sphere r 50** swept from actor origin + 55 straight down `range`; blocking WorldStatic / WorldDynamic / TresGimmick; per blocking hit: start-penetrating → `dist = 0`, false; surface angle test `−slope > −45°` → accept: `dist = ActorZ − ImpactPoint.Z` (can be negative), ground Z = ImpactPoint.Z, true; steeper → store that distance, continue; nothing → `dist = range`, false. Add a sphere sweep to `World`; return `{hit, dist, ground_z}` | 0x14063A650 (0x14063A717, 0x14063A98B, 0x14063ABB5, 0x14063AC19) |
| D2 | `near_ground` 198, `can_run` 205 re-trace | Run probe: fresh = `hit && dist < 2.0`; **cached** per tick (flag cleared at Run reset and at each airborne Run tick): a later `CanRun` in the same frame returns `dist < 2.0` without the hit flag | 0x1413A3D20 (0x1413A3DD1, 0x1413A3D36) |
| D3 | `jump_tick` re-traces in sub-step 3; sub-step 10 does nothing (642–644) | Jump probe once at the top of each tick (not on the first tick after a fall-entry Begin: Begin's values are reused); at tick end `ground_z = actor Z`. Sub-step 10: `if z > ground_z && z − ground_z ≤ 75`: swept `SetActorLocation` to `z = ground_z` (lowers the origin by `FloorDist − 2` on flat ground); then **the sub-step-11 code runs in the same tick** | 0x141383880, 0x141395834, 0x1413981DA; 0x141397358..0x141397380; 0x1413971BA → 0x1413973C8 |
| D4 | `controller_update` 161–180: move first, jump second | jump press (or held) first, then `MoveInput`, `TryRun`. Consequence D5 | 0x1413F27A0..0x1413F283F before 0x1413F2863..0x1413F288F |
| D5 | Idle + stick + jump press in one frame → jump | the later `TryRun` does `SetState(RUN)`, which clears the queue holding the Jump: **the jump is lost** | 0x141440251 → 0x141514920 |
| D6 | `queued_run` (94, 246–248, 264, 297): Run queued behind Jump | from Jump sub-step 3 with `CanRun` true (`flag 0x20` or on ground): `TryRun` = `SetState(RUN)`: the Jump ends at the next `MainProc`, Run begins (sub 0, walk 0, latch set, current anim name kept) and ticks; if still airborne and the probe fails it calls `Fall` again (Run ↔ Jump(3) flip-flop per frame). This is the step-down. Remove `queued_run` | 0x1414403C1..0x141440448 |
| D7 | `jump_request` 259: landing jump needs sub 11 && enable_change && on_ground | gate for state JUMP: `sub == 11` (nothing else; next-queued must be empty). A press on the frame the controller still sees sub 10 (the frame after touchdown) is refused and lost | 0x14142A552 |
| D8 | 656–661: turn + input only for `sub ≤ 4` | the `dt × 6` turn toward the target yaw runs in every sub-step including 10 / 11; input only when requested and not in 10 / 11 | 0x141397886; 0x1413963EF / 0x141397492 |
| D9 | 599–610, 612–640: step-down timer and landing test in sub 3 | order in sub 3: no request → clear flags 0x20/0x40; `dist > 40` → clear 0x20; if 0x20 still set: replay the loop if needed, `timer += dt`, `timer > 1.0` → clear (effective next tick), go to the tail (**no landing test while the flag is set**); else play the down loop, sub = 4 and continue into the sub-4 code. The exit "request && flag 0x20 → DoChain" in the landing code is unreachable | 0x141395F1B..0x14139608C, 0x141396084 |
| D10 | `jump_begin` 538–542: on a floor at Begin → sub 10 | sub 10 only when a land pose is set (never in stage 1); otherwise sub stays 3 and the first tick goes 3 → 4 → normal landing test (`LightLand`, fall height 0). Step-down flag also for `RunLoopW/B`, `LightLandR/RB/L/LB`; the 40 cm test reads the stored probe distance whatever the hit flag (0 after a start overlap passes; negative passes) | 0x141386BE9..0x141386D46, 0x141386B39..0x141386BD7 |
| D11 | 478–487: grace timer / walk mode only in sub 0, walk mode from the updated grace | on every **requested** tick in sub 0–3: `SetWalkMode(!isRun)` with `isRun = (grace > 0) or !walk` taken **at tick start**; then grace = 0.2 if the walk flag is 0, else `max(grace − dt, 0)`. Loop / end animation choice uses the same tick-start `isRun` | 0x1413AF495, 0x1413B09A4..0x1413B09E7 |
| D12 | 446–469: when sub 2 → 0 in a tick the sub-0 block also runs | one dispatch per tick on the sub-step at tick start; that tick only the common part runs | 0x1413B0324 |
| D13 | 505–520: no turn without a request | sub 0 / 3 with no request and no stop yet: still `Turn(target, 0.1 × dt × 60)` | 0x1413B0949..0x1413B0999 |
| D14 | 498: raw delta as blend axis | `clamp(delta, lo, hi)` of the playing asset's range; 0 when walking; `RunStart` gets the signed unclamped delta | 0x1413B0A4B..0x1413B0BE3 |
| D15 | 316–320: Idle Begin always plays | `if !OnGround: return` before any play | 0x141386200 |
| D16 | 563–574 | sub 1: animation end (start rise loop, sub 2) is tested before the short-hop test; no observable difference (D17) | 0x141395BE9, 0x141395D0D |

Confirmed readings of the implementer (keep): the take-off dip; the short hop cannot fire in
sub-step 1 (clip length equals the compared constant bit for bit, test strict) — it starts on the
first sub-2 tick with `Velocity.Z > 0` and the button not down, so a tap and a release up to ~0.1 s
give the same hop; fall height summed at the end of the tick only while sub == 4, so the landing
tick's drop is not counted; sprint timer and flag are zeroed by Reset (every new Run, also after a
jump or landing), by the turn-around and by any walk tick, sprint needs `timer > 1.0` (strict).

Already right (`states.rs`): `MainProc` loop and Idle fallback; held latch (down and not just
pressed; cleared at tick end; not set on the press frame); `can_run` whitelist; `try_run`
(`< 0.8`, Idle → sub 1 unless walk, walk honoured only in sub 0); jump yaw sources; `fall`;
`Run::End` / `Jump::End`; launch handler; `turn_toward`; `set_movement_input` assigns; Idle (three
idles then a break; `dt × 9` is a no-op); Run sub 1 / 2 / 4, turn-around `≥ 75`, sprint, stop rule
`≤ 1.0`, `MaxAcceleration = MaxWalkSpeed × 60`; `jump_begin` 680 / 3672; apex sequence; landing
thresholds `≤ 75` / `≥ 800`; sub-11 exits; restriction flags all 0 in plain play (spec §7).

---

## E. Input and host (spec §2) — `kh-app/src/player.rs`, `states.rs` `controller_update`

| # | Now | Game | Evidence |
|---|---|---|---|
| E1 | jump = `GamepadButton::South` | default jump = `Gamepad_FaceButton_Right` = **Xbox B** (East); A is attack / decide. South only as the "type 1" option | 0x140EDD580, 0x140EE0CA0, cvar `i.GamePadDecideType` default 0 |
| E2 | pad stick through Bevy's axis filter | raw `short × 1/32767` (`1/32768` when ≤ 0), no pad-layer dead zone, then the per-axis 0.25 dead zone in the core. Disable any host dead zone / live zone / threshold on the left stick | 0x142DFA100, 0x141523130, 0x1445AB3D0, 0x1445AF120 |
| E3 | no walk key | LeftAlt held: raw ±0.5 → 0.3333 after the dead zone (walk) | 0x1414B6570, 0x14442C190 |
| E4 | pad used when the key sum is zero | any held W/A/S/D drives the axes (opposite keys give 0 and still override the pad); BackSpace also jumps | 0x14442C190, 0x14442B8F0 |
| E5 | camera yaw of the current step | the camera rotation cached by the **previous** frame's camera update (camera manager ticks in a later group); roll is kept: `yaw = atan2(LeftY·sinY + LeftX·cosR·cosY, LeftY·cosY − LeftX·cosR·sinY)` | 0x1405E6B80 → 0x144434C00; 0x1405E2D30 |
| E6 (optional) | — | keyboard movement and keyboard jump reach the controller one frame later than pad input; a key press + release inside one frame gives "just pressed" with "down" false (jump requested, held never set) | `PreProcessInput` at 0x1445B2220 before the key loop |
| E7 (optional) | libm `atan2` | polynomial `FMath::Atan2` (0x142CE9900) and polynomial sin/cos: difference ≈ 1e-4° | — |

Already right: dead-zone formula and absence of a clamp; vector = (LeftY forward, LeftX right);
magnitude unclamped; active iff `≥ 0.1`; walk iff `< 0.8`; keys ±1 per axis, diagonal 1.414;
press = one-frame edge, held = level; Space = keyboard jump; no smoothing.

---

## F. Runtime probe requests (what static reading cannot settle)

Log per frame with the frame counter (`GFrameCounter` 0x149B68D50) unless noted.

| # | Question | Log |
|---|---|---|
| R1 | Frame order and threading as derived | entries (with thread id) of 0x1413F1AB0, 0x140598190, 0x1405AF030, 0x140560120 (JumpStart notify), 0x14058CC60, 0x14058C770, 0x1405BC470, 0x14150A7E0 |
| R2 | The whole stage-1 trace for regression against the port | movement component: Velocity +0x214 (3 f32), Acceleration +0x4A4, MovementMode +0x314, GravityScale +0x2F8, MaxWalkSpeed +0x388, MaxAcceleration +0x39C, AnalogInputModifier +0x4E0, flags +0xA3C, jump timer +0xC00 and +0xBFC, CurrentFloor +0x4F8 (0x98 bytes); root location (root+0x2B0) and yaw; state: id, `m_nSubStep` (+0x78), flags +0x84, Run +0x88..+0x98, +0xB8, +0xBC, Jump +0x88..+0x94, +0xB0, +0xDC, +0xE0, +0x114; pawn +0x5BC, +0x5C0, +0x5C4; dt. Scenarios: run start / stop / turn-around, sprint, jump (tap, hold, release at 0.05 / 0.15 / 0.25 s), run-jump, ledge walk-off, kerbs of 30 / 45 / 50 / 58 / 62 cm up and down, jump into a wall and a ceiling |
| R3 | Does the state's play call tick the pose at once (first advance in the same frame)? | calls of 0x14406FD10 (`TickAnimation`) between entry and exit of 0x14150A7E0; and the frame of 0x140560120 vs the frame of `Jump::Begin` 0x1413868C0 (expect +4 at 60 Hz) and of `DoJump` 0x14057ECB0 (expect +5) |
| R4 | `ImpactNormal` PhysX reports for the capsule on box edges and mesh edges (decides every edge contact) | on return of 0x14057D100 and 0x140589960: hit flags (+0), Time (+4), Location (+0xC), ImpactPoint (+0x18), Normal (+0x24), ImpactNormal (+0x30), while walking off and jumping onto a box ledge and a triangle-mesh ledge |
| R5 | Perch chain on real geometry | entry / return of 0x14057E0C0 (xmm1 radius, xmm3 max dist; al), 0x1405887B0 (al), 0x14057BD10 (al) with root location and CurrentFloor; kerbs 30 / 50 / 58 / 62 cm (expect Walking kept ≤ 59.4) |
| R6 | Step-up trace | entry / exit of 0x140597570: `CurrentFloor.FloorDist`, `InHit.ImpactPoint.Z`, root Z, the three sweep hits, return value |
| R7 | Ground probe on real geometry | at exit of 0x14063A650: return value, out+0x1C (dist), out+0x18, out+0x20, actor Z (expect ≈ +0.15 standing, negative just after an edge, 0 beside a low wall); landing snap: actor Z before / after 0x143EAC3D0 called from 0x141397380 |
| R8 | Flags found constant statically | movement +0xC18 (expect bit 0x20 set, bit 8 clear), +0x68C (expect 0x06), +0xBDC, +0x458 (bit 2 set, bits 0–1 clear), +0x308 bit 0 (0), +0x380 bits 0–1 (0); external velocity inputs +0xB84, +0xB90, +0xBB4, +0xA24 (vectors, expect 0 outside force volumes / root motion); state queue +0x258 (1); anim instance +0x390 (1); pawn +0xE60, +0xD80, +0x630, +0xA0 (1.0) |
| R9 | Console variables | `float [[0x149D86A48]]` (p.PenetrationPullbackDistance), `int [0x1487C9510]`, `int [0x1487C9514]`, `int [[0x1487C9850]]`, `int [0x1487D7370]` (expect 0.125 [stock], 0, 0, 0, 0) |
| R10 | Montage time multiplier used by the short-hop test | `[[montage instance+0x10]+0x9C]` for `JumpNormalStart` (expect 1.0) |
| R11 | Input layer in the user's setup | `UPlayerInput::InputAxis` 0x1445AB3D0 entries for LeftX / LeftY (Delta xmm2, bGamepad [rsp+0x28], caller); key-state raw (pair+0x18) vs massaged (pair+0x24); axis properties map PlayerInput+0x188 (expect 0.25 / 1 / 1 / false) |
| R12 | Camera rotation the controller reads | camera manager +0x290C bit 0 (expect 0 with the follow camera), +0x28F4; PlayerCameraManager (controller+0x478) +0x7CC at entry of 0x1413F1AB0 and after the frame's camera update |
