# Donald and Goofy as AI allies: survey and implementation plan

Written 2026-10-15 (party survey agent; read-only survey, nothing implemented). Companion of `docs/sora-agent-brief.md`
(read it first: Spreadsheet Method, crates, evidence rules, controller rule, testing rule, no attractions).

**The user's request (their words):** "Now get Donald and Goofy working as AI allies in the test level. They should behave
1:1 to the game exactly, and be made to the same quality standard as Sora. Everything about them needs to be functional,
including their animations, AI behaviours, movesets, VFX, SFX, voice lines, etc. EVERYTHING about them. They should work
with our HUD and menu that we've developed already."

**User decisions already made (2026-10-15, through the coordinator; bake in, do not re-ask):**

1. **Team attacks are IN scope**, with their own camera sequences 1:1, their situation commands / prompts, gauges and
   conditions, animations, effects, sounds and voices. All of KH3's Donald / Goofy team attacks are listed in 2.6.
   **Attraction Flow stays excluded** (no states, commands, HUD entries, effects or sounds, also not the NPC `AF*` states).
2. **The test level spawns the party at level 99 by default** (a sheet / debug option), while **their progression works
   exactly as in the game**: shared EXP, their own level tables, stat growth, ability gain, victory bonuses, equipment.

Machine-readable inventories written by this survey (all under `build/sora_tmp/party/`):

| file | what |
|---|---|
| `inventory.json` | per character: identity, model files, anim-set counts, blueprints, base / level / attack rows, the 14 weapons each, voice groups, sound banks, effects, init data, victory bonuses, decomp functions |
| `animsets.json` | every row of every anim set of n_ex001 / n_ex002 (body, face, camp `CM`, `GM`) with the sequence asset it names |
| `team_attacks.json` | the nine team attacks in scope (commands, abilities, states, link pawns, clips, cameras, effects, text keys), the out-of-scope ones, the friend gauge and unison-roll evidence |
| `decomp_index.tsv` | 5,485 decompiled functions whose class is NPC / friend / AI / BT / EQS / FR-state related (`addr class function`), from the first line of every `re/decomp` file |
| `ai_assets.json`, `ai_node_classes.json` | 414 AI-side packages (BTs, EQS, blackboards, friend-link blueprints, AI/) with their export classes; for each node class the assets that use it (170 classes are used by Donald / Goofy / the common NPC trees) |
| `bt_outline/*.txt` | readable outlines (composites, tasks, `@decorators`, `$services` with their fields) of 27 trees: Donald / Goofy roots, BattleFlow, CureFlow, Combination, TogetherAction, the four battle styles, Field, Follow, UseItem, OnDamage, SideMove ... |
| `vbonus_party.json` | the victory bonuses and abilities Donald and Goofy get from `TresVBonusData` (139 rows) |
| `tools/*.js` | the survey's scripts (re-runnable): `props.js <Content path> [maxlen] [export regex]` dumps a package's exports with resolved references, `btoutline.js <tree paths>`, `loc.js <regex>` (English text), `animsets.js`, `btinv.js`, `decomp_index.js`, `inventory.js` |

Evidence tags as in the other specs: **[A]** read from an asset / data table, **[C]** decompiled C only, **[I]**
instruction level, **[U]** stock UE 4.17 behaviour assumed, **[D]** derived, **[?]** not determined. This survey read
assets and decompiled C only; nothing here is [I].

---

## 1. Who Donald and Goofy are in KH3's code

| | Donald | Goofy |
|---|---|---|
| unique id (`ETresChrUniqueID`) | `TRES_CHR_UID_DONALD` = 11 | `TRES_CHR_UID_GOOFY` = 12 | [A]
| model folder | `Character/npc/n_ex001` | `Character/npc/n_ex002` | [A]
| native pawn | `ATresNpcPawn_n_ex001` (0xD08 bytes; getter 0x1427A8C20, natives 0x1427AB1A0, `execIsRideOn` 0x1427C88F0) | `ATresNpcPawn_n_ex002` (getter 0x1427A8D00) | [C]
| class chain | `TresNpcPawn_n_ex00x` : `TresNpcPawnBase` (195 decompiled functions) : `TresAIPawnBase` (79) : `TresCharPawnBase` | same | [A][C]
| blueprints | `Blueprints/Npc/n_ex001/n_ex001_Pawn` -> `n_ex001_PawnBase` | `Blueprints/Npc/n_ex002/n_ex002_Pawn` -> `n_ex002_PawnBase` | [A]
| AI controller | `Blueprints/Npc/Common/Controller/n_Common_SwimmingAIController` : `TresNpcControllerBase` (getter 0x14278DFA0, 39 functions, natives `execStartLead` ... 0x1427A68F0..0x1427A7040) : `TresAIController_BT` : `TresAIController` : `AIController` | same | [A][C]
| party slot | `FriendRelationDataTable` row 0, index 0 (HUD slot 1) | row 1, index 1 (HUD slot 2) | [A]
| UI | `UI/Data/Character/n_ex001_StatusData`, camp actor `UI/Blueprints/CampTop/n_ex001_UIActor`, face movie `UI/Scaleform/01_hud/face/FaceNpcExDonald` | `..._n_ex002_...`, `FaceNpcExGoofy` | [A]
| world variants (NOT in scope) | n_ca001, n_ts001, n_mi001 (Caribbean, Toy Box, Monstropolis looks; `FriendIdDataTable`) | n_ca002, n_ts002, n_mi002 | [A]

**Do not confuse:** `Character/pc/p_ex002` (with `w_aq010`, Aqua's weapon) and `Character/pc/p_ex003` (with
`w_ri_KeybladeBase`, Riku) are OTHER playable characters (Dark World), not Donald and Goofy. The party members are the
NPC pawns n_ex001 / n_ex002.

Pawn components [A] (`Default__n_ex001_PawnBase_C`): `TresCharMovement0` (movement), `TresCharMesh0`, `TresAtkColl0`,
`TresBodyColl0`, `TresEquipment0`, `TresStateQueue0` (the same state-queue component as Sora's), `TresEffectAttach0`,
`TresNpcReactor0`, `PhysObjHitCollision`, `MercunaObstacleComponent`. Fields: `m_GripHeight` 350,
`m_FriendLinkPointInterval` 5, `MyMpChargeTime` 30, `m_DangleOffsetPos` (-30, 0, -70), `m_CmnBadStatesEffect`
`ead_com_abn0`, `m_pDataTableSet`, `MyAIBehavior` (root BT), `m_DynamicAIBehavior` (tag -> tree injections),
`m_DefaultLocomotionDefinition` (`n_ex00x_Move1`), `m_Walk` (`Move_Slow`), `m_SlopeSlide`, `m_Swimming`,
`m_FollowPlayerEQSQuery` (`n_Common_FollowPlayerWithSwimQuery`). Donald: `m_FRCallMeteorBP`, `m_FRFlareForceBP`;
Goofy: `m_FRShieldGuardianBP`, `m_FRGoofyShootBP`, `m_GoofyShootProj`, `m_GoofyShootExpEnemyList` (20 enemy uids),
`m_ScrumModeCountMax` 20. Native fields (reflection, `TresNpcPawnBase`): `m_FriendLinkPointMax` (+2872),
`m_FriendLinkPointInterval` (+2876), `m_JumpSpeedRate`, `m_JumpRotRate`, `m_SuperSlideSpeed`, `m_SoundAssets_Damage`,
eye-blink timers, `m_pFriendMan`.

The manager is `UTresFriendComponent` (constructor 0x1412558B0; natives 0x14212D550..: `IsExistDonald`, `IsExistGoofy`,
`SetFriend`, `DeleteFriend`, `GetHomePos`, `IsDisableFriendWarpHome`, `PlayVoiceFriendLink`, `IsExeLink`,
`CheckDisableFriendLinkCmd` ...) with `UTresFriendData` (`Blueprints/Npc/Common/DataTable/FriendData`: id, relation,
home-position and evaluation tables, `WarpQuery`, `UnderwaterWarpQuery`) [A][C]. Meta AI: `UTresNpcMetaAI`
(0x14121CE10) and the blueprint `Blueprints/Npc/Common/B_FriendNpcMetaAI` [A][C].

---

## 2. Survey

### 2.1 Model, materials, skeleton, physics

| | Donald (n_ex001) | Goofy (n_ex002) |
|---|---|---|
| mesh / skeleton | `mdl/n_ex001_0`, `n_ex001_0_Skeleton`; glTF `assets_converted/meshes/.../npc/n_ex001/mdl/n_ex001_0.glb` | `n_ex002_0`, `n_ex002_0_Skeleton`; `.../n_ex002_0.glb` |
| materials | `mat0/n_ex001_mat_Inst`, `n_ex001_mat1_Inst`, `n_ex001_mat_eye_Inst`; textures 0d00, 0m00, 0m01, 0n00, 0o00 (DDS converted) | `n_ex002_mat0/1/2_Inst`, `n_ex002_E_mat_inst`; same texture set |
| KineDriver | `n_ex001_0_kdi` (one `SQEX_KineDriverData`) | `n_ex002_0_kdi` and a variant `n_ex002_0_kdi_shortEar` |
| Bonamik | `n_ex001_Bonamik` (3.9 MB asset: hat, coat tail ...) | `n_ex002_Bonamik`, `n_ex002_Bonamik_cs` (variant) |
| physics asset | `n_ex001_0_Physics` | `n_ex002_0_Physics` |
| weapon (default) | `WEP_DONALD00` = Mage's Staff (まどうしの杖), `w_do010`, socket `R_wep_skt`, `Blueprints/Npc/n_ex001/w_do010_Equip / _Wep`, mesh `Character/wep/w_do010` | `WEP_GOOFY00` = Knight's Shield (騎士の盾), `w_gu010`, socket `R_wep`, `w_gu010_Equip / _Wep` |
| all weapons | 14 (`w_do010`..`w_do140`, rows I03021.., `TresEquipItemData`) | 14 (`w_gu010`..`w_gu140`, rows I03041..) |

All [A]. The bases `w_do_WepBase` / `w_gu_WepBase` hold the weapon's own components. Whether the KineDriver and Bonamik
assets use operators / branches that `kh-anim` does not have yet is NOT known (the KDI data is a binary blob in the
JSON): stream A's first task is the census with `tools/sora_outfit_kdi.js`'s parser and the Bonamik `UNPORTED` bits.

### 2.2 Animation

Anim sets (`TresAnimSet`, rows = name -> sequence) [A] (`animsets.json`):

| set | Donald | Goofy |
|---|---|---|
| body `n_ex00x_AnimSet` | 220 rows (12 blend spaces) | 225 rows (12 blend spaces) |
| face `FaceAnimSet` | 80 | 81 |
| camp menu `CM_Body` / `CM_Face` | 2 / 2 (`UI/body/0001CM`, `0002CM`) | 2 / 2 |
| `GM_Body` / `GM_Face` (gummi) | 1 / 2 | 1 / 2 |
| files | `anm/rt/body` 263, `anm/rt/face` 126 (+ localisation / event) | 274 / 122 |

Naming is the same scheme as Sora's (`IdleN`, `WalkRunN/W/B` blend spaces, `RunStartN/W/B`, `RunLoopN/W/B`, `BattleStart`,
`IdleB`, `JumpNormalStart/RiseLoop/DownLoop`, `Land`, `M_02xx` damage family, `BadStat*`, `ItemUse`, `M_13xx` attacks,
`M_15xx` reactions / revive ...). Families in the Donald set: locomotion (normal / walk / battle), jump, grip,
swim / dive (0081..0092), water flow, slope slide, climb, rail slide, super slide (`SonicSlide*`), damage 0201..0230,
stun / burn / sneeze, reflect, attacks `M_1301` `M_1302` `M_1311` `M_1312`, magic `MagicFire` (1321), `MagicAirFire`,
`MagicCast` (1341), `MagicAirCast`, items `ItemUse` (0391) / `ItemAirUse`, `M_15xx`, team attacks (`CMeteoFinish`
1721, `FForceFinish` 1723, `DoUFire/Blizzard/Thunder` 1761..1763, `SGuard*` 1621..1635), other allies' team attacks
(`MCurling*`, `SCurling*`, `Mbowl*`, `SHold*`, `Deck*`, `Catch*`), talk. Goofy adds `GShootStart..End` 1636..1640,
`GooFSpin` 1764, `GooFRocket` 1765, `GCoopStart/Loop/End`, `SCSwing*`, `M_1321..M_1332` (Tornado / Turbo / Discus
clips). Several rows point at shared sequences (e.g. Donald's `RunBattleStartW` = 0013) [A].

Animation runs through `TresNpcAnimInstance` : `TresAIAnimInstance` (decompiled: 2 and 7 functions) [C] and the same
state-queue / notify system as Sora (attack windows, effects, sounds, voices in notifies). Sora's own team-attack clips
(`GShoot*`, `SGuard*`, `CMeteoFinish`, `FForceFinish`, `DoU*`, `GooF*`) are ALREADY in Sora's pack (all 427 rows of
`p_ex001_AnimSet` are packed) [A].

### 2.3 Stats, levels, abilities, equipment, progression

Base rows [A] (`DataTable/Npc/Base/n_ex00x_BaseDataTable`, `TresChrBaseParamDataTable`):

| field | Donald | Goofy |
|---|---|---|
| `m_MaxHitPoint` / `m_MaxHPRate` | 140 / 1 | 160 / 1 |
| `m_MaxMagicPoint` / `m_MaxFocusPoint` | 100 / 100 | 100 / 100 |
| `m_DefensePower` / `m_AbilityPoint` | 3 / 2 (overwritten by the level row) | 3 / 2 |
| `m_DamageMin` / `m_DamageMax` | 0 / **24** | 0 / **24** |
| body push, bio type | PPL_50, Human | PPL_50, Human |

`m_DamageMax` 24 is the defender's per-hit damage CAP in the damage function (stats spec 7; `TargetDefense.damage_max`):
a single hit takes at most 24 HP from Donald or Goofy [A + C via the stats spec].

Level tables (`DataTable/Npc/LevelUp/n_ex00x_LevelUpData`, 99 rows, `TresChrLevelUpDataTable`) [A]:

| | level 1 | level 20 | level 99 |
|---|---|---|---|
| Donald EXP / Str / Mag / Def / AP | 0 / 11 / 16 / 11 / 14 | 28,242 / 21 / 26 / 21 / 18 | 1,512,931 / 63 / 67 / 63 / 34 |
| Goofy | 0 / 14 / 13 / 11 / 14 | 28,841 / 24 / 23 / 21 / 18 | 1,520,285 / 65 / 65 / 63 / 34 |
| Sora (for comparison) | 0 / 11 / 12 / 8 / 28 | 29,440 / ... | 1,527,640 / 62 / 64 / 59 / 180 |

**EXP / progression rule [C, stats spec 5.2]:** EXP is one shared total (`GW+0x7DF0`); after an award, `CheckLevelUp(GI,
total, pawn, hud)` 0x140EBE400 runs for the player AND each of the six party slots (`FUN_141261FB0`) against the SAME
total with each character's own table: there is no split. So "party at level 99" follows from Sora's level-99 EXP
(1,527,640 >= both party tables' row 99). At the test level's level 20 (dec021) Donald and Goofy get the level their own
tables give for Sora's EXP (Donald's row 20 needs 28,242, Goofy's 28,841). The level-up HUD has a slot per character
(`kh-ui` `hud_side.rs`: chara 0 Sora, 1 Donald, 2 Goofy; 0x141659CD0) [C].

Init data [A] (`Load/Common/TresChrInitData` `m_FriendDonald` / `m_FriendGoofy`):

| | Donald | Goofy |
|---|---|---|
| weapon | `WEP_DONALD00` | `WEP_GOOFY00` |
| slots | accessory 2, item 2 (protector default) | protector 2 (accessory / item default) |
| items | Potion, Ether | Potion, Potion |
| equipped abilities | DONALD_FIRE, DONALD_BLIZZARD, DONALD_THUNDER, DONALD_AERO, DONALD_CURE, PRIZE_DRAW | GOOFY_TORNADE, GOOFY_TURBO, GOOFY_FLIP, DEFENDER, HARVEST, PRIZE_DRAW |

Ability rows [A] (`Load/Tres/TresAbilityData`): DONALD_FIRE 201 (AP 2, MP 10), DONALD_BLIZZARD 202 (2, 10),
DONALD_THUNDER 203 (3, 15), DONALD_AERO 204 (3, 15), DONALD_CURE 205 (3, MP -1 = all), GOOFY_TORNADE 206 "Goofy Tornado"
(3, 15), GOOFY_TURBO 207 (2, 10), GOOFY_FLIP 208 "Goofy Discus" (2, 10); party support: HYPER_HEALING 195 (3),
COMBI_UP 196 (5), COMBI_TIME 197 (3), SHARE_PRIZE 198 (4). Sora's team abilities: UNISON_FIRE / BLIZZARD / THUNDER
190..192, FUSION_SPIN / ROCKET 193..194 (AP 3 each, all in Sora's level-99 loadout), FRIEND_AID 27 "Assist Friends"
("Use healing items and magic on friends", the party target list).

Victory bonuses [A] (`vbonus_party.json`, from `TresVBonusData` columns `m_BonusDONALD1` / `m_AbilityDONALD1` ...):
each gets 9 bonuses: HP_UP10 x6, ITEM_SLOT_UP1, DEF_SLOT_UP1, ACC_SLOT_UP1; Donald's abilities MP_HASTE, FIRE_UP,
BLIZZARD_UP, THUNDER_UP, AERO_UP, COMBI_TIME, COMBI_UP, DAMAGE_ASPIR, HYPER_HEALING; Goofy's LUCK_UP, COMBI_TIME, COMBI_UP,
LAST_LEAVE, SHARE_PRIZE, HYPER_HEALING, CRITICAL_HALF, COMBO_LEAVE. Max HP with all bonuses = base + 60 = Donald 200,
Goofy 220 by the player's formula (stats spec 8: "max HP = base + sums") [D: confirm that the friend kind (status kind
4, 0x140576E70) uses the same sums].

Equipment [A]: weapons in `TresEquipItemData` (I03021..I03034 Donald: Str / Mag / abilities, e.g. Mage's Staff +3 / +4;
I03041..I03054 Goofy, Knight's Shield +4 / 0); armour / accessories are the shared rows. Party save records are the same
0x9C0-byte records as Sora's (`GW+0x9640 + i*0x9C0`, stats spec 8): slot counts at +3..+5, weapon / armour / accessory /
item slots, ability words, bonus counters, and the friend AI setting bytes +0x15A..+0x15C (UI spec 10.6).

### 2.4 Movesets

Each attack / action is a blueprint `TresNpcAttackDefinition*` / `TresNpcActionDefinition*` subclass used by the
trees through `TresBTTask_Action` [A]; its native class carries the logic [C]. Attack data rows: `DataTable/Npc/Attack/
n_ex001_AttackDataTable` (16 rows) / `n_ex002_AttackDataTable` (10 rows) [A].

**Donald** (`Blueprints/Npc/n_ex001/BT/`):

| action | native class / data | numbers [A] | attack rows |
|---|---|---|---|
| `Attack1_Beat` | `TresNpcAttackDefinitionMelee`; clip 1301 | max dist 200, max height 120, traction 500 | Attack1_Beat (0.8, SMALL) |
| `Attack2_Swing` | melee; clip 1302 | max dist 150, max height 120 | Attack2_Swing (0.8, SMALL) |
| `Attack3_Jump` | `TresAttack3_n_ex001_Jump` : `TresNpcAttackDefinitionJumpBase` (16 functions) | jump height 200, speed 450, limit 460, move 300..600, attack range 100 | Attack3_JumpAscent / Descent (0.8) |
| `Attack4_MagFire` | `TresNpcAttackDefinitionMagic`; `DONALD_FIRE` | dist 100..5000, shot pitch 5..30 | MAG_FIRE (1.5) |
| `Attack5_MagBlizzard` | magic, `DONALD_BLIZZARD` | as Fire | MAG_BLIZZARD (1.5) |
| `Attack6_MagThunder` | magic, `DONALD_THUNDER` | as Fire | MAG_THUNDER (1.0) |
| `Attack7_MagAero` | magic, `DONALD_AERO` | as Fire | MAG_AERO (0.5, SWIRL_UP) |
| `Action2_MagCure` | `TRES_CMD_KIND_CURE`, `DONALD_CURE`, type ABILITY_CURE | max dist 800, no target / yaw needed | MAG_CURE (0.25, RECOVER) |
| `Attack8_WaterBeat`, `Attack9_WaterSwing` (`TresAttack9_n_ex001_WaterSwing`) | under water only | | MAG_WATER*, Attack8/9 |
| `Action1_RideMarshmallow` | Arendelle set piece | out of scope | |

Donald's spells: `n_ex001_MagicSet` (`TresPlayerMagicSet`, units FIRE / BLIZZARD / THUNDER / AERO / CURE / WATER*,
effect groups 1300..1340 / 1400..; projectiles `n_ex001_ProjectileMag{Fire,Blizzard,BlizzardBurn,Thunder,Aero,
AeroCylinder,Cure,Water*}`; Fire: life 5 s, particle `vn_ex001_fir_fir00`) and `n_ex001_Magic_ead` [A].

**Goofy** (`Blueprints/Npc/n_ex002/BT/`):

| action | native class | numbers [A] | attack rows |
|---|---|---|---|
| `Attack1_Push` (盾振り, shield swing) | melee, clip 1301 | max dist 230, height 50..135 | Attack1_Push (1.0) |
| `Attack2_Sweep` (盾回転) | melee, clip 1302 | max dist 260, height 50..85, yaw tol 130 | Attack2_Sweep (1.0) |
| `Attack3_Jump` | `TresAttack3_n_ex002_Jump` | as Donald's | Attack3_JumpAscent / Descent |
| `Attack6_Tornado` (Goofy Tornado) | `TresAttack6_n_ex002_Tornado` (ctor 0x1411BB8E0) | move 3 s, fly height 60, fly speed 20, traction 640 / 1000, max dist 1000 | Attack6_Tornado (0.3) |
| `Attack7_Turbo` (Goofy Turbo) | `TresAttack7_n_ex002_Turbo` (0x1411BB980) | move 6 s at 900, max dist 2000 | Attack7_Turbo (1.0, PARABOLA) |
| `Attack8_Discus` (Goofy Discus = GOOFY_FLIP) | `TresAttack8_n_ex002_Discus` (0x1411BBA30) : Ranged | attack 2 s, start speed 2500, dist 300..3000, pitch offset 7.5 / tol 37.5 | Attack8_Discus (2.0) |
| `Attack4_WaterUpper`, `Attack5_WaterRush` (`TresAttack5_n_ex002_WaterRush`) | under water | | |

Locomotion definitions [A]: `n_ex00x_Move1` (min 140, max 680 cm/s, run-loop interpolation 0.2 s, the same numbers as
Sora's walk / run), `Move_Slow` (140 / 140, no run start, yaw rate 90). Natives
`UTresNpcLocomotionDefinitionLand` (0x1412031A0), `..._FollowPlayer` (0x141203230, 13 functions), `..._SlopeSlide`,
`UTresNpcLocomotionDefinitionSwim` (25 functions) [C].

### 2.5 States (what the pawn runs)

NPC states [A][C] (constructors from `decomp_index.tsv`): `TresNpcState_Idle`, `_Run` (: `TresAICharState_Run`, 16),
`_Turn`, `_Jump`, `_Damage` (0x14123F830; 20 functions; : `TresAICharState_Damage`, 78), Goofy's override
`TresNpcState_n_ex002_Damage` (0x1411BBC30) and `_Reflect` (0x1411BBD30), `_Reflect`, `_BadStatStun / Burn / Freeze /
Sneeze`, `_Die` (0x14123F8B0), `_UseItem` (0x141255F30), `_Warp` (0x141255F70), `_SituationAction` + Goofy's
`n_ex002_SituationAction` (0x1411BBD70), `_SlopeSlide`, `_RailSlide`, `_SuperSlide`, `_Hopping`, `_Climbing`, `_Dangle`,
`_Hidden`, `_Appear`, `_Disappear`, `_Cinematic`, `_TalkMotion`, `_BTLComical`, the FR states (2.6) and the AI char states
`TresAICharState_Action` / `AttackBase` / `PlayMotion` / `Stagger` / `JumpCodeDriven` / `NavLink*`. The state ids the
trees test are `TSID_STATE_IDLE`, `TSID_STATE_TURN`, `TSID_STATE_AI_MOVE`, `TSID_STATE_AI_SWIM`, `TSID_STATE_DAMAGE`
(`TresBTDecorator_TresStateCheck`, `m_ViableStates`) [A].

**KO and revive** [C] (`UTresNpcState_Die`, Begin 0x1412413C0, End 0x141244670): on entry the revive timer (+0x88) is
**30 s**, **40 s for unique id 11 (Donald)**, **35 s for 12 (Goofy)**, divided by `1 + min(count of HYPER_HEALING 195, 3)`
(vt+0x14E0 / vt+0x1500 with 0xC3: has / count of the ability [?: owned or equipped copies]; both get one HYPER_HEALING
from victory bonuses: 20 s / 17.5 s when it counts); the pawn is made invincible (vt+0x2068 1) and loses
collision (vt+0x6A0 0). On leaving: if HP < 1 it is recovered (vt+0x1878 with vt+0x2198's amount), invincibility off,
collision on, notify 2 to the anim interface. The HUD face shows "death" for a friend with HP < 1 (`hud_status.rs`,
UI spec 4). A heal on a KO'd ally is halved (`kh-core` `status/runtime.rs` note, Ally.dead) [C].

### 2.6 Team attacks (IN scope, user decision 1)

Nine team attacks involve Donald or Goofy (full table in `team_attacks.json`) [A][C]:

| name (en) | command (`ETresCommandKind`) | trigger | Sora side | NPC side | link pawn / projectiles | camera |
|---|---|---|---|---|---|---|
| Unison Fire / Blizzard / Thunder | FR_DONALD 22 / FR_DONALD2 23 / FR_DONALD3 24 | combo finisher near Donald (Sora's abilities 190..192) | attack kinds of the unison branch (combat spec rule 7, attack slots 137/138, kinds 0x3F..0x43); clips `DoUFire/Blizzard/Thunder` | `TresNpcState_n_ex001_FRDonaldSora` (0x1411BBBB0) | effects `vn_ex001_uni_*` | the combo camera [?] |
| Fusion Spin / Fusion Rocket | FR_GOOFY 25 / FR_GOOFY2 26 | combo finisher near Goofy (193, 194) | clips `GooFSpin`, `GooFRocket` | `TresNpcState_n_ex002_FRGoofySora` (0x1411BBCB0) | `vn_ex002_spn_*`, `vn_ex002_frt_*` | [?] |
| Donald Meteor | FR_CALLMETEOR 207 | situation command (friend gauge) | `TresPlayerState_FRCallMeteor` (state 135; `sora-states` row: vtable 0x14691EF30, begin 0x141368F90, tick 0x141375420) | `TresNpcState_FRCallMeteor` (0x14123FA10) | `FriendLink/CallMeteor_Pawn` (`ATresFriendLinkPawnCallMeteor` 0x1412C6650), `CallMeteor_Proj`, `CallMeteor_Exp_Proj`, `CallMeteorShake_CurveData` | `BP_TresPlayerStateEvent_donald` (`callmeteo_camera`, `callmeteo_finish`) -> CameraAnim `Camera/sora/Skill/donald/CallMeteo_Finish` |
| Flare Force | FR_FLAREFORCE 208 | situation command | `TresPlayerState_FRFlareForce` (136) | `TresNpcState_FRFlareForce` (0x14123FA60) | `FlareForce_Pawn` (0x1412C6960), `FlareForce_Proj`, `FlareForce_ExpProj`, `FlareForceAppear/Fire_CurveData` | same BP (`dona_Flair`) -> `Skill/donald/dona_flair` (3.93 s, FOV 70) |
| Goofy Bombardier | FR_GOOFYSHOOT 195 | situation command | `TresPlayerState_FRGoofyShoot` (101) | `TresNpcState_n_ex002_FRGoofyShoot` (0x1411BBC70) | `GoofyShoot_Pawn` (0x1412C6FB0), `GoofyShoot_Proj`; attack row FR_GOOFYSHOOT_RUSH (BLOW) | `BP_TresPlayerStateEventCamera_goofy_shoot` -> `Skill/goofy_shoot/goofy_shoot_01, _03_30f, _05, _tarl` + shakes `shake_kokizami_8sec`, `tebure_shake01_10sec` |
| Trinity Guard | FR_SHIELDGUARDIAN 193 | situation command, then its own command menu (MainCommandData `FR_SHIELDGUARDIAN`: Shield Bash / Jump / Cancel, logo `ActLogoF01`) | `TresPlayerState_FRShieldGuardian` (30) | `TresNpcState_FRShieldGuardian` (0x14123FC90), both friends (`SGuard*` clips) | `ShieldGuardian_Pawn` (0x1412EA6D0, 15 functions) | `BP_TresPlayerStateEventCamera_ShieldGuardian` -> `Skill/shield/shield_start` |

Texts [A] (`kh3_system` en): situation `fr_donald` "Unison Fire" ... `fr_callmeteor` "Donald Meteor", `fr_flareforce`
"Flare Force", `fr_goofyshoot` "Goofy Bombardier", `fr_shieldguardian` "Trinity Guard"; command
`fr_sheildguardian0/1` "Shield Bash" / "Jump".

**Producers** [C, combat spec 4.2 and 11]:
- Unison / fusion: pawn slot 1304 0x141430A60 (called by `Decide` one step before the last combo step): returns unless
  `(count of 190..194) x MyFriendComboFinishAddRate x 10 != 0`, the pawn is player-controlled and `ctrl+0x920 == 0`;
  needs party member 11 or 12 within `MyFriendComboFinishRange` (pawn props +5248..+5272: Range / RangeHalf /
  RangeNear and AddRate / AddRateRange / AddRateRangeHalf / AddRateRangeNear); `rand() % 1000` arms the request
  pawn+0x1464; slot 1303 0x141431A80 clears it. `Decide` then takes the unison branch (bit 7 of next+0x1EC).
- Team attacks: the friend gauge on the player controller (+0xC0C / +0xC10), `MyFriendPointMax` 200 (pawn prop +3316),
  `MyFriendPointTime` 30 (+3320); points from hits through slot 899 0x1413E8D70, reset by slot 901;
  `NotifySetActionCommand` 0x14140DA40 enqueues kinds 193..210 with time `x (1 + 0.5 x COMBI_TIME of the friend)`,
  capped 99.9 s. NPC side: `m_FriendLinkPointMax` / `m_FriendLinkPointInterval` (5) [A]. HUD: situation entries with
  icon type 4 `friend` (UI spec 5.6) [C]. The friend's `COMBI_UP` raises team-attack damage [?: hook not read].
- The NPC also asks for team commands from its tree: `TresNpcBTTask_SetFriendLinkCommand`, `TresNpcBTTask_SetFriendTarget`
  [A][C].

Voices: each friend's VoiceGroupSet group `FriendLink` (4 entries, `battle/FNPC_Link/bt4090230do0` ...), the FNPC_Link bank
(37 `do0` / `go0` files) and Sora's lines (`bt40500x0sr0`, `bt4090xxxsr0`, mapped in `sora-outfit-voice` rows
`voice.kh1.call_donald` / `call_goofy`) [A].

**Out of scope** (data stays in the pack, nothing runs): the world allies' team attacks whose clips sit in the Donald /
Goofy sets (Mike Bowling, Mountain Curling, Snow Curling, Snow Chase, Spinning Hold, the pirate-ship `Deck*`, `Catch*`),
Snow Cover Swing (a gimmick of the boss e_fz903), Attraction Flow (user rule), Links / summons (not ported; while a link
runs KH3 removes the friends, `IsExeLink`).

### 2.7 Damage, reactions, bad status

Hits on a friend go through the same damage function as on Sora (`kh_core::status::compute_damage`, 0x140632240) with
defender kind 4 (`DefenderKind::Friend` exists in `kh-core/src/status/damage.rs`; "player and friends: base at most 80
unless min >= 80", 0x1406326B5), the friend's Defense and the 24 cap [C]. Reaction: `UTresNpcState_Damage` over
`UTresAICharState_Damage` (the same base the enemy-reaction spec read for enemies: handlers by kind Small / Blow / Toss /
Launch / Beat / SideSpin / Parabola) with the friends' parameter asset `Blueprints/Npc/FriendDamageCommonParam` [A][C];
Goofy's `n_ex002_Damage` / `_Reflect` override [C]. Friends are "same team": `m_NeedReactionSameTeamZeroDmgAtk` false
[A]; Sora's attacks do not hurt them (team check, attack-data column `team_check`) [C, combat spec; stream B confirms
the same for a friend attacker's projectiles]. Bad
statuses use the shared `ead_com_abn0` effect [A].

### 2.8 MP, magic, items, healing

- MP 100 (base), MP charge `MyMpChargeTime` 30 s [A]. Spell costs from the ability rows (2.3); Cure costs all MP
  (`m_UseMP` -1) [A].
- Healing: `BT_n_ex001_CureFlow` (injected under tag `BehaviorTree.FNPC.CureFlow`), `BT_n_Common_UpdateCureTarget`,
  EQS `EQ_NpcTarget_CurePawn_HP` / `_MP`, tests `TresNpcEnvQueryTest_CureHpCheck` / `CureMpCheck` / `IgnoreCureTarget`,
  `TresNpcBTService_SetCureTargetBB`, `TresNpcBTTask_NpcCureAction` (0x141203130), decorators `AIStyle_Cure`,
  `IsCureInfoCommand` [A][C]. `FriendEvaluationData` (13 rows: CURE, CURE special, POTION 80, HIPOTION 160, MEGAPOTION
  160 all, ETHER 100, HIETHER 999, MEGAETHER 999 all, FOCUS 40 / 100, ALLCURE, ELIXIR, LASTELIXIR: HP / MP / focus / bad-
  status weights) [A].
- Items: `BT_n_Common_UseItem`, `TresNpcBTDecorator_HasItem`, `TresNpcBTTask_NpcUseItem` (0x14121C220),
  `TresNpcState_UseItem` (0x141255F30); the friends' item slots (Donald: Potion, Ether; Goofy: 2 Potions) [A][C].
- Camp-menu AI settings (UI spec 10.6, writer 0x1415EA860, record bytes +0x15A..+0x15C) map onto the enums [A]:
  `ETresFNpcAIStyle_Battle` TOGETHER 0 (いっしょにたたかえ) / FREE 1 / SUPPORT 2 (ソラにまかせろ) / FOCUS 3 (ねらいをさだめろ)
  = menu "Stick By Sora / Be Yourself / Hands Off / Finish the Job"; `ETresFNpcAIStyle_Ability` FREE / BALANCE / LESS =
  "Go Wild / Watch MP / Conserve"; `ETresFNpcAIStyle_Cure` OFTEN / PINCH / ALWAYS with the target bits On Sora / On Self /
  On Others (+0x15B bits 0..2). The order-to-value mapping is [D] (by position; stream F confirms it in 0x1415EA860).

### 2.9 The AI system

KH3's friends are **UE4 Behaviour Trees + Blackboards + EQS** with ~170 Square Enix node classes [A]:

- Root `n_ex00x_BT` (blackboard `n_ex00x_BB`): an unconditional sequence sets `SmaphoCameraAction`, `HasCure` /
  `HasCombination` (Donald), then `IsStopAI` gate, then `CanBattle()` -> `BT_n_Common_Battle` ("バトルAI") else
  `BT_n_Common_Field` ("フィールドAI") (`bt_outline/n_ex001_BT.txt`) [A].
- `BT_n_Common_Battle`: battle begin move (EQS + MoveTo), cure flow (dynamic `BehaviorTree.FNPC.CureFlow`, else
  `BT_n_Common_UseItem`), `BT_n_Common_Combination`, follow / keep-distance (MoveTo radius 850 / 600 to the player), and
  the four battle styles `BT_n_Common_BattleStyle_Together / Free / Support / Focus`, each running the character's own
  `BT_n_ex00x_BattleFlow` (391 / 316 outline lines: wait phase, attack-flow wait, ability cooldowns 20 s, melee
  cooldowns 1.5 s, magic chosen by `TresBTComposite_Random` with weights and target-resist checks, follow / reposition
  EQS, the `OnDamage` subtree, many boss-specific branches keyed by enemy uid) [A].
- Field: `BT_n_Common_Field`, `n_Common_FollowPlayer` / `FollowPlayerQuery` / `WithSwim` / `NonFollowPlayerLand` /
  `RandomWalkAroundPlayer`, `n_Common_InterpretPlayerIntentService` (`UTresBTService_InterpretPlayerIntent`
  0x14047A470), lead / NPC-mode trees (not needed in the test level) [A][C].
- Formation: `FriendHomePosDataTable` (6 rows; row 0 base 180 cm at +70 deg, row 1 180 cm at -70 deg; middle / large
  lengths and angles per row) with contexts `TresEnvQueryContext_FriendHomePosition(Side)` [A]; which of base / middle /
  large applies is [?].
- Warp to Sora when far: `TresNpcState_Warp` (0x141255F70; 8 functions), `EQSQuery_NpcWarp` (ring of radius >= 400
  round the player's floor position, item spacing 300..500, behind-camera dot test < 0.64, height +-300, away from allies
  >= 100, away from gimmick obstacles) and `..._ForUnderwater`; trigger distance / time [?: in `TresNpcPawnBase` /
  `UTresFriendComponent`, to read] [A][C].
- Target selection: `n_ex00x_TargetSelection` EQS, `EQ_NpcTarget_Enemies / LockOnTarget / EnemyCount / TargetInRange /
  NearAllies / Player`, `TresBTService_TargetSelection` (9 functions), `TresBTService_GameplayFocus`; per-difficulty
  coordinator curves `AI/Coordinator/*/Default_MaxAttackers_Target_FriendNPC_*`, `Default_MaxSupporters_*`,
  `Default_MaxLeaders_Target_FriendNPC` [A][C].
- Combat distance / attack validation: `TresBTDecorator_ValidateAction`, `RangeCheck`, `NpcRangeCheck`, `AngleCheck`,
  action EQS `AI/EQS/Queries/Default_FindAttackLocation(3D)` [A].
- Engine side [U]: stock UE 4.17 `UBehaviorTreeComponent` / `UBlackboardComponent` / `UEnvQueryManager` (composites
  Selector / Sequence / SimpleParallel, decorators Blackboard / Cooldown / TagCooldown / Loop / TimeLimit / ForceSuccess,
  `BTTask_RunBehavior`, `RunEQSQuery`, `Wait`, `BTService_RunEQS`, EnvQueryTest_Distance / Dot / Trace / Pathfinding,
  generators Composite / CurrentLocation / SimpleGrid / PathingGrid). Only two engine BT functions carry names in the
  decompilation; the stock semantics are public UE 4.17 behaviour.
- Navigation: KH3 uses the navmesh and Mercuna (3D) for path tests; the test level has neither (see open note N1).

### 2.10 Field behaviour

Idle / idle break, walk / run with the same 140 / 680 limits as Sora, battle stance (`BattleStart`, `IdleB`, `RunLoopB`),
jumps (`TresNpcState_Jump`, `m_JumpSpeedRate`, `m_JumpRotRate`), grips (`m_GripHeight` 350), slope slide (`n_ex00x_
SlopeSlide` BT + state), rail slide, super slide (`m_SuperSlideSpeed`), hopping, climbing, dangle, swimming (no water in
the test level), warp. They follow Sora's flowmotion by moving / warping, not by copying it [A; the per-move rules are
in the states, [C]].

### 2.11 HUD

Already modelled in `kh-ui` [code]: `hud_status.rs` has five status slots (`FRIEND_MAX` 4; slot 0 Sora, 1..2 friends,
3..4 guests), the friends' fixed 180-frame HP / MP gauges (`npcHpGg`, `npcMp`), the face looks (death when a friend's
HP < 1, dying, battle, normal), the solo / party layout switch (`change1` / `change2`) and per-slot face loading;
`hud_side.rs` the per-character level-up window; `command.rs` / `hud_command.rs` the party target list
(`initSubCommand2`, `setSubCommand2Text`) and the `friend` situation icon. The face movies `FaceNpcExDonald` /
`FaceNpcExGoofy` and their textures are ALREADY in the UI pack (`build/sora_pack/ui/movies`, `.../face/NpcExDonald.png`).
What is missing: the host feed (`kh-host/src/hud.rs` fills only `slots[0]`).

### 2.12 Camp menu

UI spec 10 [C]: the camp controller builds the party table `m_PartyParams[5]` (0x14154C710; slot 0 the player, then up to
four friends) and spawns one `TresCampPawnBase` per slot (UI actors `n_ex001_UIActor` / `n_ex002_UIActor`, camp clips
`0001CM` / `0002CM`); top screen party panels (0x1415D0020, layout `one .. five`), character select before Equipment /
Abilities / Stats, L2 / R2 (events 11 / 12, Xbox LT / RT) previous / next member in Equipment, Items, Abilities, Stats;
Equipment target mode for the party; Customize -> friend settings (2.8). `kh-ui` has the screens for Sora alone
(`camp_equip.rs` "other party members: L2 / R2 do nothing with Sora alone"; `camp_custom.rs` "the friends' layer (Sora
alone)"; `command_debug.rs` a `party on|off` stand-in).

### 2.13 Voice, sound, effects

| | Donald | Goofy |
|---|---|---|
| voice groups (`n_ex00x_VoiceGroupSet`) | QuarterHP 2, RecoveryHP 2, FriendLink 4, BattleStart 2, MagicCast 16, donald_damage_1 1, GiveRecovery 2 | QuarterHP 2, RecoveryHP 2, FriendLink 4, BattleStart 2, GiveRecovery 2, goofy_damage_1 1 |
| voice banks | `Sound/VOICE/Localization/<lang>/battle/NPC/DG/Battle_Donald` (+ `bt2160xxxdo0` cue files; 122 files in DG for both) | `.../DG/Battle_Goofy` |
| team voices | `battle/FNPC_Link/*do0` | `*go0` (37 files for both) |
| SE | `Sound/SE/NPC/00301_n_ex001` (37 banks) | `00302_n_ex002` (26 banks) |
| footsteps | `SE/footstep/10100_donald` (+ world variants 13300, 13500) | `10200_goofy` (+ 13100, 13400) |
| effects | `Effects/NPC/n_ex001/par` 46 systems (`vn_ex001_{fir,blz,thd,air,cur}_*` spells, `uni_*` unison, `fcm_*` meteor, `fff_*` flare force, `w*` water spells) + `Effects/wep/w_do010` (`vw_do010_hit0`) | `Effects/NPC/n_ex002/par` 30 (`trn_*` tornado, `trb_*` turbo, `dcs_*` discus, `gsh_*` bombardier, `spn_*` / `frt_*` fusion, `VN_bsh*` / `VN_slg*` Trinity Guard, `mcl_*`) + `w_gu010` (`vw_gu010_hit0`) |

Most per-move sounds, voices and effects are fired by anim notifies in the clips (as for Sora); the voice groups add the
state-driven lines [A]. Nine Donald effects are already in Sora's VFX pack (from Sora's team-attack clips). Japanese and
English voice exist [A].

---

## 3. What the port already has, and what is missing

| need | reuse | missing |
|---|---|---|
| assets | `tools/sora_pack.js` (anim-set packing, notifies, root tracks, sockets), `tools/sora_outfit_pack.js` / `_anims.js` / `_kdi.js` / `_anticlip.js`, `sora_material_pack.js` (outfit-owner mode, shader translation), `sora_weapon_pack.js` (`bonamikExport`), the converted glTF / DDS / anim glTFs (Donald 1,590, Goofy 1,570 anim files) | a party pack (own skeletons, own anim sets: no retarget), generated per-character anim tables, KD / Bonamik census, NPC face slots |
| pose | `kh-anim` (Animator per instance, saved-pose blend, face slots, leg IK, `kine.rs` incl. ConnectEquals / TargetPoscns / TargetDircns from ER73, Bonamik with plane / kdi twist), `kh-look` materials, `kh-app` `char_material.rs` | drawing more than one character in `kh-app`; whatever operators the census finds |
| simulation | `kh-core`: the state-machine host (`states/host.rs`, the game's queue), the movement component (`cmc.rs`, private), anim clock / notifies (`anim.rs`), status (`status/`: level from EXP, damage function with `DefenderKind::Friend`, items, abilities), magic projectiles, hit delivery through `Targets` (`HitRecord`, `HitAttacker`), `Targets::allies` / `Ally` | an NPC pawn: NPC / AI states, AI movement (move-to, locomotion definitions, turn), action definitions, Donald's projectiles, Goofy's specials, KO / revive, item use, warp; friend status construction |
| AI | nothing | a BT / blackboard / EQS runtime and the Tres nodes (decompiled C exists for all of them: `decomp_index.tsv`), an AI-tree pack, a nav stand-in |
| team attacks | situation-command queue and HUD entries (`combat/situation.rs`, `kh-ui`), the `decide` branch points (rule 7 named, not implemented), Sora's FR clips in his pack, `sora-states` rows with every FR state's addresses | Sora's FR states, unison branch, friend gauge, link pawns and projectiles, CameraAnim player + the three state-event camera scripts, Trinity Guard's command menu |
| enemies to fight | training dummies (`fixtures.rs`) with `kh-react` reactions, `dummy-attack` / `dummy-auto` words | dummies that can target and hit Donald / Goofy; enemy uid / flying / lock-on component answers for the AI |
| HUD / menu | `kh-ui` party slots, face movies, level-up per chara, party target list, camp screens; `kh-host` hud / menu drivers; `menu_model.rs` UI-actor drawing | the host feed of slots 1..2, party table, character select, party Equipment / Items / Abilities / Stats / Customize data, UI actors n_ex001/2 |
| sound / effects | `kh-audio` (banks, rules, routes, outfit voice), `kh-vfx` + `kh-app/vfx.rs` (the game's shaders), the seam (`Audio`, `Effects`, `Attach::{World, Sora, Target}`) | party banks / effects in the packs, an `Attach` that names a party member's bone, voice-group selection for NPCs, NPC footsteps |

---

## 4. Architecture of the port

```
build/sora_pack/party/n_ex001/   model.glb, manifest.json (skeleton, clips, notifies, sockets), anims/, face/, materials/,
build/sora_pack/party/n_ex002/   kinedriver.json, bonamik.json, weapons/<w_do010..>, camp/ (CM clips)
build/sora_pack/party/ai/        trees/*.json, blackboards/*.json, eqs/*.json, actions/*.json (decoded from the packages)
build/sora_pack/party/team/      camera_anims/*.json, link pawns / projectiles data
build/sora_pack/{audio,vfx}/     party banks and effects added to the existing packs (rows in sheets)

crates/kh-party  (NEW, engine-free; deps kh-core, glam)   Party, NpcPawn, NPC / AI states, status of a friend, actions,
                                                          projectiles, KO / revive, items, warp; team/ (link pawns, FR NPC states)
crates/kh-ai     (NEW, engine-free; deps glam only)       BT + blackboard + EQS runtime, Tres nodes, AI controller, styles;
                                                          talks to a pawn through traits (AiBody, AiWorld)
crates/kh-core   Sora: FR states, unison, friend gauge, the seam additions (friends in Targets, Attach::Party)
crates/kh-camera camera_anim.rs (UE CameraAnim player) + state-event camera scripts
crates/kh-app    spawns and draws the party, steps it after Sora, HUD / menu feed (through kh-host), test fixtures
```

Per fixed step (proposal; KH3's own tick order between the player, the AI controllers and the NPC pawns is [U]/[?] and
stream B confirms it from the tick groups): Sora `step_hosted` -> `party.step(dt, &SoraView, &mut PartyHost)` (for each
member: AI controller tick (BT, services, EQS) -> pawn tick (movement with pose tick, then states, as Sora)) -> camera ->
poses. Engine-free rules hold (no Bevy, no files, no clocks; deterministic RNG streams per member).

Data flow is the Spreadsheet Method's: new port sheets `sheets/port/party-*.json` (params, attacks, states, ai node
table, team attacks, sounds, effects, test loadout) registered in `tools/lib/sheet_defs.js` and generated by
`tools/gen_port.js` (registry `tools/lib/port_gen_defs.js`) or by a new `tools/gen_party.js` for the per-character anim
tables. A `null` cell is unchecked; never guess.

---

## 5. Work streams (six, in parallel)

Common to every stream: read `docs/sora-agent-brief.md`; keep a progress note
`build/sora_tmp/party/IMPL_PARTY_<letter>_STATE.md` current after every milestone; one shared `target/`; no git; no
Ghidra; no machine-wide kills; tests only for what you add plus one short scripted app run of it (pad path); report which
pad inputs drive your feature. Edits to shared files (`tools/lib/sheet_defs.js`, `port_gen_defs.js`, `kh-core/src/lib.rs`,
`states/mod.rs`, `controller.rs`, `kh-app/src/main.rs` / `player.rs` / `script.rs`) are small, targeted, re-read first,
and listed in your note. `kh-app` `main.rs`, `player.rs`, `script.rs` belong to stream F: other streams put their code in
their own files and ask F (or add one clearly marked line).

### Stream A: assets, pack, rendering, animation

- **Owns:** `tools/party_pack.js` (new), `tools/gen_party.js` (new; `build/gen/rust/party_anim_n_ex001.rs` /
  `_n_ex002.rs`: per-character `NpcAnimId`, anim / sequence / notify tables in the same types Sora's generator emits),
  `build/sora_pack/party/n_ex00x/**` and `party/weapons/**`, `sheets/port/party-assets.json`, `crates/kh-app/src/
  party_draw.rs` (new: spawn N skinned characters from a pack folder, upload poses, weapons on sockets, materials via
  `kh-look` / `char_material`), `kh-anim` additions the census requires (KineDriver operators, Bonamik branches, NPC face
  slots), `kh-look` owner-remap additions for the n_ex00x material instances.
- **Work:** (1) census: anim-set rows -> sequences (`animsets.json`), notify classes used by the two sets, KDI operator
  kinds in `n_ex001_0_kdi` / `n_ex002_0_kdi(_shortEar)`, Bonamik `UNPORTED` bits on both assets and the weapons; (2) pack
  model, skeleton, every body / face / CM clip of both sets (same pack format as Sora so `kh_anim::Pack::load` works),
  attack sockets of the weapons per clip (as `manifest.attack_sockets`), root tracks; (3) materials translated with the
  game's shaders (no stand-ins), textures; (4) Bonamik + KineDriver per character (which variant the pawn uses:
  `_cs` / `shortEar` [?: from the pawn / mesh component settings]); (5) 14 + 14 weapon models with their own physics;
  (6) camp UI actor clips for F; (7) `party_draw.rs` with a debug pose word for F to wire.
- **Interfaces out:** pack folders + `manifest.json` schema note in the progress file; generated anim tables (B
  includes them); `party_draw` API `spawn_member(id, pack_dir) / set_pose(id, &Pose) / set_weapon(id, item)`.
- **Acceptance:** pack validation (220 / 225 body rows and 80 / 81 face rows resolve; every notify decoded or listed);
  unit test: bind pose + `IdleN` evaluate for both; Bonamik fallback bits 0 and KD at rest = bind pose (the ER73 test
  pattern); app pictures: both characters front / back / side in `IdleN` and `RunLoopN` with hat / ears / coat moving,
  staff in Donald's right hand at `R_wep_skt`, shield at Goofy's `R_wep`.

### Stream B: party simulation (`crates/kh-party`)

- **Owns:** `crates/kh-party/**` except `src/team/**` (stream D); `sheets/port/party-params.json` (base, level, init,
  victory-bonus sums, locomotion definitions, jump params, revive times ...), `party-attacks.json` (action definitions +
  attack rows), `party-states.json` (like `sora-states`: state id, class, size, vtable, reset / begin / tick / end
  addresses); `tools/party_sheet.js`. Small exposures in `kh-core` (make the movement component and anim slot usable by
  another pawn: `pub` re-exports, no behaviour change; `Status` constructor for a friend: kind 4, level table, save
  record fields) — coordinate with nobody else in those files.
- **Work:** `NpcPawn` (movement, anim slot with notifies, state queue: reuse `kh-core`'s host or a generic copy),
  states of 2.5 (Idle, AI move / Run with locomotion definitions, Turn, Jump, Damage + Goofy override, Reflect, BadStat*,
  Die with KO / revive 30 / 40 / 35 s / HYPER_HEALING, UseItem, Warp, SituationAction, SlopeSlide, RailSlide, SuperSlide,
  Hopping), action definitions as commands (`Action { def, target }`: melee with traction, jump attack, Donald's magic
  with his projectiles and MP, Cure on the chosen ally, Tornado / Turbo / Discus), hits through `Targets::apply_hit` with
  the friend's attacker data, damage to the friend through `compute_damage` (kind Friend, cap 24), status (level from the
  SHARED total EXP with its own table, stats, equipment, abilities, AP, MP charge), events (sound / effect / voice requests
  through the seam with E's `Attach::Party`).
- **Interfaces:** `kh_party::{Party, MemberId, MemberCommand, MemberView, MemberOutput, PartyHost}`. `MemberCommand` is
  what C's tasks issue (MoveTo{goal, locomotion, walk, acceptable radius, path-following flag}, TurnTo, Action{def,
  target}, UseItem{item, target}, Cure{target}, Warp{goal}, SetBattleFollowMode, Stop, PlayMotion{name}); `MemberView`
  is what C's decorators read (state id, HP / MP / max, position / yaw / velocity, in battle, swimming, ride-on, can-battle,
  ability cooldowns, items held, ...). Publish the trait skeleton FIRST (phase 0) so C and D compile against it.
- **Acceptance:** unit tests per state with the decompiled numbers; scripted runs (debug words from F): Donald casts Fire /
  Blizzard / Thunder / Aero at a dummy (projectile, hit, damage = formula), Cure heals Sora (amount per magic spec's cure
  rule for a friend caster [?]), Goofy Tornado / Turbo / Discus; a friend at 0 HP is KO'd, revives after 40 s / 35 s
  (20 s / 17.5 s with HYPER_HEALING counted) with the face going death -> normal; EXP award levels the party with their own tables.

### Stream C: party AI (`crates/kh-ai`)

- **Owns:** `crates/kh-ai/**`, `tools/party_ai_pack.js` (decode trees, blackboards, EQS queries, action-definition CDOs,
  coordinator curves into `build/sora_pack/party/ai/`), `sheets/port/party-ai-nodes.json` (one row per node class used:
  class, decompiled functions, verdict, implemented yes / no), `build/sora_tmp/party/bt_outline/` refresh.
- **Work:** (1) BT runtime with stock UE 4.17 semantics [U] (execution order, decorator flow-abort Self / LowerPriority /
  Both, services intervals with random deviation, `RunBehavior` + dynamic injection by gameplay tag, cooldown and tag
  cooldown, loop, time limit, force success, simple parallel, blackboard observers); (2) EQS runtime (generators: OnRing,
  OnRing3D, Context, Composite, CurrentLocation, SimpleGrid, PathingGrid, SideMove; tests: Distance, Dot, Trace,
  Pathfinding(Batch), MinDistance, WallDistance, Volume, ActorOverlap, the TresNpc tests; scoring equations, clamping,
  filter / score order); (3) every Tres node reachable from Donald's and Goofy's trees transcribed from the decompiled C
  (`decomp_index.tsv`; 170 classes are used by them and the common trees, `ai_node_classes.json`); (4) the controller
  (`TresNpcControllerBase`, `TresNpcAIBattleController` styles, the parts of `TresNpcMetaAI` the trees reach), target
  selection, coordinator limits per difficulty, home positions, warp decision, cure / item decision with
  `FriendEvaluationData` and the AI styles; (5) a nav stand-in behind a trait (open note N1).
- **Interfaces:** `trait AiBody` (B implements: views, commands, function-by-name for `ArithmeticComparison` /
  `BoolPropertyCheck`: `CanBattle`, `IsRideOn`, `IsSwimming`, `IsHostileTarget`, `GetMagicPoint` ...), `trait AiWorld`
  (targets with uid / flying / lock-on components / hate, allies, player view incl. "attacked by enemies", camera, traces,
  nav). Settings in: the three styles + target bits per friend (from F's Customize).
- **Acceptance:** a reachability report: every node instance in the trees under `n_ex001_BT` / `n_ex002_BT` maps to an
  implementation (zero unknown); unit tests of BT semantics and of one EQS query scored by hand (`EQSQuery_NpcWarp`);
  scripted runs: out of battle both stand at their home positions (180 cm, +-70 deg) and follow / warp when Sora runs
  away; in battle they engage the dummies; Donald cures Sora below the style's threshold; Goofy drinks a Potion when low;
  switching Combat Style to Hands Off keeps them back (pictures + AI trace of the active tree path).

### Stream D: team attacks and Sora's side

- **Owns:** `kh-core/src/states/fr_call_meteor.rs`, `fr_flare_force.rs`, `fr_goofy_shoot.rs`, `fr_shield_guardian.rs`
  (+ their rows in `states/mod.rs`, dispatch in `controller.rs`), `kh-core/src/combat/friend.rs` (friend gauge, the
  unison roll and branch: rule 7 of `decide`, attack kinds 0x3F..0x43, the producer rows of 11.6), the Trinity Guard
  command menu entries; `crates/kh-party/src/team/**` (link pawns CallMeteor / FlareForce / GoofyShoot / ShieldGuardian,
  their projectiles and curves, NPC states FRCallMeteor / FRFlareForce / FRShieldGuardian / n_ex002_FRGoofyShoot /
  n_ex001_FRDonaldSora / n_ex002_FRGoofySora); `crates/kh-camera/src/camera_anim.rs` (UE CameraAnim: `InterpTrackMove`,
  `InterpTrackFloatProp` FOV, the Tres interest group) and the three state-event camera scripts as data;
  `tools/party_team_pack.js`; `sheets/port/party-team.json`. Seam: defaulted methods on `kh_core::host::Targets` for
  friends (positions / ids / "is near", request a friend's FR state) so other hosts keep compiling.
- **Work:** read the Sora FR states from `sora-states` addresses, the NPC FR states, the link pawns (`decomp_index.tsv`),
  the camera blueprints' ubergraphs (`callmeteo_camera`, `callmeteo_finish`, `dona_Flair`, goofy_shoot's actions,
  ShieldGuardian's notify events); implement the gauge -> situation command -> execution -> both pawns' states -> link
  pawn -> projectiles / hits -> camera -> end; unison / fusion finishers.
- **Acceptance:** unit tests: gauge to 200 enqueues the friend's command with 30 s x (1 + 0.5 x COMBI_TIME); the unison
  roll's gate and probability; FR state sequences. Scripted runs through the pad path: fill the gauge (debug word),
  **Y** runs Flare Force, Donald Meteor, Goofy Bombardier, Trinity Guard (then A = Shield Bash, B = Jump, cancel), a
  combo near Donald ends with a Unison finisher; pictures across each camera sequence (start / middle / end), damage on the
  dummies, both friends back to their AI afterwards.

### Stream E: effects, sound, voice

- **Owns:** `tools/party_audio_pack.js` (or a party list in `sora_audio_pack.js` if its owner layout allows; say which),
  party rows in the sound / effect sheets (`sheets/port/party-sounds.json`, `party-effects.json`, `party-voice.json`),
  the effect pack additions (`tools/sora_vfx_pack.js` list or a party variant), `kh-core/src/host/{audio,effects}.rs`
  `Attach::Party { member, bone, offset }` (the one seam change; everyone else uses it), the attach resolution in
  `kh-app/src/audio.rs` / `vfx.rs` / `seam.rs`, NPC voice-group selection (`TresVoiceGroupSet`: group -> entry pick,
  `m_bPlayOnGroupNotFound`), NPC footsteps by surface, language switch.
- **Work:** every notify-driven sound / voice / effect of the two anim sets (and the weapons) packed and playable; the
  state-driven voice groups (QuarterHP, RecoveryHP, BattleStart, MagicCast, GiveRecovery, FriendLink, damage); spell /
  ability effects (46 + 30 systems + weapon hits) through the game's shaders; Flare Force / Meteor / Bombardier / Trinity
  Guard effects for D.
- **Acceptance:** a coverage report like `impl_audio/SOUND_COVERAGE.md` for the party (rows that can play / never run);
  audio log of a scripted fight: Donald's MagicCast voice, Fire / Blizzard sounds, footsteps on two surfaces, Goofy's
  damage voice, the low-HP (QuarterHP) line; pictures of each spell and Goofy's three specials.

### Stream F: HUD, camp menu, test-app integration

- **Owns:** `kh-host/src/hud.rs` (party slots 1..2: HP / MP / charge / battle / face; level-up per chara; the party
  target list with Assist Friends), `kh-host/src/menu.rs` / `kh-ui` camp files (`camp.rs`, `camp_menu.rs`,
  `camp_equip.rs`, `camp_custom.rs`: party table, panels, character select with LT / RT, Equipment / Items / Abilities /
  Stats for Donald and Goofy, Customize friend settings stored in the friend's record bytes +0x15A..+0x15C),
  `kh-app/src/menu_model.rs` (UI actors n_ex001 / n_ex002 with their CM clips), `kh-app/src/main.rs`, `player.rs`,
  `script.rs`, `fixtures.rs`, `loadout.rs` (party loadout), `sheets/port/party-test.json` (level-99 party loadout and test
  stock: all 14 + 14 weapons, armour / accessories), feature status rows.
- **Work:** spawn the party at level 99 by default (`--party` on by default, `--no-party`; `--party-level` follows the
  shared EXP per the game rule, see 2.3), the step order (Sora -> party -> camera), a combined `Targets` so dummies can
  target and hit friends and Sora's / the friends' hits never hit each other, dummy answers the AI needs (uid, flying,
  lock-on components), debug / script words (`party on|off`, `party-hp N donald|goofy`, `party-ko`, `party-style ...`,
  `team-gauge full`, `party-trace`), F1 pad-help rows, View resets the party too.
- **Acceptance:** pictures: HUD in party layout with both gauges and faces (normal, battle, dying, death); camp top with
  three panels; LT / RT cycles Sora -> Donald -> Goofy in Equipment / Abilities / Stats (pad path), equipping another
  staff changes Donald's Strength / Magic and the model; Customize settings change the AI (C's trace shows the style
  subtree); level-up popup for each friend after `F4`.

---

## 6. Dependencies and order

```
phase 0 (contracts, first hours):  B publishes kh-party skeleton + MemberCommand / MemberView / PartyHost;
                                   C publishes kh-ai traits (AiBody, AiWorld); E lands Attach::Party in kh-core;
                                   A publishes the pack schema + a first pack with IdleN; F adds --party wiring stubs.
phase 1 (parallel):                A pack + draw | B states / actions / status | C BT + EQS + nodes | D Sora FR states,
                                   gauge, camera_anim | E packs + attach + voice groups | F HUD feed + camp party
phase 2 (joins):                   C drives B (AI moves the pawns); D's NPC FR states run inside B's host; E's effects on
                                   A's bones; F wires everything in the app and runs the scripted tours
```

| consumer | needs from | what |
|---|---|---|
| B | A | generated anim tables, notifies, root tracks, sockets |
| C | B | `AiBody` implementation; `MemberCommand` execution |
| D | B, A, E, C | NPC state host, clips, effects / sounds, AI hand-over (`SetFriendLinkCommand`, the friends stop their tree while an FR state runs) |
| E | A, B | bones to attach to; notify events from B |
| F | A, B, C, D, E | everything for the app; status snapshots from B; AI settings to C |

---

## 7. Open notes (user decisions are made; each note has the recommended default the streams use unless told otherwise)

- **N1 Navigation:** the test level has no navmesh / Mercuna. Default: a nav stand-in (straight path with capsule sweeps;
  EQS `Pathfinding` tests = "a sweep from querier to item is clear"; `PathingGrid` = grid points with that test), recorded
  as a port departure (decision row), game behaviour unchanged where the path is straight.
- **N2 Test loadout at level 99:** default = default weapons (Mage's Staff, Knight's Shield), all victory bonuses (HP
  200 / 220, slots +1), every ability they own equipped as far as AP allows, items as `TresChrInitData`, all 14 + 14
  weapons in stock for the Equipment screen.
- **N3 Level switching (LB + View, dec021):** the party's level follows the shared EXP by their own tables (game rule),
  not forced to the same number as Sora.
- **N4 Customize setting mapping** (2.8) is by menu order until the writer 0x1415EA860 is read; defaults: Stick By Sora,
  Watch MP, Use As Needed + On Sora / Self / Others all on [?: the game's defaults are in the init path, to read].
- **N5 Boss / world branches** of the trees (enemy uids, Marshmallow ride, swim, lead, NPC modes) are packed and run as
  data; in the test level their conditions are false. Under-water moves wait for water (as Sora's swim).
- **N6 Randomness:** `rand()` uses (unison roll, `TresBTComposite_Random`, service deviations) go through per-member
  deterministic streams, so replays stay bit-identical.
- **N7 Elden Ring:** not now; `kh-party` / `kh-ai` stay engine-free so a later DLL host can run them.
- **N8 Team-attack cameras** play the game's CameraAnims 1:1 (user decision 1); the port's own shot camera (dec020) is not
  used for them.

---

## 8. Evidence index

- Packages: `assets_converted/packages/KINGDOM HEARTS III/Content/{Character/npc/n_ex00x, Blueprints/Npc/{n_ex001,n_ex002,
  Common}, Blueprints/FriendLink, Blueprints/Camera/p_ex001_StateEvent/{Friend,BP_TresPlayerStateEventCamera_ShieldGuardian},
  Camera/sora/Skill/{donald,goofy_shoot,shield}, Effects/NPC/n_ex00x, Effects/wep/w_{do,gu}010, UI/Blueprints/CampTop,
  UI/Data/Character, AI/}`.
- Tables (sheets): `sheets/entities/DataTable/Npc/{Base,LevelUp,Attack}/n_ex00x_*`, `sheets/entities/Load/Common/
  TresChrInitData.json`, `sheets/entities/Load/Tres/{TresVBonusData,TresEquipItemData,TresAbilityData,TresItemData}.json`,
  `sheets/entities/Blueprints/Npc/Common/DataTable/*`, `sheets/systems/UI/Data/MainCommandData.json`,
  `sheets/port/sora-states.json` (Sora FR state rows), enums `ETresChrUniqueID`, `ETresCommandKind`, `ETresAbilityKind`,
  `ETresFNpcAI*`.
- Audio: `assets_converted/audio/KINGDOM HEARTS III/Content/Sound/{VOICE/Localization/<lang>/battle/{NPC/DG,FNPC_Link},
  SE/NPC/0030{1,2}_*, SE/footstep/*donald*|*goofy*}`.
- Text: `assets_converted/loose/.../Localization/Game/en/kh3_system.locres.json` (`situation.fr_*`, `ability.name_act_0190..
  0320`, `command.fr_sheildguardian*`).
- Decompiled C: `re/decomp/<addr>.c`; class-tagged list in `build/sora_tmp/party/decomp_index.tsv`; the specs that
  already read neighbouring code: `docs/sora-combat-spec.md` (4.2 rule 7, 11.2 .. 11.6), `docs/sora-stats-spec.md`
  (5.2, 7, 8), `docs/sora-ui-spec.md` (4, 5.4, 5.6, 10), `docs/sora-enemy-reaction-spec.md` (the AI damage state).
