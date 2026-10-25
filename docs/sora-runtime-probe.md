# Runtime probe: recording the original game and replaying it through the rebuild

The rebuilt Sora (`crates/kh-core`) must control exactly like the game. Reading the code left
assumptions (order of the per-frame updates, effective speeds and timers, landing timing, blends).
This is the measuring tool that settles them:

| piece | file | what |
|---|---|---|
| field table (data) | `tools/probe_fields.spec.json` → `node tools/probe_fields.js` → `build/gen/probe/fields.json` | what to read: pointer paths and fields, offsets from the sheets |
| probe | `tools/native/kh3probe.c` → `build/native/kh3probe.exe` (`tools/native/build.ps1 kh3probe`) | reads the running game once per game frame, writes a CSV |
| replay | `crates/kh-replay` (`kh-replay.exe`) | feeds a CSV's inputs to `kh-core` with the recorded frame times, reports the first divergence |
| offline tests | `node tools/probe_test.js`, `cargo test -p kh-replay` | everything that can be checked without the game |
| shortcut | `probe.bat` | `selftest`, `record NAME [SECONDS]`, `compare FILE.csv`, `test` |

**Status: the probe has never been run against the game** (the tooling cannot start it). Every
pointer path below comes from the decompiled exe and is unverified against the live process until
the first `probe.bat selftest`. Section 9 lists what was verified offline.

## 1. What it does and does not do

- A **separate process**. It finds `KINGDOM HEARTS III.exe`, opens it with
  `OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ)` and calls `ReadProcessMemory`.
  Nothing else touches the game: **no writes, no DLL injection, no code patching, no debugger
  attach, no thread suspension, no handles with more rights.**
- The pad is read with `XInputGetState` and the keyboard with `GetAsyncKeyState` **in the probe's own
  process**.
- If the open or a read is refused (access denied) it prints that and stops (exit code 4). It does
  not try anything stronger.
- **It refuses another build.** Before reading anything else it compares the running image with
  the installed exe the sheets were made from: PE `TimeDateStamp` and `SizeOfImage`, and the
  FNV-1a hash of four 4 KiB code pages (the pages of `FEngineLoop::Tick`, the Sora input update,
  the movement component tick and the Jump state tick; none has a relocation). Any mismatch:
  exit code 3, nothing read.
- It is for the user's own single-player copy on this machine; the recordings stay under
  `build/probe/` (doctrine `d12`).
- It cannot log the order in which functions are entered (that needs code in the game). The tick
  order is inferred from data instead (section 6).

## 2. How to run it

In a console in the project folder (PowerShell: write `.\probe.bat`):

```
probe.bat selftest               game running, a save loaded, Sora standing where he can be moved
probe.bat record NAME 20         record 20 s to build\probe\<time>_NAME.csv   (no number: until Ctrl+C)
probe.bat compare build\probe\<time>_NAME.csv
probe.bat test                   offline tests, no game needed
```

**Self test** (do this first, once): start the game, load a save, stand somewhere Sora can walk.
Run `probe.bat selftest`, switch back to the game and walk around for 8 seconds (the probe samples
once at the start and once after 8 s), then return to the console. It writes `build\probe\selftest_<time>.txt` — **send that file back as it
is** (or paste its text). It contains: the build check, every pointer path with the object's name
and class, every field read twice (a `*` marks values that changed while you moved), the values the
specifications expect next to the ones read (`[spec 680]`, `DIFFERENT` when they differ), and a
few plain checks (`frame counter advances: yes`, `world position equals the root's relative
position: yes`, …). Its last lines are `RESULT: ALL PATHS RESOLVED` or `RESULT: PROBLEMS FOUND`.

Exit codes of `kh3probe.exe`: 0 ok · 1 usage / table · 2 game not running · 3 build mismatch ·
4 access denied · 5 self test found a problem. Options: `--label`, `--seconds`, `--fields FILE`,
`--out-dir DIR`, `--max-frames N`, `--no-explore` (leave out the raw "explore" columns),
`--wait S` (self test), `--process NAME.exe`.

## 3. When a row is taken

The game increments `GFrameCounter` once per engine tick, in `FEngineLoop::Tick` (0x14041FB60,
increment at 0x140420293), **after** `GEngine->Tick` — after the world, all actors and the camera
have been updated. The probe spins on that counter (one core busy) and, when it changes, resolves
all paths again and reads all fields. So **a row is the state at the end of one game frame**:

- `ctl_*`, `key_*`: the input the game's controller computed in that frame;
- `world_dt`: the dt that frame was ticked with (`UWorld::DeltaTimeSeconds`); `app_dt` the engine's
  (`FApp::DeltaTime`);
- everything else: the result of that frame.

`missed` = counter values that went by unseen before this row. `torn` = 1 when
`UWorld::RealTimeSeconds` changed while the row was read (the next world tick had begun: the row
may mix two frames; the replay does not compare it). `read_us` = how long the read took. With the
game capped at 60 frames per second both stay 0; uncapped at several hundred they will not.

Objects can be reallocated (map load, cutscene): paths are resolved **every row**, a null on the
way leaves that node's cells empty, and the capture goes on.

## 4. Pointer paths and their evidence

Addresses are for image base 0x140000000; the probe adds the RVA to the real base of the module
(`EnumProcessModulesEx`). "Reflected" = the offset is a row of `sheets/reflect/properties` (the
game's own registration of the property), which is as good as evidence gets statically.

| what | path | evidence | confidence |
|---|---|---|---|
| engine | `GEngine` = [0x149DE3B98] | the `UGameplayStatics` helpers 0x144106970 / 0x1441041F0 / 0x1441053E0 pass it as `this` to `GetWorldFromContextObject` 0x1445786A0; `FEngineLoop::Tick` calls its virtuals with the frame time | high |
| frame counter | `GFrameCounter` u64 at 0x149B68D50 | `inc` at 0x140420293 in `FEngineLoop::Tick` (found by its `r.OneFrameThreadLag` lookup), followed by the stock "frame 6" test | high |
| frame time | `FApp::DeltaTime` f64 0x14871E340, `FApp::CurrentTime` f64 0x149B5C530 | `UEngine::UpdateTimeAndHandleMaxTickRate` 0x144590F00 ("Detected negative delta time" string) writes them | high |
| world | engine → `GameViewport` +0x768 → `World` +0x98 | both reflected | high |
| world times | world +0x938 `TimeSeconds`, +0x940 `RealTimeSeconds`, +0x948 `DeltaTimeSeconds` | bodies of `GetTimeSeconds` 0x144106840, `GetRealTimeSeconds` 0x1441057A0, `GetWorldDeltaSeconds` 0x144106970 | high |
| world settings, volumes | world → `PersistentLevel` +0x48 → `WorldSettings` +0x310; world → `DefaultPhysicsVolume` +0xE0; root → weak `PhysicsVolume` +0x274 | reflected; weak pointers resolve through `GUObjectArray` (items [0x14872B020], count 0x14872B02C, 0x18 bytes each, serial +0x10: 0x142F71BE0) | high |
| game instance, local player | engine → `GameInstance` +0xE30 (`UGameEngine`) → `LocalPlayers` +0x50 [0] | reflected; `TArray` = pointer, count at +8 (0x143F11660) | high |
| controller | local player → `PlayerController` +0x48 | reflected | high |
| pawn | controller → `Pawn` +0x3E8 (`AcknowledgedPawn` +0x458 is recorded next to it) | reflected | high |
| root component | pawn → `RootComponent` +0x188; world transform at +0x2A0 (quaternion) / +0x2B0 (location); `RelativeLocation` +0x2F0, `RelativeRotation` +0x2FC, `ComponentVelocity` +0x368 | reflected, except the world transform: `K2_GetActorRotation` 0x144614430 reads the quaternion at +0x2A0; the location at +0x2B0 follows from the `FTransform` layout and the self test checks it against `RelativeLocation` | high (location +0x2B0: medium until the self test) |
| movement component | pawn → `MyMovement` +0x578; `Velocity` +0x214, `MovementMode` +0x314, `GravityScale` +0x2F8, `JumpZVelocity` +0x304, `MaxWalkSpeed` +0x388, `MaxAcceleration` +0x39C, `Acceleration` +0x4A4, `CurrentFloor` +0x4F8 (`FindFloorResult`, `HitResult` inside), jump curve +0xBE8 / +0xBEC / +0xBF0 | all reflected | high |
| … its unreflected fields | +0xA3C flag word (walk mode bit 0), +0xBF8 gravity rate, +0xBFC / +0xC00 jump-duration flag and timer, +0x944 root-motion weight, +0x9A0 the velocity the camera uses | movement spec 2.5 / 4.3 (instruction level: 0x1405955F0, 0x140582270, 0x140577F30, 0x140594910, 0x1405795F0); camera spec 2.6.2 | medium |
| state machine | pawn → `MyStateComp` +0x5A0 → `m_pCurrentState` +0x270; `MyStateID` +0x58, `MyStateName` +0x50, `m_nSubStep` +0x78, flag byte +0x84 (`m_bEnableChange` bit 0, `m_bReverseLeg` bit 1); queue flags +0x268; `m_StateQueue` +0x278 | reflected, except +0x268 (movement spec 1.3, `MainProc` 0x14150A7E0) | high |
| … state latches and timers | state +0x88 … +0x120 (move request, target yaw, magnitude, jump held / walk, run grace, sprint timer, jump type, power, **fall time +0xAC, fall height +0xB0**, last Z, ground distance, flags) | movement spec 3.1 / 4.2 and `build/sora_tmp/native_B.md`; the offsets overlap between Idle, Run and Jump: read them by `state_id` | medium |
| pawn jump plumbing, land pose | pawn `bPressedJump` +0x5BC, +0x5C0 flag, `JumpKeyHoldTime` +0x5C4, `ControlInputVector` +0x428, analog modifier +0x63C; **land pose +0x6E8**; +0xD80 / +0xE60 / +0x1460 restriction flags | reflected, except +0x5C0 (0x1405ADF50), +0x6E8 (setter vfunc +0x1790 from the play wrapper 0x1405A0E50), the three flags (movement spec Q10) | medium |
| animation instance | pawn → `MyMesh` +0x580 → `AnimScriptInstance` +0xB48 | reflected | high |
| … current animation name | anim instance `m_LastPlayedInfo` +0x488 (map slot name → info), the entry whose key = `AnimPlayUseSlotNodeName` +0x438 (`FullBody`); info +0 = the animation-set name, +8 a time, +0x10 `m_pAsset` | lookup 0x14056E150 (element 0x30 bytes, value at +8); get-current-anim-name 0x14055E1F0 returns the first 8 bytes; `m_pAsset` reflected | medium |
| … montage position, blend, blend-space input | anim instance montage array +0x98 / count +0xA0, last instance with a montage, weight ≠ 0 and slot = `FullBody`; instance +0x110 position, +0x114 play rate, +0x138 weight, +0x140 desired weight, +0xA0 playing, +0x1A4 blend-space input (x, y, z), +0x10 blend-space asset (its `AnimLength` scales the position) | 0x143F11660 (array walk), 0x14056A800 (position, × blend-space length), 0x14056A710 (rate), 0x14056BFD0 (is-anim-end), 0x143F4D960 (stores the blend-space input) | medium: the meaning of +0x138 vs +0x140 is from how the accessors test them |
| controller stick | controller +0x848 forward, +0x84C right (after the dead zone; **normalised in place when the magnitude ≥ 0.1**), +0x854 magnitude, +0x858 rotator (**moveYaw at +0x85C**), +0x878 flags | Sora input update 0x1413F1AB0 (movement spec 2.1 / 2.2) | high |
| the game's own key state | controller → `PlayerInput` +0x500 → key-state map +0x278 (element 0xF8, `FKeyState` at +0x18: raw value +0, value +0xC, down = bit 0 of +0x1C, press events of the frame = i32 at +0x30 > 0); the entries of the jump key (FName at 0x1487D7778), `Gamepad_LeftX/LeftY` (0x149C705F8 / 0x149C70610), `Gamepad_RightX/RightY` (0x149C70628 / 0x149C70640) | `IsPressed` 0x1445ADDB0, `WasJustPressed` 0x1445BA2C0, `GetInputAnalogStickState` 0x1444333A0 | medium |
| camera | controller → `m_pCameraManager` +0x830; `CameraCache.POV` location +0x7C0, rotation +0x7CC, FOV +0x7D8; view target POV +0x1720…; input-freeze view +0x28F4 / +0x2900 / +0x290C | reflected; the freeze fields from 0x1405E6B80 (the function the input update asks for the camera rotation) | high |
| field camera actor | camera manager → `m_CameraNormal1st` +0x28D8 → its fields and root component | reflected pointer; the actor's fields are the list of `docs/sora-camera-spec.md` section 7 (31 of them reflected) | as that document |
| names | `FName` → text: table pointer [0x149C6A450], `chunks[index >> 14][index & 0x3FFF]`, characters at entry +0x10 (wide when bit 0 of the entry's first int) ; object name at object +0x20, class at +0x18 | `FName` registration 0x142DCEDE0; `StaticAllocateObject` 0x142F78270 and 0x142F66E30 | high |

The self test prints, for every path, the object's own name and class name read through that
last row. A wrong offset on the way shows up as a class that is not what the path says
(`UNEXPECTED CLASS`), or as `NOT RESOLVED`.

## 5. The field table

`tools/probe_fields.spec.json` is the authored list: `nodes` (name, parent, path steps, a word the
class name must contain, evidence) and `fields` (name, node, offset, type, options).
`node tools/probe_fields.js` turns it into `build/gen/probe/fields.json`, which the probe loads at
start; `build/gen/probe/fields.md` is the full generated table (node paths, every field with
offset and type). **To add a field: add a row, run the tool. Nothing is recompiled.**

- An offset is `Class.Property` (looked up in the sheets, supers followed, struct members added
  with `+`: `TresCharMovementComponent.CurrentFloor+FindFloorResult.FloorDist`) or a hex literal;
  a literal must name an entry of the spec's `evidence`, or the tool fails.
- Path steps: `deref` (follow a pointer), `add`, `elem` (array element), `weak` (weak object
  pointer), `scan` (the array element that passes tests: non-zero bytes, or bytes equal to those at
  another node's offset — used for the map entries and the montage instance), `global` (exe + RVA).
- Types: `u8 i8 u16 i16 u32 i32 u64 f32 f64`, `bool` (a bit), `vec3` (columns `_x _y _z`), `rot`
  (`_pitch _yaw _roll`), `quat` (`_x _y _z _w`), `fname`, `ptr`, `objname` / `objclass` (name of
  the object at that address / of its class); `count` n gives columns `_0 … _n-1`.
- `"once": true`: a constant, written in a `# once` comment line when a pawn first appears and
  again at the end, not every frame. `"g": "explore"`: raw words not yet identified. `"x": value`:
  what the specifications expect, shown by the self test.

Now 46 nodes, 392 fields, 699 columns per frame (group `bones`: 189 of them, the component-space
and saved-pose bone transforms; `explore`: raw words; `--no-explore` leaves both groups out). By
node (`*` = once):

| node | fields |
|---|---|
| exe globals | `app_dt`, `app_time`, `jump_key_setting`\*, `jump_key`\*, `attack_key`\* |
| world, settings | `world_time`, `world_real_time`, `world_dt`, `time_dilation`, `sqex_time_dilation`, `matinee_time_dilation`, `world_gravity_z`\*, `global_gravity_z`\*, `kill_z`\*, `physvol_terminal_velocity`\*, `defvol_terminal_velocity`\* |
| controller | `ctl_fwd`, `ctl_right`, `ctl_mag`, `ctl_move_{pitch,yaw,roll}`, `ctl_flags`, `control_rot_*`, `pc_pawn`, `pc_ack_pawn`, `ctl_look_ignored_421` |
| key state | `key_jump_down`, `key_jump_pressed`, `key_attack_*`, `key_lx_raw`, `key_lx_value`, `key_ly_*`, `key_rx_*`, `key_ry_*` |
| pawn | `input_vec_*`, `last_input_vec_*`, `pressed_jump`, `jump_flag_5c0`, `jump_key_hold_time`, `analog_modifier`, `land_pose`, `pawn_floating_d80`, `pawn_restrict_e60`, `pawn_resume_1460`, `pawn_battle_bits`, `pawn_time_dilation`, sprint / jump powers\* |
| root | `pos_*`, `rot_q_*`, `rel_pos_*`, `rel_rot_*`, `comp_vel_*`, collision object type, profile and responses\* |
| movement | `vel_*`, `accel_*`, `move_mode`, `gravity_scale`, `jump_z_velocity`, `max_walk_speed`, `max_accel`, `walk_speed_rate`, `analog_input_modifier`, `cmc_flags_a3c`, `gravity_rate`, `jump_dur_finished`, `jump_dur_timer`, `root_motion_weight`, `last_update_pos_*`, `floor_blocking`, `floor_walkable`, `floor_dist`, `floor_line_dist`, `floor_hit_point_*`, `floor_hit_normal_*`, `floor_hit_impact_normal_*`, `cam_velocity_9a0_*`, every tunable of movement spec section 8\* |
| state | `state_id`, `state_name`, `sub_step`, `enable_change`, `reverse_leg`, `queue_flags`, `queue_len`, `queue0_id`, `st_move_req_88`, `st_yaw_8c`, `st_mag_90`, `st_u94`, `st_f98`, `st_ia0`, `st_fa4`, `st_fac` (fall time), `st_fb0` (fall height), `st_fb4`, `st_fb8` (sprint timer), `st_ud4`, `st_fdc`, `st_fe0`, `st_ue8`, `st_u114`, `st_f120` |
| animation | `anim_name`, `anim_asset`, `anim_last_time`, `mi_pos`, `mi_rate`, `mi_weight`, `mi_desired_weight`, `mi_playing`, `mi_flags`, `mi_bs_input_*`, `mi_len`, `mi_blend_in_time`, `mi_blend_in_option`, `mi_asset`, `mi_bs_len`, `mi_bs_interp_time`, `anim_dt`, `mi_raw_0..71` (explore) |
| camera manager | `cam_pos_*`, `cam_rot_*`, `cam_fov`, `cam_cache_time`, `cam_vt_*`, `cam_override_*`, lock-on pointers, `cam_u2a34` … `cam_size_override_2a44` |
| requests R2 / R8 / R9 (movement) | `cmc_flags_c18`, `cmc_u68c`, `cmc_ubdc`, `cmc_u308`, `cmc_u380`, `cmc_ext_vel_b84_*`, `cmc_ext_vel_b90_*`, `cmc_ext_vel_bb4_*`, `cmc_root_vel_a24_*`, `floor_raw_0..37` (the whole `CurrentFloor`), `queue_u258`\*, `anim_notify_timing_390`\*, `cvar_penetration_pullback`\*, `cvar_int_87c9510`\*, `cvar_int_87c9514`\*, `cvar_int_87c9850`\* |
| requests R2 – R9, R11 (animation) | saved pose `sp_{center,l_momo,kubi}_{q,t}_*`, `proxy_flags_528`, `proxy_initial_pose_530`, `proxy_bytes_3e0_*`\*, `proxy_i230`\*; `mi_notify_offset_17c`, `mi_bs_current_198_*`, `mi_raw2_*`; `leg_ik_alpha`, `leg_ik_offset`, `arm_ik_alpha`, `leg_ik_rate_{r,l}`, `ikinema_sw`, `pawn_leg0_interp_*`, `pawn_leg1_interp_*`, `pawn_ik_disable`, `pawn_ik_init_value`; `mesh_read_buffer`, `mesh_edit_buffer`, both component-space buffers `cst0_*` / `cst1_*` for `kh_sk`, `center`, `koshi`, `L/R_momo`, `L/R_sune`, `L/R_ashi1`, `L/R_ashi2`, `kubi` (quaternion and translation), `mesh_world_pos_*`, `mesh_rel_*`; `look_at_rate`, `face_anim_blend_rate`, `blend_*_weight`, `pawn_f788`, `ctl_look_target_954/994`; Bonamik `kbd_*`, `bonamik_*`, `bonamik_chain_roots`\*, `bonamik_particles`\* |
| field camera | `cn_rot_target_*`, `cn_stick_delta_*`, `cn_rot_prev_*`, `cn_look_smooth_*`, `cn_look_position_*`, `cn_dist_smooth`, `cn_current_distance`, `cn_auto_power`, `cn_stick_wait`, `cn_still_wait`, `cn_reset_wait`, flags, `cn_base_rot_*`, slope ring, `cnr_pos_*`, `cnr_rel_rot_*`, `cnr_fov`, parameters `cnp_*`\*, `option_bytes`\* |

Fixed columns before them: `frame, missed, torn, t_wall, read_us, pad_connected, pad_lx, pad_ly,
pad_rx, pad_ry, pad_lt, pad_rt, pad_buttons, pad_pressed, keys` (XInput raw values; `pad_pressed`
= buttons that went down since the previous row, polled about every millisecond; `keys` = a bit
per key, legend in the file's header). Lines starting with `#` are comments (format, build, `once`
values, a summary at the end). An empty cell = not readable in that frame.

## 6. What a capture answers by itself

`kh-replay` prints these under "OBSERVATIONS" (also alone with `--facts`):

| question | how the capture shows it |
|---|---|
| movement spec Q1 / R1 (revision 2 derives controller → movement → state queue): controller before state queue? | the controller sets the state's latch +0x88 (`st_move_req_88`), the state tick clears it at its end. Row = end of frame: latch clear while the stick is held → controller ran before the state tick (assumed); still set → after |
| … state queue before movement? | the state writes the pawn's `ControlInputVector`, the movement tick consumes it: zero at the end of the frame → state queue before movement (order A, the revision 1 assumption); non-zero → movement before state queue (order B, what revision 2 derives) |
| which camera yaw the stick is relative to (camera spec 1.4) | `ctl_move_yaw − atan2(right, forward)` against `cam_rot_yaw` of the previous and of the same row |
| Q2 effective values | distinct values of `max_walk_speed`, `max_accel`, `gravity_scale`, `jump_z_velocity`, `analog_input_modifier`, `cmc_flags_a3c` |
| Q4 gravity, terminal velocity | `world_gravity_z`, `global_gravity_z`, `physvol_terminal_velocity` (once); `vel_z` per frame of a ledge fall |
| Q5 collision channel | `root_object_type`, `root_collision_profile`, `root_responses_*` (once) |
| Q6 jump key | `jump_key`, `jump_key_setting` (once) |
| Q7 dead zone | `key_lx_raw` vs `key_lx_value` against the 0.25 formula; `pad_lx / 32767` vs `key_lx_raw` |
| Q9 standing height | `pos_z − floor_hit_point_z` while standing |
| Q10 restriction flags | `pawn_floating_d80`, `pawn_restrict_e60`, `pawn_resume_1460` |
| Q3, assumption 3: root motion of `RunStartN` | `pos_*` per frame from a standing start, with `vel_*` and `root_motion_weight` |
| animation spec open question 1 (blend) | `mi_weight` per frame after each play, `mi_blend_in_time`, `mi_blend_in_option`: the ramp's length and shape. Whether the source pose is refreshed every frame is **not** observable from these (it needs bone transforms; not recorded) |
| assumption 6 (step = frame) | `world_dt` per row |
| camera spec section 7 | the `cn_*`, `cnr_*`, `cam_*`, `key_rx_*` columns; its "entry hooks" cannot be done read-only |

## 7. Replay: `kh-replay`

```
kh-replay CAPTURE.csv [--world FILE.json] [--trace OUT.csv] [--jump auto|game|pad-b|pad-a|space]
          [--jump-shift N] [--no-resync] [--tol-pos CM] [--tol-vel CM_S] [--tol-yaw DEG] [--tol-anim S]
          [--context N] [--facts]
kh-replay --synth OUT.csv [--script "stop 0.5; run 1; jump; run 1; stop 1"] [--jitter 0.2]
```

- **Start.** The first row where the game shows Sora at rest (state 1 Idle, on the ground, speed
  < 1 cm/s, stick < 0.1, jump key up) for two rows in a row. `Sora::new` is created there with the
  recorded position and yaw.
- **World.** Without `--world`: one horizontal plane, placed so that the core rests at the recorded
  height (a difference to the game's own floor hit is printed as a note: question Q9). With
  `--world`: planes and axis-aligned boxes in the game's coordinates (`crates/kh-replay/src/world.rs`
  shows the format), for a ledge or a step at the test spot.
- **Each following row = one `Sora::step(dt, input, world)`** with
  - `dt` = `world_dt` × `pawn_time_dilation`; rows with dt 0 (pause) are skipped;
  - stick = the controller's dead-zoned vector (`ctl_fwd`, `ctl_right`, × `ctl_mag` when it is
    normalised), turned back into the raw value `kh-core` wants (it applies the 0.25 dead zone
    itself);
  - camera yaw = `ctl_move_yaw − atan2(right, forward)`: exactly the yaw the game used that frame;
  - jump pressed / held = the game's own key state (`key_jump_pressed`, `key_jump_down`). If the
    capture has none: the pad's B button (the default jump key; `--jump pad-a` / `space` for other layouts) as the probe polled it, which may be
    a frame off — `--jump-shift` moves them.
- **Compared per row**, with tolerances: position xy / z (0.5 cm), velocity xy / z (1 cm/s), yaw
  (0.1°, from the recorded quaternion), state id, sub-step (only while the state is the same),
  on-floor (movement mode Walking), animation name, animation time (0.02 s; not for the idle
  animations, whose phase at the start is unknown; a blend space's position is converted with its
  length).
- **Report.** Per segment: the first divergence overall with the rows around it (game and core
  side by side), then per quantity the first divergence (frame, game value, core value), how many
  rows are over tolerance, the maximum error and where. A summary over all segments. Exit code 0 =
  no divergence, 1 = divergence, 2 = unusable input. `--trace` writes every compared row.
- **Segments.** After position, state, sub-step or on-floor diverged, the replay starts again at
  the next rest (two rest rows): one capture with several actions gives a verdict per action
  (`--no-resync` turns that off).
- **Limits.** Torn rows are stepped but not compared. A missed frame is stepped blind with the next
  row's input and an even share of the elapsed time — what follows a gap is less certain; the
  report counts them. A state outside Idle / Run / Jump is reported as a state divergence with a
  note (the core has no such state). Inputs the capture does not carry (attack, guard, …) are not
  replayed.

## 8. Capture protocol

Preparation: an early save, so that no later ability (high jump, double jump, glide, air slide)
changes what the jump button does; Keyblade put away (stand a few seconds after any fight); no
enemies, no NPC to bump into. In the game's options cap the frame rate at 60 (V-sync).
**Jump is B on an Xbox pad by default** (gamepad East; A attacks) — the capture's header says which
key this install uses (`jump_key`). **Do not touch the right stick** except in run 16.

Spot: a wide, level, paved floor with (a) a clear ledge next to it that Sora can run off, and, if
the place offers them, (b) kerbs / steps / low walls of several heights between about 30 and 65 cm
(the questions are at 47, 49.4 and 59.4 cm: any heights in that range help, each one is measured
by the capture itself), (c) a slope he can stand on (about 15° and, if there is one, about 30°),
(d) a wall and a low ceiling to jump against. Suitable early: the first town plaza of Olympus
(Thebes) or the tram square of Twilight Town right after it; any place that fits the description
does (the spots were not checked in the running game). Nothing has to be written down: heights
and slopes are in the capture (`pos_z`, `floor_hit_normal_*`).

Each run: type the command, switch to the game, **stand still 2 s**, do the action, **stand still
2 s**. Repeat the action within the run where it says ×3. Runs marked ○ depend on the spot.

| # | command (`probe.bat record …`) | what to do | decides |
|---|---|---|---|
| 1 | `stand 15` | nothing: stand on level ground | sole height, leg IK at rest, idle clock |
| 2 | `tap 20` | tap the stick fully forward and let go at once; wait until he stands ×3 | run start that cannot be interrupted, root motion |
| 3 | `run 30` | hold forward 3 s, let go, wait until he stands ×3 | sprint onset, stop foot, weight ramps, saved pose |
| 4 | `flick 25` | hold forward 2 s, snap the stick fully back and hold 1 s, let go ×3 | turn-around |
| 5 | `walk 25` | half stick forward 3 s, let go ×2; then half stick 2 s and push to full without stopping, 2 s, let go | walk speed, 0.8 threshold, 0.2 s grace |
| 6 | `turn 25` | hold forward 1 s, steer a full circle left at full tilt, let go; the same to the right | run lean, turn rate |
| 7 | `jumptap 25` | standing: the shortest tap of jump; wait until he stands ×4 | launch delay (frames from the key press to Falling), short hop |
| 8 | `jumphold 25` | standing: press jump and keep it down until he has landed ×3 | full jump, hang phase, landing without stick |
| 9 | `jumprelease 30` | standing: press jump and release it early / a little later / late in the rise, one of each, twice | early release (0.05 / 0.15 / 0.25 s: whatever you manage; the capture has the exact time) |
| 10 | `jumprun 35` | hold forward 2 s, press and hold jump, keep forward through the landing 1 s more, let go ×2; then the same but let the stick go at the top ×1 | take-off speed dip, landing with / without stick |
| 11 | `jumpland 25` | standing: jump, and press jump again the moment he touches down ×4 (vary the timing) | jump during landing (accepted from the notify on) |
| 12 | `jumpstick 25` | standing, stick centred: push the stick forward and press jump at the same instant ×5 | stick + jump on one frame from Idle |
| 13 | `ledge 35` | run straight off the ledge, forward held until 1 s after landing; climb back; run off again and let the stick go the moment he leaves the edge; once more walking (half stick) | ledge fall, edge behaviour, landing with / without stick |
| 14 ○ | `kerb_NN 30` (one run per kerb, NN = your guess of its height) | run at it head-on and keep forward held 2 s; then come back down over it the same way; then walk (half stick) up and down | step-up limit, perch, floor kept while dropping |
| 15 ○ | `slope 30` | stand on the slope facing up 3 s, across 3 s, down 3 s; run up and down it once | leg IK on slopes, slope speed |
| 16 | `camera 35` | stand; right stick right 2 s, left 2 s, up 2 s, down 2 s; then (stick released) run forward 5 s; stop; press R3 | camera stick, automatic follow (1.5 s, rate), reset |
| 17 ○ | `wall 25` | jump straight up under the low ceiling ×2; run-jump into the wall ×2 | ceiling and wall contact in the air |
| 18 ○ | `keyboard 25` | (if you also play with the keyboard) hold W 2 s, W+D 2 s, tap the jump key ×2 | how keys feed the stick axes |

Then `probe.bat compare` on each file, or send the CSV files back (a 30 s run is roughly 10 MB with
the bone columns; `kh3probe --no-explore` halves it and drops them).

## 8a. The consolidated requests: what the probe covers, and what needs code in the game

Requests: `docs/sora-movement-spec-changes.md` section F (M-R1 … M-R12), `docs/sora-animation-spec.md`
section 15 (A-R1 … A-R11), `docs/sora-camera-spec.md` section 7. The probe reads memory at the end
of a frame. It cannot see a function being entered, its arguments, its return value, or a value
that is overwritten again within the same frame. For those the table gives the best end-of-frame
proxy; where there is none it says so.

| request | read-only? | what is recorded / the proxy |
|---|---|---|
| M-R1, A-R1 frame order and threads (function entries) | **no** | proxies: the state's move latch `st_move_req_88` and the pawn's `input_vec_*` at the end of the frame (which of state queue / movement ran last, section 6); the frame of each visible effect: key press event → `state_id` 3 → `pressed_jump` → `move_mode` 3 (the "launch delay" answer); `mi_weight` / `mi_pos` on the first frame of a play (did the pose tick in the same frame). Thread ids: none |
| M-R2 the whole stage-1 trace | yes | all listed fields are columns (section 5); `floor_raw_*` is the whole `CurrentFloor` |
| M-R3 play call ticks the pose at once; notify frame vs `Jump::Begin` vs `DoJump` | partly | no call counts; proxy: frames from the press to `state_id` 3 (Begin), to `pressed_jump` = 1 (the notify handler ran that frame, if the flag survives to the end of it), to `move_mode` 3 and `vel_z` = launch speed (DoJump); first-frame `mi_pos` of `JumpNormalStart` |
| M-R4 `ImpactNormal` on edges, at the return of the floor / landing functions | partly | the movement component keeps the result of its **last** floor query: `floor_hit_normal_*`, `floor_hit_impact_normal_*`, `floor_hit_point_*`, `floor_hit_time`, `floor_dist`, `floor_line_trace`, per frame, while he stands on or crosses an edge. Hits of the move sweeps and of `IsValidLandingSpot` are not kept anywhere: not observable |
| M-R5 perch chain (arguments, returns) | **no** | proxy: on a kerb, `move_mode` stays 1 or becomes 3, with `floor_dist`, `floor_line_dist`, `floor_line_trace`, `floor_walkable` and `pos_z` per frame: the outcome of the chain, not its steps |
| M-R6 step-up trace (three sweeps) | **no** | proxy: `pos_*` and `floor_*` per frame while going up a kerb (was he lifted in one frame, by how much) |
| M-R7 ground probe results; landing snap | partly | the Jump state caches its probe: `st_fdc` (distance) and `st_fe0` (ground Z) per frame; the snap is the `pos_z` step between the landing frame and the next. The Run state's probe result is not stored: not observable |
| M-R8 flags found constant | yes | `cmc_flags_c18`, `cmc_u68c`, `cmc_ubdc`, `cmc_bits_458`, `cmc_u308`, `cmc_u380`, the four external-velocity vectors, `queue_u258`, `anim_notify_timing_390`, `pawn_restrict_e60`, `pawn_floating_d80`, `pawn_battle_bits` (+0x630), `pawn_time_dilation` (+0xA0) |
| M-R9 console variables | yes | `cvar_penetration_pullback`, `cvar_int_87c9510`, `cvar_int_87c9514`, `cvar_int_87c9850`, `jump_key_setting` (once; the self test shows the expected values next to them) |
| M-R10 montage time multiplier | yes | `mi_bs_len` (the object at montage instance +0x10; empty when it is null, i.e. the multiplier is not applied) |
| M-R11 input layer | partly | `key_lx_raw` / `key_lx_value` (and LY, RX, RY): raw and massaged per frame, which shows the dead zone, exponent and inversion in effect; `pad_*` beside them. The `InputAxis` call arguments (delta, gamepad flag, caller) and the axis-properties map are not recorded (the map's element layout was not derived) |
| M-R12 camera rotation the controller reads | yes, by proxy | `cam_override_bits` (+0x290C), `cam_override_rot_*`, `cam_rot_*` at the end of every frame; the yaw the controller actually used is recovered from `ctl_move_yaw` and compared with the previous and the same frame's camera ("camera yaw" observation) |
| A-R2 saved-pose recurrence | yes | `sp_{center,l_momo,kubi}_{q,t}` every frame, `proxy_flags_528`, `proxy_initial_pose_530`. The answer: does the saved pose change every frame during a blend-in (recursive) or stay as it was at the play call (snapshot). The mesh's final LOCAL transforms are not recorded (offset not derived); the component-space ones are (`cst*`) |
| A-R3 montage weights and positions | mostly | the `FullBody` instance: `mi_pos`, `mi_rate`, `mi_weight`, `mi_desired_weight`, `mi_playing`, `mi_notify_offset_17c`, `mi_flags`, `mi_bs_current_198_*`, `mi_bs_input_*`, raw +0xA0 … +0x260 (`mi_raw*`, covers the sample cache and filter header); `anim_montage_count`. Not recorded: the other instances of the array, the filter's entries behind its pointer, and the slot node's three weights (the node's offset inside the blueprint-generated instance is not in the sheets) |
| A-R4 leg IK alpha timeline | yes, except one value | `leg_ik_alpha`, `leg_ik_offset`, `ikinema_sw`, `pawn_leg0_interp_*`, `pawn_leg1_interp_*`, `pawn_ik_disable`, `pawn_ik_init_value`. The float returned by the game instance's virtual is a return value: not observable |
| A-R5 leg IK solve | partly | the result: component-space `center`, `koshi`, `L/R_momo`, `L/R_sune`, `L/R_ashi1`, `L/R_ashi2` after the graph (both buffers, `mesh_read_buffer` says which is current), with `pos_z`, `floor_dist`, `floor_hit_point_z`. Not observable: the node's internals (its offset inside the instance is not in the sheets), the sweep's arguments and result, the pose before the node |
| A-R6 sole height | yes | `cst*_l_ashi1/2_t_z`, `cst*_r_ashi1/2_t_z` + `mesh_world_pos_z` against `floor_hit_point_z` (bone origins; the sole vertices are a fixed offset below the toe bone) |
| A-R7 root handling | partly | `root_motion_params_*` (+0x940, the transform), `root_motion_weight`, `cmc_root_vel_a24_*`, `pawn_battle_bits` bit 0, component-space `kh_sk` (`cst*_kh_sk_*`). The local `kh_sk` after the slot node is mid-graph: not observable |
| A-R8 pose extraction | partly | `proxy_bytes_3e0_*`, `proxy_i230`. A virtual's result and the time argument of a call: not observable |
| A-R9 look-at and face | mostly | `look_at_rate`, `face_anim_blend_rate`, `blend_*_weight`, `anim_blend_body_weight`, `pawn_f788`, `ctl_look_target_954/994`. The `Face` / `Mouth` slot weights: not recorded (slot node offsets) |
| A-R10 KineDriver | **no** | everything asked is at entry / return of one function |
| A-R11 Bonamik | partly | `kbd_*` timers, `bonamik_bucket_3824`, `bonamik_step_3830`, `bonamik_sim_time_3714`, `bonamik_weight_36f8`, `bonamik_reset_33f4`, `bonamik_chain_roots` (37 or 49), `bonamik_particles`, `bonamik_global_874e21c`, `mesh_read_buffer` / `mesh_edit_buffer`. Not recorded: the 57 particles' state (can be added as rows if wanted: 57 × 13 floats per frame), the 86 driven bones before / after, random values and call counts (mid-frame) |
| camera spec section 7 | yes, except its entry hooks | section 5 |

Reading these would need code inside the game (hooks or a debugger), which this tool does not do
by design: function entry order and thread ids, call counts, arguments and return values (perch
chain, step-up sweeps, ground-probe calls, `ImpactNormal` of move sweeps, KineDriver, the IK
sweep), and intermediate poses inside the anim graph.

**Answers `kh-replay` prints directly** (module `answers`; each only when the capture contains the
situation): the install's jump key; launch delay per jump in frames and seconds from the game's own
press event to the Jump state, to `bPressedJump`, to Falling, with the launch `Velocity.Z`; stick
+ jump on the same frame from Idle (the state that frame ends in); take-off speed dip of a running
jump; hang-phase tick count, timer and measured acceleration; rising / falling acceleration;
sprint onset in seconds of run loop; per play the montage weight ramp (frames and seconds below 1
against the montage's blend-in time, first-frame weight, largest gap to a linear ramp, the first
samples); saved pose rewritten every frame or held; leg IK alpha range per state and each moment
it starts to move, with its rate; ankle and toe bone heights above the floor while standing;
Bonamik chain-root count; camera `stickWait` peak, delay until `autoPower` rises and its rate;
camera distance identity; plus the observations of section 6 (tick order A / B, camera yaw latency,
dead zone, effective values, standing height).

## 8b. Requests added after section 8a (not yet rows of the field table)

| request | from | what to record | what it settles |
|---|---|---|---|
| G-J1 jump out of a guarded hit | actions spec II.1 (Guard), coordinator decision 2026-10-11 | per frame through: guard (X, stick neutral) → an enemy hit is guarded (`GuardHit` plays) → B pressed 0.3 – 0.5 s into the hit clip: `state_id`, sub-step (state +0x78), Guard state +0x84 (u32: bit 0 `m_bEnableChange`, bit 5 `m_bEnableInput`), Guard +0xDC (u8), the queue's entry count and next id, `mi_pos` of the `FullBody` montage, `pressed_jump`, `vel_z` | whether state id 3 begins on the frame after the press (the port's reading: `JumpRequest` 0x14143D8F0 → `ClearQueue` 0x1405A8010 empties the whole queue, so the Guard is replaced at once) or only when `GuardHit` ends at 0.9667 s (the first-pass reading: the jump waits behind the Guard because its `TresEnableChange` handler never sets `m_bEnableChange`). Port switch: `guard::JUMP_OUT_OF_GUARD_AT_CLIP_END` |
| C3-1 Sonic Rave start clip | combat spec 10.6 (third pass 2026-10-12) | the FullBody slot's animation name on the first frames of state 33 for a Sonic Rave (finish level 2) started standing on the ground | by the code `SetMovementMode(5)` (0x1413E2569) precedes the `OnGround` test (0x1413E257A), so the clip is always `M_1424`; `M_1423` would mean the mode change is deferred. The port follows the code (`style_finish.rs` `SONIC_RAVE_START_BY_CODE`) |
| C3-2 form clock during the transformation | combat spec 10.4 | controller+0xB54 (f32) per frame from the Triangle press through state 31 to the first Idle frame | by the code the gauge tick 0x14141C4F0 returns while the state is STYLECHANGE: +0xB54 must stay at its full value (30.0) until state 31 ends. Revision 2 said it counts (27.8 s left) |
| C3-3 start of the stun state | combat spec 9.9.2 | per frame after a hit whose row has `TRES_BADSTAT_STUN`: pawn+0x85D (u8), pawn+0x860 (f32), `state_id`, sub-step, on-ground; once taken on the ground, once in the air | who calls the stun trigger 0x1405B2A50 (anim-interface slot 78): the port tests it every pawn tick, so a grounded stun replaces the flinch at the next `MainProc`; if the game keeps the Damage state first, the caller is an event (landing / reaction end) |
| C3-4 stun shake-off | combat spec 9.9.2 | in state 122: pawn+0x860 per frame while (a) flicking the left stick, (b) pressing each face button in turn | each stick flick must take 0.3 s off; which button the second source (`WasInputKeyJustPressed` of the key object at 0x149C6F9F8) is |
| C3-5 burn | combat spec 9.9.4 | in state 124: pawn+0x860, HP (pawn+0x5FC), Velocity, MaxWalkSpeed (movement+0x388), GravityScale (movement+0x2F8), state+0x8C / +0x94 / +0x98 per frame | hop height 200, speed 300, gravity scale 2, 5 % of max HP per hop after the first; who writes state+0x8C bit 0 / +0x94 (a hop direction given from outside) |
| C3-6 Final Blow | combat spec 5.6.4 | Attack state +0x278 (ptr), +0x234 (u32), +0x258 (f32), +0x1E1 (u8), +0x1B0 (i32) and the actor location per frame from a guard counter's hit through the next attack press; the victim's request-interface slot 89 value if reachable | that bit 19 arms on the counter's hit, the warp countdown is 0.2 s, the teleport ends 150 cm from the target's lock point and 100 cm below it; what the victim values 2 / 3 / 4 are |
| C3-7 map-hit recoil | combat spec 8.5.1 | Attack state +0x264 (u32), +0x268 (i32) and `state_id` per frame while the first swing of a combo (no target) is made against (a) a BSP wall, (b) a static-mesh wall | that the level repels the swing only inside the reflect window 0.2333–0.3333 s of clip 1301, and which kinds of level geometry answer GUARD / REFLECT (`AActor::m_DefaultAttackHitResponse`, actor+0x3B8) |
| C3-8 lock-on camera distance | combat spec 7.12 | lock-on camera actor (`TresCameraNormalLoc`): +0x9E0, +0x9E4 (f32 distance wanted / smoothed), +0x9E8 and +0x9F4 (vec3 leads), +0xA9C (f32 release interpolator alpha), component location per frame, target dead ahead at 3 m / 8 m / 20 m and after orbiting 90° | the solver as read keeps the distance at 400 for a target straight ahead and widens it from the side; the 0.6 s release curve (interpolator type 2) |
| C3-9 Auto-Finish | combat spec 11.5 | HUD command part: +0x708 (i32), +0x9BB (u8), +0xAC8 (i32), +0x7D0 (i32), first entry of the array at +0x820 field +0x18 (f32) per frame over the last 2 s of a Second Form with AUTO_FINISH equipped, in and out of battle | what hud+0x708 is ("in combat" assumed) and that the finish command fires at 0.5 s left |
| C4-1 Last Arcanum count | combat spec 10.9 (fourth pass 2026-10-12) | StyleFinish state +0x128 (i32), +0x141 (u8), +0x28C (u32), +0x288 (i32) per frame through a Last Arcanum with every extra used | by the code the count rises by one at every hit window (0x1413DD7D0: seven in `M_1427`) and `finishEnd` (+0x141) stays 0 to the end; whether the HUD's `LMEX_THEEND` extra is gone after the `remove(LMEX_BREAK)` call |
| C4-2 sneeze latch | combat spec 9.9.5 | controller +0x920 (u8), pawn +0x85D (u8), Attack state +0x128 / +0x1B0 (i32) per frame through a ground combo with the SNEEZE status | the latch rises when the step `MaxCombo - 2` is played and the next attack is kind 0x1E (the sneeze replaces the finisher); cleared at the chain's end |
| C4-3 attack camera | combat spec 7.12.1 | controller +0xAB0 (i32), +0xAD8 (ptr), +0xAE0 (u8); normal camera +0x44C, +0x450, +0x804, +0x918 (f32) per frame through a burn, a Buzz Saw out of a pole spin and a Sonic Rave | modes 4 / 5 / 3 as read (lag 2.0 / 2000 and yaw limit 0; distance scale 1.6; lock on the dash target); what +0xAE0 is (scale 1.35 at the off) |
| C4-4 hit sweep frame | combat spec 8.2.2 | Attack state +0x164 (u8) and pawn +0xE24 (i32 hits landed) per frame with a target inside the blade at the window's first frame; `PrimaryActorTick.TickGroup` (actor+0x48, u8) of Sora's pawn and of the `ATresAtkCollManager` | that a window opened in frame N hits in frame N (manager in group 8, pawn in group 3) |
| C4-5 body push | combat spec 6.5 | game object (0x141522B30) +0x84C (u8); Sora's `MyBodyColl` +0x498 (vec3 push), +0x4A4 (u8 level) per frame while walking into an enemy and during a formchange finish | which mode uses the sequential variant; the level 8 during STYLEFINISH |
| C4-6 Sonic Cleave descent | combat spec 4.3 / 8.8 | Attack state +0x1D4 (f32 DirZ), +0x228 (i32 Z phase), +0x234 / +0x23C (u32), movement Velocity and location per frame through a Sonic Cleave that misses | vz = 0 while the traction window is open (bit 3), DirZ −1000 kept (bit 4) and written after 1.0 s; no `SHOCKWAVE_S` without a hit |

## 9. What was verified offline, and what was not

Verified (`probe.bat test`):

- `node tools/probe_fields.js`: every `Class.Property` resolves in the sheets; every literal offset
  names its evidence; the four code pages are identical in the installed exe and in `base/KH3.exe`
  and none has a relocation.
- The probe compiles without warnings (MSVC 14.44, `/W3`).
- `node tools/probe_test.js` builds a **synthetic memory image from the real field table** (each
  path planted by running it backwards, a value in every field, a chunked name table with ANSI,
  wide and numbered names, weak-pointer table, decoys in every scanned array, a frame counter that
  advances as it is polled and skips every fifth frame) and runs the real `kh3probe.exe` on it:
  the self test resolves all 46 nodes and prints every planted value; a 40-frame capture matches
  the planted values in all cells, reports the skipped frames as `missed`, writes the `once` and
  end lines; `--no-explore` drops the explore columns. One changed code byte → exit code 3, no
  CSV. A null pawn → the capture continues with empty pawn cells and the self test says
  `NOT RESOLVED` (exit 5).
- Against live processes that are not the game (`node.exe`, checked also with `explorer.exe`):
  opened read-only, refused at the build check with exit code 3; a missing process: exit code 2.
- `cargo test -p kh-replay` (18 tests; the two in `tests/answers.rs` check the direct answers on a hand-made capture with known launch delay, speed dip, hang ticks, sprint onset, weight ramp, saved-pose behaviour, leg IK ramp and camera follow timing): a capture **made by `kh-core` itself** (stand, run, sprint,
  180° turn, stop, held jump, walk, running tap jump) replays with **zero divergence**, also with
  the frame time varying ±30 % per frame and at 0.25× / 3× frame length; a capture shifted by 3 cm
  from one row on is reported at exactly that frame and in no other quantity; a single bad sample
  is one row; a changed sub-step and a changed landing animation are reported at their frames; a
  jump whose key press is missing from the input columns diverges at the frame the game enters the
  Jump state; after a divergence the next action is judged on its own; missed frames, paused rows,
  torn rows and rows without a pawn are handled; a run off a 2 m block diverges on a flat floor and
  does not with the block described in a world file.

Not verified — needs the first self test and captures:

- every pointer path and offset against the live process (section 4's confidence column);
- that plain `ReadProcessMemory` is permitted on the running game (if it is not, the probe says
  "access denied" and that is the result to report);
- the timing claims of section 3 on the real game (`missed` / `torn` / `read_us` will show them);
- whether XInput in the probe's process sees the pad (the game uses its own input plugin; with a
  non-XInput pad the `pad_*` columns stay 0 — the replay does not need them, it uses the game's
  own key state and stick values).

## 10. Frame time: what `kh-core` supports and what a 1:1 match needs

The original ticks Sora **once per rendered frame with that frame's dt**; `kh-core` is hosted
with a fixed 1/60 s step (`kh-app`, port plan section 3). Findings (reading `crates/kh-core`,
`cargo run -p kh-replay --example dt_survey`, and the replay tests):

- `Sora::step(dt, …)` already takes the dt in every formula (state timers, slerp factors,
  `CalcVelocity`, braking sub-steps, the `PhysWalking` / `PhysFalling` sub-step loops with 0.1 s /
  4 iterations, the animation clock, notify crossing, root motion). Stepping it with dt from
  1/240 to 1/20 s, constant or changing every step, gives finite and consistent runs (held jump:
  apex 266.13 cm at 30 and 60 per second, 266.23 at 120 / 144 and with ±50 % jitter; the same
  animation sequence). `dt = 0` is harmless (nothing moves, everything finite).
- So **for the replay nothing in the core has to change**: `kh-replay` passes the recorded dt.
- What is fixed-60 today is the *contract* and the *host*: the doc comments of `lib.rs` /
  `sora.rs` ("always pass the same value"), `kh-app`'s accumulator loop (`player.rs`),
  `tools/port_smoke.js` and `tests/sora.rs`, which assert frame counts at 60 Hz.
- The original is itself frame-rate dependent in its per-tick rules, and `kh-core` reproduces them
  per step: ground acceleration `MaxWalkSpeed × 60` (full speed in one tick only at ≤ 1/60 s),
  the launch on the tick after the `TresJumpStart` notify, one-tick input latches, slerp factors
  clamped to 1, the short-hop test on animation time. At exactly 60 frames per second a fixed
  1/60 s step and the original coincide; at any other frame rate they do not, by the frame's
  quantisation.
- For a 1:1 match at other frame rates the host would have to step once per rendered frame with
  the frame's dt (as the original does) instead of accumulating fixed steps — that conflicts with
  the fixed-step rule the test level was built to, so it is a decision, not a fix. If taken:
  an option in `kh-app` (`--variable-dt`: one `step(frame_dt)` per frame, dt clamped as the
  engine clamps it — `world_dt` against `app_dt` in a capture shows the clamp), the contract text,
  and tests at other rates. Whether the captures show any *further* dt dependence (a rule that is
  per-frame in the game but per-second in the core, or the other way round) is exactly what
  `kh-replay` reports once real captures exist: record run 3 and run 8 again at an unlocked or
  30-per-second frame rate and compare.

## 11. Files

`tools/probe_fields.spec.json`, `tools/probe_fields.js`, `tools/probe_test.js`,
`tools/native/kh3probe.c`, `tools/native/build.ps1` (target `kh3probe`), `probe.bat`,
`crates/kh-replay/` (`src/{capture,replay,facts,synth,world}.rs`, `src/main.rs`, `tests/replay.rs`,
`examples/dt_survey.rs`), `build/gen/probe/fields.json` and `fields.md` (generated),
`build/native/kh3probe.exe`, `build/probe/` (recordings and self-test reports),
`build/sora_tmp/PROBE_STATE.md` (progress note), scratch in `build/sora_tmp/probe/`.
