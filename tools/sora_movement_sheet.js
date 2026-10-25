// Writes sheets/port/sora-movement.json: the fixed parameters of Sora's on-foot movement and jump (docs/sora-movement-spec.md)
// Authored rows: every value was read from base/KH3.exe (constructor or code instruction), a class default object of
// Blueprints/Player/p_ex001, a converted animation or an ini file; the row says which. Unknown values are null, never guessed.
//
//   node tools/sora_movement_sheet.js          write the sheet
//   node tools/sora_movement_sheet.js --check  exit 1 if the sheet on disk differs from the rows below
//
// ref_conf: confirmed = value and use read at instruction level or in a class default object / ini;
//           inferred  = derived from confirmed values, animation data, or stock UE 4.17 behaviour identified by shape.
const fs = require('fs'), path = require('path');
const VERSION = '2.0.0';
const R = path.resolve(__dirname, '..');
const BP = 'assets_converted/packages/KINGDOM HEARTS III/Content/Blueprints/Player/p_ex001/p_ex001_PawnBase.uasset.json';
const AN = 'assets_converted/animations/KINGDOM HEARTS III/Content/Character/pc/p_ex001/anm/rt/body/';
const C = 'confirmed', I = 'inferred';
// id, value, unit, source, evidence, ref_addr, ref_conf, note
const rows = [
['gravity.scale',1.5,'-',BP+' export TresCharMovement0 property GravityScale','class default object value; native default 1.0 at 0x1405710F4; used by GetGravityZ 0x140582270 (same)','0x140582270',C,'restored by pawn slot 715 0x1405B6230 at the jump apex and at Jump end'],
['gravity.rate',1.0,'-','UTresCharMovementComponent ctor 0x140570EB0, mov dword [rdi+0BF8h] at 0x140571808','unreflected multiplier in GetGravityZ 0x140582270 (same)','0x140582270',C,'changed by glide/floating states only'],
['gravity.jump_up_scale',2.8,'-',BP+' export TresCharMovement0 property m_JumpUpGravityScale','class default object value; native 1.0 at 0x1405717F3; applied while Velocity.Z > 0 in 0x140577F30 (movss [rbx+0BE8h] at 0x140577F7A)','0x140577F30',C,'only while a DoJump is in progress (flag +0xBFC bit 0 clear)'],
['gravity.jump_duration_time',0.15,'s',BP+' export TresCharMovement0 property m_MaxJumpDurationTime','class default object value; native 0; compared with the timer +0xC00 in 0x140577F30 (0x140577FAE..0x140577FC4)','0x140577F30',C,'hang phase length from the apex; timer advances by the PhysFalling sub-step'],
['gravity.jump_duration_scale',0.1,'-',BP+' export TresCharMovement0 property m_JumpDurationGravityScale','class default object value; native 1.0 at 0x1405717FE; movss [rbx+0BF0h] at 0x140577FCE','0x140577F30',C,'gravity multiplier during the hang phase'],
['gravity.small_jump_scale',3.0,'-','ATresPlayerPawnBase ctor 0x141423210, MySmallJumpGravityScale (pawn+0xB8C) at 0x1414237C1; not present in the Sora blueprints','written to GravityScale by TresPlayerState_Jump::Tick 0x141395450 at 0x141395D43 and 0x141395EF5 when the jump button is not held while rising','0x141395450',C,'replaces GravityScale 1.5 (not multiplied) until the apex'],
['jump.power_normal',1480,'cm/s',BP+' export Default__p_ex001_PawnBase_C property MyJumpPowerNormal','class default object value; native 800 at 0x14142377B; selected by pawn slot 1347 0x141434010, written to JumpZVelocity by pawn slot 837 0x1405B97C0; DoJump 0x14057ECB0 (same) sets Velocity.Z = JumpZVelocity','0x14057ECB0',C,'stage 1 jump type 0 (no high jump ability)'],
['jump.max_height',266.1,'cm','derived: 1480^2 / (2 * 980 * 1.5 * 2.8); formula of GetMaxJumpHeight 0x140583A40','derived value, button held','0x140583A40',I,'rise time 0.360 s'],
['speed.run',680,'cm/s','ATresPlayerPawnBase ctor 0x141423210: MyMovement->MaxWalkSpeed at 0x141423412','no blueprint override; ResetMaxWalkSpeed (pawn slot 687 0x1405B6350) restores the class default','0x141423210',C,'GetMaxSpeed 0x140583BF0 returns it for Walking and Falling'],
['speed.sprint',980,'cm/s','ATresPlayerPawnBase ctor: MySprintSpeed (pawn+0xB28) at 0x141423709','SetMaxWalkSpeed(MySprintSpeed) in TresPlayerState_Run::Tick 0x1413AED70 at 0x1413AFEE3','0x1413AED70',C,'-'],
['speed.sprint_delay',1.0,'s','ATresPlayerPawnBase ctor: MySprintDelay (pawn+0xB2C) at 0x141423713','timer state+0xB8 compared in 0x1413AED70 (0x1413AFD77..0x1413AFEF0)','0x1413AED70',C,'continuous running in Run sub-step 0'],
['speed.walk_rate',0.205882,'-',BP+' export TresCharMovement0 property WalkSpeedRate','class default object value 0.2058819979429245; native 0.16 at 0x14142341C; becomes AnalogInputModifier in walk mode in 0x14057D050','0x14057D050',C,'walk speed = 680 * 0.205882 = 140 cm/s'],
['speed.force_walk',140,'cm/s','ATresPlayerPawnBase ctor: MyForceWalkSpeed (pawn+0xB30) at 0x141423723','used only with the pawn restrict-run flag','0x141423210',C,'stub in stage 1'],
['input.stick_active_min',0.1,'-','ATresPlayerControllerSora input update 0x1413F1AB0, comiss at 0x1413F206D','magnitude of the massaged stick vector, not clamped to 1','0x1413F1AB0',C,'-'],
['input.walk_run_threshold',0.8,'-','TryRun 0x141440030, comiss at 0x141440092','walk when the magnitude is below it','0x141440030',C,'-'],
['input.run_grace_time',0.2,'s','TresPlayerState_Run::Tick 0x1413AED70, immediate 0x3E4CCCCD at 0x1413B09C0','run mode and run animation persist this long after the stick drops below the threshold','0x1413AED70',C,'-'],
['accel.ground_factor',60,'1/s','TresPlayerState_Run::Tick 0x1413AED70: MaxAcceleration = MaxWalkSpeed * 60 (0x1413B06FE, 0x1413B09F9, 0x1413B0C39)','pawn slot 712 0x1405B98A0 writes MyMovement->MaxAcceleration','0x1413AED70',C,'40800 cm/s^2 running, 58800 sprinting'],
['accel.air_factor',5.4,'1/s','TresPlayerState_Jump::Begin 0x1413868C0, constant at 0x141386E5D','MaxAcceleration = default MaxWalkSpeed * 5.4 = 3672 cm/s^2','0x1413868C0',C,'not reset at Jump end'],
['accel.default',1000000,'cm/s^2','UTresAICharMovementComponent ctor 0x140512380 at 0x14051240A','value before any Run or Jump state has run','0x140512380',C,'-'],
['air.control',0.95,'-','ATresPlayerPawnBase ctor: MyMovement->AirControl at 0x141423454','GetAirControl 0x140580B30 = AirControl * FallAcceleration, no boost','0x140580B30',C,'air input acceleration 3672 * 0.95 = 3488 cm/s^2'],
['air.braking_deceleration',2048,'cm/s^2','ATresPlayerPawnBase ctor: BrakingDecelerationFalling at 0x14142345E','passed to CalcVelocity by PhysFalling 0x14058D0C0','0x14058D0C0',C,'-'],
['air.lateral_friction',0,'1/s','UTresCharMovementComponent ctor: FallingLateralFriction at 0x1405711B5 (qword store)','passed to CalcVelocity by PhysFalling 0x14058D0C0','0x14058D0C0',C,'-'],
['ground.friction',8.0,'1/s','UTresCharMovementComponent ctor: GroundFriction at 0x1405710FE','passed to CalcVelocity by PhysWalking 0x1405900B0','0x1405900B0',C,'-'],
['ground.braking_friction_factor',2.0,'-','UTresCharMovementComponent ctor: BrakingFrictionFactor at 0x1405711CA','ApplyVelocityBraking 0x140577890 (same)','0x140577890',C,'-'],
['ground.braking_deceleration',2048,'cm/s^2','UTresCharMovementComponent ctor: BrakingDecelerationWalking at 0x1405711D4','passed to CalcVelocity by PhysWalking 0x1405900B0','0x1405900B0',C,'-'],
['ground.stop_accel_sq_max',1.0,'(cm/s^2)^2','TresPlayerState_Run::Tick 0x1413AED70, 0x1413B085C..0x1413B0876','Acceleration.XY squared at or below it with no move request: velocity zeroed and the stop animation starts','0x1413AED70',C,'-'],
['turn.run_alpha_per_60hz_frame',0.1,'-','TresPlayerState_Run::Tick 0x1413AED70: 0.1 at 0x1413AF153, * dt * 60 at 0x1413B0982','slerp alpha = clamp01(0.1 * dt * 60); slerp helper 0x1405DDD90 (same)','0x1413AED70',C,'-'],
['turn.run_stop_rate',12,'1/s','TresPlayerState_Run::Tick 0x1413AED70 at 0x1413B03E3','alpha = clamp01(dt * 12) during the stop animation','0x1413AED70',C,'-'],
['turn.air_rate',6,'1/s','TresPlayerState_Jump::Tick 0x141395450 at 0x141397886','alpha = clamp01(dt * 6)','0x141395450',C,'-'],
['turn.idle_rate',9,'1/s','TresPlayerState_Idle::Tick 0x1413944D0 at 0x141394FDF','alpha = clamp01(dt * 9) toward the own yaw','0x1413944D0',C,'-'],
['turn.turn_around_angle',75,'deg','TresPlayerState_Run::Tick 0x1413AED70 at 0x1413AF8E0','yaw delta at or above it while running: velocity zeroed, RunStart animation replayed','0x1413AED70',C,'-'],
['turn.rotation_rate_yaw',1000,'deg/s','ATresPlayerPawnBase ctor: RotationRate at 0x1414233E9','unused: PhysicsRotation 0x140590DA0 returns early (bOrientRotationToMovement and bUseControllerDesiredRotation are 0)','0x140590DA0',C,'do not implement'],
['floor.walkable_angle',49,'deg','UTresCharMovementComponent ctor: WalkableFloorAngle at 0x140571134','no blueprint override','0x140570EB0',C,'-'],
['floor.walkable_z',0.656059,'-','UTresCharMovementComponent ctor: WalkableFloorZ 0x3F27F37C at 0x14057113E','IsWalkable 0x140589A60 compares ImpactNormal.Z with it','0x140589A60',C,'cos 49 deg'],
['floor.max_step_height',47,'cm',BP+' export TresCharMovement0 property MaxStepHeight','class default object value; native 45 at 0x140571148','0x140570EB0',C,'-'],
['floor.always_check',true,'-','ATresPlayerPawnBase ctor: bAlwaysCheckFloor at 0x14142347C','-','0x141423210',C,'-'],
['floor.can_walk_off_ledges',true,'-','UTresCharMovementComponent ctor: or dword [rdi+460h],72h at 0x140571376','-','0x140570EB0',C,'-'],
['floor.maintain_horizontal_ground_velocity',true,'-','UTresCharMovementComponent ctor at 0x140571383','-','0x140570EB0',C,'-'],
['capsule.radius',60,'cm',BP+' export TresRoot property CollisionShapesSrc[0].Size.X (ShapeType CAPSULE1)','UTresRootComponent body build 0x1405DF310: Sphyl radius = max(Size.X, 1) (decompiled C only)','0x1405DF310',I,'-'],
['capsule.half_height',89,'cm',BP+' export TresRoot property CollisionShapesSrc[0].Size.Z','0x1405DF310: Sphyl length = 2 * (Size.Z - Size.X) = 58 (decompiled C only)','0x1405DF310',I,'total half height including the hemispheres'],
['capsule.center_z',91,'cm',BP+' export TresRoot property CollisionShapesSrc[0].RelativeLocation.Z','0x1405DF310: Sphyl centre = RelativeLocation','0x1405DF310',I,'capsule bottom is 2 cm above the actor origin'],
['sim.max_time_step',0.1,'s','UTresCharMovementComponent ctor: MaxSimulationTimeStep at 0x14057118D','-','0x140570EB0',C,'stock UE default is 0.05'],
['sim.max_iterations',4,'-','UTresCharMovementComponent ctor: MaxSimulationIterations at 0x140571197','-','0x140570EB0',C,'stock UE default is 8'],
['fall.step_down_height',40,'cm','TresPlayerState_Jump::Begin 0x1413868C0 at 0x141386BCF; Tick at 0x141395F2E','ground distance limit for the step-down case','0x1413868C0',C,'-'],
['fall.step_down_time',1.0,'s','TresPlayerState_Jump::Tick 0x141395450 at 0x141396077','run animation kept at most this long','0x141395450',C,'-'],
['land.light_height_max',75,'cm','TresPlayerState_Jump::Tick 0x141395450 at 0x14139628B','fall height at or below it plays LightLand','0x141395450',C,'-'],
['land.roll_height_min',800,'cm','TresPlayerState_Jump::Tick 0x141395450 at 0x1413962DC','fall height at or above it with the stick active plays RollLand','0x141395450',C,'-'],
['land.length',0.4,'s',AN+'0034.anim.json and 0120.anim.json SequenceLength','state returns to Idle when the animation ends','0x141395450',I,'Land and LightLand'],
['land.roll_control_return',0.4325,'s',AN+'0108.anim.json notify TresEnableChange LinkValue','-','0x1411FBA40',I,'RollLand'],
['land.roll_length',0.7667,'s',AN+'0108.anim.json SequenceLength','-','0x141395450',I,'RollLand'],
['anim.run_start_length',0.2333,'s',AN+'0007.anim.json SequenceLength (samples 0007, 0007L, 0007R, 0007BL, 0007BR)','no movement input during it; root motion only','0x1413AED70',I,'-'],
['anim.run_end_length',0.5,'s',AN+'0011.anim.json and 0012.anim.json SequenceLength','-','0x1413AED70',I,'-'],
['anim.walk_end_length',0.5667,'s',AN+'0005.anim.json and 0006.anim.json SequenceLength','-','0x1413AED70',I,'-'],
['gravity.world_z',-980,'cm/s^2','UPhysicsSettings ctor 0x1443FB130 (+0x50, 0xC4750000); assets/Engine/Config/BaseEngine.ini line 1654 DefaultGravityZ; no override in the game ini','AWorldSettings::GetGravityZ 0x1445C4F70 falls through to DefaultGravityZ; chain 0x144016000 -> 0x144017730 -> 0x1444324D0 -> 0x1445C4FF0 read; none of the 3809 cooked maps sets a gravity key','0x1445C4F70',C,'no physics volume in the Olympus maps; one water volume in the whole game'],
['gravity.terminal_speed',4000,'cm/s','UPhysicsSettings ctor 0x1443FB130 (+0x54, 0x457A0000) -> APhysicsVolume ctor 0x144421C30 store at 0x144421CF0 (volume+0x420); DefaultEngine.ini line 2952 is commented out','NewFallVelocity 0x14058BCA0: OverrideMaxFallingSpeed (ctor 0 at 0x14057117D) > 0 ? it : |volume.TerminalVelocity|','0x14058BCA0',C,'-'],
['jump.launch_delay',0.0666,'s',AN+'0031.anim.json notify TresJumpStart LinkValue (JumpNormalStart, SequenceLength 0.1)','notify fires in the pose tick inside the movement tick (0x140595730 -> pawn slot 365 0x1405AF030); handler 0x14139AB60 sets the pawn jump latch; DoJump runs in the NEXT movement tick (CheckJumpInput 0x1405A5B20)','0x14139AB60',C,'at 60 Hz: press frame 0, clip first advanced in frame 1, notify in frame 4, DoJump in frame 5'],
['jump.small_jump_min_time',0.1,'s','TresPlayerState_Jump::Tick 0x141395450, comiss/jbe at 0x141395D36 (strict >)','sub-step 1 test on the current animation time; JumpNormalStart length is the same f32 (0x3DCCCCCD), so the test never passes for the normal jump','0x141395450',C,'the short hop can only start on the first sub-step-2 tick (0x141395EE5), i.e. about 0.1 s after the state begins'],
['jump.max_hold_time',0,'s','JumpMaxHoldTime (pawn+0x5C8): no constructor or code in the pawn ranges writes it','ClearJumpInput 0x1405A5D80 (called in PerformMovement at 0x14058CE88, before StartNewPhysics) clears bPressedJump in the tick of the notify; the latch pawn+0x5C0 bit 0 carries the request to the next CheckJumpInput','0x1405A5D80',C,'holding adds no force; one impulse per jump'],
['jump.coyote_time',0,'s','jump gate 0x14142A3D0: IDLE needs on-ground (0x14142A47F), RUN needs on-ground (0x14142AD11)','no timer on the path','0x14142A3D0',C,'no coyote time'],
['jump.buffer_time',0,'s','controller update 0x1413F1AB0: WasInputKeyJustPressed at 0x1413F1BD9, JumpRequest at 0x1413F280B, result ignored','a refused press is dropped','0x1413F1AB0',C,'no jump buffering; a press on the frame the controller still sees landing sub-step 10 is lost'],
['input.axis_dead_zone',0.25,'-','assets/KINGDOM HEARTS III/Config/DefaultInput.ini lines 31-32 AxisConfig Gamepad_LeftX / Gamepad_LeftY (Sensitivity 1, Exponent 1, bInvert False)','UPlayerInput::MassageAxisInput 0x1445AF120 read at instruction level: v > 0 ? max(v - dz, 0) / (1 - dz) : -max(-v - dz, 0) / (1 - dz); no clamp; applied in ProcessInputStack at 0x1445B2524; GetKeyValue 0x1445A95B0 returns the massaged value','0x1445AF120',C,'the XInput layer 0x142DFA100 applies no dead zone (short * 1/32767, or 1/32768 when <= 0)'],
['input.move_scale',1.0,'-','TresPlayerState_Run::Tick 0x1413AED70 call at 0x1413B0C6F; TresPlayerState_Jump::Tick 0x141395450 call at 0x141397BC8','pawn slot 724 0x1405B9BD0 assigns ControlInputVector = (dir.x, dir.y, 0) and stores clamp01(scale) in pawn+0x63C','0x1405B9BD0',C,'speed is never proportional to the stick'],
['floor.dist_min',1.9,'cm','constant 0x145D3A028 in AdjustFloorHeight 0x1405744E0','floor distance kept inside 1.9..2.4 by one move to 2.15','0x1405744E0',C,'-'],
['floor.dist_max',2.4,'cm','constants 0x145D3A030 (2.4), 0x145D3A034 (2.4001), 0x145D3A058 (-2.4) in FindFloor 0x14057FA80','-','0x14057FA80',C,'-'],
['floor.dist_target',2.15,'cm','constant 0x145D3A02C in AdjustFloorHeight 0x1405744E0','one SafeMoveUpdatedComponent of (0, 0, 2.15 - FloorDist) per sub-step','0x1405744E0',C,'-'],
['floor.sweep_distance_walking',49.4001,'cm','FindFloor 0x14057FA80 at 0x14057FAF4..0x14057FB1D: max(2.4, MaxStepHeight + 2.4001)','sweep and line distance while walking','0x14057FA80',C,'-'],
['floor.sweep_distance_falling',44.6,'cm','FindFloor 0x14057FA80: max(2.4, MaxStepHeight - 2.4)','sweep and line distance while falling (landing test)','0x14057FA80',C,'-'],
['floor.perch_radius',1.0,'cm','GetValidPerchRadius 0x1405861F0 and ShouldComputePerchResult 0x140596260: max(max(PerchRadiusThreshold, 0), 1.0)','perch test runs whenever the floor contact is more than this far (2D) from the capsule axis; the stock threshold <= 0.15 early-out is absent','0x140596260',C,'perching is ACTIVE for Sora'],
['floor.falling_perch_additional_height',10,'cm','ATresPlayerPawnBase ctor 0x141423210 at 0x141423408 (component+0x448)','added inside MaxPerch in FindFloor 0x14057FA80 (0x14057FD88..0x14057FDBD) and in slot 260','0x14057FA80',C,'-'],
['floor.perch_additional_height',0,'cm','UTresCharMovementComponent ctor at 0x140571152 (component+0x444)','-','0x14057FA80',C,'-'],
['floor.max_perch_dist_walking',59.4001,'cm','derived in FindFloor 0x14057FA80: max(2.4, 2.4001 + 47 + 10) + max(0, 0)','an edge keeps Sora walking while the ground under the capsule axis is at most this far below the perch origin','0x14057FA80',C,'kerbs up to 59.4 cm are rolled down without leaving Walking'],
['floor.max_perch_dist_falling',54.6,'cm','derived in FindFloor 0x14057FA80: max(2.4, -2.4 + 47 + 10)','-','0x14057FA80',C,'-'],
['floor.perch_probe_min_half_height',15,'cm','constant 0x145C9C510 in ComputeFloorDist 0x14057D100 at 0x14057D376','perch capsule: radius 1.0, half height 15 - 1.4, centre 15 above the perch origin; second try radius 0.8499, half height 2.4','0x14057D100',C,'shrink scales 0.1 (0x145D3A004) and 0.9 (0x145D3A020)'],
['floor.sweep_edge_reject',0.15,'cm','constant 0x145D3A008 in IsWithinEdgeTolerance 0x140589BA0: dist2D^2 < max(R - 0.15, 1e-4)^2','used only inside the perch capsule sweep and (with radius literal 0) in StepUp','0x140589BA0',C,'-'],
['edge.fall_check_enabled',true,'-','ATresPlayerPawnBase ctor 0x141423210 at 0x141423494: m_bNeedFallTravelCheck (component+0x68C bit 1)','slot 260 0x1405887B0 part 1','0x1405887B0',C,'-'],
['edge.sphere_radius',25,'cm','constant 0x145CD77EC in slot 260 0x1405887B0','sphere multi-sweep from z0 + 37.5 to z0 - 5.0 at the pawn pushed max(24 - d, 0) away from the edge contact, when the contact is within 25 cm (625 = 25^2 at 0x145D3A048)','0x1405887B0',C,'keeps the floor when another component is found on the pawn side'],
['edge.contact_limit_walking',57,'cm','slot 260 0x1405887B0 at 0x140588D13..0x140588D5E: MaxStepHeight + max(FallingPerchAdditionalHeight + PerchAdditionalHeight, 0)','beyond this 2D distance from the contact the floor is dropped','0x1405887B0',C,'47 when falling'],
['edge.line_max_traces',4,'-','slot 260 0x1405887B0 at 0x1405890CE','line from the origin that slides down unwalkable faces (step back 0.5 cm, constant 0x145C9C4D0) looking for walkable ground','0x1405887B0',C,'-'],
['edge.float_enabled',true,'-','UTresCharMovementComponent ctor at 0x1405718FE: m_bCheckEdgeFloat (component+0x68C bit 2)','slot 409 0x14057BD10 runs in PhysWalking before CheckFall when the previous floor was walkable','0x14057BD10',C,'-'],
['edge.float_slide',3.225,'cm','constant 0x145D3A05C in slot 409 0x14057BD10 at 0x14057BEA6','slide (0, 0, -3.225) along the unwalkable floor normal; if it lands on a valid spot Sora stays Walking','0x14057BD10',C,'-'],
['step_up.floor_dist_offset',2.0,'cm','constant 0x145C9C4F4 in StepUp 0x140597570 at 0x1405976AC','FloorDist used by StepUp = max(CurrentFloor.FloorDist - 2.0, 0)','0x140597570',C,'-'],
['step_up.down_extra',4.8,'cm','constant 0x145D3A03C in StepUp 0x140597570 at 0x1405976CC','down sweep = MaxStepHeight + 4.8 = 51.8','0x140597570',C,'-'],
['step_up.side_z_max',0.08,'-','constant 0x145D3A000 in StepUp 0x140597570 at 0x140597AB7','reject when ending higher with no floor found and the step side is this steep','0x140597570',C,'-'],
['step_up.capsule_radius_arg',0,'cm','literal 0 passed to IsWithinEdgeTolerance at 0x1405976E0','StepUp uses no capsule dimensions at all','0x140597570',C,'the stock top-of-capsule and edge-tolerance rejections do not exist'],
['walk.ramp_flat_limit',0.9999,'-','constant 0x145CF35B8 in ComputeGroundMovementDelta 0x14057DC90','stock','0x14057DC90',C,'-'],
['walk.launch_z',200,'cm/s','constant 0x145C9C544 in PhysWalking 0x1405900B0 at 0x1405904D8','force-volume Z velocity above it leaves the ground; 0 without force volumes','0x1405900B0',C,'-'],
['move.penetration_default_depth',0.125,'cm','constant 0x145C9C4C0 in GetPenetrationAdjustment 0x144016CA0 at 0x144016D30','adjustment = Normal * ((depth > 0 ? depth : 0.125) + |p.PenetrationPullbackDistance|); no clamp to a maximum','0x144016CA0',C,'-'],
['move.penetration_pullback',null,'cm','console variable p.PenetrationPullbackDistance (pointer 0x149D86A48)','value not read; stock default 0.125','0x144016CA0','guess','runtime: float [[0x149D86A48]]'],
['move.over_max_speed_factor',1.01,'-','constant 0x1476162F4 in IsExceedingMaxSpeed 0x14401AA70','-','0x14401AA70',C,'-'],
['move.braking_sub_step',0.030303031,'s','constant 0x145D39FF8 (1/33) in ApplyVelocityBraking 0x140577890','-','0x140577890',C,'-'],
['move.spawn_frame_skip',true,'-','ATresPlayerPawnBase ctor at 0x1414233FB: m_bDisableMoveOnSpawnFrame; consumed in StartNewPhysics 0x140596E10 (component+0xB70 bit 0x20)','the first physics call after InitializeComponent is skipped','0x140596E10',C,'-'],
['move.encroach_check',3,'-','ATresPlayerPawnBase ctor at 0x141423486: bComputeEncroachCheck bits','slot 404 0x14057AE30 pushes the body out of overlaps before physics (decompiled C for the body of the function)','0x14057AE30',I,'no effect when nothing overlaps at tick start'],
['fall.two_wall_nudge',0.01,'cm','constant 0x145CADFC4 in PhysFalling 0x14058D0C0 at 0x14058DD69','fork block: two near-vertical walls -> Velocity.XY = 0, vertical slide + normal * 0.01','0x14058D0C0',C,'-'],
['fall.xy_zero_threshold',0.001,'(cm/s)^2','constant 0x145D39FEC in PhysFalling 0x14058D0C0 at 0x14058E23C','Velocity.XY zeroed when its squared size is at or below it','0x14058D0C0',C,'-'],
['gravity.hang_steps_60hz',8,'sub-steps','derived from 0x140577F30 (strict < at 0x140577FC1) with f32 accumulation of 1/60','4 at 30 Hz, 8 at 60 Hz, 17 at 120 Hz','0x140577F30',I,'the hang is 0.1333 s at 30 and 60 Hz, 0.1417 s at 120 Hz'],
['probe.range',2000,'cm','calls at 0x1413AF098, 0x14142D239, 0x141386AF9, 0x141395829','travel of the probe sphere','0x14063A650',C,'-'],
['probe.sphere_radius',50,'cm','constant 0x145C9C51C in the ground probe 0x14063A650 at 0x14063A98B','sphere sweep, channel Pawn, blocking WorldStatic / WorldDynamic / TresGimmick only, ignores the pawn','0x14063A650',C,'not a line'],
['probe.start_height',55,'cm','constant 0x145D785A4 in 0x14063A650 at 0x14063A717','sphere centre starts at actor origin + 55 and ends at origin + 55 - range','0x14063A650',C,'distance reported = actor origin Z - ImpactPoint.Z (can be negative)'],
['probe.max_slope',45,'deg','constant 0x145D3F970 (-45) in 0x14063A650 at 0x14063ABB5','a hit is accepted only when the surface is less steep than this; a steeper hit stores its distance but returns false','0x14063A650',C,'-'],
['probe.near_ground_max',2.0,'cm','Run probe 0x1413A3D20 (same) at 0x1413A3DD1: hit && dist < 2.0; cached path 0x1413A3D36: dist < 2.0 without the hit flag','Run does not fall while this holds','0x1413A3D20',C,'-'],
['land.snap_height',75,'cm','TresPlayerState_Jump::Tick 0x141395450 at 0x141397358..0x141397380','sub-step 10: if actor Z > ground Z and the difference <= 75: SetActorLocation (sweep) 0x143EAC3D0 to the probe ground Z','0x141395450',C,'lowers the origin by FloorDist - 2 on flat ground'],
['land.control_return',0.0991,'s',AN+'0034.anim.json and 0120.anim.json notify TresEnableChange LinkValue','sets m_bEnableChange through 0x1411FBA40; needed for the STICK to end the landing (0x1413973F4..0x141397411)','0x1411FBA40',I,'a JUMP is accepted from the first frame the controller sees sub-step 11, without this notify (0x14142A552)'],
['collision.profile','TresPlayerPawn','-','ATresPlayerPawnBase ctor at 0x14142339F: SetCollisionProfileName (FName 0x1487CABE8); assets/KINGDOM HEARTS III/Config/DefaultEngine.ini line 1397','object type Pawn, QueryAndPhysics; blocks WorldStatic, WorldDynamic, PhysicsBody, Vehicle, Destructible, TresEquip, TresProjectile, TresGimmick, TresSound, TresBlockDetect; ignores Pawn and the enemy / special pawn channels','0x141423210',C,'other pawns never block the capsule'],
['frame.order','controller, movement (input, jump check, pose tick + notifies, move), pawn tick with state MainProc','-','AController::AddPawnTickDependency 0x144078C80; UMovementComponent::RegisterComponentTickFunctions 0x14401DA60; ATresCharPawnBase::PostInitializeComponents 0x1405B4A30; ATresCharPawnBase::Tick 0x1405BC470 (MainProc call at 0x1405BC51D); 0x140595730','all four tick functions are in TG_PrePhysics; prerequisites controller -> movement -> pawn actor and movement -> mesh','0x1405BC470',C,'state writes reach the movement component one frame later; a landing is seen by the state in the same frame'],
['frame.max_delta_time',0.05,'s','assets/KINGDOM HEARTS III/Config/DefaultEngine.ini [/Script/Engine.GameEngine] MaxDeltaTime; read at 0x1445913F3','frame time is variable and clamped to this; no fixed step','0x144590F00',C,'-'],
['anim.pose_tick_in_movement',true,'-','UTresCharMovementComponent ctor at 0x14057185E: component+0xC18 bit 0x20, never cleared','0x140595730 ticks the pose (clock, root motion, notifies) every frame after CheckJumpInput and before PerformMovement; notify timing mode OnPostUpdateAnimation (0x140559854)','0x140595730',C,'the mesh tick never advances the clock a second time'],
['pad.jump_button','Gamepad_FaceButton_Right','-','0x140EDD580 with i.GamePadDecideType (int 0x1487D7370) = 0; default written by 0x140EE0CA0 (UTresGameSettings +0x50 = 0)','jump / cancel key global 0x1487D7778','0x140EDD580',C,'Xbox B; attack / decide is Gamepad_FaceButton_Bottom (Xbox A)'],
['key.jump','SpaceBar','-','key table 0x1487D7790 slot 8 (sub key BackSpace), defaults written by 0x140EDF630; registered to the jump key in 0x1414B6570','keyboard emulation 0x14442B8F0 injects the pad key','0x1414B6570',C,'keyboard input reaches the controller one frame later than pad input'],
['key.move','W S A D = +LeftY -LeftY -LeftX +LeftX, raw 1.0','-','key table slots 2, 3, 0, 1 (0x140EDF630); axis emulation 0x14442C190','InputAxis(pad axis, +-1.0) every frame while held; opposite keys give 0; any held key overrides the pad','0x14442C190',C,'diagonal magnitude 1.414, not clamped'],
['key.walk_modifier',0.5,'-','key table slot 4 = LeftAlt; scale 0.5 registered at 0x1414B6813, applied at 0x14442C326','raw 0.5 -> 0.3333 after the dead zone -> walk','0x14442C190',C,'-'],
['anim.run_start_root_distance',180,'cm','build/sora_pack/manifest.json root_motion.tracks (clips 0007, 0007L, 0007R, 0007BL, 0007BR)','from docs/sora-spec-reconciliation.md A2: 180 cm in 0.2333 s, not uniform in time','-',I,'RunEnd, WalkEnd and the landing clips have no root motion (reconciliation A1)'],
];
const ids = new Set(); for (const r of rows) { if (r.length !== 8) throw new Error('row width: ' + r[0]); if (ids.has(r[0])) throw new Error('duplicate id: ' + r[0]); ids.add(r[0]); }
const hdr = { sheet: 'sora-movement', group: 'port', columns: ['id', 'value', 'unit', 'source', 'evidence', 'ref_addr', 'ref_conf', 'note'] };
const out = JSON.stringify(hdr).replace(/\}$/, ',"rows":[\n') + rows.map((r) => JSON.stringify(r)).join(',\n') + '\n]}\n';
const file = path.join(R, 'sheets/port/sora-movement.json');
if (process.argv.includes('--check')) {
  const same = fs.existsSync(file) && fs.readFileSync(file, 'utf8') === out;
  console.log(same ? `sora-movement: ${rows.length} rows, current` : 'sora-movement: sheet differs from tools/sora_movement_sheet.js ' + VERSION);
  process.exit(same ? 0 : 1);
}
fs.mkdirSync(path.dirname(file), { recursive: true }); fs.writeFileSync(file, out);
console.log(`sora-movement: ${rows.length} rows written (tools/sora_movement_sheet.js ${VERSION})`);
