// Stream C (party AI): sheets/port/party-ai-nodes.json, one row per node class the friends' AI pack uses
// (census of build/sora_pack/party/ai/index.json) with its decompiled behaviour functions (sheets/exe/vfuncs,
// own overrides), how kh-ai implements it and on what evidence; plus the port parameters kh-ai holds that were
// not read from the game (kind "param", value null = not determined).
// usage: node tools/party_ai_sheet.js   (after node tools/party_ai_pack.js)
'use strict';
const fs = require('fs'), path = require('path');
const ROOT = path.resolve(__dirname, '..');
const index = JSON.parse(fs.readFileSync(path.join(ROOT, 'build/sora_pack/party/ai/index.json'), 'utf8'));
const sheet = (f) => { const s = JSON.parse(fs.readFileSync(path.join(ROOT, f), 'utf8')); return s.rows.map((r) => Object.fromEntries(s.columns.map((c, i) => [c, r[i]]))); };
const vf = sheet('sheets/exe/vfuncs.json');
const cls = Object.fromEntries(sheet('sheets/reflect/classes.json').map((c) => [c.id, c]));
const byClass = {}; for (const r of vf) (byClass[r.class] = byClass[r.class] || []).push(r);
// behaviour slots (vfuncs sheet numbering): tasks 86 Execute / 87 Abort / 88 Tick / 89 OnMessage / 90 OnTaskFinished;
// aux 89 OnBecomeRelevant / 90 OnCeaseRelevant / 91 TickNode; decorators 96..99 (Activation, Deactivation, Processed, Calc), 100+;
// composites: their next-child delegate is bound in the constructor (not a slot); EQS: 72..74
const SLOTS = new Set([72, 73, 74, 86, 87, 88, 89, 90, 91, 96, 97, 98, 99, 100, 101]);
function kind(name) {
  let c = name, chain = [];
  while (c && cls[c]) { chain.push(c); c = cls[c].super; }
  if (chain.includes('BTTaskNode')) return 'task';
  if (chain.includes('BTDecorator')) return 'decorator';
  if (chain.includes('BTService')) return 'service';
  if (chain.includes('BTCompositeNode')) return 'composite';
  if (chain.some((x) => /EnvQueryGenerator/.test(x))) return 'eqs-generator';
  if (chain.some((x) => /EnvQueryTest/.test(x))) return 'eqs-test';
  return 'other';
}
// evidence per class: [conf, text]. conf: confirmed (read at instruction level) / inferred (decompiled C read, or derived) / guess
const C = (t) => ['inferred', 'decompiled C read: ' + t];
const D = (t) => ['guess', 'from the class name and fields (C not transcribed): ' + t];
const U = (t) => ['inferred', 'stock UE 4.17 behaviour: ' + t];
const EV = {
  BTComposite_Selector: U('selector'), TresBTComposite_Custom: C('ctor 0x14045D030 (ReturnCondition default 0 = FirstSuccess), next child 0x140469340'),
  TresBTComposite_Random: C('ctor 0x14045D1C0, next child 0x140469390, weighted pick 0x14118A3B0 (Period -1: pure weighted; bFailOnAll retries)'),
  BTDecorator_Blackboard: C('Calc 0x143C52FA0, key change 0x143C65530 (result vs executing branch); OperationType 0 Set / 1 NotSet'),
  BTDecorator_Cooldown: C('KH3 RandomDeviation: init 0x143C63470, Calc 0x143C53390, Deactivation 0x143C66900, Tick 0x143C6D200'),
  BTDecorator_ForceSuccess: C('OnNodeProcessed 0x142F5F760'), BTDecorator_Loop: C('Activation 0x143C666A0, Deactivation 0x143C66A00 (bTerminateIfChildFails, SearchId guard)'),
  BTDecorator_SetTagCooldown: C('Deactivation 0x143C66AE0'), BTDecorator_TagCooldown: C('Calc 0x143C53DB0, Deactivation 0x143C66BC0, Tick 0x143C6D450'),
  BTDecorator_TimeLimit: C('KH3 Min/Max: Activation 0x143C66760 sets the aux timer; abort when it runs out'),
  BTService_RunEQS: U('runs the query on its interval; the result reaches the blackboard on the next tick (asynchronous)'),
  BTTask_RunBehavior: U('pushes the subtree (KnownInstances memory)'), BTTask_RunEQSQuery: U('query, result next tick, Failed without items'),
  BTTask_Wait: C('Execute 0x143C5BD40 (KH3 RandomDeviation), Tick 0x143C6D990'), TresBTTask_RunBehaviorDynamic: U('injection by gameplay tag from the pawn m_DynamicAIBehavior'),
  TresBTTask_Noop: C('Execute 0x1404AC250 (Succeeded)'),
  TresBTTask_Action: C('Execute 0x140494D70, update 0x1404A2370 (retries until the state component accepts, then waits for the end message), Abort 0x140492CB0 (cancels), Tick 0x1404A4540 (m_AbortTime)'),
  TresNpcBTTask_NpcCureAction: C('Execute 0x14120DF60: clears the MetaAI record, then the Action task; issues Cure on m_Target'),
  TresBTTask_MoveTo: D('MoveTo command with the locomotion definition (BB key or fixed), acceptable radius; Abort stops'), TresNpcBTTask_NpcMoveTo: D('MoveTo with m_bWalk'),
  TresBTTask_TurnTo: D('turn to key + Offset + random within RandomAngleLimit'), TresBTTask_Idle: D('idle, stays in progress'), TresBTTask_QuickTurnIdling: D('idle turning past TurnLimit'),
  TresBTTask_BlackboardBoolValueModifier: C('Execute 0x140494FD0: sets the Bool key, Succeeded'), TresBTTask_BlackboardClassModifier: C('Execute 0x140495060: sets the Class key, Succeeded (Failed when the set fails)'), TresBTTask_BlackboardObjectModifier: D('set / clear'), TresBTTask_BlackboardVectorValueModifier: D('set from key B location'),
  TresBTTask_FindAttackLocation: D('action definition m_ActionEQSQuery result'), TresBTService_FindAttackLocation: D('same on an interval'),
  TresBTService_GameplayFocus: C('ctor defaults (0.2 s, 0 dev, tick on search start); body: focus = key actor (D)'), TresBTService_RunEQSQuery: D('EQSQueryOverride / BB query -> Result'),
  TresBTService_InterpretPlayerIntent: D('follow preferences from the node curves over player / self still time'),
  TresBTDecorator_AngleCheck: C('Calc 0x1404622D0 cos(Tolerance) <= dot; dot helper D'), TresBTDecorator_ArithmeticComparison: C('Calc 0x140462330; operands D (function argument = other key)'),
  TresBTDecorator_BoolPropertyCheck: C('Calc 0x140462400 (pawn function by name), Tick 0x140476E30'), TresBTDecorator_RangeCheck: C('Calc 0x14047F760; distance helper D'), TresNpcBTDecorator_NpcRangeCheck: C('as RangeCheck; distance 0x141205900 D'),
  TresBTDecorator_TresStateCheck: C('Calc 0x14047FA50'), TresBTDecorator_ValidateAction: D('action definition distance window'), TresBTDecorator_IsStopAI: C('Calc 0x14047D6C0 = the AI controller vt+0x9C0 (stop flag); ctor 0x140478FF0 sets bNotifyTick, no abort mode in the root'),
  TresBTDecorator_IsPlayerTeamCharacter: D('uid list'), TresBTDecorator_IsInViewport: D('camera cone'), TresBTDecorator_RandomEntry: D('probability'),
  TresBTDecorator_EntryLimit: D('passes'), TresBTDecorator_FloorHeightCheck: D('false in the test level'), TresBTDecorator_Interrupt: D('false'), TresBTDecorator_TimeMotionlessCheck: D('false'),
  TresNpcBTDecorator_AIStyle_Ability: C('Calc 0x141205E60'), TresNpcBTDecorator_AIStyle_Battle: C('Calc 0x141205EF0'), TresNpcBTDecorator_CanAttackFlowWait: C('Calc 0x1412062B0 -> battle controller 0x14121DAC0 (+0x238 on and the attack phase +0x239 off; phase = controller tick 0x14123CDA0)'),
  TresNpcBTDecorator_CanExecuteNpcAbility: C('Calc 0x1412063B0 (pawn vt+0x17E0 read as MP recharge: D)'), TresNpcBTDecorator_IsCureInfoCommand: C('Calc 0x141207870 (MetaAI cure record +0x2A4, written by the MetaAI tick 0x14123E440 from 0x141264E30: kh-ai cure.rs)'),
  TresNpcBTDecorator_IsEnemy: C('Calc 0x1412078C0'), TresNpcBTDecorator_ObjectComparison: C('Calc 0x141208AD0'), TresNpcBTDecorator_IsTargetAttrResistKind: C('Calc 0x141208260; resist kind from the rate D'),
  TresNpcBTService_SetCureTargetBB: C('Tick 0x14121B340'), TresNpcBTTask_EnableAttackFlowWait: C('Execute 0x14120DA10 -> 0x141239090 (on change; on clears the phase); phase: battle controller tick 0x14123CDA0 (player attacking near / own target within 1000 cm starts it; 10 s, T > 1500 cm from P or N > 1200 cm from P ends it)'),
  TresNpcBTTask_NpcUseItem: C('Execute 0x1412234A0 (record command on the record target via 0x141274340, record cleared 0x141231C40; result from the USE_ITEM state 0x14122F380), Tick 0x14123C9D0'), TresNpcBTTask_SetCureTargetBB: D('SupportTargetActor = record target'), TresNpcBTTask_SelectSideMoveDirction: D('side away from the player'),
  TresNpcBTTask_SetBattleFollowMode: D('flag + command'), TresNpcBTTask_GetCameraActor: D('no camera actor: clears, fails'), TresNpcBTTask_SetCombinationPhase: D('MetaAI phase'),
  TresNpcBTDecorator_CoverWaitCheck: D('player between member and target'), TresNpcBTDecorator_EnemyCountCheck: D('enemies within radius'), TresNpcBTDecorator_ExistsLockonTargetComponents: D('any lockable enemy'),
  TresNpcBTDecorator_IsFlyingEnemy: C('Calc 0x141207980 = the m_Source pawn vt+0x2600 (flying): ActorView.flying'), TresNpcBTDecorator_IsPlayer: C('Calc 0x141207F30: the m_Source actor is the player controller pawn (0x1441053E0)'), TresNpcBTDecorator_IsInHoldCameraTrickAction: D('player view flag'),
  TresNpcBTDecorator_IsCombiState: D('MetaAI combination state (DISABLE without a partner)'), TresNpcBTDecorator_IsCombiPhase: D('MetaAI phase'), TresNpcBTDecorator_IsCombiRole: D('false (no partner)'),
};
// C3 (2026-10-15): bodies read in the decompiled C (addresses in crates/kh-ai/src/nodes/mod.rs, validate.rs, eqs.rs)
Object.assign(EV, {
  TresBTDecorator_ValidateAction: C('Calc 0x14047FB20 (bUseBlackboardQuery def, flags +0xE8) -> CDO slot 125 0x140432AD0 (distance 0x14043F040, height 0x14043F140, direct path 0x14043E970, LOS 0x14043EAE0), 127 0x140432D20 (yaw 0x14043FC00, pitch 0x14043F4F0 / Ranged 0x141248060), 129 NpcBase 0x14121E9F0 / Magic 0x14121EBF0 / JumpBase 0x14121EB60; native ctor defaults 0x14042FD30 / 0x14121BBF0 / 0x14123F050 / 0x14123F0D0; Discus pitch 0x1411CA530 and vector-target variants D'),
  TresBTTask_MoveTo: C('Execute 0x1404ABCE0, PerformMoveTask 0x1404B56C0 (component goal floor-projected + tether, locomotion 0x1404AEAB0 / pawn +0x9E8), Tick 0x1404BD470 (DAMAGE -> stop, Failed), Abort 0x1404A8700 (keep moving m_AbortMoveTimeToKeepMoving), ctor 0x1404A5B20'),
  TresNpcBTTask_NpcMoveTo: C('Execute 0x14120E040 / OnTaskFinished 0x141218880: m_bWalk = battle controller +0x22D for the run, then MoveTo'),
  TresBTTask_TurnTo: C('Execute 0x1404AD040 (FRand in +-RandomAngleLimit), update 0x1404B69B0 (0.1 deg done, turn request retried until accepted, TurnFinished), Tick 0x1404BE2C0'),
  TresBTTask_Idle: C('Execute 0x1404ABCC0, update 0x1404B5600 (pawn IDLE -> vt+0x16E8, IdleFinished), Tick 0x1404BD410 (2 s, then Failed); IdleFinished sender D'),
  TresBTTask_QuickTurnIdling: C('Execute 0x1404AC320, update 0x1404B5E50, facing 0x1404B2520 (cos(TurnLimit) - 0.01), message 0x1404B4260 (cooldown), ctor 0x1404A5FC0'),
  TresBTTask_BlackboardObjectModifier: C('Execute 0x140495720: object key = m_Value (null clears), Succeeded when set'),
  TresBTTask_BlackboardVectorValueModifier: C('Execute 0x140495920: m_bUseBlackboard (ctor 0x1404909A0 off) ? key B : m_Value, base 0x1404958A0'),
  TresNpcBTTask_SelectSideMoveDirction: C('Execute 0x141223C60: sign of the normalised yaw difference (self yaw vs target / away from the player)'),
  TresNpcBTTask_SetBattleFollowMode: C('Execute 0x141224610 -> battle controller 0x1412390C0(m_bFollowMode)'),
  TresNpcBTTask_SetCureTargetBB: C('Execute 0x141224780: curer == self -> SupportTargetActor = record target, Succeeded; else Failed'),
  TresNpcBTTask_GetCameraActor: C('Execute 0x14120DB30: camera manager view target -> key, Succeeded; the port has no camera actor (key cleared, Failed; photo mode only)'),
  TresNpcBTTask_SetCombinationPhase: C('Execute 0x141224730: MetaAI +0x28C = m_PhaseIndex'),
  TresBTService_InterpretPlayerIntent: C('Tick 0x14048F440, keys 0x14048A8B0: the three preference formulas; TimeMotionless / average-direction accumulators D'),
  TresBTDecorator_EntryLimit: C('Calc 0x1404632B0, init 0x14046FA20, Deactivation 0x1404727F0, ctor 0x14045DD70'),
  TresBTDecorator_Interrupt: C('Calc 0x140464170, hooks 0x140472CB0 / CC0 / CD0, ctor 0x14045E350; hook caller D (port: entering TSID_STATE_DAMAGE)'),
  TresBTDecorator_FloorHeightCheck: C('Calc 0x1404632D0, height 0x140461C60, ctor 0x14045DDC0'),
  TresBTDecorator_TimeMotionlessCheck: C('value 0x14046EE10 (movement +0xC68, GetTimeMotionless 0x141927B00), base Calc 0x1404621A0; accumulator D'),
  TresBTDecorator_IsInViewport: C('Calc 0x1404645F0: target location projected into the viewport'),
  TresBTDecorator_IsPlayerTeamCharacter: C('Calc 0x14047D600: source vt+0xD70 uid in m_CharactorUID'),
  TresBTDecorator_RandomEntry: C('Calc 0x14047F630: FRand < key float or literal'),
  TresNpcBTDecorator_CoverWaitCheck: C('Calc 0x141206980: player in TSID_STATE_COVER and |P-T| - m_AdjustLength < |N-T|'),
  TresNpcBTDecorator_EnemyCountCheck: C('Calc 0x141206CB0: lockable / ignore-uid filter, 2D distance minus both radii, count <op> m_EnemyNum'),
  TresNpcBTDecorator_ExistsLockonTargetComponents: C('Calc 0x141207060: lock-on target components of the pawn (mode 2) or MetaAI +0x2E8 count > 0 (MetaAI list D: empty)'),
  TresNpcBTDecorator_IsCombiPhase: C('Calc 0x1412076B0: MetaAI +0x28C == m_PhaseIndex'),
  TresNpcBTDecorator_IsCombiState: C('Calc 0x141207820 -> 0x141229730: MetaAI +0x238 == m_State'),
  TresNpcBTDecorator_IsCombiRole: C('Calc 0x141207700: battle controller +0x22C == m_Role (no partner: never set)'),
  TresNpcBTDecorator_IsInHoldCameraTrickAction: C('Calc 0x141207B70: NPC-mode object +0x788 checks (inactive) then player vt+0x2A30'),
  TresEnvQueryGenerator_OnRing: C('GenerateItems 0x1404C6420, rings 0x1404C90C0, contexts 0x1404C9920, ctor 0x1404C0FD0'),
  TresEnvQueryTest_MinDistance: C('RunTest 0x1404E7E80 (radii, clamp >= 0, empty context skips)'),
  TresNpcEnvQueryTest_BorderDistance: C('RunTest 0x141232DE0 (1 inside m_Border, 0 outside)'),
  TresNpcEnvQueryTest_CameraAngle: C('RunTest 0x141233220 (1 within m_AngleYaw of the camera yaw)'),
  TresNpcEnvQueryTest_RandomAngle: C('RunTest 0x141238460 (reference yaw = querier yaw + FRand in +-m_AngleYaw)'),
  TresNpcEnvQueryTest_DirectTargetEnemy: C('RunTest 0x141234CD0 (item actor uid vt+0xD78 == m_EnemyUID)'),
  TresEnvQueryTest_IsCharacterState: C('RunTest 0x1404E4260 (querier state, m_bContextStateCheck: context actors)'),
  TresEnvQueryTest_IsSwimmingUnderWater: C('RunTest 0x1404E5270 (bQuerierCheck: querier, else context actors; FUN_140545F80)'),
});
// C4 (2026-10-15): the remaining EQS generators / tests, the EQS service, FindAttackLocation, and
// the callers C3 could not find (addresses in crates/kh-ai/src/eqs.rs, nodes/mod.rs, controller.rs, cure.rs)
Object.assign(EV, {
  TresEnvQueryGenerator_Context: C('GenerateItems 0x1404C4BB0 (ActionDefinition / Actor / Point + ProjectAndFilterNavPoints 0x143C882C0 / SceneComponent by ContextItemType), ctor 0x1404C0E30 (Context Querier, Point, trace None), PostLoad 0x1404D2D30; contexts Allies 0x1404D3BF0 / Enemies 0x1404D4CF0 / Target 0x1404DA2C0 / PlayerFloorPosition 0x1404D6770 read, the others by name'),
  TresEnvQueryGenerator_OnRing3D: C('GenerateItems 0x1404C5150 (actor centres with radius, else points; CeilToInt((Max-Min)/SpacingZ)+1 heights from MinHeight), ctor 0x1404C0EA0 (ItemSpacingZ 100, MinHeight 0, MaxHeight 500, trace None), rings 0x1404C90C0'),
  TresNpcEnvQueryGenerator_SideMove: C('GenerateItems 0x141225520 (querier + 3 points along normalise(clamp((|d|-(Radius+reach))/AdjustValue,-1,1) x ForwardPowerRate, side), s = r+70, +100, +100; side flip >= 150 deg written to SideMoveDirction; swim roll), ctor 0x14121C730 (300 / 100 / 1, Navigation)'),
  TresEnvQueryTest_ActorOverlap: C('RunTest 0x1404DCB50 (gimmick obstacle components / actor bounds, items-box prefilter, (r + querier r)^2 and half height), ctor 0x1404C1330 (bAddQuerierRadius on, bool); context GimmickObstacles 0x1404D5540'),
  TresEnvQueryTest_DistanceToPath: C('RunTest 0x1404E0F50 (FindPathSync per start x end pair, closest point FUN_140530e20; no path -> skipped), ctor 0x1404C1750'),
  TresEnvQueryTest_FloorHeight: C('RunTest 0x1404E2B90 (floor FUN_140542040 down TestHeight; no floor -> false; z <op> floor + FloorHeight), ctor 0x1404C1850 (200 / 1000)'),
  TresEnvQueryTest_Volume: C('RunTest 0x1404EC590 (VolumeClass actors overlapping the items box; bSkipIfNoVolumeFound; inside bounds + EncompassesPoint), ctor 0x1404C23C0 (skip on, bool)'),
  TresEnvQueryTest_WallDistance: C('RunTest 0x1404ED0E0 (navmesh wall distance, none -> MaxDistance), ctor 0x1404C2440 (MaxDistance 1000)'),
  TresNpcEnvQueryTest_Around: C('RunTest 0x1412324D0 (reference yaw: target yaw - m_Angle, or bearing +- m_WayAngle past 45 deg; value 180 - |diff|), ctor 0x14121C840'),
  TresNpcEnvQueryTest_Escape: C('RunTest 0x1412358F0 (escape vector from context actors + player team within m_Radius, else away from TargetActor / SubTargetActor; none -> items fail; value |yaw diff|), ctor 0x14121CB50; team list FUN_14043cdb0'),
  TresNpcEnvQueryTest_CanUseFreeMovePoint: C('RunTest 0x141233850: runs only with AI-manager free-walk points (+0x7E8; none in the port: not run), else value 1 for unused way points of the querier uid'),
  TresBTService_RunEQSQuery: C('TickNode 0x1404A3FC0 (no new query while one runs), finished 0x1404A1DF0 (first item -> Result, none + bInvalidateBlackboardKeyOnFailure -> cleared), InitializeFromAsset 0x14049F310 (bUseBlackboardQuery = no override), cease 0x1404A1B70, ctor 0x14048FFB0'),
  TresBTTask_FindAttackLocation: C('Execute 0x140497130 (not an attack definition -> Failed; no m_ActionEQSQuery -> Result = pawn location; else query with CDO params vt+0x480 0x140438C90), finished 0x1404A20B0, Abort 0x140492F80, ctor 0x140491190'),
  TresBTService_FindAttackLocation: C('TickNode 0x14048ED70, finished 0x14048D580 (none -> invalid location), cease 0x14048BF60, ctor 0x14047A300; params 0x140438C90'),
  TresBTTask_Idle: C('Execute 0x1404ABCC0, update 0x1404B5600 (pawn IDLE -> vt+0x16E8, wait IdleFinished), Tick 0x1404BD410 (2 s, then Failed), OnMessage 0x1404B4120 (stock); IdleFinished sent by UTresNpcState_Idle exit 0x141245380 and tick 0x14124CD40 (battle mode: every idle clip length; else at the clip end) = the Idle ticket of B'),
  TresBTService_InterpretPlayerIntent: C('Tick 0x14048F440, keys 0x14048A8B0: the three preference formulas; TimeMotionless / average direction = movement FUN_140591690 (+0xC68 / +0xC4C, 3.0 s interpolation)'),
  TresBTDecorator_TimeMotionlessCheck: C('value 0x14046EE10 (movement +0xC68, GetTimeMotionless 0x141927B00), base Calc 0x1404621A0; accumulator FUN_140591690 (> 0.0001 cm/s on any axis = moving)'),
  TresBTDecorator_Interrupt: C('Calc 0x140464170, hooks 0x140472CB0 / CC0 / CD0, ctor 0x14045E350; caller: damage 0x1405BB850 -> pawn vt+0x20F0 0x14054ACD0 -> BT OnDamage 0x143C3DC20 -> aux slot 86 0x143C49250 -> hook + slot 92 0x143C3D510 RequestExecution; cleared by WrappedOnNodeDeactivation 0x143C491A0'),
  TresBTDecorator_ValidateAction: C('Calc 0x14047FB20 (bUseBlackboardQuery def, flags +0xE8) -> CDO slot 125 0x140432AD0 (distance 0x14043F040, height 0x14043F140, direct path 0x14043E970, LOS 0x14043EAE0), 127 0x140432D20 (yaw 0x14043FC00, pitch 0x14043F4F0 / Ranged 0x141248060 / Discus 0x1411CA530), 129 NpcBase 0x14121E9F0 / Magic 0x14121EBF0 / JumpBase 0x14121EB60; vector targets slots 124 / 126 / 128 = 0x14041F1E0 (true); native ctor defaults 0x14042FD30 / 0x14121BBF0 / 0x14123F050 / 0x14123F0D0'),
});
const WORLD_MODE =/Lead|FreeWalk|NpcMode|RideGigas|Smapho|IsActionOn|Ascension|InnerWaterCurrent|PlayingFieldVoice|UpdateLeadPoint/;
const EQS_D = D('generator / test transcribed from its fields; contexts by name');
const rows = [];
for (const [name, count] of Object.entries(index.census)) {
  const k = kind(name);
  const own = (byClass[name] || []).filter((r) => SLOTS.has(+r.slot)).map((r) => r.ref_addr || r.id).sort();
  let ev = EV[name] || (WORLD_MODE.test(name) ? D('world-specific mode (lead, free walk, NPC mode, photo mode, water current): condition false / task fails in the test level (plan N5)') : (k.startsWith('eqs') ? (/^EnvQuery/.test(name) ? U('stock EQS node; score normalisation from 0x143C63FC0 (KH3 equations)') : EQS_D) : null));
  rows.push([name, k, count, own.length ? own : ['-'], ev ? ev[1] : null, ev ? ev[0] : null, ev ? 'yes' : null, '-']);
}
const P = (id, value, note) => rows.push([id, 'param', 0, ['-'], note, value === null ? 'guess' : 'inferred', 'yes', value === null ? 'value in kh-ai controller::params not read from the game' : '-']);
P('param.cure_hp_ratio', 'OFTEN < 50 % (35 in battle), PINCH <= 25 % or dying, ALWAYS < 80 % (50 in battle)', 'decompiled C read (C2): MetaAI tick 0x14123E440 (every 2 s; nearest curer-target pair < 2000 cm, else a random far one), decision 0x141264E30, HP need 0x14126A870 / flag 0x14126AC70, MP 0x14126AF20 / 0x14126B550 (<= 24 %, recharge >= 8 s), select 0x141263E10 (FriendEvaluationData scores), item fit 0x14125F380 / 0x14125F7F0; items only in battle; Cure first unless ALWAYS in battle (kh-ai cure.rs). C3 / C4: restriction 6, ITEM_UP, player focus (focus-break bit +0x7C0 0x140545D70, ALWAYS focus fit FUN_14125ef60, record flag FUN_14126a1f0), other members MP fed through AiBody / AiWorld');
P('param.warp_distance', 'nav path >= 3000 cm (1000 cm for 3 s after the battle starts), checked every 2 s, only off-screen', 'decompiled C read (C2): pawn tick 0x14127B980 (2 s timer +0xC98), condition 0x141258790 (own / player states, view frustum 0x140E5AC70, no floor within 5000 cm, swim distance, nav path length), 0x141267FD0 (SetEnableWarp +0xB96; in battle only outside +-120 deg of the camera yaw), friend component 0x14127BC10 (+0x314 3 s window). Frustum (C4): the camera manager POV near / far (slots 256 / 257 0x144434580 / 0x144432390, UCameraComponent 3 / 2097152 cm)');
P('param.bt_priority', 'stock UE: SearchEnd = the requester next sibling; TakesPriorityOver instance first', 'decompiled C read (C2): RequestExecution 0x143C42290 (drops a restart whose child / branch decorators fail; SearchEnd; merge rules), ProcessExecutionRequest 0x143C3F8C0 (the found task must take priority over SearchEnd, else rollback)');
P('param.cond_tick', 'base TickNode 0x140476D90 re-checks flow-abort decorators', 'decompiled C read (C2): base TickNode 0x140476D90 (vtable slot 0x2D8 of 111 Tres decorator classes): FlowAbortMode != None and (raw ^ inverse) != IsExecutingBranch -> RequestExecution; ctors set bNotifyTick (field_0x70 |= 4/5; not TimeMotionlessCheck 0x14045E570)');
P('param.eqs_timing', 'one manager tick (10 ms budget, breadth first); the result reaches the tree on the next BT tick', 'decompiled C read (C2): UEnvQueryManager ctor 0x143C4FF30 MaxAllowedTestingTime 0.01, bTestQueriesUsingBreadth true (stock UE 4.17); C4: the manager Tick 0x143C6C920 (10 ms budget, breadth round-robin, delegates in the tick) is stock');
const out = { sheet: 'party-ai-nodes', group: 'port', columns: ['id', 'kind', 'instances', 'functions', 'evidence', 'ref_conf', 'implemented', 'note'], rows };
fs.writeFileSync(path.join(ROOT, 'sheets/port/party-ai-nodes.json'), '{"sheet":"party-ai-nodes","group":"port","columns":' + JSON.stringify(out.columns) + ',"rows":[\n' + rows.map((r) => JSON.stringify(r)).join(',\n') + '\n]}\n');
const missing = rows.filter((r) => r[6] !== 'yes').map((r) => r[0]);
console.log('party-ai-nodes:', rows.length, 'rows;', missing.length, 'without evidence:', missing.join(', '));
