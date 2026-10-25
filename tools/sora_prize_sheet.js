// Writes sheets/port/sora-prizes.json: the numbers of KH3's prize orbs (docs/sora-prize-spec.md), the
// test level's drop tables (a TEST-LEVEL CHOICE) and the Elden Ring host's one drop rule (a HOST CHOICE).
// Registers the sheet in tools/lib/sheet_defs.js once. Idempotent.
// Then: node tools/extract.js; node tools/preflight.js --summary; node tools/gen_port.js
// Evidence scripts: build/sora_tmp/prize/{enemy_drops,prize_bps,rd,callers}.js
'use strict';
const fs = require('fs');
const path = require('path');
const root = path.resolve(__dirname, '..');

const I = 'confirmed'; // read from the exe's data or an asset, or an immediate of the named function
const C = 'inferred'; // decompiled C only (the function's verdict is not "same")
const BP = 'assets_converted/packages/KINGDOM HEARTS III/Content/Blueprints/Prize/';
const TEST = 'TEST-LEVEL CHOICE (2026-10-12; not game data)';
const HOST = 'HOST CHOICE of the Elden Ring hookup (2026-10-12; not data of either game)';
const DEF = 'static data of the exe at 0x1486DD5C0 (the default velocity block every table drop copies into its request: 900, 0, 0 | 0 | 50, 65 | 0, 360 | 0.955), read with build/sora_tmp/prize/rd.js';

const rows = [
  // ---- the movement component (UTresPrizeMovementComponent : UTresProjectileMovementComponent : UProjectileMovementComponent)
  ['move.gravity_scale', 2.3, '-', 'ATresPrizeBase ctor 0x14146EFA0: MyMovement->ProjectileGravityScale = 2.3', 'x the world gravity (row move.world_gravity_z): 2254 cm/s2', '0x14146EFA0', C, 'set to 0 when the orb rests and when it leaves the Appear state'],
  ['move.world_gravity_z', -980, 'cm/s2', 'sheets/port/sora-movement row gravity.world_z (UPhysicsSettings DefaultGravityZ, no override in the game ini)', 'stock UE 4.17 UProjectileMovementComponent::GetGravityZ = world gravity x ProjectileGravityScale; the Tres subclass was not read for an override', '-', C, '-'],
  ['move.bounciness', 0.7, '-', 'ATresPrizeBase init (vtable slot 241) 0x14147B200: MyMovement->Bounciness = 0.7', 'stock UE 4.17 ComputeBounceResult: the velocity along the hit normal is reversed and scaled by this', '0x14147B200', C, 'x 0.7 once more when the orb enters water'],
  ['move.friction', 0, '-', 'ATresPrizeBase init 0x14147B200: MyMovement->Friction = 0', 'the velocity along the surface is kept by a bounce', '0x14147B200', C, '-'],
  ['move.stop_speed', 5, 'cm/s', 'ATresPrizeBase ctor 0x14146EFA0: BounceVelocityStopSimulatingThreshold = 5', 'stock UE 4.17: a bounce that leaves less than this speed stops the simulation', '0x14146EFA0', C, '-'],
  ['move.damping', 0.955, 'per 1/60 s', DEF + ' (last float: the air resistance); the same number is the ctor default of prize+0x440 (0x3F747AE1)', 'Appear tick 0x14147B760: velocity X and Y x pow(this, dt x 60) every tick; Z is not damped; init clamps it to 0..1', '0x14147B760', C, '-'],
  // ---- how a table drop is thrown (ATresPrizeMan queue tick 0x141489120)
  ['spawn.speed', 900, 'cm/s', DEF + ' (the length of the first vector)', 'queue tick 0x141489120: initial speed of every orb of a table drop', '0x141489120', I, 'a gimmick may give its own block (TresGimmickPrizeVelocityData)'],
  ['spawn.speed_rand', 0, 'cm/s', DEF, 'when above 0 the speed is speed + rand x 2 x this - this, not below 0', '0x141489120', I, '-'],
  ['spawn.pitch', [50, 65], 'degrees up', DEF, 'uniform in the range, added to the block direction\'s own pitch', '0x141489120', I, '-'],
  ['spawn.yaw', [0, 360], 'degrees', DEF, 'uniform in the range: all round', '0x141489120', I, '-'],
  ['spawn.max_requests', 256, 'count', 'ATresPrizeMan table spawn 0x141480420: no request is taken while 0x100 are queued', 'the request queue (0x38 bytes each)', '0x141480420', C, '-'],
  // ---- life
  ['life.appear_time', 0.8333333, 's', 'Appear begin (slot 253) 0x141473550: m_Timer = 0.8333333 (50 frames)', 'Appear tick: until it has run out the orb cannot be collected', '0x141473550', C, '0.5 while the player is in state 119 (small airplane)'],
  ['life.living_time', 15, 's', 'table spawn 0x141480420 writes 15.0 (0x41700000) into every request; ATresPrizeBase ctor default m_fLivingTime 15', 'Appear tick 0x14147B760: after the appear time the timer is set to this; at 0 the state becomes Disappear, whose tick removes the orb at once (no blink state in the code)', '0x14147B760', C, 'negative = never'],
  ['life.box_living_time', 180, 's', 'item box spawn 0x141473110: 180.0 (0x43340000)', 'an item box stays three minutes and can be collected from its first frame (0x141473590)', '0x141473110', C, '-'],
  ['bounce.sound_max', 4, 'count', 'bounce handler 0x14147EDF0: m_SoundBound is played while the bounce count is below 4', '-', '0x14147EDF0', C, '-'],
  ['rest.min_bounces', 4, 'count', 'bounce handler 0x14147EDF0', 'from the 4th bounce on, when the orb is at least rest.min_age old and the surface normal . up is above rest.floor_normal_z: gravity scale 0 and velocity 0', '0x14147EDF0', C, '-'],
  ['rest.min_age', 1, 's', 'bounce handler 0x14147EDF0 (prize+0x43C, the seconds since it was made)', '-', '0x14147EDF0', C, '-'],
  ['rest.floor_normal_z', 0.2, '-', 'bounce handler 0x14147EDF0', '-', '0x14147EDF0', C, '-'],
  // ---- attraction
  ['magnet.radius', [300, 600, 900, 900, 900], 'cm', 'table 0x1469AB6D8 (90000, 360000, 810000, 810000, 810000: squared), read with build/sora_tmp/prize/rd.js', 'FindGetter (slot 243) 0x1414794C0: index = copies of PRIZE_DRAW (41, Treasure Magnet) clamped to 0..4; the nearest party pawn inside it that can take the prize (HP above 0 and not in the die state: 0x14142C600) becomes the getter', '0x1414794C0', I, 'MASTER_DRAW (61) or the magic-draw flag of the drop: no limit; player state 138: 10000 cm, state 119: 5000 cm'],
  ['get.time', 0.6666667, 's', 'Get begin (slot 256) 0x1414736D0: m_Timer = 0.6666667 (40 frames)', 'the flight to the getter; the prize is applied when it has run out (0x141479360 -> ApplyPrize 0x1405AB950)', '0x1414736D0', C, '-'],
  ['get.rate', 1.5, '1/s', 'Get tick (slot 257) 0x14147BC90', 'position = getter point + direction x start distance x (timer x this); the actor scale is the same factor (1 -> 0)', '0x14147BC90', C, '-'],
  ['get.turn_gain', 722.29297, 'degrees/s', 'Get tick 0x14147BC90', 'the direction turns about the vertical by min(get.turn_ref / (start distance x timer x get.rate) x this, get.turn_max) degrees per second', '0x14147BC90', C, 'x 3 in player state 119'],
  ['get.turn_ref', 100, 'cm', 'Get tick 0x14147BC90', '-', '0x14147BC90', C, '-'],
  ['get.turn_max', 1020, 'degrees/s', 'Get tick 0x14147BC90', '-', '0x14147BC90', C, '-'],
  ['get.sound_at', 0.2, 's', 'Get tick 0x14147BC90: m_SoundGet is played once when the timer is at or below 0.2', '12 frames before the prize is applied', '0x14147BC90', C, '-'],
  ['get.target_height', null, 'cm', 'Get begin / tick ask the getter\'s virtual +0x6E0 for the point to fly to; that function was not read', 'the port flies to the capsule centre the host gives', '0x1414736D0', 'guess', 'UNCHECKED: a runtime probe item'],
  // ---- per kind (index = PRIZE_KIND 0 HpS, 1 HpL, 2 MpS, 3 MpL, 4 MunnyS, 5 MunnyM, 6 MunnyL, 7 FocusS, 8 FocusL)
  ['kind.radius', [13, 26, 13, 26, 17, 25.5, 34, 13, 26], 'cm', BP + 'Prize{HpS,HpL,MpS,MpL,MunnyS,MunnyM,MunnyL,FocusS,FocusL}: TresRoot CollisionShapesSrc[0] SPHERE Size.X (build/sora_tmp/prize/prize_bps.json)', 'the sphere that moves through the level', '-', I, '-'],
  ['kind.effect_scale', [0.5, 1, 0.5, 1, 0.5, 0.75, 1, 0.5, 1], '-', BP + '...: RelativeScale3D of the ParticleSystemComponent (absent = 1)', 'the small prize is the large one\'s effect at half size', '-', I, 'effects: vs_plz_hpb0 (HP), vs_plz_mpb0 (MP), VS_PLZ_MNY0 (munny), vs_plz_fop0 (focus)'],
  ['kind.value', [1, 5, 3, 5, 1, 5, 10, 3, 5], 'points', 'ApplyPrize 0x1405AB950 (sheets/port/sora-stats-rules prize.values)', 'HP +1 / +5, MP +3 / +5, munny 1 / 5 / 10, focus +3 / +5', '0x1405AB950', C, 'MP while the gauge recharges: the recharge timer -2 s / -6 s instead'],
  // ---- item boxes (ATresPrizeBox; PRIZE_KIND 13 heal, 14 material, 15 foodstuff, 16 rare)
  ['box.radius', 26, 'cm', BP + 'PrizeBox{Heal,Mate,Fstf,Rare}: SPHERE 26', '-', '-', I, 'effects vs_plz_prb00 (heal), prb02 (material), prb03 (foodstuff), prb01 (rare); sounds se00000_026 / 025'],
  ['box.speed_xy', 300, 'cm/s', 'item box spawn 0x141473110', 'velocity X and Y each = (rand - rand) x rand x this', '0x141473110', C, '-'],
  ['box.speed_z', [600, 1200], 'cm/s', 'item box spawn 0x141473110', 'velocity Z = 600 + rand x 600', '0x141473110', C, '-'],
  ['box.fly_time', [0.00083333335, 0.5, 1], 's/cm, s, s', 'box Get begin 0x1414738C0', 'flight time = distance x [0] clamped to [1]..[2]; progress p = sin((1 - timer / time) x 90 degrees); each tick the box moves along its direction at (distance left) / (1 - p)', '0x1414738C0', C, 'the arc: rows box.arc'],
  ['box.damping', 0.98, '-', 'item box spawn 0x141473110: the init call (slot 241) is passed 0.98 (0x3F747AE1) where the default block of the orbs has 0.955', 'horizontal velocity x pow(this, dt x 60) while it appears: a box keeps its sideways speed much longer than an orb', '0x141473110', C, '(loot) added 2026-10-13: the port had used the 0.955 of the orbs'],
  ['box.arc', [600, 1, 15], 'cm, 1/s, 1/s', 'ATresPrizeBox constructor 0x14146F420: +0x4B0 = 600.0, +0x4B4 = 1.0, +0x4B8 = 15.0, +0x4BC = 1 (turn by slerp)', 'Get begin 0x1414738C0: direction = normalize(getter - box + (0, 0, [0])); Get tick 0x14147C0A0: the direction turns toward the getter by the fraction clamp(([1] + p x [2]) x dt, 0, 1) of the angle between them, then the box moves along it', '0x14147C0A0', C, '(loot) the rotation helper 0x142D90CA0 was taken for a quaternion slerp from its use, not read'],
  ['box.spin', 30, 'degrees', 'Get tick 0x14147C0A0: the yaw of the actor is set to p x 30', 'the box turns 30 degrees about the vertical on its way in', '0x14147C0A0', C, '(loot) which of the three rotator fields is written was read from the decompiled C only'],
  ['box.shrink', [0.1, 10], '-, 1/-', 'Get tick 0x14147C0A0: when 1 - p < [0] the actor scale is (1 - p) x [1]', 'it shrinks to nothing over the last tenth of the eased progress', '0x14147C0A0', C, '-'],
  ['box.stock_max', 99, 'count', 'TresItemData m_MaxNum: 99 for every battle item, camp item, material, armour and accessory row (build/sora_tmp/loot/items.tsv)', 'a collected item beyond the maximum is not added; the popup HudLeftGetItem then shows its stock-full variant (setParam(icon, name, stockFull))', '-', I, '(loot) what slot +0x18A8 does at a full stock was not read'],
  // ---- the drop table of an enemy (FTresChrBaseParamDataTable m_DropPrize1 .. m_DropItemRate3)
  ['table.item_roll', 99, 'count', 'table spawn 0x141480420: (int)(rand x 99 + 1) <= rate', 'each of the three item slots is rolled by itself against its rate (percent); with flag bit 0 of the packed table one roll over the three (at least 100 wide) picks at most one', '0x141480420', C, '-'],
  ['table.prize_fever', 0.5, '-', 'table spawn 0x141480420: count += (count + 1) / 2 when a party member has PRIZE_FEEVER (175)', 'HP / MP / focus / munny / light prizes only (0x141488030)', '0x141480420', C, '-'],
  ['table.luck_up', 0.3, '-', 'table spawn 0x141480420: item rate x (1 + this x summed LUCK_UP (39) count), rounded', 'material, foodstuff and heal item ids only', '0x141480420', C, '-'],
  // ---- the test level (kinds as ETresDropItemID: 1 HpS, 2 HpL, 3 MpS, 4 MpL, 5 FpS, 6 FpL, 7 MunnyS, 8 MunnyM, 9 MunnyL)
  ['test.dummy1', [1, 3, 8, 2], 'drop id, count, drop id, count', TEST + ': dummy 1 drops what five KH3 enemy rows drop (HpS x3 + MunnyM x2)', 'build/sora_tmp/prize/enemy_drops.tsv', '-', I, 'the pair is a real row; giving it to a dummy is the choice'],
  ['test.dummy2', [3, 3, 7, 2], 'drop id, count, drop id, count', TEST + ': MpS x3 + MunnyS x2 (four KH3 rows, e.g. e_ex009)', 'build/sora_tmp/prize/enemy_drops.tsv', '-', I, '-'],
  ['test.dummy3', [1, 2, 7, 1], 'drop id, count, drop id, count', TEST + ': HpS x2 + MunnyS x1 (the Shadow e_ex001 and nine more rows)', 'build/sora_tmp/prize/enemy_drops.tsv', '-', I, '-'],
  ['test.dummy4', [2, 3, 9, 4], 'drop id, count, drop id, count', TEST + ': HpL x3 + MunnyL x4 (three KH3 rows)', 'build/sora_tmp/prize/enemy_drops.tsv', '-', I, '-'],
  ['test.dummy5', [6, 1, 5, 3], 'drop id, count, drop id, count', TEST + ': FpL x1 + FpS x3 (three KH3 rows)', 'build/sora_tmp/prize/enemy_drops.tsv', '-', I, '-'],
  // (loot) the ITEM tables of the test level tables: game item ids (0x101 Potion, 0x104 Ether, 0x701 Blazing Shard, 0x721 Mythril Shard, 0x735 Fluorite, 0x404 Defense Belt, 0x50C Power Ring ...) with a rate in percent, rolled each by itself like the three item slots of KH3
  ['test.dummy1.items', [257, 100, 1793, 100, 1028, 100], 'item id, percent x 3', TEST + ': dummy 1 always drops a Potion (heal box), a Blazing Shard (material box) and a Defense Belt (rare box): one of each box kind a Sora can meet', 'KH3 enemy rows name heal items, materials and ingredients only; a rare box (equipment) comes from other callers of 0x141473110', '-', I, 'the rates are of the test level'],
  ['test.dummy2.items', [260, 100, 1825, 50, 1292, 25], 'item id, percent x 3', TEST + ': Ether always, Mythril Shard 50 %, Power Ring 25 %', '-', '-', I, '-'],
  ['test.dummy3.items', [257, 8, 1793, 8, 1797, 4], 'item id, percent x 3', TEST + ': a table like the Shadow has: 8 % / 8 % / 4 %', 'e_ex001 drops two materials at 8 % and 4 % (build/sora_tmp/prize/enemy_drops.tsv)', '-', I, '-'],
  ['test.dummy4.items', [262, 30, 1845, 30, 1287, 30], 'item id, percent x 3', TEST + ': Elixir, Fluorite, Skill Ring at 30 % each', '-', '-', I, '-'],
  ['test.dummy5.items', [0, 0, 0, 0, 0, 0], 'item id, percent x 3', TEST + ': no items', '-', '-', I, '-'],
  ['test.respawn_time', 4, 's', TEST + ': a dummy brought to 0 hit points bursts into its prizes and is back at full after this', '-', '-', I, '-'],
  ['test.kill_hp', 60, 'points', TEST + ': the hit points the script word `dummy-hp N` without a number leaves a dummy with, so that one combo kills it', '-', '-', I, '-'],
  // ---- Elden Ring: THE ONE RULE for what a killed enemy drops (kh-er/src/prize.rs; off unless prize.kh = 1)
  ['er.tier_hp', [300, 1500], 'Elden Ring hit points', HOST + ': an enemy below [0] of maximum HP is tier 0 (weak), below [1] tier 1, else tier 2', 'KH3 has no such tiers: its tables are authored per enemy. The tiers stand for three of its commonest pairs: HpS x2 + MunnyS x1 (10 rows, the Shadow among them), HpS x3 + MunnyM x2 (5 rows), HpL x2 + MunnyL x2 (4 rows)', '-', I, 'MARKED SCALING'],
  ['er.slot1_weights', [67, 36, 10], 'rows', HOST + ': the first prize slot of a kill is HP, MP or focus with these weights', 'the share of KH3\'s own enemy rows whose first slot is an HP (45 + 22), MP (16 + 20) or focus (1 + 9) prize, of the 116 rows that drop prizes (build/sora_tmp/prize/enemy_drops.js). KH3 ITSELF HAS NO RULE THAT LOOKS AT SORA\'S HP OR MP: the drop is the enemy\'s fixed table', '-', I, 'MARKED SCALING; control key prize.weights = hp,mp,focus overrides'],
  ['er.slot1_count', [2, 3, 2], 'count', HOST + ': orbs of the first slot per tier; small orbs in tiers 0 and 1, large ones in tier 2', 'KH3 slots over all enemy rows: HpS x2 (25 slots), HpS x3 (21), HpL x2 (15)', '-', I, 'MARKED SCALING'],
  ['er.munny_count', [1, 2, 2], 'count', HOST + ': munny orbs per tier: small, medium, large by tier', 'KH3 slots over all enemy rows: MunnyS x1 (10) / x2 (14), MunnyM x2 (12), MunnyL x2 (8)', '-', I, 'MARKED SCALING'],
  ['er.munny_per_rune', 1, 'munny per rune', HOST + ': with the rune value of the enemy known (NpcParam getSoul), the munny orbs of a kill together are worth runes x this, split evenly over them', 'munny has to buy what runes buy (user decision), so it is on the rune scale; an orb then carries its amount instead of KH3\'s 1 / 5 / 10', '-', I, 'MARKED SCALING; control key prize.munny_rate overrides'],
  ['er.munny_per_max_hp', 0.4, 'munny per hit point', HOST + ': WITHOUT a rune value: munny = ceil(maximum HP x this), like the EXP rule exp.per_max_hp', 'a guess at the game\'s own ratio (a Limgrave soldier of about 200 HP gives about 80 runes); NOT MEASURED', '-', 'guess', 'used until the rune reader is confirmed in the game (IMPL_PRIZE_STATE.md)'],
  ['er.spawn_height', 80, 'cm', HOST + ': the orbs burst from this far above the CENTRE of the dead enemy (the kill position the EXP rule uses)', 'KH3 spawns at the pawn\'s location (its capsule centre) after a per-pawn offset that was not read (status component virtual +0x1D8)', '-', I, '-'],
];

const columns = ['id', 'value', 'unit', 'source', 'evidence', 'ref_addr', 'ref_conf', 'note'];
const out = '{"sheet":"sora-prizes","group":"port","columns":' + JSON.stringify(columns) + ',"rows":[\n' + rows.map((r) => JSON.stringify(r)).join(',\n') + '\n]}\n';
fs.writeFileSync(path.join(root, 'sheets', 'port', 'sora-prizes.json'), out);

const defs = path.join(root, 'tools', 'lib', 'sheet_defs.js');
let s = fs.readFileSync(defs, 'utf8');
if (!s.includes("'sora-prizes'")) {
  const anchor = "  'sora-er-fx': {";
  const at = s.indexOf(anchor);
  if (at < 0) throw new Error('sheet_defs.js: anchor not found');
  const entry =
    "  // (prize) KH3's prize orbs: movement, life, attraction, values; the test level's drop tables; the Elden Ring host's one drop rule. Written by tools/sora_prize_sheet.js, generated into build/gen/rust/sora_prizes.rs (crates/kh-prize)\n" +
    "  'sora-prizes': { group: 'port', file: 'sora-prizes.json', by: 'tools/sora_prize_sheet.js', from: 'the prize actors and the prize manager of the game (docs/sora-prize-spec.md); rows test.* are choices of the test level and rows er.* choices of the Elden Ring host, not game data', external: true, cols: [\n" +
    "    ['id', K, 'str', '-', 'move.* / spawn.* / life.* / bounce.* / rest.* / magnet.* / get.* / kind.* / box.* / table.* = the game; test.* = the test level; er.* = the Elden Ring drop rule'], ['value', V, 'json', '-', 'a number or a list of numbers; null = unchecked'],\n" +
    "    ['unit', V, 'str', '-', '-'], ['source', V, 'str', '-', 'where the value was read, or who chose it'],\n" +
    "    ['evidence', V, 'str', '-', 'how the game uses it', true], ['ref_addr', V, 'str', '-', 'the function that reads it; \"-\": data or a choice'],\n" +
    "    ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = read from data, an immediate, or an authored choice; inferred = decompiled C only'],\n" +
    "    ['note', V, 'str', '-', '-']] },\n";
  s = s.slice(0, at) + entry + s.slice(at);
  fs.writeFileSync(defs, s);
  console.log('registered sora-prizes in tools/lib/sheet_defs.js');
}
console.log('sheets/port/sora-prizes.json: ' + rows.length + ' rows');
