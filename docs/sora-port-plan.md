# Sora port: plan, host seam, how to run

Goal: rebuild the player character Sora from the decompilation so that he controls exactly like
the game, 1:1: he stands, walks, runs, sprints, turns around, stops, jumps, falls and lands in a
test level by the game's own rules, under the game's own camera, drawn with the game's pose
pipeline (saved-pose blending, face, leg IK, helper bones). Later the same simulation runs inside
another game (an injected DLL in Elden Ring: not Rust-hosted, not Bevy), so everything that is the
game's behaviour lives in engine-free crates.

| piece | files | state |
|---|---|---|
| movement specification (revision 2) | `docs/sora-movement-spec.md`, `docs/sora-movement-spec-changes.md`, `sheets/port/sora-movement.json` (111 rows) | input |
| animation specification, asset pack | `docs/sora-animation-spec.md`, `docs/sora-animation-spec-changes.md`, `build/sora_pack/` (`tools/sora_pack.js`) | input |
| camera specification | `docs/sora-camera-spec.md`, `sheets/port/sora-camera.json` | input |
| where the first two specs disagreed | `docs/sora-spec-reconciliation.md` | input |
| the specifications of everything else (combat, magic, actions, stats, audio, UI, VFX) | `docs/sora-combat-spec.md`, `sora-magic-spec.md`, `sora-actions-spec.md`, `sora-stats-spec.md`, `sora-audio-spec.md`, `sora-ui-spec.md`, `sora-vfx-spec.md`, their sheets under `sheets/port/` | input, NOT implemented yet: section 9 says where each goes |
| the simulation | `crates/kh-core` | KH3's model, stage-1 subset (`kh_core::MOVER` = `kh3-tres-stage1`), on a general state-machine host, with the host seam and the status container for the rest |
| the field camera | `crates/kh-camera` | `FUN_1406033C0` steps 1 - 16 |
| the pose | `crates/kh-anim` | saved pose, face slots, leg IK, KineDriver, Bonamik (hair, hood, sleeves, coat) |
| the test level | `crates/kh-app` (`khgame.exe`) | runs, draws the pack's model; training dummies, sound backend, effect markers, events page |
| replay against captures of the game | `crates/kh-replay` (another agent's) | drives `kh-core` with recorded per-frame dt |
| build and test | `build.ps1`, `play.bat`, `tools/gen_port.js`, `tools/port_smoke.js` | |

## 1. Architecture

```
sheets/port/sora-movement.json ─┐                        ┌─> build/gen/rust/sora_params.rs         SoraParams (109 fields)
sheets/port/sora-camera.json   ─┤                        ├─> build/gen/rust/sora_camera_params.rs  CameraParams (80 fields)
sheets/port/<other sheets>     ─┤ node tools/gen_port.js ├─> build/gen/rust/<sheet>.rs             parameter structs / tables (section 5)
sheets/reflect/enum-values     ─┤                        ├─> build/gen/rust/tres_enums.rs          ETresStateID, ETresAbilityKind ...
build/sora_pack/manifest.json  ─┘                        └─> build/gen/rust/sora_anim.rs           animations, notify times, root tracks
                                             │ include! (paths from the crates' build.rs)
crates/kh-core    engine-free (glam + std, #![forbid(unsafe_code)]): controller, state-machine host
                  and states, movement component, animation clock, status, the host seam.
                  kh_core::{Sora, Input, Output, World, Host, Targets, Audio, Effects, UiSink, Event}
crates/kh-camera  engine-free: the field camera. kh_camera::{FieldCamera, CameraTarget, CameraInput, CameraView}
crates/kh-anim    engine-free: reads the pack into plain arrays; pose per step.
                  kh_anim::{Pack, Animator, Frame, BodySlot, Pose, IkWorld, Settings}
crates/kh-app     Bevy 0.19.1 test level: collision (implements World and IkWorld), training
                  dummies (Targets), sound backend (Audio), effect markers (Effects), HUD model
                  (UiSink), devices, drawing, unattended test mode.   (later: a DLL host next to it)
```

- Workspace `Cargo.toml` at the project root, members `crates/*`. ONE target directory: `target/`.
  Never pass `--target-dir` or set `CARGO_TARGET_DIR` to anything else.
- The engine-free crates never depend on Bevy and never read clocks or globals. `kh-core` and
  `kh-camera` read no files (their constants are generated tables); `kh-anim` reads the pack
  folder once, at start.

### kh-core (`crates/kh-core/src`)

Stage 1 (movement). Its behaviour is pinned by `tests/sora.rs` (59 tests against the specs'
numbers, each citing its section) and `tests/golden.rs` (bit for bit).

| file | what | written from |
|---|---|---|
| `sora.rs` | the API (`Sora`, `Input`, `Output`, `Tuning`, `DebugEmit`) and the order of one frame | movement spec 1.2 |
| `controller.rs` | the controller's input update: polls stick and buttons, calls the request interface in the game's order | movement spec 2 |
| `states/mod.rs` | THE REGISTRY of player states (`player_states!`: one row per state) and how a state is added | movement spec 1.3 |
| `states/host.rs` | the state-machine host: `PlayerState` trait, the queue (`SetState` / `PushState` / `ClearQueue` / `DoFinish` / `DoChain` / restart), `MainProc`, `StateFlags` | movement spec 1.3 |
| `states/idle.rs`, `run.rs`, `jump.rs` | `TresPlayerState_Idle / _Run / _Jump` and their requests (`TryRun`, `JumpRequest`, `JumpHeld`, `MoveInput`, `Fall`) | movement spec 2.5, 3.1, 4; change list D |
| `states/common.rs` | "on ground", the ground probe, `Turn`, `SetMovementInputXY` | movement spec 1.3, 4.5 |
| `pawn.rs` | `Pawn` (everything of Sora but the running state) and `Ctx` (the pawn + the host for one step): what a state can reach, and THE functions through which the core talks to the host | |
| `cmc.rs` | the movement component: the fork's walking / falling, floor, perch, edge checks, step-up | movement spec 3.3, 3.4, 4.3 - 4.6, 5; change list A - C |
| `anim.rs` | the `FullBody` slot's clock, weight, notify windows, blend-space filter, root motion | animation spec 8, 9; animation change list A2 - A13 |
| `input.rs` | the pad's buttons at device level (`PadButton`, `PadState`) | movement spec 2.4 |
| `world.rs` | the `World` trait and `Hit` | movement spec 6 |
| `params.rs`, `enums.rs` | the generated `SoraParams` and reflected enums (includes) | section 5 |
| `test_world.rs` | planes and boxes, for tests | |

The ground for everything else (round 4: frames, no gameplay rule; section 9):

| file | what |
|---|---|
| `host/` | the host seam beyond collision: `targets.rs`, `audio.rs`, `effects.rs`, `ui.rs` (traits and their plain-data records), `events.rs` (the per-step event list), `recording.rs` (a host for tests), `mod.rs` (`Host`, `NullHost`, `Attach`) |
| `status/` | `Status` (level, EXP, HP / MP / focus, stats, abilities keyed by `ETresAbilityKind`, magic, equipment, items) and `Loadout` (the rows of `sheets/port/sora-loadout-99.json`): data only |
| `ui_model/` | `UiSnapshot` (gauges, command menu, lock-on, popups): the plain data published to `UiSink` each step |
| `combat/`, `magic/`, `abilities/` | EMPTY frames: each `mod.rs` names its spec, its owner and what belongs there |
| `tests/seam.rs` | what a host can rely on: events, handles, the recording host, save state, the pad |

### kh-camera, kh-anim

`kh-camera/src/lib.rs`: one file, the camera update in the spec's step order; `tests/camera.rs`
(19 tests). `kh-anim/src`: `pack.rs` (skeleton, clips, notifies, data files; pack space -> UE
space on the way in), `blend.rs` (the game's interpolation and blend rules), `pose.rs` (the
per-step recurrence), `legik.rs` (`TresAnimNode_TwoBoneIKMultiped`), `kine.rs` (KineDriver), `bonamik/` (a port of
`tools/lib/bonamik/*`, file for file: 30 Hz evaluation with the built-in interpolation, driven by
the step dt; jitter zeroed, no world wind, no ground plane unless the host gives them);
`keychain.rs` (the Keyblade's keychain: a second Bonamik instance on the weapon skeleton and asset, stepped with the weapon's world transform; `tests/keychain.rs`, one test with a bit-for-bit comparison against `tools/lib/bonamik/weapon.js`); `tests/anim.rs` (15 tests on the real pack), `tests/bonamik.rs` (10 tests: the numbers of
`tools/lib/bonamik/validate.js` and a bit-for-bit cross-check against the JS evaluator, whose
dump `build/sora_tmp/bnm_crosscheck/dump.js` writes).

### kh-app (`crates/kh-app/src`)

`main.rs` (command line, wiring, screenshots, exit), `coords.rs` (THE core <-> Bevy conversion),
`level.rs` (test level of oriented boxes; `impl kh_core::World`, `impl kh_anim::IkWorld`),
`player.rs` (fixed-step loop through the host seam, devices, trace, capsule stand-in), `script.rs`
(words of `--script`), `camera.rs` (places the Bevy camera; the debugging orbit camera), `hud.rs`
(debug text, F1; events page, F2), `pack.rs` (the pack's model; steps `kh-anim`; uploads joints; the Keyblade and its simulated keychain, `--no-keychain`),
`xinput.rs` (the pad: sticks and every button). This host's side of the seam (section 3):
`fixtures.rs` (training dummies = `impl kh_core::Targets`; floating labels), `seam.rs` (audio and
effect queues = `impl kh_core::Audio` / `Effects`, HUD model = `impl kh_core::UiSink`, the event
log), `audio.rs` (the sound backend and bank), `markers.rs` (effect markers), `loadout.rs`
(`--loadout`).

## 2. Conventions

KH3's own (UE4's): centimetres, seconds, degrees; X forward, Y right, Z up, left-handed; yaw about
Z from +X toward +Y (yaw 90 is to the right of yaw 0); pitch positive = looking up. **A position
is the actor origin, which for Sora is at foot level**: the capsule (radius 60, half height 89) has
its centre 91 cm above it, so its bottom is 2 cm above the origin and, standing, 1.9 - 2.4 cm
above the floor (aimed at 2.15): the origin stands at about +0.15 cm. The mesh component sits at
the actor origin turned -90 degrees of yaw: in mesh ("component") space he faces +Y.

Bevy is metres, Y up, right-handed. The one conversion (`crates/kh-app/src/coords.rs`):
core `(x, y, z)` cm -> Bevy `(x, z, y) / 100` m (its own inverse up to the factor); core yaw `a`
-> Bevy rotation of `-a` about Y of something whose forward is Bevy +X. It is the swap the pack
bakes in; `kh_anim::pack` undoes it on reading, `kh_anim::pack::trs_to_pack` redoes it for drawing.

## 3. The host seam

Everything the simulation needs from the game that hosts it, and everything it hands back. This
section is written to be followed cold by someone hosting Sora in a game that is not Bevy and not
Rust-hosted (the engine-free crates then sit behind a C ABI of the host's making; nothing in them
depends on how). Authoritative text: the doc comments of `kh-core/src/sora.rs`, `world.rs`,
`host/*.rs`, `input.rs`, `anim.rs`, `kh-camera/src/lib.rs`, `kh-anim/src/pose.rs`, `legik.rs`.
A complete host to read: `crates/kh-app/src/player.rs` `simulate` (the loop), `level.rs`
(`World`), `fixtures.rs` (`Targets`), `seam.rs` (`Audio`, `Effects`, `UiSink`, the event log).

### One frame

**Reference frame rate 60 fps.** The game has no fixed step: controller, movement, animation,
states and camera all get the frame's dt (at most 0.05 s). The port runs them at a fixed 1/60 s
per step whatever the display rate, and interpolates what it draws: that is the game as it runs at
60 fps. `kh-replay` drives the same `kh-core` with the per-frame dt recorded from the running
game, for comparison against captures.

```
per fixed step N:
  input.camera_yaw = camera.input_yaw()            the camera as step N-1 left it
  host = kh_core::Host { world, targets, audio, effects, ui }      borrowed for this step only
  out = sora.step_hosted(1/60, &input, &mut host)  C(N) controller -> M(N) movement tick with the
                                                   pose tick inside -> S(N) state tick, last;
                                                   then one UiSnapshot is published to host.ui
  act on out.events                                (if the host did not act inside the trait calls)
  view = camera.step(1/60, &camera_input, &target(out), &world)     after the pawn
  pose = animator.step(&Frame { dt, body(out.anim, out.anim_ticked), component_to_world(out) }, &ik_world)
per drawn frame:
  draw out.lerp, view.lerp, previous_pose.lerp(&pose, alpha)        never re-evaluate
```

`sora.step(dt, &input, &world)` is the same step with a host that has only a world
(`kh_core::NullHost`: no targets, silent, nothing drawn, no UI); its events are still returned.

### The host supplies

1. `Sora::new(SORA_PARAMS, Pose { position, yaw })`: `position` is the actor origin; on a floor
   at height `h` use `h + Sora::origin_height_on_floor(&SORA_PARAMS)`. His status starts as the
   explicitly empty placeholder; give him one with `sora.with_status(Status::from_loadout(&loadout))`
   (`kh_core::status`: a `Loadout` is filled row by row from `sheets/port/sora-loadout-99.json`,
   which the HOST reads: the core reads no files; `kh-app/src/loadout.rs` is 60 lines).
2. **Input** (`kh_core::Input`), once per step: `move_stick` = the left stick's RAW axes (the
   game's 0.25 per-axis dead zone is the core's; the host must apply none); `jump_pressed` on the
   one step the button went down (latch a press until a step consumes it), `jump_held` while it is
   down; `camera_yaw` = the camera's yaw at the end of the PREVIOUS step; `pad` = every button of
   the pad (`PadState { held, pressed, released }` over the 16 `PadButton`s: the four face
   buttons, shoulders, triggers as buttons, D-pad, stick clicks, the two special buttons).
   `Input::default().with_pad(pad)` fills the jump from the pad's right face button, which is the
   jump in the game's default layout (`i.GamePadDecideType` 0: jump / cancel = right face button,
   attack / decide = bottom one; movement spec 2.4). No other button has an action yet: the specs
   assign them, and their implementers name them in `kh-core/src/input.rs`.
3. **World queries** (`kh_core::World`; cm, core axes; positions are shape CENTRES). One shape: an
   upright capsule (`half_height == radius` is a sphere, both 0 a line).
   - `sweep_capsule(start, end, radius, half_height) -> Option<Hit>`: the first blocking contact.
     A surface merely touched at `start` is reported only if the move goes into it.
   - `line_trace(start, end) -> Option<Hit>`.
   - `sweep_capsule_all(...) -> Vec<Hit>`: every component met, each with its first contact,
     ordered by time (the 25 cm edge sphere and the states' 50 cm ground probe).
   - `Hit { time, position (ImpactPoint), normal (the sweep's), impact_normal (the surface's),
     start_penetrating, penetration_depth, component }`.
   - **R4 (runtime item)**: what PhysX reports as `ImpactNormal` when the capsule's round end
     touches a box edge or a mesh edge decides every edge contact (walkable or not: perching,
     kerbs, riding up ledges). The test worlds return, for a box, the face most opposed to the
     move among the faces the touched point lies on (UE's `FindBoxOpposingNormal`): the tread
     when coming down onto an edge, the riser when moving into it sideways. A host over real
     PhysX passes on what it gets.
4. **Targets** (`kh_core::Targets`; `host/targets.rs`): the things he can lock on to and hit.
   The host owns them; the core asks and tells:
   - `enumerate(&self, out: &mut Vec<TargetInfo>)`: every candidate, in an order that depends
     only on the host's own history. `TargetInfo { id: TargetId (the host's own number, 0 = none),
     position (its actor origin), center, velocity, yaw, radius, half_height (bounds: an upright
     capsule about `center`), flags (ALIVE, HITTABLE, LOCKABLE, AIRBORNE), hp, hp_max }`.
   - `info(id) -> Option<TargetInfo>`: one target now; `None` when it no longer exists.
   - `hurt_shapes(id, out: &mut Vec<HurtShape>)`: the volumes an attack is tested against:
     capsules `{ a, b, radius, part }` in world space. A host without per-part shapes gives one
     capsule equal to the bounds.
   - `apply_hit(&mut self, &HitRecord) -> HitResult`: a hit is delivered; the host takes
     `HitRecord::damage` (already final: the core's combat rules computed it) off its target and
     answers `{ accepted, damage_dealt, hp_after, killed, guarded }`. `HitRecord` also carries the
     attack's row and name, the contact point and direction, `ETresDamageKind`,
     `ETresDamageAttribute`, an impulse (knock-back request, cm/s) and free flag bits.
   Target answers feed the simulation exactly like world answers: they must be deterministic.
   NO RULE lives in this seam: who is eligible, how damage is computed and what a hit does to
   Sora are the combat and magic specs' business (section 9); until those are implemented the
   core never calls `Targets` by itself.
5. **Audio** (`kh_core::Audio`): `play(SoundHandle, &SoundPlay)`, `stop(SoundHandle, fade_out)`.
   `SoundPlay { id: SoundId, name, at: Attach, volume, pitch, looping, spatial }`. `SoundId` is the
   row of the generated sound table (once the audio sheets are registered in the generator);
   `name` is that row's name. The host owns decoding, mixing, attenuation; nothing is returned.
6. **Effects** (`kh_core::Effects`): `spawn(EffectHandle, &EffectSpawn)`, `stop(EffectHandle,
   immediate)`. `EffectSpawn { id: EffectId, name, at: Attach, rotation, scale, persistent }`.
   `Attach` (both seams): `World(point)`, `Sora { bone: Option<name>, offset }` (the host follows
   him; the bone's transform is in the `Pose` it draws), `Target { target, offset }`.
7. **UI** (`kh_core::UiSink`): `publish(&UiSnapshot)` exactly once per step, at its end:
   `UiSnapshot { step, gauges, command_menu { visible, cursor, entries }, lock_on, popups, board }`
   (`kh-core/src/ui_model`). Since the status side exists (2026-10-11) `gauges` carries the
   status HUD's numbers WITH THE TIMELINE FRAMES the game's native side computes (UI spec
   section 4): level, EXP, EXP to the next level, HP / MP / focus and maxima, `hp_base_frame`,
   `hp_frame`, `hp_red_frame` (the red damage-lag gauge: 0.8 s wait, then 10 ticks of 33.3 ms, as
   a fixed-step model), `mp_base_frame`, `mp_frame`, `mp_charging`, `mp_charge_value`,
   `mp_charge_frame`, `focus_frame`, `focus_is_max`, `pinch`, AP and the three stats; frames are
   CLIP frames (the helper `gauge_frame` + 1). `popups` = raised in this step (`UiPopup { kind:
   KIND_EXP_POINT / KIND_LEVEL_UP / KIND_NEXT_LV / KIND_MUNNY / KIND_ITEM / KIND_DEBUG, text, value,
   position, detail }`); `board` = what is on screen with ages (EXP points 3.0 s, level up 272
   frames, Next LV and munny 146, item 114 with a queue). The command menu and the lock-on are
   still empty.
8. **Camera** (`kh_camera`): `CameraInput { right_stick (raw), recentre (R3), extra_rotation }`
   and the same `World`. `CameraTarget` is built from `Output` (`position`, `yaw`,
   `displacement_velocity`, state).
9. **Leg IK** (`kh_anim::IkWorld`): `sweep_sphere(start, end, radius) -> Option<SweepHit { time,
   impact_point, impact_normal, start_penetrating }>` in UE world space: the game's query on
   collision channel `TresIKCollision`, the owner ignored. Two sweeps per step at most.

Handles (`SoundHandle`, `EffectHandle`) are allocated by the CORE, counting up from 1 (never 0),
so that a replay of the same inputs gives the same handles whatever the host does. A host that
cannot act in the middle of a step implements `Audio` / `Effects` as queues and drains them after
it (`kh-app/src/seam.rs` does: its audio device is not `Send` and Bevy's `Commands` are not at
hand inside the fixed step), or implements nothing and reads `Output::events`.

### The host consumes

`kh_core::Output` (not `Copy` since round 4: `clone()` to keep two): `position` (actor origin),
`capsule_center`, `yaw`, `velocity`, `displacement_velocity` (what the camera reads), `state`
(`StateId` = the game's `ETresStateID`: Idle 1, Run 2, Jump 3 ...) / `sub_step` (the game's),
`movement_state` (a readable summary; `Other` for states beyond stage 1), `on_floor`, `anim` (the
`FullBody` slot at the end of the frame: what is shown) and `anim_ticked` (the slot as the pose
tick left it: whose notifies fired), `step` (steps taken, this one included) and **`events`**:
everything this step told the host, in the order it happened (`Event { step, kind }`):

| `EventKind` | when |
|---|---|
| `StateChange { from, to }` | the state queue ended one state and began another (`MainProc`) |
| `SoundPlay { handle, play }`, `SoundStop { handle, fade_out }` | `Audio::play` / `stop` was called |
| `EffectSpawn { handle, spawn }`, `EffectStop { handle, immediate }` | `Effects::spawn` / `stop` was called |
| `Hit { record, result }` | `Targets::apply_hit` was called, with the target's answer |
| `Popup(UiPopup)` | a popup was raised (also in this step's `UiSnapshot::popups`) |

The trait calls and the event list carry the same information in the same order: act on either.

`AnimRequest`: `name`, `play_id`, `time`, `active` (false once a one-shot has ended: the slot
shows the saved pose), `weight` (montage weight of this update), `looping`, `blend_in`, `axis`
(after the blend space's filter; draw with `AnimId::sample_weights`), `root_motion`,
`notify_window`.

`kh_camera::CameraView`: position, rotation, look point, distance, the timers.

`kh_anim::Pose { local, component }`: every bone's transform in KH3 space. Skin with `component`
(the game skins with component-space transforms; a host whose scene graph multiplies matrices must
not rebuild them from `local` under non-uniformly scaled parents: `kh-app` flattens the skeleton).

Also readable between steps: `sora.status()` / `status_mut()`, `sora.ui_snapshot()`,
`sora.state_flags()` (`m_bEnableChange`), `sora.notes()`.

**What a host reports to the status** (`kh-core/src/status/requests.rs`; applied at the start of
the next step, popups come back through the seam): `sora.give_exp(base_exp, rate, position)` when
one of its enemies is defeated (stats spec 4: `ceil(rate x base)`, difficulty, Zero EXP, 999 per
award, 9,999,999 in total, level-ups from the level table; popups EXP point, level up, Next LV),
and `sora.status_request(StatusRequest::Damage { amount, hit } | Recover { .. } | FullRecover |
UseMp { .. } | UseItem(..) | ObtainItem(..) | Prize(..))`. The amount of a hit on him is
`kh_core::status::compute_damage(&Attacker::enemy(attack), &sora.status().as_defender(), &attack,
&DamageContext::of(sora.status()))`: the game's damage function 0x140632240. A host whose targets
have KH3-style numbers (Defense, damage floor / cap, attribute resists, Heartless / Nobody)
overrides `Targets::defense(id) -> Option<TargetDefense>`; the default `None` is a target without
a status object (Defense 0, no floor, cap or resist).

Determinism: same `(dt, Input)` sequence + same world and target answers = bit-identical outputs
on the same build. `Sora`, `FieldCamera` are plain data (`Clone` = save state; the status is part
of it).

### Testing a host without gameplay

`sora.debug_emit(DebugEmit::Sound(..) | Effect(..) | Hit(..) | Popup(..) | StopSound(..) |
StopEffect(..))` queues something for the start of the next step and sends it through the same
seam functions the states will use (`pawn.rs`), with no rule behind it. `kh_core::RecordingHost`
is a ready host for tests: a list of simple targets and a record of every call
(`tests/seam.rs`).

## 4. How to run and test

```
.\build.ps1            preflight -> node tools\gen_port.js -> cargo test (kh-core, kh-camera, kh-anim)
                       -> cargo build --release -> node tools\port_smoke.js
.\build.ps1 -Run       ... then launch the game          (-NoSmoke skips the smoke test)
play.bat               launch (builds first if needed); play.bat --no-pack for the capsule
powershell -NoProfile -ExecutionPolicy Bypass -File build.ps1 > build\last-build.log 2>&1     (unattended; read the log)
```

- Preflight exit code 1 stops the build; 2 (unfinished rows elsewhere) continues with a note.
- `cargo test --release --workspace` runs everything, the app's and `kh-replay`'s tests included
  (`--release` so Bevy is not built twice): 166 tests as of round 4. `crates/kh-core/tests/golden.rs`
  pins Idle / Run / Jump bit for bit (two 20 000-step sessions hashed, both `Tuning` modes): a
  change that is not meant to touch stage 1 must leave it green. The `kh-anim` tests need `build/sora_pack`; without
  it they say SKIPPED and pass.
- A new file under `tools/` needs its kernel row: `node tools/extract.js`.
- The generator needs the sheets AND the pack (`node tools/sora_pack.js` builds the pack).

Controls, as the game maps them (movement spec 2): left stick or W A S D (camera-relative; any
held key overrides the stick; Left Alt = walk; a stick pushed less than 0.8 walks); **jump = the
pad's right face button (Xbox B), Space or Backspace** (hold for the full jump); right stick turns
the camera, R3 recentres it; F1 toggles the debug text. NOT game behaviour: a left click takes the
mouse and mouse motion turns the camera (Esc lets go); the game's mouse path was not specified.

**The pad is read through XInput directly** (`kh-app/src/xinput.rs`), converted as the game's
engine does (`short / 32767`, `/ 32768` when <= 0, per axis, nothing else), so that the stick the
core's dead zone sees is the one the game's sees: a diagonal at an Xbox pad's rim has a raw radius
above 1.0 and stays a RUN request. Bevy's pad (gilrs over Windows.Gaming.Input, per-axis values
clamped to +-1, polled every 8 ms on a thread; Bevy's own dead zone 0.05 / threshold 0.01 filter
switched off) is only the fallback for pads XInput does not see; the debug text says which is in
use, shows the raw stick, its radius, the magnitude after the dead zone, whether that is a walk
request, and counts the `RunStartN` replays by trigger (turn-around / walk -> run / from the stop
clip; `docs/sora-movement-spec-turning.md`). `--input-log FILE` records a played session's raw
input per step, `--input-replay FILE` plays it back exactly. Measurements of turning:
`build/sora_tmp/turning/` (`measure.js` -> `report.md` and plots, `shaping.js` -> `shaping.md`,
`frames.js`).

Unattended (`crates/kh-app/src/main.rs` lists every option):

```
target\release\khgame.exe --script "stop 0.5; run 1.5; turn 180; run 1.2; stop 1; hold-jump 0.6; stop 1" ^
    --frames 420 --shots "37:a.png,127:b.png" --trace trace.csv
```

- `--frames N`, `--shot f.png`, `--shots "F:file,..."`, `--trace file.csv` (one line per step;
  header in `player.rs` `TRACE_HEADER`: input, pawn, animation, camera, leg IK, face), `--hz N`,
  `--dt S`, `--pack DIR`, `--no-pack`, `--model-yaw DEG`, `--pose-dump FILE`, `--frame-log FILE`
  (what each drawn frame showed), `--realtime` (a scripted run on the real clock with vsync).
- `--orbit-camera` (+ `--camera-yaw`, `--camera-distance`, `--camera-height`, `--camera-pitch`):
  a debugging camera that never turns by itself; scripts' "forward" is then a fixed direction.
- `--no-leg-ik`, `--no-kine-driver`, `--no-bonamik`, `--no-face`: that part of the pose off, for
  comparisons.
- `--events FILE.csv` (one line per event of the host seam: `step,time,kind,detail`),
  `--show-events` (the F2 page shown from the start), `--hide-debug-text`, `--mute`, `--audio`
  (unattended runs are muted unless it is given), `--sound ID=FILE` (a WAV or Ogg Vorbis file into
  the sound bank; repeatable), `--loadout FILE.json`, `--level N` (the loadout at level N: EXP,
  stats and the level table's abilities of that level), `--no-dummies`.
- Script words (advanced per step; `script.rs`): `stop N`, `walk N`, `run N`, `stick X Y N`,
  `raw X Y N`, `stick-sweep A0 A1 N`, `stick-line X0 Y0 X1 Y1 N` (raw device values), `turn DEG`,
  `jump`, `hold-jump N`, `cam X Y N` (right stick), `recentre`. Stick values in a
  script are what the game sees after its dead zone.
- Pad buttons in a script: `press BUTTON` (down until `release BUTTON`), `tap BUTTON` (one step),
  `hold BUTTON N` (N seconds). BUTTON: `FaceBottom FaceRight FaceLeft FaceTop LeftShoulder
  RightShoulder LeftTrigger RightTrigger DPadUp DPadDown DPadLeft DPadRight LeftThumb RightThumb
  SpecialLeft SpecialRight`, the Xbox labels (`a b x y lb rb lt rt up down left right l3 r3 back
  start`) or `jump` (= `b`) / `attack` (= `a`), the two actions whose button is specified.
- Fixture words (NOT gameplay: they push something through the host seam with
  `kh_core::DebugEmit`): `emit-sound [ID]`, `emit-effect [ID]`, `emit-loop` / `emit-stop` (a
  looping sound and a persistent effect), `emit-hit N [DAMAGE]` (training dummy N), `emit-popup`,
  `reset-dummies`. Interactive: F5 resets the dummies, F6 sound + effect, F7 a hit on dummy 1,
  F8 / F9 the loop on / off, F2 the events page.
- Status words (requests to the status rules of `kh-core/src/status`, not fixtures):
  `give-exp N [BASE [RATE]]` (the EXP of a defeated enemy, awarded from dummy N's position;
  default 100 x 1), `hurt DAMAGE` (a damage-function amount applied to Sora: Second Chance,
  Withstand Combo, the red gauge), `use-mp COST` (255 = all, as Cure: starts the MP charge),
  `use-item ID` (257 Potion, 258 Hi-Potion, 260 Ether ...), `recover`. Interactive: F4 = 500 EXP
  from dummy 1, F10 = hurt 30, F11 = full recover. The debug text's `status` lines print the HUD
  model: values, gauge frames, the red gauge, PINCH, MP CHARGE, stats, AP, the popups on screen.
- Unattended runs do not read the machine's devices, do not need the focus, have fixed-length
  frames and compile render pipelines synchronously: frame N is step N.

The smoke test (`node tools/port_smoke.js`, output in `build/smoke/port/`) runs scenarios A - J
and asserts the specifications' numbers from the traces; its header lists them. Scenario J is the
seam's fixtures: debug words, pad buttons and dummies must leave the trace byte-identical to the
same run without them, and the event file must hold what was emitted, in order, with the dummies'
answers (`j_fixtures`, `j_events`). Screenshots:
`a_*` ... `e_*` (pawn and camera), `f_slope_ik`, `g_stairs_ik` (leg IK), `h_run_*` and
`h_run_*_nokd` (with / without KineDriver), `i_face`, `i_face_off`.

The test level (`level.rs`; metres; start at the origin facing +X, Z to the right): floor
100 x 100 with lines every metre; boxes 0.25 / 0.5 / 1 / 2 m at x 8, z 3..12; staircase (8 x
0.2 m) to a 1.6 m platform at x 14..20, z 3..6; 30 degree ramp to a 2 m platform, 3 m gap, second
platform at z -6..-3; 50 degree ramp at z -11..-8; wall at x 26. Every box is one collision
component and blocks the pawn, the camera and the leg IK alike.

BEHIND the start (so that the lanes above stay as they were) stand the five TRAINING DUMMIES
(`fixtures.rs`: 1 standing A at x -5, z 0; 2 standing B at -5, 2.5; 3 small at -5, -2.5; 4 large
at -8.5, 1.5; 5 floating at -8, -2.5, 2.5 m up). They are this host's `kh_core::Targets`: 100 000
hit points each, shown above them with the hits received; they block nothing (in the game other
pawns do not block the player's capsule either). An effect the core spawns is drawn as a
labelled ball (`markers.rs`), a sound is played from the bank (`audio.rs`: the pack's sounds,
`--sound`, two built-in debug tones).

## 5. Sheets -> code

`node tools/gen_port.js` writes every generated file under `build/gen/rust/` (plain data; the
crates never read a file at run time). Its header is the reference; THE REGISTRY an implementer
edits is `tools/lib/port_gen_defs.js` (three lists: `params`, `tables`, `enums`). A crate includes
a generated file with `include!(concat!(env!("KH_GEN_DIR"), "/<file>.rs"));` (`kh-core/build.rs`
exports the folder and watches every file in it; `src/enums.rs` is the example).

- **Parameter sheets** (columns id, value, unit, source, evidence, ref_addr, ref_conf, note; one
  row = one constant): one struct per sheet, one field per row (id with dots as underscores), doc
  comment = source, evidence, address. Typing: number -> `f32` (`i32` when the unit is int, count,
  frames or index and the value is whole), true / false -> `bool`, string -> `&'static str`, array
  of numbers -> `[f32; N]`. Generated with the struct: `<PREFIX>_PARAMS` (the values),
  `<PREFIX>_PARAM_ROWS`, `<PREFIX>_PARAM_INFORMATIONAL`.
  - REGISTERED sheets (`params` in the registry): `sora-movement` -> `sora_params.rs`
    (`SoraParams`), `sora-camera` -> `sora_camera_params.rs` (`CameraParams`). A null value fails
    the generator unless the row id is in that sheet's `informational` list (rows that record a
    question, not a code input; the list says what the code uses instead): `tick.order`,
    `anim.run_start_root_distance`, `move.penetration_pullback` (stock 0.125 assumed, runtime item
    R9), `fade.character_near`.
  - Every OTHER parameter sheet under `sheets/port/` is generated too, with default names:
    `sora-combat-params.json` -> `sora_combat_params.rs`, `SoraCombatParams`, `SORA_COMBAT_PARAMS`.
    If such a sheet has an unchecked (null) or untypable value it is NOT GENERATED (the tool says
    which rows; a stale file of it is deleted) and the run goes on: research sheets appear there
    unfinished and must not break the build. **When code starts to depend on a sheet, register
    it** with its informational list: from then on a null is an error.
- **Table sheets** (one row = one attack / spell / ability / sound ...) are generated only from a
  schema in the registry's `tables`: `{ sheet: { file, row, table, doc, key, key_prefix, columns:
  { 'sheet column': { type, optional, field, doc } } } }`. Column types `str`, `f32`, `i32`, `u32`,
  `bool`, `f32[]`, `i32[]`, `str[]`, `enum` (with `enum: 'ETresX'`; the cell is the value's game
  name or number). Output: `pub struct <Row>`, `pub static <TABLE>: &[<Row>]`, with a key column
  `pub const <KEY_PREFIX>_<VALUE>: usize` per row and `pub fn <table>_by_<key>(&str)`. A null cell
  is `None` in an `optional` column and an error in any other. A sheet that is neither a parameter
  sheet nor registered is skipped with a line saying so.
- **Reflected enums** (`enums` in the registry) -> `tres_enums.rs`: `#[repr(i32)] pub enum
  <GameName>` with the game's values from `sheets/reflect/enum-values`, `ALL`, `name()` (the Rust
  variant: `AirSlide`), `game_name()` (`TSID_STATE_AIR_SLIDE`), `value()`, `from_value()`,
  `from_game_name()`. Generated now: `ETresStateID` (`kh_core::StateId`), `ETresAbilityKind`,
  `ETresCommandKind`, `ETresDamageKind`, `ETresDamageAttribute`. Add a name to the list when code
  needs another.
- `build/sora_pack/manifest.json` -> `sora_anim.rs`: per logical animation its play mode bits,
  blend-in, blend-space axis, samples and input-filter window; per sequence its length, the
  `TresJumpStart`, `TresEnableChange`, `TresReverseLeg` notify times (with their trigger offsets)
  and the root track where `bEnableRootMotion` is set. (An implementer who needs more notifies or
  more animations extends this part of the tool and `kh-core/src/anim.rs` together.)
- `--check` compares without writing (`build.ps1` runs it after generating); `--sheet FILE
  --out-dir DIR` tries a draft of the movement sheet (build against it with
  `KH_GEN_RUST_DIR=DIR`); `--defs FILE --sheets-dir DIR` point the tool at fixtures
  (`build/sora_tmp/ground/gen/run_fixture.sh` is one: parameters, an unfinished sheet, a table with
  every column type, an enum; `check_all.sh` there compiles every generated file with `rustc`).

`kh-anim` reads the rest of the pack at run time: skeleton, clips, every clip's notifies (TresIK
windows, face plays), `data/legik.json`, `data/kinedriver.json`.

## 6. What only the running game can confirm

Implemented as the specifications state; each is marked in the code. The specs' own lists of
runtime requests: movement change list F (R1 - R12), animation spec 15 (R1 - R11).

| # | assumption | where | settles it |
|---|---|---|---|
| 1 | **Frame order** C -> M (pose tick inside) -> S | `sora.rs` | movement R1 |
| 2 | **First update of a montage**: the movement spec (rev 2, followed) has the clock advance in the first pose tick after the play (launch notify in frame 4, `DoJump` in frame 5); the animation spec (A3) holds the position at 0 on that update (frames 5 and 6). One switch: `kh_core::anim::FIRST_UPDATE_HOLDS_POSITION` | `anim.rs` | movement R3, animation R3 |
| 3 | **Edge normals** (R4, section 3) | `world.rs`, `level.rs` | movement R4 |
| 4 | **Ledges of 48 - 57 cm**: with the test world's edge normals, WALKING into a 48 or 50 cm ledge still rides up it (the floor sweep right after the refused step-up accepts the edge under the round bottom, the perch probe confirms it because the lower floor is within 59.4 cm, the floor adjustment lifts him 2 cm a tick, and soon the step-up's own test passes); running into one does not; 58 cm and above is a wall. The change list expected "climbed iff the step-up accepts". A consequence of the written rules plus item 3 | `cmc.rs`; test `ledge_heights_30_to_62_cm` | movement R4, R6 |
| 5 | **Penetration pull-back 0.125** (the sheet row is null) | `cmc.rs` | movement R9 |
| 6 | **Terminal speed 4000 cm/s**, **world gravity -980** (physics-volume defaults) | sheet rows | per-map check |
| 7 | **Restriction flags 0** (restrict-run, force-walk, battle stance, floating) | `states.rs` | movement R8 |
| 8 | **The pose shown in the frame of a play call** is the saved pose (the old montage is cut, the new one has weight 0), evaluated after the state tick | `kh-anim/src/pose.rs`, `Output::anim_ticked` | animation R1, R2 |
| 9 | **Leg-IK blend**: each solved bone is made local against its parent's NEW transform when the parent is the entry before it in the sorted list (stock 4.17 `LocalBlendCSBoneTransforms`); the notes (`native_D_legik.md` 4.1) word it as "the unmodified parent", which would throw the foot off its target | `legik.rs` | animation R5 |
| 10 | **Leg-IK global factor** from the game instance = 1; no `SQEX_bDisableIKOnBase` floors | `pose.rs` | animation R4 |
| 11 | **Notify order of a blend space's samples** = sample order (run loop at t = 0: `0003L`, `0003`, `0003R`, so the mouth is `0000EM` again until `0051M` at 0.089 s) | `pose.rs` | animation R9 |
| 12 | **Pad**: Bevy's axis filter is switched off; what the pad library does below it is not controlled; keyboard input is not delayed by a frame (change list E6, optional); `atan2` is libm's (E7) | `player.rs` | movement R11 |
| 13 | **Camera** open points are listed in `docs/sora-camera-spec.md` (mouse path, fade of the character when the camera is near) | `kh-camera` | |

Consequences of the rules as written that look surprising and are NOT tuned away:

- The launch is 5 frames after the press; a press in the frame after touchdown is lost; a jump
  pressed in Idle together with the stick is lost (the run request clears the queued jump).
- At a jump from a run the speed dips for one frame (680 -> 464.5), then +61.2 a frame.
- Letting go of the stick: one braking frame (680 -> 464.5) before the stop state zeroes the speed.
- The short hop starts on the first rise-loop tick with the button up: a tap and any release
  before then give the same hop.
- A kerb of up to 59.4 cm is rolled down without leaving Walking; the Run state goes on for the
  frames in which its probe sphere still rests on a lip.
- Walking into a wall keeps the commanded speed; a ceiling hit keeps `Velocity.Z`.
- The per-axis dead zone bends the stick's direction: a stick 80 degrees from forward reads as 90.
- Leg IK: the hips controller's steady state is 0.916 of the wanted drop at 60 Hz, and a leg that
  is already as straight as the animation has it cannot reach further: on flat ground the ankles
  come down 0.137 of the 0.15 cm between origin and floor. On a slope the capsule's axis floats
  above the surface (9.3 cm at 30 degrees) and the IK lowers the hips by most of it.
- Leg IK reuses a cached hit every other frame while standing, with the old sweep fraction.

## 7. Deliberate departures from the original (user decisions)

These are NOT the game's code. Each is a named option of `kh_core::Tuning` (not a generated
constant), ON by default, with the original rule kept behind a switch; `Tuning::GAME_EXACT` turns
all of them off, and `kh-replay` (comparison with captures of the real game) always uses that.
Decisions: `sheets/decisions.json` rows `dec013`, `dec014`, `dec015`, `dec016`.

| option | the user's words (2026-10-11) | what it does | the original, behind the switch |
|---|---|---|---|
| `continuous_turning` (`--game-exact-turning` turns it off), `reversal_angle` 135 (`--reversal-angle N`), `reversal_window` 5 ticks (`--reversal-window-frames N`), `start_lunge_window` 0.30 s (`--start-lunge-window S`) | "When I spin the stick around I don't want Sora to stop running, he should do his lurch animation only when the stick isn't pushed anymore"; after playing it: "his animation for changing directions should still play when you flick the stick into the opposite direction, also his lurch animation isn't playing when you first move from standstill" | While the stick is rolled round, the run never stops to replay `RunStartN`. The game's turn-around (`StopMovement`, `RunStartN` with the signed delta as axis) plays only for a FLICK: when, besides the game's own `\|target yaw - facing\| >= 75`, the target yaw has swung by `reversal_angle` or more within `reversal_window` ticks (a tick without a request holds the last target, so a flick through the centre counts); a roll round the rim stays lunge-free up to 1620 degrees a second (4.5 turns a second); otherwise the input keeps being written and the facing eases with the unchanged `Turn(target, 0.1 x dt x 60)`. A run request after a walk frame inside Run goes on in the loop, EXCEPT as the start from standstill: in a Run state entered from Idle, before its first run tick and within `start_lunge_window`, the game's path (`RunStartN`) is kept: a real stick takes a few frames from the centre to the rim, so the state begins with walk requests and this path is the game's start lunge. Idle -> Run with a run request and a run request during the stop clip play `RunStartN` as in the game; a centred stick is "not pushed" and the game's stop logic applies unchanged (one centred frame: no stop; two or more: the stop clip begins and the next push starts with `RunStartN`) | `docs/sora-movement-spec-turning.md` triggers A and B; tests `a_turn_of_75_degrees_or_more_replays_the_run_start`, `run_keeps_its_loop_for_the_grace_time_when_the_stick_drops` |
| `jump_cut_on_release` (`--game-exact-jump`), `release_rise_speed` 400 cm/s (`--jump-release-speed N`), `min_jump_height` 110 cm (`--min-jump-height N`) | "Sora should stop gaining height in his jump as soon as I let go of the jump button"; after playing a hard cut: "make the jump a bit higher after you let go of B before he falls"; then: "make the minimum jump height just high enough so he can land on the orange block" (the test level's 1 m box) | At the start of a movement tick in which he is in the air in the Jump state after the launch (sub-step 1 or 2) with the button up: `Velocity.Z = min(Velocity.Z, cap)`, `cap = max(release_rise_speed, sqrt(2 g (min_jump_height - risen + g dt^2 / 8)))`, `g` = the rise gravity the movement code applies (4116), `risen` = height above the launch. So a tap tops out at 110 cm above the take-off (0.14 cm more at most), a later release keeps 400 cm/s (about 19 cm more rise), a held jump is the game's 266 cm. The game's own short-hop rule is not applied while this is on; its apex handling follows unchanged (hang, then fall; fall loop). The launch tick is never clamped | movement spec 4.2: a release counts from the first rise-loop tick and sets `GravityScale` 3.0 until the apex; test `the_short_hop_starts_on_the_first_rise_loop_tick_with_the_button_up` |
| `speed_ramp` (`--game-exact-speed`), `speed_ramp_time` 0.25 s (`--speed-ramp-time S`) | "is there any way to make the transition from walk to run smooth?" - "do the speed ramp" | While he stays in the Run state's loop, the ground speed limit moves linearly between the walk gait's limit (140) and the run gait's (680) over 0.25 s when the gait switches, in either direction; a switch back mid-ramp ramps back from where it is. Not ramped: the start from standstill, the stop, the turn-around, jumps and landings, run -> sprint (the limit's run end is MaxWalkSpeed, so 980 arrives at once; any walk tick resets the sprint as in the game). Acceleration, the 0.2 s run grace and the WalkLoopN <-> RunLoopN swap (0.2 s blend-in) are the game's; the cycles are not speed-matched, so the feet slide during the ramp | movement spec 3.1 / 3.2: walk mode = MaxWalkSpeed x 0.205882, switched in one tick; test `run_680_sprint_980_after_more_than_one_second_walk_140` |
| `speed_matched_anim` (`--game-exact-anim`), `speed_match_sprint` off (`--speed-match-sprint`) | "I want speed matched animation" | In the Run state's loop the slot plays one continuous locomotion loop instead of swapping WalkLoopN / RunLoopN: the walk cycle (0002) and the run blend space (0003L / 0003 / 0003R with its lean and filter) on one shared phase, blended by his actual horizontal speed (weight 0 at 140 cm/s, 1 at 680), the phase advancing at speed / stride, stride = lerp(140 x 1.0667, 680 x 0.5333) cm by the same weight; at exactly the walk or run speed each clip plays at rate 1.0. The run samples are read 0.0099 of a cycle earlier (the measured phase difference of the two cycles: `build/sora_tmp/turning/phase_check.js`). No play call when the gait changes; entering the loop is a play call with the 0.2 s blend-in. Notifies are the dominant side's (weight >= 0.5: run): the reverse-leg window for the stop foot, the leg-IK windows (walk 0.9, run none), the face plays; the animation name reported is the dominant side's. Below the walk speed the rate is the walk's; above the run speed (sprint) the game's playback unless `speed_match_sprint`. `kh-core` decides weight and phase (`AnimRequest::locomotion`), `kh-anim` blends (`BodySlot::blend_with`) | animation spec 2.3: the loop is WalkLoopN or RunLoopN at play rate 1.0, swapped with a 0.2 s blend-in |

The debug text shows which mode each is in.

## 8. Outside stage 1

Everything movement spec section 7 lists (swim, glide, ledge grab, climbing, wall slide, battle
stance, attacks ...); `RollLand` (fall >= 800 cm with the stick: `Land` is played and
`Sora::notes()` says so once); `LightLandR/L`, `TurnN`, `RunSway*` are never requested; look-at
(an exact pass-through without a target). Bonamik is ported from the reference evaluator and
agrees with it bit for bit, but neither has been compared with the running game (animation R11:
start state at spawn, random draws, wind, ground); its bones are drawn interpolated between two
steps like every other bone, one step of display lag on top of its own one-evaluation lag. Bevy warns "Unknown vertex
attribute TEXCOORD_2" per mesh primitive (a third UV set it does not use).

No DLL host exists yet. For it: implement `World` and `IkWorld` over the host game's shape casts,
`Targets` over its enemies, `Audio` / `Effects` / `UiSink` over its sound, particles and HUD (or
leave those three null and act on `Output::events`), feed `Input` (sticks, jump, every pad button)
and `CameraInput` from its pad state, apply `Output`, `CameraView` and `Pose` to its character
and camera, and run the per-step sequence of section 3 from a fixed-step accumulator. Nothing in
the engine-free crates should need to change.

## 9. The ground for combat, magic, abilities, sound, UI (round 4)

Nothing of the specs beyond movement is implemented. What exists is the frame they are built
into: a state-machine host, the host seam (section 3), the status container, the generator
(section 5) and the test level's fixtures (section 4). NO GAMEPLAY RULE was invented for any of
it: no damage formula, no combo logic, no timing, no cost. Whoever implements a spec writes its
rules from the spec, citing its sections, as the movement code does.

### Adding a player state

`crates/kh-core/src/states/mod.rs` has the procedure: one file `states/<name>.rs` with a
plain-data struct (the fields of the `TresPlayerState_*` object) and `impl PlayerState` (`ID`,
`sub_step`, `begin`, `tick`, `end`, optionally `on_notifies`, `movement_state`), plus its request
functions (`TryX`); ONE ROW in `player_states!`; one call in `controller.rs` at the place the
game's dispatch order gives. A state reaches everything through `&mut Ctx` (`pawn.rs`):
`cx.pawn.cmc` (movement component), `cx.pawn.anim` (the slot), `cx.pawn.queue` (`set_state`,
`push_state`, `clear`, `do_finish`, `do_chain`, `do_restart`, `flags.enable_change`),
`cx.pawn.status`, `cx.pawn.input.pad` (buttons), `cx.world`, and the seam functions
`cx.play_sound`, `cx.stop_sound`, `cx.spawn_effect`, `cx.stop_effect`, `cx.deliver_hit`,
`cx.popup`, `cx.targets`, `cx.target`, `cx.hurt_shapes` (each makes the trait call AND records the
event). `run.rs` and `jump.rs` show the patterns: a state's own tick, a request that changes the
running state in place, one that queues a new state.

The queue is the game's (movement spec 1.3): `SetState` clears and pushes (the change happens at
the next `MainProc`, a state queued earlier in the frame is lost), `PushState` appends behind the
running state, `DoFinish` / `DoChain` end it, an empty queue yields Idle. The game's component has
no state stack. `CanRun` and the jump gate answer "no" for a state they have no row for
(`run.rs`, `jump.rs`): a new state's spec adds its row there.

What the animation side still lacks for new states: `AnimId` and the generated `sora_anim.rs`
list only the 14 stage-1 animations and three notifies (`TresJumpStart`, `TresEnableChange`,
`TresReverseLeg`). An implementer who plays more extends `tools/gen_port.js` (the `sora_anim.rs`
part), `anim.rs` (`AnimId`, `Notifies`) and the pack together; `kh-anim` already draws any clip of
the pack.

### Who owns what (so that work in parallel does not collide)

| owner (spec) | owns | shared files it touches, and how little |
|---|---|---|
| actions (`sora-actions-spec.md`) | `states/<each action>.rs`, `abilities/` for what they share | one row in `states/mod.rs`, one call in `controller.rs`, a row in `can_run` / the jump gate if its spec gives one |
| combat (`sora-combat-spec.md`) | `combat/` (targeting, hit detection, damage, combos), `states/attack*.rs`, `states/guard.rs`, `states/damage.rs` ... | the same; fields it needs in `host/targets.rs` (`HitRecord`, `TargetInfo`) |
| magic (`sora-magic-spec.md`) | `magic/` (spells, projectiles, shotlock, items), `states/fire_magic.rs` ... | the same |
| stats (`sora-stats-spec.md`) | `status/` (the rules that read and write `Status`; `loadout.rs`), `abilities/` passive hooks | `Sora` API for EXP / damage to Sora |
| audio (`sora-audio-spec.md`) | the sound table (a registered table sheet), the calls `cx.play_sound` in the states it lists, `kh-app/src/audio.rs` (`Bank::add_pack`, the attenuation curve) | `host/audio.rs` if `SoundPlay` needs more parameters |
| VFX / weapon (`sora-vfx-spec.md`) | the effect table, the calls `cx.spawn_effect`, `kh-app/src/markers.rs` -> real rendering | `host/effects.rs` likewise |
| UI (`sora-ui-spec.md`) | `ui_model/` (command menu state machine, popups), `kh-app/src/hud.rs` -> a real HUD drawn from `UiSnapshot` | `host/ui.rs` |
| the state-machine host, the seam's shape, `sora.rs` step order | the Rust implementer (this round) | changes need a line in this document |

Every implementer: register the sheets you compile against in `tools/lib/port_gen_defs.js`, add
the button names your spec fixes to `input.rs`, keep `tests/golden.rs` green (it pins stage 1 bit
for bit; if your change is MEANT to alter Idle / Run / Jump, update its constants and say so), put
your tests in your own file under `crates/kh-core/tests/`, and extend `tools/port_smoke.js` with a
scenario of your own rather than editing the existing ones.

### What is deliberately a stub

- `magic/`: an empty module (`combat/` and `abilities/` are being filled by their implementers).
- `Status`: NO LONGER A STUB (2026-10-11): the stats spec is implemented in `status/` (level from
  EXP, stats, victory bonuses, equipment, difficulty, EXP awards, the damage function, HP / MP /
  focus rules, prizes, items) and the numeric passive abilities in `status/` and
  `abilities/passive.rs`; the progress note `build/sora_tmp/IMPL_STATUS_STATE.md` has the public
  API and the table of which ability is whose. Still stubs there: meals (an input struct), the
  buff level, the code-menu options, Hidden Potential.
- `UiSnapshot`: the status HUD's numbers and popups are real (section 3, item 7); no command
  menu and no lock-on yet.
- The core calls `Targets`, `Audio` and `Effects` only for `DebugEmit`.
- Buttons other than jump reach the core (`Input::pad`) and do nothing.
- `kh-app`: the dummies' "damage" is `hp -= HitRecord::damage`; the effect markers are balls with
  labels; the sound bank holds two debug tones, `--sound` files and whatever a pack manifest's
  `sounds` list names (`Bank::add_pack` is to be fitted to the real audio pack); the attenuation
  curve is a placeholder; the HUD is a line of debug text.

### How round 4 was verified to change nothing

- `crates/kh-core/tests/golden.rs`: hashes of two 20 000-step pseudo-random sessions (game-exact
  and user options), taken from the round-3 code before the refactoring; unchanged after it.
- `build/sora_tmp/ground/traces.js` + `compare.js`: 16 `khgame.exe` traces (8 scripts, game-exact
  and user options, with and without the pack) from the round-3 exe
  (`build/sora_tmp/ground/khgame_round3.exe`) and from the new one: byte-identical.
- The bodies of Run and Jump were moved into `states/run.rs` / `jump.rs` textually by
  `build/sora_tmp/ground/split_states.js` (from `states_round3.rs` there), not retyped.
- Smoke scenario J: emissions through the seam, pad buttons and dummies leave a trace
  byte-identical to the same run without them.


## 10. Animation foundation and melee combat (round 5, combat implementer)

State note: `build/sora_tmp/IMPL_COMBAT_STATE.md`. Specs: `docs/sora-combat-spec.md` with the second-pass
strands `build/sora_tmp/combat/P2_C_section.md` (hits), `P2_D_section.md` (targeting), `P2_E_section.md`
(Sora being hit), which govern where they contradict the first pass.

### Animation tables (replaces the last bullet of section 5 and "what the animation side still lacks" of 9)

- `tools/sora_pack.js` 1.5.1 packs EVERY row of `p_ex001_AnimSet`: 427 names, 499 sequences (one
  `anims/<sequence>.glb` each; 46 `BlendSpace1D`, 6 two-axis `BlendSpace`), all notifies with nested object
  references resolved; `manifest.attack_sockets` = the Keyblade's `ATK_00` / `ATK_01` per animation frame in
  actor space for the 99 clips with a hit window. `sora_all.glb`, the KineDriver baked-track check and the
  per-frame root tracks of clips without `bEnableRootMotion` stay limited to the stage-1 clips.
- `tools/gen_port.js` 3.1.0 -> `sora_anim.rs`: `enum AnimId` (427; the 14 stage-1 values pinned 0..13, a
  leading digit gets the prefix `A`: `AnimId::A1301`), `ANIMS` / `SEQUENCES` (length, frames, root track with
  Z where `bEnableRootMotion`, sockets), `enum NotifyClass` (47 classes present), `NotifyData` (decoded
  properties: attack windows, IK, sounds, voices, effects, face ...), the raw remainder as (name, JSON text),
  `BlendSpace2d` grids, `NOTIFY_ASSETS`.
- `kh-core/src/anim.rs`: `AnimPlayer::play` (stage 1, unchanged) and `play_with(id, PlayArgs::mode(bits,
  blend_in))` for every other state; one pose tick returns `Notifies { jump_start, enable_change, events }`
  with `events` = every notify in dispatch order (instant; state End / Begin / Tick; the 1e-4 window shift;
  `SQEX_AllAnimation`). The running state gets them in `on_notifies(&Notifies)`; `combat/hit.rs` opens and
  closes hit windows; `notify.rs` turns sound / voice / effect notifies into seam events
  (`NOTIFY_ID_BASE` + index in `NOTIFY_ASSETS`; a state notify stops what it started at its End).
- `kh-anim` reads the clips beyond stage 1 on first use. `kh-app`: `play-anim NAME` (debug word), the
  Keyblade glb on `R_wep_skt`.

### Melee combat

| file | what |
|---|---|
| `combat/tables.rs` | generated tables (`sora-combat-attacks / -hits / -collisions / -attack-data / -rules`, params) |
| `combat/decide.rs` | `Decide` 0x14134ACA0, `MaxCombo`, `Finishers`, the ability upgrades (pure; spec 4, 15) |
| `combat/targeting.rs` | `UpdateLockon`: search along his facing, soft target, pin, RB tap lock, RT tap switch (strand D) |
| `combat/hit.rs` | hit windows, shapes from the socket tracks, sampled sweep, hit record, damage call, effect / sound events (strand C) |
| `states/attack.rs` | `TresPlayerState_Attack` (12): `CanAttack`, `TryAttack`, Begin / PreDecide / PlayAttack / Tick / End, facing, traction (XY, root scale, Z) |
| `states/damage.rs`, `die.rs` | victim side of `TakeDamage`, reaction gate, `TresPlayerState_Damage` (19): Small / Blow / Toss / Launch / Parabola / Beat; `TresPlayerState_Die` (59) (strand E) |
| `cmc.rs` | direct velocity records (`set_velocity_xy / _z`), `gravity_off`, `MovementMode::Flying` |

Frame order of a step now: C (tap timers -> `UpdateLockon` -> attack request -> jump -> stick) -> hits on Sora
delivered by the host (`Sora::take_hit`) -> M (pose tick: notifies -> state, hit windows, generic handlers;
movement) -> hit windows sweep (`combat::hit::tick`) -> S (`MainProc`).

Host seam additions: `TargetFlags::{ENEMY, GIMMICK, PRIORITY_HIGH, RUSH_SUPPRESSION}`,
`Targets::lock_on_range` (default 0), `HitRecord::info: HitInfo` (the rest of `TresDamageInfo`, victim /
attacker result), events `LockCursor`, `CameraLock`, `WeaponVisible`, `PlayerHit`, `PlayerDied`;
`Sora::{take_hit, attack, damage_reaction, combat, debug_play_anim}`. The attack button is
`PadButton::ATTACK` (bottom face button), lock-on RB (`RightShoulder`), target change RT (`RightTrigger`).

App words: `play-anim NAME`, `warp X Y [YAW]` (metres, core axes), `dummy-attack N KIND POWER`. Smoke scenario
L = the level-99 crowd combo. Tests: `crates/kh-core/tests/combat.rs`, `damaged.rs`, unit tests of `decide.rs`.

Assumptions and literal readings (all in the code's comments; to confirm with the runtime probe): the direct
velocity record replaces the movement's velocity for one tick; Flying = move by velocity + root motion; the
root scale is applied per tick; `End` of the attack restores everything unconditionally; pawn+0xE48 bit 5
("finish end") is cleared when a combo-0 attack begins; hit windows tick after the movement of the frame; the
sweep is sampled; every target is a DEFAULT body (no guard / armour / clash / wall hit); the combo step of the
damage formula (state+0x130) stays 0 in base form by the literal reading of the request path.

## 11. Sound (audio implementer)

State note: `build/sora_tmp/IMPL_AUDIO_STATE.md`. Spec: `docs/sora-audio-spec.md`.

- `crates/kh-audio` (engine-free: glam, std, `kh_anim::json`): the SEAD playback model as data and logic. `Pack`
  reads `build/sora_pack/audio/manifest.json` and `tables.json` (`tools/sora_audio_tables.js`: category events,
  static SE table); `Driver` = what plays when a sound index is started (selection by sound type, sequences,
  tracks, volume / pitch products, start macros, category pools with stealing, category events, 3D) and returns
  VOICE COMMANDS per step; `AutoSe` = the bone-motion detector (footsteps, jump / landing, cloth), run at the
  simulation step; `Routes` = which sound a seam NAME means; `Engine` = all of it for a host; `Mixer` = a
  software backend (device output and offline render). Constants: `sheets/port/sora-sound-rules` (registered).
- THE SIMULATION DOES NOT DEPEND ON IT: `kh-core` has no dependency on `kh-audio`, the engine is fed from the
  outputs of the steps, and its random choices use its own host-seeded generators (`--audio-seed`).
- Seam addition (section 3, item 3): `World::surface_type(&Hit) -> u8` with default 0: the `SurfaceType` of the
  physical material of a hit. The core never calls it; the host's audio side asks it about its own floor probe
  under him (the footstep bank is chosen by surface). `kh-app`: one default surface plus painted patches
  (`surfaces.rs`).
- A host feeds the engine per step: seam sounds by name (`Engine::seam_play` / `seam_stop`; unknown names are
  logged once), effect spawns (`effect_spawned`: sounds carried by particle systems), the 14 bones of
  `kh_audio::autose::BONES` in world space with the grounded flag and the shown animation (`auto_se`), his HP for
  the low-HP alarm unless its HUD plays static SE 9 itself, UI static SEs; then `Engine::update(dt, listener,
  resolve_anchor)` and plays the voice commands. The listener is the camera (the spec's assumption).
- `kh-app`: `audio.rs` (rodio carries the mixer's frames), `--voice-language en|ja`, `--audio-log`,
  `--audio-render`, `--audio-do`, the F12 audio page (`audio_page.rs`). Smoke scenario S.

Second pass (2026-10-11, coverage: `build/sora_tmp/impl_audio/SOUND_COVERAGE.md`, regenerate with
`khgame.exe --audio-coverage buildsora_tmpimpl_audiocoverage_notifies.jsonl` then
`node build/sora_tmp/impl_audio/coverage.js`). SEAM ADDITIONS, all marked "(audio implementer)" in the code:

- THE VOICE GROUP NUMBER (pawn+0x4D8) is carried: `Ctx::set_voice_group_no(n)` -> `Audio::voice_group_no(n)` (a
  default method: other hosts need not implement it) and `EventKind::VoiceGroupNo`. It is reset to 0 at every state
  begin (`states/host.rs`), set by the attack state from its kind (`host::audio::attack_voice_group_no`), by the
  magic states from the spell's voice index, and to 1 by the death state. A voice notify that names a group plays
  the unit with that number; `"group:N"` as a seam name names the unit directly.
- `SQEXSEADAnimNotifyState_AttachSound`'s `LoopEndSound` is emitted at the notify's End (`notify.rs`); a
  `PlayWeaponSwingSE` without `m_bFollow` is a fixed point at the mesh location, not a sound on the weapon.
- A SPELL / SHOT ACTOR is named by its blueprint class on both seams: the effect seam's spawn of the class plays the
  class's fire SE and the sounds of its fire and body particle systems (they end with the actor); its hit on a pawn /
  on the level is a sound whose NAME is the class and whose id is `host::audio::ACTOR_PAWN_HIT` / `ACTOR_MAP_HIT`
  (tables.json `projectiles`, built from the audio and VFX packs by `tools/sora_audio_tables.js`).
- A collected prize plays `"<PrizeClass>.get"` (`status/requests.rs`); `".bound"` is there for a host's orbs.
- The heal of Cure and of an item spawns the heal effect of its group (`host::audio::cure_heal_effect`,
  `item_heal_effect`; the item mapping is inferred from the asset names) instead of a placeholder name.
- `kh-app`: sockets (`R_wep_skt`, `vfx_root` ...) are placed through their parent bone's transform and the socket's
  relative location (tables.json `sockets`); `Seam::step_events` holds the events of every step of a frame for the
  audio side; the engine pause of the HUD's menus sends STATE_MENU_OPEN / STATE_MENU_CLOSE and the audio engine keeps
  running on the frame clock while the simulation stands, so menu sounds play.
- NOT RENDERED: the reverb send (the spec gives the send level per sequence, not one parameter of the reverb on bus
  "Revibe"); the low-pass keeps the first pass's provisional factor -> cut-off mapping (spec open question 3).

## 12. Action states, tier 1 (actions implementer; docs/sora-actions-spec.md Part II)

Implemented: Dodge Roll (16), Guard (15) with the guarded hit, Air Slide / Aerial Dodge / Aerial
Recovery (20), Glide (4), the jump types (High Jump, Doubleflight, the super jump's entry point),
RollLand and the land-pose landings, the battle stance and the Keyblade's draw / put-away.

- Files: `crates/kh-core/src/actions.rs` (the pawn's fields of these states `PawnActions`, `Draw` /
  `Sheathe`, the per-frame part `pawn_tick`, the pose notifies, the counter window, name selectors;
  constants generated from `sheets/port/sora-actions.json` into `sora_actions.rs`),
  `states/{dodge_roll,guard,air_slide,glide}.rs`, the type / landing parts of `states/jump.rs`, the
  stance parts of `states/idle.rs` and `states/run.rs`; tests `crates/kh-core/tests/actions.rs`.
- Pad (the game's default layout): X = guard with the stick neutral, dodge roll with it pushed, air
  slide in the air with it pushed (`PadButton::GUARD`); B = jump, second press in the air =
  Doubleflight, held while falling = glide. Keyboard extra: right Shift / right mouse = X, Space = B.
  Controller order (`controller.rs`): guard button (air slide -> guard / dodge), jump press, jump
  held (glide), stick.
- Host seam: `Targets::engaged_count()` (default 0) drives the battle flag (+ 1.0 s linger in the
  core); events `BattleMode`, `PlayerDodged`, `PlayerGuarded`, and `WeaponVisible` + the effect
  `VW_SO010_APP0` for the Keyblade (the host runs the 0.22 s delay and the 0.06 s fade:
  `kh-app/src/pack.rs weapon_fade`). What a hit on Sora meets is classified by the combat side
  from `PawnActions::reaction` (11 = the hit does not exist) and `guard_parts`.
- Test level rule: a living dummy is "engaged" while Sora is within 400 cm of it or for 3 s after
  he hit it (`fixtures.rs ENGAGE_RANGE`, `ENGAGE_AFTER_HIT_STEPS`).
- Port option added: `Tuning::glide_ends_on_ground` (`--glide-ends-on-ground`), default OFF: by
  the code as read nothing ends a glide but releasing the button (to confirm in the game).
- `Tuning::jump_cut_on_release` covers the normal jump and the High Jump; a Doubleflight keeps the
  game's rule (no height cut). With speed-matched locomotion on, the `W` / `B` run sets (Keyblade
  out) use the game's own playback.
- Stage-1 traces: byte-identical to the round-3 baseline when run without the jump abilities
  (`build/sora_tmp/impl_actions/traces_extra.js`); with the level-99 loadout every ground jump is
  a High Jump (resolver 0x141434010): new baseline `build/sora_tmp/impl_actions/traces_default/`.
- Scripted runs: `node build/sora_tmp/impl_actions/runs.js`.

### 12a. Risk Dodge, tier 2 (flowmotion, terrain) and tier 3 (second actions implementer, 2026-10-11)

Implemented: Risk Dodge (17, four types) with the Reflect state's 0.6 s window; Wall Run (8), Pole Turn (24, spin
and swing), Rail Slide (22, level rails and the Blizzard spell's ice rails), Dangle (6), Slope Slide (10), Swaying
(11), Hopping (9); Floating (5). Wall Kick and Superslide gained their wall-run / pole / rail entries.

- Files: `crates/kh-core/src/actions/probes.rs` (the world probes: `WallValid`, `WallPoint`, pole, rail, hop, ledge,
  slope), `states/{risk_dodge, wall_run, pole_turn, rail_slide, dangle, slope_slide, swaying, hopping, floating}.rs`;
  tests `crates/kh-core/tests/actions2.rs`. Hooks in the stage-1 states (each a marked block at the head of the
  tick, without effect in a world that has none of the affordances; `tests/golden.rs` unchanged): Idle (Begin: hop
  point; tick: rail, slide floor, floating), Run (rail, slide floor, hop point, the bump into a pawn; `CanRun` rows
  for SlopeSlide / Swaying), Jump (rail, ledge, hop point, slide floor on landing, floating; gate rows).
- CHANGES TO THE HOST SEAM (section 3): `World` gained, all with defaults that mean "a level without it":
  `surface_flags(component) -> u32` (`kh_core::surface::{DANGLE, WALL_RUN, WALL_KICK, AUTO_RUN, SLOPE_SLIDE,
  FORCE_SLOPE_SLIDE}`: the `SQEX_bEnable*` bits of the collision component a `Hit` reports; default = the engine's
  mesh default: hangable and kickable), `poles() -> &[Pole]`, `rails() -> &[Rail]`, `hop_points() -> &[HopPoint]`
  (registries: the game walks lists, it does not query collision), `water_surface(position)` (for Swim, not built).
  Rails and hop points are laid where his FEET go (the pawn's origin). `Sora::offer_risk_dodge(attacker, type,
  seconds)` (an enemy attack's `DamageCaution` warning), `Sora::set_floating(on, special)`, `Sora::actions_mut()`
  (`flow_unlocked`, `traversal.wall_run_unlocked`). Events: `RiskDodgePrompt { shown, used }`, `Traversal { kind,
  id, on }`. Sway asks `Targets` for a living pawn whose body touches his.
- CHANGE TO THE MOVEMENT COMPONENT: `MovementMode::Custom` (`MOVE_Custom` 6: no physics at all), used for the
  ledge hang (the game's own use) and for the states that place the actor themselves: rail, pole, hop point.
- CHANGE TO THE ANIMATION SLOT (coordinator decision): the user option `speed_matched_anim` covers the `W` / `B`
  clip sets (`anim::LocoSet`); a blend space whose samples differ in length (`RunStartB`, `RailSlideLoop`) plays on
  a normalised clock (`AnimRequest::length`, `body_samples()`).
- Pad: X in a wall run = wall kick (ends in a Superslide), on a pole / rail = Superslide; B in a wall run / on a
  pole / on a rail = super jump, hanging = drop, on a hop point / slide = jump; B while the risk-dodge prompt shows =
  Risk Dodge; left stick: wall-run direction, turn-around on a rail (against the travel direction), climb (toward
  the wall) or let go (away) on a ledge, next hop point (above 0.85).
- Test level: the traversal yard (`kh-app/src/level.rs` header: one instance of each affordance at z >= 14 m), the
  bystander pawn (dummy 6), script words `dummy-risk N TYPE`, `float 0/1`, `warp X Y YAW HEIGHT`; key F12 toggles
  floating. The stage-1 solids are kickable only (not hangable), so the stage-1 lanes behave as before.
- Scripted runs: `node build/sora_tmp/impl_actions/runs2.js`. Not built: Swim, Climbing (dead in retail), the
  interaction stubs; simplifications are listed in `build/sora_tmp/IMPL_ACTIONS_STATE.md` and at the head of each
  state file. Ability audit: `build/sora_tmp/impl_actions/ABILITY_AUDIT.md`.


## 13. Magic, items, the command model, shotlock, airstep (magic implementer)

State note: `build/sora_tmp/IMPL_MAGIC_STATE.md`. Spec: `docs/sora-magic-spec.md` (parts A - K; later parts govern).
Tests: `crates/kh-core/tests/magic.rs` (part K numbers), `crates/kh-camera/tests/aim.rs`.

| file | what |
|---|---|
| `tools/lib/port_gen_defs_magic.js` | the five `sora-magic-*` sheets registered as LOOKUP TABLES (`values: true` of `tools/gen_port.js`): `(id, value)` sorted by id; their 87 null rows are listed informational with what the code does instead |
| `magic/tables.rs` | `Sheet::{Spells, Proj, Mp, Shot, Items}.num / flag / text / vec3 (id)` |
| `magic/spells.rs` | magic-set units by command id, MP costs, attack rows (`MAG_*`, `SF_KCSHOT`, `SF_KCRAGNAROK`) |
| `magic/projectile.rs` | every spell actor and shotlock projectile as data (`StageDef` = one blueprint of the sheet): motion, homing, root sweep, growing shapes, hit list, respawn chain, Blizzard rail, Aero counter and pad, Water barrier / bolt, Cure, Ragnarok helix |
| `magic/items.rs` | the 8-slot pouch (exists / fixed / auto-reload / used), stock, refill, the 0.5 s delayed item effect |
| `magic/shotlock.rs` | the aim mode (controller slot 800): entry, locks, focus, MAX, fire / airstep / cancel, airstep target choice |
| `magic/grand.rs` | grand magic: element tallies on the style gauge, the roll, lifetimes (for the queue's owner) |
| `command/mod.rs` | the command system: menu state machine on the pad, shortcuts, `CanExecuteCommand` / `ExecuteCommand`, command timers, `CommandSnapshot` |
| `states/fire_magic.rs` (18), `use_item.rs` (111), `shoot_flow.rs` (41), `athletic_flow.rs` (42) | the states with their requests |
| `kh-camera/src/aim.rs` | `AimCamera` (`ATresCameraShootFlow`) and `AimOverlay` (the 0.5 s ease-out blend over the camera below) |
| `kh-app/src/magic_host.rs` | the reticle's view, the aim camera overlay, the level's airstep point |

Frame order additions: C: command timers, then the aim mode (while it is open the normal dispatch is skipped); L1 held
or an open sub-list masks the jump press, L1 held masks the guard press. After the hit windows: projectiles (real dt),
Aero pads, queued item effects. U: situation queue (combat), then the command menu (`command::ui_tick`): the Attack row's
decide is `ExecuteCommand(27)`. Sora's own step runs at `time table[PLAYER]`, the controller's timers at
`time table[VEHICLE]` (host/time.rs; both 1.0 outside the aim mode / airstep / menu slow option).

Host seam additions: `Input::view: Option<magic::View>` (camera of the last frame: position, axes, FOV, rectangle; the
shotlock reticle test); `Targets::allies` / `heal_ally` (default: no party); events `AimMode { open, reason }`,
`LockMarker { target, locks }`, `SoundState { name }`; `UiSnapshot::command: CommandSnapshot` (mirrors
`kh_ui::CommandModel`); `Sora::{magic, magic_mut, aim, cast, command, command_mut, command_request, on_unpause,
dilation}`; `PawnMagic::{projectile_views, rails, pads, airstep_points, pouch, shotlock_unlocked, airstep_unlocked}`.
A spell actor is one persistent effect event (name = its blueprint class) whose position the host reads from
`Sora::magic().projectile_views()` by effect handle; hits arrive through `Targets::apply_hit` with
`HitRecord::attack_name` = the row and `flags` = command id | shotlock kind << 8; `HitInfo::bad_status` carries FREEZE /
DISCHARGE (already rolled) and `damage_kind` SWIRL_UP / BLOW for the host's reaction.

Pad: D-pad up / down = menu cursor (wraps), A = decide, B = cancel, D-pad right / left = open / back, L1 held + B / Y / X
/ A = shortcut slots 0..3, L1 + D-pad down / up = shortcut page, RB held 0.2667 s = aim mode (right stick, or left stick
while the right one rests, turns the aim camera), A = fire, X = airstep, RB released = cancel. Script words `press /
release / tap / hold BUTTON` and `cam` go through the same path; `cast NAME` and `refill` are direct test words.

### Second pass (2026-10-11, second magic implementer; tests `crates/kh-core/tests/magic2.rs`)

- **Gates** (magic spec A 2.2, A 2.3, A 5.4, E I2): `CanMagic` / `CanUseItem` have the rows of GLIDE (always; the cast is
  an air cast with the move flag forced), GUARD (only out of a guarded hit, from its `TresEnableInput`), DODGEROLL and
  AIR_SLIDE (`m_bEnableChange`), BADSTAT_STUN; SUPER_SLIDE, WALL_KICK, REFLECT and the form states refuse. The reverse
  rows (guard / dodge roll / air slide out of a cast or an item) sit in `states/{guard,dodge_roll,air_slide}.rs`, marked.
- **Grand magic** (I 7): `magic/grand.rs` tallies what each spell's hits add to the formchange gauge per element
  (`on_damage` -> `combat::style::add`), `combat/style.rs roll_grand_magic` calls `grand::roll` at a full gauge (before the
  formchange offer) and enqueues the `S_*` commands, `combat/situation.rs grand_magic` executes one through
  `command::execute` (no MP, the gauge and tallies cleared). Pad: Y uses the selected situation command, LT steps to the
  next. MAGIC_ROULETTE's pre-roll and the Curagan roll are in (neither ability is in the level-99 loadout).
- **Airstep** (H 4.4 - 4.7, actions II.5): intermediate hops fly with world collision off (`Cmc::world_collision_off`),
  the warp sweep of intermediate target points, the landing fix of a cut hop, the kick stop at clip time 0.2
  (`AnimPlayer::set_time`), the enemy stop's order and attack gate, and the FLOW ATTACK: `attack::flow_attack` (forced kind
  0x36) with the flowmotion attack choice of Attack slot 124 (`Attack::flow_refine`: Sonic Slash 0x36 / Sonic Cleave 0x37 /
  Buzz Saw 0x38 by SONIC_SLASH / SONIC_DOWN / TURN_CUTTER; rows `kind_36 .. kind_38` and `FF_SONIC_*` come from
  `tools/sora_combat_sheet.js`). Pad: RB held, X = airstep, A after the arrival at an enemy = flow attack. Stubs left: pole
  stop / `TryPoleTurn`, ledge -> DANGLE (no such states), the enemy-turn swing (ETURN enemies).
- **Camp menu writes**: `kh_core::command::customize` (the table at the top of `build/sora_tmp/IMPL_MAGIC_STATE.md`).
- **Shortcut rows of the loadout**: authored in `sheets/port/sora-shortcuts.json`; `tools/sora_stats_sheet.js` copies them
  into `sora-loadout-99` on every run (that tool has no dry run: any call without `--check` rewrites its sheets).

## 14. Combat, second pass (combat implementer; docs/sora-combat-spec.md revision 2)

State note: `build/sora_tmp/IMPL_COMBAT_STATE.md` (the dated section of 2026-10-11 evening). Tests:
`crates/kh-core/tests/combat2.rs`, unit tests in `combat/{classify,guard,situation,push,decide}.rs`,
`crates/kh-camera/tests/lockon.rs`.

| file | what | spec |
|---|---|---|
| `combat/classify.rs` | the 33-row guard / armour / body classification, used for Sora's hits and for hits on him | 8.2.6 |
| `combat/hit.rs` | classification per hit, effect choice by result, map hits (wall spark `VW_SO010_WHT0`), the clash pass, the on-contact handler with the recoil record | 8.2.4 - 8.2.6, 8.5, 8.6 |
| `combat/guard.rs` | the hook with the actions side: Sora's body from `actions.reaction` / `guard_parts`, guarded hits -> `OnGuardedHit`, forced counter kinds, the prompts mirrored into the situation slots | 5.6, 8.4, 9.5 |
| `states/reflect.rs` | `Reflect` (66), the weapon recoil (its Risk Dodge parts are the actions implementer's) | 5.6.3, actions II.1 |
| `states/damage.rs` | classification first, the wall hit of Blow / Parabola, Beat's slide, the auto-revive, the reaction block; `states/badstat_{stun,freeze,burn}.rs` are named stubs | 8.4, 9 |
| `combat/push.rs`, `cmc.rs` (`push_move`, `last_block`) | the soft body push (decompiled C only) | 6.5 |
| `combat/situation.rs` | the situation-command queue: list of 6, slots A / B, timers, L2, Triangle, the executor | 11 |
| `combat/style.rs`, `combat/form.rs` | the formchange gauge, `AwardAtMax`, `StartStyle`, finish levels, `EndStyle`, the wear form with its abilities | 10.2 - 10.7 |
| `states/style_change.rs`, `combo_limit.rs`, `style_finish.rs` | STYLECHANGE (31), COMBO_LIMIT (90), STYLEFINISH (33) with Stun Impact, Sonic Rave, Last Arcanum | 10.4 - 10.6 |
| `kh-camera/src/lockon.rs`, `rig.rs` | the lock-on camera and the 0.5 s blends (`CameraRig` is what a host steps) | 7.12 |

Host seam additions (section 3 applies to them): `Targets::{body, take_armor, take_reaction_power,
push_body, attack_shapes, touched}` (all defaulted: a host that implements none has DEFAULT bodies
without push bodies), `HitRecord::takes_hp()` (results GUARD / REFLECT / ARMOR_BREAK take no HP: a
host's `apply_hit` asks it), `HitInfo::{reflect_reaction, armor_left}`, `HitOnPlayer` row facts
(`ignore_guard`, `weak_guard`, `rapid_fire`, `reflect_level`, `reflect_reaction`, `bad_status`,
`bad_time`), `host/time.rs` (THE per-object-type time dilation: `EventKind::TimeDilation`,
`Sora::time_dilation`), `UiSnapshot::{lock_on, situation}` (`ui_model/situation.rs`), events
`SituationSlot / SituationRemoved / SituationSelect / FormBegin / FormEnd / WearForm / TimeDilation /
PlayerGuard / WallHit / Clash / AutoRevived`. Pad: situation command = Y (`PadButton::SITUATION`), next
entry = left trigger (`SITUATION_NEXT`), lock-on RB tap, target change RT tap, launch = B during a
combo, counters = A in their windows; all through `Input::pad`.

App words: `dummy-guard N 0/1`, `dummy-armor N POINTS`, `dummy-body N TYPE`, `dummy-push N LEVEL`,
`dummy-auto N KIND POWER PERIOD [REFLECT]`, `fill-style POINTS`. Asset pack: `tools/sora_pack.js`
1.6.0 packs the 91 face rows the body clips' face notifies can start (`tools/lib/face_type_rows.json`).

(The list of what was unread at the end of the second pass - the lock-on camera's framing solver,
obstruction blend and velocity lead, the map-hit handler and recoil, the clash sort and effect, the
`DOWNIMPACT` shot, the GUARD_REFLECT guard reaction, the bad-status states - is closed by the section "Combat, third pass" below.)


## 15. Effect rendering (VFX implementer; docs/sora-vfx-spec.md 8, 11)

The effects seam is drawn for real. `crates/kh-vfx` (engine-free: glam + serde_json) evaluates the
effect pack's particle systems in KH3 space at the fixed step: emitter timeline (start / intro /
loop / ending), node transforms and children of the Square Enix modules, the stock Cascade modules,
sprite / mesh / AnimTrail / tail / decal emitters, a deterministic random stream. Host API:
`World::register(EffectDef::parse(json))`, `spawn(name, Xform, SpawnOptions) -> EffectId`,
`set_transform`, `set_trail(first, second)` (the Keyblade trail's two sockets, once per step),
`stop(id, immediate)` (= `EnableLoopEnd` or destroy), `fade_out(id, seconds)`, `step(dt)`,
`draw(alpha, &Camera, &mut DrawList)` -> quads (four corners, UV rect, colour, material id),
ribbons, mesh instances (mesh id, transform from mesh cm to world cm, colour). A module the
evaluator does not know is skipped with one log line (`take_log`), never a panic. What is
evaluated / approximated / skipped per module: `sheets/port/sora-vfx-modules.json` (note column,
"kh-vfx: ..."); per effect: `build/sora_tmp/vfx/RENDER_COVERAGE.md`.

`crates/kh-app/src/vfx.rs` is the host side: effect names of the seam -> pack effects (a notify
path's base name; a spell actor's blueprint class through `PACK/vfx/host.json`, written by
`node tools/sora_vfx_host.js`: sockets of his skeleton, the Keyblade's sockets, class -> body /
launch effects), anchors from the pose (`Pack::bone_world`), placement rows from
`PACK/vfx/triggers.json`, materials composed on the CPU from the pack's textures into
`StandardMaterial`s (unlit, additive / blend / multiply), one dynamic mesh per material, drawn
interpolated with `Sim::alpha`. Everything loads when an effect is first spawned.

Options: `--effect-markers` (the stand-in balls in addition), `--effect-test "NAME;NAME@X,Y,Z"`
(spawn pack effects by name half a second in), environment `KH_VFX_DEBUG=1` (draw-list counts).
Probe without the app: `KH_VFX_PROBE=<effect> cargo test -p kh-vfx --test probe -- --nocapture`.

Seam changes: none to the traits. Core edit (notify.rs, marked): a state-notify entry of common
attach data on the weapon emits the persistent effect `key_trail0` (the Keyblade trail,
`VW_SO010_LOC0`); before, nothing was emitted for it. Open on the core side: the effect-group rule
(the host drops other keyblades' `vw_soNNN_*` rows by name), hit normals (hit effects are spawned
unrotated), attach rows' offsets (the host re-reads them from the trigger table by effect + socket).

### 15a. Second pass (VFX implementer 2, 2026-10-12)

What changed since the text above (which stays as the description of the evaluator's API):

- **Materials are the game's own pixel shaders.** `node tools/sora_vfx_shaders.js` reads the compiled shader maps of the
  cooked materials (DXBC, disassembled with d3dcompiler through `tools/lib/dxbc_disasm.ps1`) and translates the base pass of
  each shader owner to WGSL: `PACK/vfx/shaders/materials.wgsl` + `shading.json` (constant buffer, texture slots, blend mode
  per material). 551 of 657 shaders translate; a material whose owner does not gets the generic texture x colour shader.
  The app side is `crates/kh-app/src/vfx_material.rs` (a custom Bevy material, submodule of vfx.rs) with the harness
  `vfx_material.wgsl`. The `StandardMaterial` composition of the first pass is gone. Not drawn: distortion / refraction
  (no scene colour texture).
- **Materials are named by package file**, not by base name: 17 base names exist twice in the pack (the Firaga hue error:
  the fire bolt was drawn with the water material of the same name).
- **Camera**: HDR, depth prepass and bloom (threshold 3, so the white floor does not bloom) are set by `setup_camera`;
  effects apply the ACES curve in their shader, the scene keeps its tone mapper, the sky colour is pre-compensated.
  `KH_VFX_HDR=0` gives the camera of before. Measured scene change with HDR on: mean 0.98 / 255.
- **Evaluator**: tails traced (points along the node's path, ribbon or sprites), nested systems
  (`SQEX_SpawnParticleSystem`), sub meshes, particle lights (`DrawList.lights`, the app draws 6), ground snapping
  (`World::set_ground`), projected decals, `EmitterRenderMode`, `EmitterBlendType`, TypeSpecific sprites as X x Y.
- **Host**: everything of the pack that Sora can spawn is loaded on a background thread at start (`KH_VFX_PRELOAD=0` turns
  it off); Blizzard's ice rail is drawn from the actor's meshes (`host.json` `actors`).
- **Core (marked "vfx implementer 2")**: `notify.rs` `effect_group_passes` + `pawn.effect_group` /
  `next_effect_group` (a state sets the group of its next play; WallKick sets 0x139); `combat/hit.rs`
  `normal_rotation`: hit effects carry the rotation of the hit normal.
- **Regenerate** (in this order; the pack run deletes host.json): `node tools/sora_vfx_pack.js`,
  `node tools/sora_vfx_host.js`, `node tools/sora_vfx_shaders.js`, `node tools/sora_vfx_assets_rows.js` (the vfx rows of
  `sheets/port/sora-assets.json`, including host.json and the shaders), then extract + preflight.
- **Still open**: parent half of the transform chain and child-emitter looping (not traced), Ring, Quadrangle, CurlNoise,
  LocalParticle, ParticleScale, Range deviation, VertexSpawn, particle collision, tail twist / wave / inertia; camera
  shake, radial blur, scene colour change and filter (need a host post-effect hook); spline bending of the ice rail.
  Per effect: `build/sora_tmp/vfx/RENDER_COVERAGE.md`.

### 15b. Third pass (VFX implementer 3, 2026-10-12)

State note: `build/sora_tmp/IMPL_VFX_STATE.md` (latest section). Scratch: `build/sora_tmp/impl_vfx3/`.

- **The node chain is the traced one.** Emitter instance vtable: +0x250 `0x1442A0CA0` (component), +0x398 `0x1442A3E80`
  (scales), +0x248 `0x144290A60` (position / rotation with the parent), +0x2D8 `0x1442881B0` (a spawn point into the
  simulation space). The chain is rigid (rotation + translation); scales are vectors kept beside it: component scale
  (cut by `SQEX_ComponentCutoff`), chain scale (init x track x parent chain x child offset), world scale = component x
  chain x `SQEXBaseScale`, and the particle size multiplier that only `SizeScale` / `InitSizeScale` / `ParticleScale`
  apply. `bUseLocalSpace` = the component's space, + `bUseEmitterLocalSpace` = the node's own (`sim.rs` `Space`).
  The functions the older notes named as the "parent half" (0x1442A57C0, 0x144280490) are not part of it.
- **Spawn shapes**: Range's deviation (0x144352710, was inverted), Ring (a polygon in the XY plane about Z, was about X;
  Interval, second radius, arc), Quadrangle (faces of a box of half extents), the velocity rule (0x14435B130), mesh
  particles turned by the spawn direction. **Noise**: `SQEX_CurlNoise` is three samples of Perlin's improved noise
  (`kh_vfx::perlin`, reference permutation) added to the velocity. **LocalParticle**, **decal geometry**
  (0x144281110), stock Size / SizeScale / Location / Velocity bodies as read.
- **GPU sprite emitters** hold as many particles as `EmitterInfo.MaxParticleCount` of their type data says (the hard
  capacity of the game's GPU simulation) and draw all of them; before: 1000 simulated, `MaxDrawCount` drawn.
  `KH_VFX_GPU_CAP=n` restores a cap for comparisons.
- **Camera shake**: `World::take_shakes()` -> `kh_camera::shake::ShakeSet` (stock `UCameraShake` oscillators, class
  defaults in `host.json` `camera_shakes`) -> `camera.rs place` adds `Vfx::shake_offset()` to the drawn view (marked).
- **Core (marked "vfx implementer 3")**: `combat/form.rs end_finish_effects` + two calls in `states/style_finish.rs`
  (the `EndEffect` calls of a finish command's end; test in `tests/vfx_seam.rs`).
- **Frame cost** (`KH_VFX_PROFILE=1` prints the split; King of Hearts volley with 23 locks): quads and mesh instances
  are written straight into the arrays their batch's Mesh asset already holds, in parallel on the engine's task pool;
  systems step on scoped threads above 4000 particles. `KH_VFX_MESH_REF=1` = the plain path, for comparisons.
- **Tools**: `tools/sora_vfx_shaders.js` (texture indices of materials that call an /Engine/ material function; loops
  and integer counters; a scene-colour read in a loop is refused), `tools/sora_vfx_host.js` 1.3.0 (`camera_shakes`).
  Regenerate order unchanged (pack -> host -> shaders -> assets rows).
- **Still open**: whether a child emitter loops (`SQEX_Parent`), how many particles `EVARTEX_ODER_TYPE_ALL` spawns,
  `DisplayType` billboards of nodes, tail twist / wave / inertia, sub-mesh offsets, `VertexSpawn`, `SphereForce`,
  collision, radial blur / scene colour / filter, distortion (no scene colour copy), the six shader shapes Sora's
  nine fallback effects need, the ice rail's bending and end pieces.

## 16. Combat, third pass (third combat implementer, 2026-10-12)

State note: `build/sora_tmp/IMPL_COMBAT_STATE.md` (section "2026-10-12 THIRD COMBAT IMPLEMENTER"). Spec:
`docs/sora-combat-spec.md` 16.8 lists what changed. Tests: `crates/kh-core/tests/combat3.rs` (20),
`crates/kh-camera/tests/lockon.rs` (7). Ability audit: `build/sora_tmp/impl_combat/ABILITY_AUDIT.md`.

| file | what | spec |
|---|---|---|
| `combat/badstat.rs` | the bad status on Sora: status byte, timer, apply / clear / tick, immunity, the frozen flag, STOP / HONEY (his own time dilation), HP_BREAK (max-HP cap), TIME_BREAK, the stun's stick flick, the effects (rows of `ead_com_abn0` by particle) | 9.9.1 |
| `states/badstat_stun.rs`, `badstat_freeze.rs`, `badstat_burn.rs` | states 122 / 123 / 124 with their real bodies | 9.9.2 - 9.9.4 |
| `states/attack.rs` | the sneeze (kind 0x1E); the shot handler (`DOWNIMPACT`, `SHOCKWAVE_S`); Final Blow's chase (kind 0x22: countdown, warp); CHARGE_BERSERK's endless combo and the combo-step gate; `CanAttack` rows and `TryAttack` cases of the action states (wall-run attacks 0x31 - 0x34, flow attacks out of super slide / rail / wall kick / pole, the Somersault 0x39) | 9.9.5, 8.8, 5.6.4, 3.2 - 4.2 |
| `combat/hit.rs` | the Attack state's map-hit handler and the map-hit recoil; shots as stationary growing spheres with their own hit list; the sort's tie-break; the manager's guard / clash / no-damage effects by their real names; the chase arming | 8.2.5, 8.5.1, 8.8, 8.6, 5.6.4 |
| `combat/guard.rs`, `states/damage.rs` | the counter-allowed gate; a GUARD_REFLECT result on Sora -> the guard reaction; the bad status of a hit before the reaction gate; Damage+0x90 | 5.6.3, 8.4, 9.9.1 |
| `combat/situation.rs` | AUTO_FINISH | 11.5 |
| `kh-camera/src/lockon.rs` | the framing solver, the velocity lead, the obstruction blend with its 0.6 s release | 7.12 |

Host seam additions (all defaulted): `Targets::reaction_state(id)` (the damage kind of the reaction a
target is in: Final Blow), `World::attack_hit_response(component)` (does the level repel a swing: `None`
= geometry without an actor, which does), `HitOnPlayer::bad_param`, `EventKind::BadStatus { from, to,
time }`, `PawnCombat::{bad, shots, map_reflect, revenge, chase_armed}` (read through `Sora::combat()`;
`bad.hud_locked` = the command menu is greyed by a bad-status state, `bad.dilation` = Sora's own rate).
Step order (`sora.rs`, three marked edits): Sora's dt is also scaled by `bad.dilation`; the pose tick gets
dt 0 while he is frozen; `combat::badstat::tick` runs after `Status::tick`. `controller.rs`: the stick
flick of the stun.

Pad: nothing new to bind. A stun is shaken off with the LEFT STICK (each flick 0.3 s); the sneeze is the
attack button (A) while the status lasts; Final Blow is A again after a guard counter (X guard, A counter,
A chase); the wall-run / pole / slide attacks are A in those states; Auto-Finish needs no input.

App words: `dummy-status N STATUS TIME [KIND POWER]` (a fixture hit that carries a bad status:
7 freeze, 8 stop, 12 stun, 13 sneeze, 14 honey, 17 burn, 18 HP break, 19 time break). The dummies answer
`reaction_state` for 1.5 s after a hit that took HP.

Not done, and why: the adhere actors of CLOUD / DISCHARGE (blueprint actors, not read); the attack camera
(mode 4 while burning, mode 3 in Sonic Rave); HOPPING / SLOPESLIDE / CLIMBING attack requests; the shot's
homing drift; P3 strand items G 1 - 4 and H 3 - 5 (not reached: listed in spec 16.8 with the probe
requests C3-1 ... C3-9 of `docs/sora-runtime-probe.md` 8b).

## 16. Integration pass: the seams between the parts (2026-10-12)

State note: `build/sora_tmp/IMPL_INTEGRATION_STATE.md`. The user's table of every feature with its pad inputs, status
and gaps: `docs/sora-feature-status.md`. Tests: `crates/kh-core/tests/seams.rs`, `crates/kh-camera/tests/traversal.rs`,
`crates/kh-anim/tests/keychain_routing.rs`, unit tests in `kh-core/src/{config.rs, ui_model/prompts.rs}` and
`kh-app/src/{fixtures.rs, loadout.rs}`.

- **HUD model additions** (section 3, item 7): `UiSnapshot::soft_target` (the AUTO cursor's target), `::prompt`
  (`ui_model/prompts.rs`: the push-button prompt, "DODGE!" of the risk-dodge window or "ATTACK!" of an airstep's enemy
  stop; one HUD function for both, 0x1416793E0, decompiled C) and `::aim` (`AimHud`: scope, lock counter, shotlock marks,
  airstep marks), filled by `ui_model::fill` wherever `Sora` builds a snapshot. `kh-ui`: `HudTarget::push_button_mark`,
  `TargetDriver::{show_push_mark, hide_push_mark}`. `kh-app/src/gfx_hud/drive.rs`: `Hud::aim` (static SE 18 / 25 at the
  scope, 19 per lock and per airstep mark, 20 at the maximum) and `Hud::push_prompt`. The reaction commands (counter,
  Payback Strike, the risk dodge's counter) were already shown in the situation slot. NOT READ: where the game puts the
  prompt clip on screen (the port: above the enemy it is about).
- **Airstep hand-offs** (`states/athletic_flow.rs`, magic spec H 4.6): `pole_check` (a kind-6 point on a pole of
  `World::poles`: `TryPoleTurn` on the last target, else the in-state pole stop, simplified), `end_check` (the ledge
  probe -> DANGLE with the climb-up). `AirstepPoint::pole`. Test level: airstep points 2 (on the pole) and 3 (in front of
  the ledge) in `kh-app/src/magic_host.rs`.
- **Camera** (`kh-camera/src/lib.rs`, camera spec section 5 rows 3 and 20): `TargetState::{WallRun(class), RailSlide,
  SlopeSlide}` (base pitch 0 and yaw +-40, pitch limit 60, distance toward 700; base pitch 5; the slope pitch) from
  `Sora::camera_state()`; `FieldCamera::set_auto_camera` = Config "Camera Work". The pole turn and the risk dodge ask
  for the attack camera through the combat side's `combat::camera::request` (modes 5 and 3). The host feeds the step's
  real displacement as the camera's velocity in those states (the port's rail and wall run place the actor themselves).
- **Config** (`kh-core/src/config.rs`): `UserConfig::{to_text, from_text, master_volume}`. `kh-app`: the configuration
  is written to `khgame_config.txt` beside the executable when the Config screen applies and read at start (not in an
  unattended run; `KH_CONFIG_FILE` names another file); readers added: Camera Work, Master Volume
  (`AudioOut::set_master_volume`, linear), Vibration (`xinput::rumble` is gated by it; NOTHING calls it: the specs give
  no force-feedback pattern).
- **Test level**: the engaged rule (`fixtures.rs`: a passive dummy engages for 3 s after a blow either way, a hostile
  one also by proximity), pad View / Back = reset dummies + full recover + refill, the test stock (sheet
  `sora-test-stock`, authored: battle items into the pouch stock, spare armour and accessories into the menu's
  inventory), F1 cycles debug text / pad help page (`hud.rs PAD_HELP`) / nothing, Home = floating mode (F12 is the audio page).
- **Keychain** (`kh-anim/src/keychain.rs` `Keychain::route`, `Bonamik::set_speed_rate`; `kh-app/src/pack.rs`): the
  chara-wind and speed-rate notify states active in the clip Sora plays are routed to the weapon's Bonamik instance. The
  13 clips whose wind names group KEY are the underwater selfie clips (`Selfie*Sea`); the speed-rate notifies sit in those,
  in `M_1483`, `M_0270` and two other forms' transformation clips.
- **Sheets**: 13 numeric rows appended to `sora-actions` for the literals of the traversal states (the states now read
  the generated fields); `sora-assets` merged from the sub-packs' rows without a pack rebuild
  (`build/sora_tmp/impl_integration/merge_assets.js`: 7 audio WAV rows, `vfx/host.json`). `tools/sora_actions_sheet.js`
  is OLDER than the three sheets it writes (rows were added to the sheets directly): do not run it without merging.

## 17. Combat, fourth pass (fourth combat implementer, 2026-10-12)

State note: `build/sora_tmp/IMPL_COMBAT_STATE.md` (section "FOURTH COMBAT IMPLEMENTER"). Spec:
`docs/sora-combat-spec.md` 16.9 lists what changed. Tests: `crates/kh-core/tests/combat4.rs` (11),
`crates/kh-camera/tests/lockon.rs` (10), the sneeze test of `combat3.rs` rewritten.

| file | what | spec |
|---|---|---|
| `combat/camera.rs` (new) | the attack camera: `request(cx, on, mode, target)` / `off(cx)` with the controller's mode; callers in `states/attack.rs` (Buzz Saw mode 5, Hook Upper mode 3, off at End), `style_finish.rs` (Sonic Rave mode 3), `badstat_burn.rs` (mode 4); the traversal states call it too (pole turn, risk dodge: integration pass) | 7.12.1 |
| `kh-camera/src/rig.rs`, `lockon.rs` | `CameraRig::set_attack_mode(mode, restore)` (look-point lag, lock yaw limit, distance scale, the timed return), `interp_curve` (the game's interpolator curves; the obstruction release is sin), the lock-on camera's own yaw limit; `lib.rs`: `FieldCamera::params_mut` (marked) | 7.12, 7.12.1 |
| `states/style_finish.rs` | Last Arcanum's count at every hit window, `finishEnd` never set, Stun Impact's rumble | 10.9 |
| `states/attack.rs` | state+0x234 bit 3 / +0x23C bit 4 (Rising Spiral, Sonic Cleave), the HOPPING / SLOPESLIDE requests with kind 0x2F, the sneeze latch, rush suppression | 3.3, 4.3, 6.3, 7.10, 9.9.5 |
| `combat/hit.rs`, `combat/push.rs` | the shot location (socket `ATK_01`); `push::touching` | 8.8, 6.5 |
| `sora.rs` (marked) | `combat::hit::tick` moved after `states::main_proc`: the attack-collision manager ticks after the pawn, in the same frame | 8.2.2 |

Host seam additions (all defaulted or new events): `EventKind::AttackCamera { mode, restore }` (the host
calls `CameraRig::set_attack_mode`; the lock half of modes 3 / 6 arrives as `CameraLock`),
`EventKind::ForceFeedback { effect, looping }` (the game's `ClientPlayForceFeedback` by table index: for
the vibration seam), `Targets::rush_suppression(id)`, `PawnCombat::{attack_cam, sneeze_next}`
(`sneeze_next` = controller+0x920: the HUD shows the Attack command as "Sneeze" while it is set).
Sheets: `tools/sora_combat_sheet.js` has kind 0x2F and keeps the `run_*` attack rows
(`sora-combat-attacks`, `-hits`, `-attack-data` regenerated; `gen_port` run).

Pad: nothing new to bind. A on a hop point / in a slope slide attacks (kinds 0x10 / 0x2F); the attack
cameras need no input; with the SNEEZE status the combo's finisher is the sneeze.

Not done, and why: the cameras of attack-camera modes 1 / 2 / 8 (other keyblades' finishers, shooter
mode); the `EndEffect` calls at a finish command's end (the effect-group seam is the effect
implementer's); the push solver's sequential variant and the enemies' push levels (no enemy in the test
level; a game flag whose writer was not found); CLIMBING attack requests (no such state in the port).
Needs the running game: probe requests C4-1 ... C4-6 (`docs/sora-runtime-probe.md` 8b).

## 18. Enemy-side hit reaction (enemy-reaction implementer, 2026-10-12)

Spec: `docs/sora-enemy-reaction-spec.md`. What a generic KH3 enemy does when a hit lands (the victim's damage state),
which the port had not had: `HitRecord::impulse` was always zero and the dummies never moved.

- New engine-free crate `crates/kh-react` (owner: this strand): `EnemyReaction` (gate, Setup, the handlers per damage
  kind, the body's movement, reaction endurance, revenge value), `ReactWorld` (one body sweep, supplied by the host),
  `Body` / `BodyClass` / `ClipTimes` / `DamageParams`. Constants: sheet `sora-enemy-reaction` (tool
  `tools/sora_enemy_reaction_sheet.js`, registered in `tools/lib/sheet_defs.js` and `tools/lib/port_gen_defs.js`) ->
  `build/gen/rust/sora_enemy_reaction.rs`. Tests: `crates/kh-react/tests/spec_numbers.rs` (13).
- `kh-core`, marked edits: `host/targets.rs` `HitAttacker` + `HitRecord::attacker` (filled in `combat/hit.rs` and
  `magic/projectile.rs`); `HitRecord::impulse` kept and documented as always zero. A host answers
  `Targets::take_reaction_power` / `body` / `reaction_state` from its `EnemyReaction`.
- `kh-app`, marked edits: `fixtures.rs` (each dummy owns an `EnemyReaction`; `step_reactions`, `update_visuals`,
  `Dummy::set_class`), `player.rs` (one call per fixed step; the `dummy-class` arm), `script.rs` (`dummy-class N CLASS`),
  `main.rs` (the `update_visuals` system), `Cargo.toml`.
- Not touched: `crates/kh-er*` (the hand-over is section 9 of the spec).
