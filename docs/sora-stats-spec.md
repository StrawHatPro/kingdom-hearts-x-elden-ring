# Sora: character progression and status systems — specification

Scope: the player status object, stats and how the final numbers are computed, EXP and levelling,
every ability (catalogue with hook points), HP / MP / focus runtime rules that are not part of the
magic system, difficulty modes, equipment and the save-data shape, down to a concrete level-99
configuration. Research only; no Rust was written.

Sheets (written by `tools/sora_stats_sheet.js`, declared in `tools/lib/sheet_defs.js`):

| sheet | rows | content |
|---|---|---|
| `sheets/port/sora-levels.json` | 99 | per level: EXP, Strength / Magic / Defense / AP (absolute and delta), ability per path |
| `sheets/port/sora-abilities.json` | 183 | per `ETresAbilityKind`: data row, sources, copies, effect, hook functions and call sites, status, test idea |
| `sheets/port/sora-stats-rules.json` | 50 | objects, formulas and constants of this spec |
| `sheets/port/sora-difficulty.json` | 4 | Beginner / Standard / Proud / Critical |
| `sheets/port/sora-equipment.json` | 195 | Sora's keyblades, Kingdom Key upgrade levels, armour, accessories, slot counts |
| `sheets/port/sora-loadout-99.json` | 33 | the level-99 test configuration |
| `sheets/port/sora-newgame.json` | 37 | the start of a new game (section 14; authored, not written by the tool) |

Evidence tags: **[I]** read at instruction level (address given), **[C]** decompiled C only (the
`verdict` of the function is in section 13), **[A]** read from an asset or data table, **[D]** derived
by arithmetic from tagged values, **[?]** not determined. Addresses are virtual addresses of
`base/KH3.exe`. `ceil(x)` is the engine form `-(cvtss2si(-0.5 - 2x) >> 1)`, `floor(x)` is
`cvtss2si(2x - 0.5) >> 1` (round-to-nearest-even conversion); a port must use these forms or exact
integer equivalents. [I] e.g. 0x1406326EF..0x140632705

Related specs: MP cost, MP charge, magic damage, Cure, focus gauge and items are specified in
`docs/sora-magic-spec.md` (parts C, D, E). This document does not repeat them; section 10 lists the
cross-checks and one disagreement.

---

## 0. The model in one page

- Nothing about Sora's level or stats is stored except **one number, the total EXP** (global work
  `GW+0x7E00`). The level is the highest row of `p_ex001_LevelUpData` whose `m_Exp` is not above it,
  and that row gives Strength, Magic, Defense and AP as **absolute values**. [C] 0x140582FD0, 0x140576E70
- Max HP and max MP have **no level term**. They are the base row (90 / 100) plus bonuses that are
  recounted from the **victory-bonus flags** every time the status is rebuilt. [C] 0x140575C80, 0x140E90EE0
- The opening choices: the first one (Vitality / Balance / Wisdom) is implemented by the level
  blueprint as one of three victory bonuses (HP +30, HP +15 and MP +10, MP +20); the value it also
  stores (`GW+0x7E0C`) is never read. The second one stores the **ability path** 0 / 1 / 2
  (`GW+0x7E0D`), which selects `m_AbilityKind1 / 2 / 3` of each level row. All three paths hold the
  same 28 abilities by level 52; only the order differs. [A]+[I]
- Abilities are one 32-bit word per id in the character's save record: three have / equipped bit
  pairs (three stackable copies), two 3-bit counters (copies from victory bonuses, copies from
  levels) and a 7-bit counter for copies supplied by worn equipment. The gameplay query is
  `count(id)` = equipped copies + equipment copies + 1 if the active meal bonus is that ability.
  [I] 0x140E8AAB0, 0x140580810
- Critical mode: base HP / MP halved (floor), every HP / MP bonus roughly halved by its own table,
  AP +30, three extra start abilities, EXP x0.75, enemy damage x2, enemy attack +5, MP charge +20 s.
  Beginner: EXP x1.25, player damage x1.25, enemy damage x0.5. Proud: enemy damage x1.5.
- Level 99: 1,527,640 EXP; Strength 62, Magic 64, Defense 59, AP 180 before equipment; with the
  Kingdom Key Strength 66, Magic 67. Max HP 250 / 265 / 280 and max MP 179 / 169 / 159 (Wisdom /
  Balance / Vitality, every story and Re Mind bonus; Standard). Section 9 has the full table.

---

## 1. Objects and storage

### 1.1 Game instance and global work

- `FUN_141522B30()` returns the `UTresGameInstance` (GI) through the game singleton. [C] 0x141522B30
- `FUN_140EA8620(GI)` = `GI + 0x870`: the block the game calls `TresGameGlobalWork` in its log
  strings (GW below). [I] 0x140EA8620. Pawn `vt+0x8C0` returns GI; the player pawn's `vt+0x8C8`
  returns GW. [C] 0x1414358B0, 0x141431AD0
- Fields of GW used here (offset from GW; GI offset in brackets):

| offset | type | content | evidence |
|---|---|---|---|
| +0x7DF0 (0x8660) | i32 | game level, `ETresGameLevelID`: 0 BEGINNER, 1 STANDARD, 2 PROUD, 3 CRITICAL | [I] getter 0x140EA85E0, `cmp [..],3` in 0x140576294, 0x140EAB4D0 |
| +0x7E00 (0x8670) | i32 | Sora's total EXP, 0..9,999,999 | [I] 0x140575FA6 (read), [C] 0x140E864B0 (add), 0x140E960E0 (set) |
| +0x7E09 (0x8679) | u8 | battle level (index into `EnemyBaseStatus`, clamped 1..99) | [C] 0x140EA85D0, 0x140EA83C0 |
| +0x7E0C | u8 | `ETresPlayerGrowthType` (0 POWER, 1 MAGIC, 2 DEFENSE): **written only** | [I] 0x140E97490; field scan of 0x140400000..0x142D00000 finds no reader |
| +0x7E0D | u8 | ability path (`ETresPlayerAbilityGrowthType` 0 TYPE_A, 1 TYPE_B, 2 TYPE_C) | [I] write 0x140E97496, reads 0x14058339C, 0x14059EFF0 |
| +0x86B4 | struct | inventory: `FUN_140E99330(inv, itemId)` returns the count byte | [C] 0x140E86C10 |
| +0x95B0 | u8 | action-enable flags set by the `BP_SetAbility*` script functions (section 5.2) | [C] 0x1414358B0 |
| +0x9640 + i*0x9C0 | record[16] | per-character save records (section 8); a second array of 16 at +0x9572AC | [C] 0x140E86C10, 0x140E87AA0 |
| +0x13280 | set | game flags (victory-bonus flags are tested with `FUN_140E8AB80`) | [C] 0x140E90EE0 |
| GI+0x870.. | bits | code-menu option sets tested by `FUN_140EAB0E0(GI, n)` (set A) and `FUN_140EAB160(GI, n)` (set B) | [C]; see 2.7 |

  The 0x20 bytes from +0x7DF0 are copied as one block by the save code (0x140E9CDE0, 0x1414E4640). [I]

### 1.2 Pawn

`ATresCharPawnBase` fields (reflected names) [A]: `+0x4D0 m_pBaseParam`, `+0x5F8 MaxHitPoint`,
`+0x5FC m_HitPoint`, `+0x600 MaxMagicPoint`, `+0x604 m_MagicPoint`, `+0x60C MyMpChargeTime`,
`+0x610 MaxFocusPoint`, `+0x614 m_FocusPoint`, `+0x624 m_AttractionHP`, `+0x630` flag bits
(`m_bIsInvincible`, `m_bNoHpDamage`, `m_bIsHpLimitOne`, `m_bIsHpLimitOneExceptPlayerAttack`, ...).
Unreflected: `+0x7C0` bit 1 MP charging, `+0x7C4` f32 MP-charge timer, `+0x838..+0x844` walk-regeneration
timers, `+0x848` f32 Withstand Combo timer. [C] 0x1405AF5B0, 0x1405A2A00

Status virtuals of the pawn (slot = offset / 8). The magic spec lists the MP ones; these are the rest:

| slot | offset | function | meaning | evidence |
|---|---|---|---|---|
| 668 | +0x14E0 | player 0x1414358B0 → base 0x1405ACE20 | `HasAbility(id)` (count > 0, after the script gates) | [C] |
| 672 | +0x1500 | player 0x141424130 → 0x1405A03C0 | `AbilityCount(id)` | [C] |
| 673 | +0x1508 | player 0x1414242F0 → 0x1405A03C0 | `AbilityLevel(id)` = the same count with fewer gates | [C] |
| 674 / 675 | +0x1510 / +0x1518 | — | `AbilityAdd(id, bEquip)` / `AbilityRemove(id)` (script natives 0x141D81630 / 0x141D81820) | [C], bodies not read |
| 676 | +0x1520 | 0x1405A06B0 | apply one victory bonus (kind, ability, equip flag) to the live pawn | [C] |
| 758 | +0x17B0 | 0x1405AADB0 | `GetHitPoint` | [C] |
| 760 / 761 | +0x17C0 / +0x17C8 | 0x1405AAE20 / 0x1405AAE90 | HP percent 0..100 / HP rate 0..1 | [C] |
| 769 | +0x1808 | 0x1405AAD70 | `GetFocusPoint` | [C] |
| 771 | +0x1818 | — | `GetChrLevel` (script native 0x141D836D0) | [C] |
| 772 / 773 / 774 | +0x1820 / +0x1828 / +0x1830 | 0x1405AB200 / 0x1405AB230 / 0x1405AB1E0 | max HP / max MP / max focus (tail into 0x140583990 / 0x140583B40 / 0x1405838E0) | [C] |
| 775 | +0x1838 | 0x1405AB190 | max attraction HP (`bp+0x7A`) | [C] |
| 776 | +0x1840 | 0x1405B9700 | `SetHitPoint` | [C] |
| 781 | +0x1868 | 0x1405B94F0 | `SetFocusPoint` | [C], body not read |
| 782 | +0x1870 | 0x1405A3970 | percent damage (ratio of max or current HP) | [C] |
| 783 | +0x1878 | 0x1405B57D0 | `Recover(hp, mp, focus, kind, flag)` | [C] |
| 786 | +0x1890 | 0x1405B5700 | full recover: HP, MP, focus, attraction HP to max, MP charge cleared | [C] |
| 787 | +0x1898 | 0x1405AB950 | apply prize | [C] |
| 803 / 805 / 807 | +0x1918 / +0x1928 / +0x1938 | 0x1405AB800 / 0x1405AB8A0 / 0x1405AB8C0 | Strength / Defense / Magic (tail into 0x140580E70 / 0x140581830 / 0x1405836E0) | [C] |
| 801 / 802 | +0x1908 / +0x1910 | — | level / next-level EXP (used by the EXP award) | [C] 0x140EBE400 |
| 812 | +0x1960 | 0x1405A3E10 | rebuild the base param from the tables and the save record | [C] |
| 928 | +0x1D00 | 0x141433570 | max combo length | [C] |
| 1191 | +0x2538 | 0x1405AD600 | HP pinch test | [I] |

`FUN_1405AA250` fills the 27-int status snapshot used by the camp menu and the HUD in this order:
level, EXP, next-level EXP, this-level EXP, HP, max HP, MP, max MP, focus, max focus, Strength, Magic,
Defense, max AP (0x1405808B0), base Strength (0x140581190), base Magic (0x140581310), base Defense
(0x140581240), base AP (0x140581130), resist 0..7 (0x1405850B0), flags. "Base" = level value +
boost items, before equipment and meals (the menu shows both). [C] 0x1405AA250 (verdict same)

### 1.3 `UTresChrBaseParam` (pawn+0x4D0, 0x178 bytes, constructor 0x140571980)

No reflected properties. Layout from the constructor, the player initialiser 0x140575C80 and the
getters [C]:

| offset | type | content |
|---|---|---|
| +0x40 | i32 | level (clamped 0..99) |
| +0x44 / +0x48 | i32 | `m_Exp` of the next level (0 at level 99) / of the current level |
| +0x4C | i32 | EXP reward (enemies: `ceil(m_ExpRate * EnemyBaseStatus.m_BaseExp)`) |
| +0x50 / +0x54 / +0x58 | i32 | max HP / max MP / max focus base |
| +0x5C / +0x60 / +0x64 | i32 | Strength / Magic / Defense of the level |
| +0x68 | i32 | AP of the level |
| +0x6C / +0x6E / +0x70 | i16 | max HP bonus / max MP bonus / max focus bonus |
| +0x7A | i16 | max attraction HP = `ceil(row[+0x30] * max HP)` (enemies) |
| +0x7C | u8 | character kind (0 player with level table, 4 friend, 1 / 5 enemy kinds) |
| +0x7D | u8 | biological type of the base row (1 Heartless, 2 Nobody for the buster abilities) |
| +0x88 | u8 | buff level 0..3 (x5 Strength), refreshed every tick by 0x140594EF0 [?] source |
| +0xA4 / +0xA8 | i32 | `m_DamageMin` / `m_DamageMax` of the base row |
| +0xBC + 4*attr | i32 | base attribute resist |
| +0xE0 | u64[] | status-resist bit set (`m_bResistEffect*`) |
| +0x130 | ptr | the character's save record (ability words, bonus counters, boosts) |
| +0x138 | ptr | active meal effect: `+0x1C` HP, `+0x20` MP, `+0x24` Strength, `+0x28` Magic, `+0x2C` Defense, `+0x30` bonus ability id |
| +0x140 | ptr | equipment row of the current weapon |
| +0x148 / +0x150 | ptr[] / i32 | equipment rows: 0..2 keyblade slots, 3..10 armour, 11..18 accessories (19 entries) |
| +0x160, +0x164 / +0x168 / +0x16C, +0x170 | i32 | level-1 copies of HP, Strength / Magic / Defense, AP (used by the "default status" option, 2.7) |

A second, flat status struct with the same content at other offsets (max HP at +0x0C, save record
at +0x68, weapon row at +0x78) is built by 0x1405751B0 for characters without a live pawn (menus);
its getters are 0x140580CC0, 0x140583530, 0x140581710, 0x1405838F0, 0x140583AA0. A port needs only
one of the two. [C]

---

## 2. Stats

### 2.1 Base row [A]

`sheets/entities/DataTable/Player/Base/p_ex001_BaseParamData.json` (row `Default`):
`m_MaxHitPoint` 90, `m_MaxHPRate` 1, `m_MaxMagicPoint` 100, `m_MaxFocusPoint` 100, `m_AttackPower` 2,
`m_MagicPower` 6, `m_DefensePower` 2, `m_AbilityPoint` 2, `m_ExpRate` 1, `m_DamageMin` 0,
`m_DamageMax` 255, all attribute resists 0, no weak points, no status resists, biological type Human.
The four stat fields of the row (2 / 6 / 2 / 2) are overwritten by the level row for a character
with a level table and never reach gameplay. [C] 0x140576E70

### 2.2 Level table [A]

`sheets/entities/DataTable/Player/LevelUp/p_ex001_LevelUpData.json`, 99 rows named "1".."99", struct
`TresChrLevelUpDataTable` (32 bytes): `m_Exp` +8, `m_AttackPower` +0xC, `m_MagicPower` +0x10,
`m_DefensePower` +0x14, `m_AbilityPoint` +0x18, `m_AbilityKind1..3` +0x1C..+0x1E. All rows are in
`sora-levels`. Samples:

| level | total EXP | Strength | Magic | Defense | AP | path A | path B | path C |
|---|---|---|---|---|---|---|---|---|
| 1 | 0 | 11 | 12 | 8 | 28 | - | - | - |
| 2 | 80 | 11 | 13 | 9 | 28 | COMBO_PLUS | MAGIC_COMBO_SAVE | CRITICAL_HALF |
| 10 | 5,410 | 15 | 17 | 12 | 40 | FUSION_SPIN | FUSION_SPIN | FUSION_SPIN |
| 30 | 87,850 | 25 | 28 | 23 | 76 | BLIZZARD_UP | COMBO_PLUS | LAST_LEAVE |
| 50 | 376,030 | 36 | 38 | 34 | 118 | CRITICAL_HALF | AIRCOMBO_UP | MAGIC_COMBO_UP |
| 99 | 1,527,640 | 62 | 64 | 59 | 180 | - | - | - |

Use [C]:
- `LevelFromExp` 0x140582FD0: for L = 1..99, look up row "L"; stop at the first row whose `m_Exp`
  is greater than the EXP; the level is the last row passed. `m_Exp` is therefore the cumulative total.
- `ApplyLevel` 0x140576E70 → `ReadLevelRow` 0x1405832D0: level = clamp(L, 0, 99); `bp+0x5C` =
  `m_AttackPower`, `bp+0x60` = `m_MagicPower`, `bp+0x64` = `m_DefensePower`, `bp+0x68` =
  `m_AbilityPoint` (plain copies); `bp+0x48` = this row's `m_Exp`; `bp+0x44` = next row's `m_Exp`
  (left 0 at level 99). The ability column is chosen by `GW+0x7E0D` (0 → Kind1, 1 → Kind2, 2 → Kind3).
- The abilities of all rows up to the level are regranted on every rebuild (0x1405751B0 /
  0x14059EF70 collect the ids and call `SetLevelAbilities` 0x140E90E00, which clears the level
  counters of all 0x200 words and counts again). No "level reached" state is stored.

Paths: each path grants 28 abilities, the same multiset on all three (COMBO_PLUS x2, AIRCOMBO_PLUS x2
and 24 single ones; the list is in `sora-levels`), only at different levels. [A] The flavour of the
first grants identifies them: path A (Kind1) starts with Combo Plus, path B (Kind2) with Magic Combo
Thrift, path C (Kind3) with Damage Control, i.e. the attack-, magic- and defence-leaning orders
("Warrior", "Mystic", "Guardian" in the English game). [A] since 2026-10-12: the mirrors of the second
question store 0 for the warrior, 1 for the mystic, 2 for the guardian (section 14.1).

### 2.3 Opening choices [A]+[I]

Level blueprint `Levels/ew/ew_01/umap/ew_01_gameflow` (ubergraph offsets 63701..69157) [A]:

| first choice | script call | victory bonus row given | effect |
|---|---|---|---|
| 0 | `BP_TresSetPlayerGrowthType(0, second)` | `Vbonus_079` flag `gameflow_VBONUS_079_Select_HP` (enum text "パラメータ選択・体力") | `HP_UP30` |
| 1 | `BP_TresSetPlayerGrowthType(2, second)` | `Vbonus_080` `..._Select_NT` ("平均") | `HP_UP15` + `MP_UP10` |
| 2 | `BP_TresSetPlayerGrowthType(1, second)` | `Vbonus_081` `..._Select_MP` | `MP_UP20` |

`BP_TresSetPlayerGrowthType` (native 0x142223CF0 → 0x140E4B180 → 0x140E97490) only stores its two
arguments at `GW+0x7E0C` / `+0x7E0D`. [I] The first value has no reader, so the first choice acts
through the bonus alone: starting HP / MP 120 / 100 (Vitality), 105 / 110 (Balance), 90 / 120
(Wisdom); in Critical 60 / 50, 52 / 55, 45 / 60. [D] The second choice (0 / 1 / 2) is the ability
path of 2.2. Neither choice changes Strength, Magic, Defense or AP. [C]

Default: the data marks none (the opening always asks). The loadout sheet uses Balance + path A and
gives the numbers for every combination; at level 99 the path no longer matters.

### 2.4 HP / MP bonuses [C]+[A]

- Source: `sheets/entities/Load/Tres/TresVBonusData.json` (`TresVictoryBonusDataTable`, 139 rows):
  per row a game flag, and per character one or two (`m_BonusSora1/2`, `m_AbilitySora1/2`) pairs of
  `ETresVictoryBonusKind` and ability. The plan tables `Blueprints/Level/VBonus/MacroLib_TresPlanVBonusTable`
  (121 rows) and `VBonus_DLC/...` (15 rows) name which rows the levels award.
- `RecountBonuses` 0x140E90EE0(record, GI): clears the 32 counters `record+0x960..+0x97F`, walks all
  bonus rows whose flag is set in the game-flag set, decodes the row for this character
  (0x140586280) and increments `record[0x960 + kind]` and the ability word's victory-bonus counter.
- `BonusTotals` 0x140E8CE20(record, gameLevel, &hp, &mp): for kinds 1, 2, 3, 4, 5, 8, 9, 10, 11, count
  times the amount of `BonusAmount` 0x1405864C0:

| kind | 1 HP_UP3 | 2 HP_UP5 | 3 HP_UP10 | 4 HP_UP15 | 5 HP_UP30 | 8 MP_UP3 | 9 MP_UP5 | 10 MP_UP10 | 11 MP_UP20 |
|---|---|---|---|---|---|---|---|---|---|
| game level < 3 | 3 | 5 | 10 | 15 | 30 | 3 | 5 | 10 | 20 |
| Critical | 1 | 2 | 5 | 7 | 15 | 1 | 2 | 5 | 10 |

  The totals go to `bp+0x6C` / `bp+0x6E` (i16). Other kinds: 16 / 17 / 18 armour / accessory / item
  slot +1 (counters `record+0x970..0x972`, applied by 0x140E98BE0), 24..29 magic element tiers.
- Sora's rows [A] (totals; lists in `build/sora_tmp/stats/vbonus_sora_grouped.tsv`):

| group | HP | MP | Critical HP | Critical MP | other |
|---|---|---|---|---|---|
| story rows `Vbonus_001..084` without the three choice rows | +160 (15 x HP_UP10, 2 x HP_UP5) | +35 (7 x MP_UP5) | +79 | +14 | 2 armour slots, 2 accessory slots, 3 item slots, 18 magic tiers (3 per element), 29 abilities |
| choice row (one of three) | +30 / +15 / 0 | 0 / +10 / +20 | +15 / +7 / 0 | 0 / +5 / +10 | - |
| minigame rows `VBonus_Minigame001..013` | 0 | 0 | 0 | 0 | 13 abilities |
| Re Mind `VBonus_DLC_001..015` | 0 | +24 (8 x MP_UP3) | 0 | +8 | 2 item slots |
| Re Mind `VBonus_DLC_debug_Init_01..03` | 0 | 0 | 0 | 0 | 6 abilities (Quick Slash .. Last Charge) |

  [?] which flags a given playthrough has set; the table rows `Vbonus_debug_*` and
  `Vbonus_debug_ItemSlot_*` are debug / event-preset rows and are not counted.
- When a bonus is awarded during play (`TresGetVictoryBonus` → pawn slot 676 0x1405A06B0): the record
  is updated (0x140574450 → 0x140E86BC0), the bonus totals are recomputed, and the current HP / MP
  are raised by the amount of the new bonus (`SetHitPoint(HP + amount)`). [C]
- Critical: a row whose `m_InitAbilityEquipOnCriticalMode` is not NONE gives only its stat bonuses;
  its abilities are skipped (0x140586280 case 1), because Critical grants them at the start (2.6).
  13 Sora rows carry the flag (Aerial Recovery, Counter Slash, Rising Spiral, Counter Impact, Combo
  Master, Groundbreaker, Risk Dodge, Air Slide x2, Superslide x2, Counter Kick, Final Blow). [A]+[C]
  [?] the code that grants them at a Critical start (not the three `m_Critical*Ability` entries of 2.6).

### 2.5 Equipment, boosts and meals [C]+[A]

- Equipment rows (`TresEquipItemDataTable`, 0x80 bytes): `m_AP` +0x20, `m_AttackPlus` +0x24,
  `m_MagicPlus` +0x28, `m_DefensePlus` +0x2C, `m_AttrResist*` +0x30..+0x4C, `m_AppendAbility`
  array +0x50 (count +0x58). The rows of the worn items are resolved at rebuild (0x140575C80 →
  0x14059E9B0; 0x1405751B0 shows the mapping: record+0x80 three keyblade slots, +0x98 eight armour
  slots, +0x D8 eight accessory slots, current weapon = slot index `record+6`).
- Only the **current** keyblade's row contributes stats (`bp+0x140`); armour and accessories are
  summed over `bp+0x148[3..18]`. Entries 0..2 of the array are skipped by every getter. [C]
- Equipment adds Strength, Magic, Defense, AP, attribute resists and abilities. It adds **no max HP
  and no max MP** (no row field, no getter term). [C]+[A]
- Abilities of worn items: `AddEquipAbilities` 0x140E87590 increments the 7-bit equipment counter
  (bits 18..24, cap 99) of each listed ability, after `ClearEquipCounters` 0x140E87F10; the weapon's
  formchange abilities are added the same way by 0x140E87610. They cost no AP.
- Boost items: bytes `record+0x980` Strength, `+0x981` Magic, `+0x982` Defense, `+0x983` AP, added
  by the getters. [C] [?] the item code that increments them and their caps (the Critical start
  value of `+0x983` is capped at 100 in 0x140E92280).
- Meals: the active meal object `bp+0x138` adds max HP, max MP, Strength, Magic, Defense and at most
  one bonus ability (`+0x30`, one of ids 170..186; table `TresFullcourseAbilityList`). Per dish
  values are in `TresFoodItemEffectData` (pointer only; cooking is out of scope). With
  POWER_LUNCH counted, each meal term becomes `ceil(term * 1.25)`. [I] 0x1405818E6 (1.25)
- Kingdom Key [A] (`sora-equipment`): row `I03001` `WEP_KEYBLADE_SO_00`, item id 0x301,
  Strength +4, Magic +3, ability PRIZE_DRAW (Treasure Magnet). Ten upgrade rows (`IW_0..IW_9`,
  `TresItemWeaponEnhanceData`): alternately Magic +1 and Strength +1, and FORM_TIME at level 4;
  fully upgraded Strength +9, Magic +8. Shotlocks King of Hearts and Ragnarok, formchange
  `TRES_WEAR_FORM_05` (`TresKeybladeAbilityData`). [?] the function that builds the upgraded row
  (the lookup 0x14112EBD0 reads an override map at `+0xDC0` before the base table).
- Slots [C]+[A]: three keyblade slots from the start (record init 0x140E92280 opens slots 1 and 2
  for the player kind); armour 1, accessory 1, item 3 at the start (`UTresChrInitData` constructor
  0x140571C26 / C32 / C41; `m_PlayerSora` does not override them) plus the slot bonuses: armour 3,
  accessory 3, item 6 with the story, item 8 with Re Mind. Arrays hold 8.

### 2.6 Start state [A]+[C]

`sheets/entities/Load/Common/TresChrInitData.json`, `m_PlayerSora` (`FTresChrInitEquip`):
- `m_Weapons` [WEP_KEYBLADE00]; no armour, accessories or items.
- `m_EquipAbility` (owned and equipped): LIBRA, DODGE, AIRSLIDE, AIRDODGE, REFLECT_GUARD, SUPERSLIDE,
  FRIEND_AID, SONIC_SLASH, SONIC_DOWN, TURN_CUTTER, SUMMERSALT, POLE_SPIN, POLE_SWING, WALL_KICK,
  CRITICAL_HALF, AUTO_LOCK_MAGIC. `m_HaveAbility` (owned, not equipped): EXPZERO.
- `m_CriticalEquipAbility`: CRITICAL_COUNTER, CRITICAL_CHARGE; `m_CriticalHaveAbility`:
  CRITICAL_CONVERTER; `m_CriticalUseAPUpNum` 30 → `record+0x983` when the game level is above 2. [C] 0x140E92280
- `m_BaseParamData`, `m_LevelData` → the two tables above; `m_FormAbilityAsset` → `p_ex001_FormAbility`.

### 2.7 Stat getters (the numbers combat uses) [C], constants [I]

All are methods of the base param; the pawn virtuals jump into them.

- **Max HP** 0x140583990 = `bp[0x50] + i16 bp[0x6C] + meal`, meal = `[meal+0x1C]` if > 0,
  `ceil(meal * 1.25)` with POWER_LUNCH. `bp[0x50]` = 90, or `floor(90 * 0.5)` = 45 in Critical, minimum 1
  (0x140575D73, 0x140575DB1). No cap. `SetHitPoint` clamps the current value to 0..max.
- **Max MP** 0x140583B40 = `bp[0x54] + i16 bp[0x6E] + meal` (100, Critical 50).
- **Max focus** 0x1405838E0 = `bp[0x58] + i16 bp[0x70]` = 100; no writer of `bp[0x70]` was found for
  Sora, no difficulty term. [C]
- **Strength** 0x140580E70:
  1. `s = bp[0x5C]` (level value); with option A0 ("default status") and kind 0 / 3 / 4: `s = bp[0x164]`
     (the level-1 value);
  2. `s += record[0x980]` (boost items);
  3. with option B0: `s *= 3`;
  4. `s += weapon.m_AttackPlus + Σ equip[3..18].m_AttackPlus`;
  5. OVERCOME (id 179) counted: `s = max(s, EnemyBaseStatus[battleLevel].m_BaseDefensePower + ceil(0.15 * battleLevel + 1.5))`;
  6. `s += meal Strength` (x1.25 ceil with POWER_LUNCH);
  7. CHARGE_BERSERK (id 178) counted and the actor's `vt+0xF8` true (MP charge running): `s += 1`;
  8. `s += 5 * bp[0x88]`.
- **Magic** 0x1405836E0: steps 1..6 with `bp[0x60]`, `bp[0x168]`, `record[0x981]`, row field +0x28,
  meal +0x28. No steps 7, 8.
- **Defense** 0x140581830: `d = bp[0x64]` (option A0: `bp[0x16C]`; option A1: 0); `d += record[0x982]`;
  `d += meal Defense`; `d += weapon.m_DefensePlus + Σ equip.m_DefensePlus`; DEFENDER (id 50) count n:
  n = 1: +2 when HP% <= 25; n = 2: <= 50; n = 3: <= 75; n = 4: always (HP% from the actor's `vt+0xE8`).
  [I] thresholds 0x140581AA8 (25), 0x140581A4D (50), 0x1405819F2 (75)
- **Max AP** 0x1405808B0 = `bp[0x68] + record[0x983] + weapon.m_AP + Σ equip.m_AP`.
- **Attribute resist** 0x1405850B0(attr 0..7) = `bp[0xBC + 4*attr] + 100 - ceil(100 * Π(1 - r_i / 100))`
  over the weapon row and equipment rows 3..18 (multiplicative stacking; computed as
  `int(product * 1000) * 0.1`). Damage uses `clamp(100 - resist, 0, 200) / 100`.
- Options A0 / A1 / B0 are bits of the Re Mind code menu (table `TresCodeMenuCheatData`); a port
  without that menu treats them as off. [?] the option names (the magic spec lists the same uses).
- Caps: none in the getters. Stored limits: EXP 9,999,999; level 99; ability copies 3 per id (plus
  up to 99 equipment copies); bonus counters u8; boost bytes u8.

---

## 3. Difficulty (`sora-difficulty`)

Game level at `GW+0x7DF0`. Every use found by a field scan of the game module was classified; the
ones that concern Sora:

| rule | Beginner 0 | Standard 1 | Proud 2 | Critical 3 | evidence |
|---|---|---|---|---|---|
| base max HP / MP | 90 / 100 | 90 / 100 | 90 / 100 | 45 / 50 | [C] 0x140575C80, [I] magic spec 0x140575D73 |
| HP_UP / MP_UP amounts | 3,5,10,15,30 / 3,5,10,20 | same | same | 1,2,5,7,15 / 1,2,5,10 | [C] 0x1405864C0 |
| EXP awarded | `e + ceil(0.25 e)` | `e` | `e` | `ceil(0.75 e)` | [I] 0x140632D79, 0x140632DA1 |
| damage dealt by the player side | x1.25 | x1 | x1 | x1 | [I] 0x140642820 (0x1406429E1) |
| damage dealt by the other side | x0.5 | x1 | x1.5 | x2 | [I] 0x1406429EB, 0x1406429D3, 0x1406429C5 |
| enemy attack stat at spawn | +0 | +0 | +0 | +5 (not vehicles) | [C] 0x140576130, 0x140575ECF |
| AP | +0 | +0 | +0 | +30 (`m_CriticalUseAPUpNum`) | [C] 0x140E92280, [A] |
| start abilities | - | - | - | Critical Counter, Critical Recharge (equipped), Critical Converter (owned) | [A] |
| MP charge time | 50 s | 50 s | 50 s | 70 s | magic spec part C 3 |
| attraction marker chance | base | base | base | x0.5 unless 0x140EAB9F0 | [C] 0x141432610 |

"Player side" in 0x140642820: attacker actor kind (`actor+0x3B0`) 1 or 10, or an actor of kind 3 / 5 /
11 whose owner is of kind 1 or 10; everything else is the other side. [C]
Critical does not change Strength, Magic, Defense or the level table. [C]

---

## 4. EXP and levelling

### 4.1 Reward of an enemy [C]

Enemy base param init 0x140576130: `bp+0x4C = ceil(row.m_ExpRate * EnemyBaseStatus[battleLevel].m_BaseExp)`
(`sheets/entities/Load/Common/EnemyBaseStatus.json`, 99 rows: `m_BaseHP` +8, `m_BaseExp` +0xC,
`m_BaseAttackPower` +0x10, `m_BaseDefensePower` +0x14; the row index is the battle level
`GW+0x7E09`, 0 → 1, capped 99; 0x140EA83C0). The enemy's death path calls the EXP component thunks
0x1405D2860 / 0x140F54D00 with that value and the hit position. Battle levels per area are data
(`System/BattleLevel/*`); pointer only.

### 4.2 `GiveExp` 0x140E9E830(GI, exp, flag, position) [C]

1. Nothing when `exp == 0` or `flag != 0` (the caller passes its owner's `vt+0xD90` / `vt+0x838`
   result; [?] meaning: it is 0 for normal enemies, the same test guards the EXP field of the
   status snapshot).
2. `pawn` = player 0 (0x140EA8DD0). `gain = AdjustExp(exp, pawn, gameLevel)` 0x140632CB0:
   - 0 if the pawn is pending kill, has HP <= 0, or is in one of the form ids 5, 7, 11, 12 (`vt+0xD80`);
   - 0 if `AbilityCount(EXPZERO 26) > 0`;
   - game level 0: `gain = exp + ceil(0.25 * exp)`; game level >= 3: `gain = ceil(0.75 * exp)`;
   - `AbilityCount(EXP_BARGAIN 174) > 0`: `d = max(battleLevel - level, 0)`;
     `gain += ceil((0.1 * d + 0.5) * gain)`;
   - `gain = min(gain, 999)`. [I] constants 0x140632D79, 0x140632DA1, 0x140632E0A, 0x140632E12, 0x140632E37
3. `added = AddExp(GW+0x7DF0, gain)` 0x140E864B0: total = clamp(total + gain, 0, 9,999,999); returns
   the amount really added.
4. If `added > 0`: for the player pawn and for each of the six party slots (`FUN_141261FB0`), call
   `CheckLevelUp(GI, total, pawn, hud)` 0x140EBE400. **There is no split**: every party member is
   checked against the same total with its own table.
5. HUD: if a position was given and the next-level EXP before the add was > 0: EXP popup
   0x141575840(hud, position, added) (entry lifetime 3.0 s); then, if the next-level EXP (after any
   level-up) is > 0: next-level counter 0x141589570(hud, nextLevelExp - total).

So EXP accrues at the moment of the kill, also beyond level 99 (up to the cap); with Zero EXP
equipped nothing accrues at all (not merely no level-up).

### 4.3 `CheckLevelUp` 0x140EBE400 [C]

If `0 < nextLevelExp <= total`: pawn `vt+0x1960` (0x1405A3E10) rebuilds the base param with
0x140575C80: the level is recomputed from the EXP, so one award can raise several levels at once,
with one panel. Then, if the HUD exists and the pawn is not hidden (`vt+0x1978`): `LevelUpDelta`
0x140583470(bp, newLevel) = row(newLevel) - row(newLevel - 1) for Strength, Magic, Defense, AP and
the ability of the new row for the current path; the non-zero entries are sent to the HUD
(0x141589330) as (kind, value) pairs: 1 Strength, 2 Magic, 3 Defense, 4 AP, 7 ability. The panel is
`UTresUIP_HudRightLevelUp` (text keys `levelup_0010..0040`, builder 0x141596150). For the player at
level 99 the achievement 0x2D is reported (0x140E77230). [C]

Not done by a level-up: no HP / MP refill, no change of max HP / MP, no equip of the new ability by
this function ([?] whether the granted copy starts equipped: section 5.1).
[?] the sound and voice of the panel (HUD / audio specs).

### 4.4 Values for HUD and menu

Level `bp+0x40`; total EXP `GW+0x7E00`; "next level" = `bp+0x44 - total` when `bp+0x44 > 0`, hidden
at level 99; this-level EXP `bp+0x48` (gauge fraction = `(total - bp[0x48]) / (bp[0x44] - bp[0x48])`
[D]). Snapshot function 0x1405AA250 (1.2).

---

## 5. Abilities

### 5.1 Storage [C]

Per character 0x200 u32 words at `record+0x160 + 4*id`:

| bits | meaning | evidence |
|---|---|---|
| 0, 4, 8 | copy 1 / 2 / 3 owned | 0x140E98CF0, 0x140E86B30 |
| 1, 5, 9 | copy 1 / 2 / 3 equipped | 0x140E8AAB0 (pair == 3 counts), 0x140E95FC0 |
| 2, 6, 10 | set together with the owned bit when a bonus is granted with the equip flag | 0x140E86B30 [?] exact use |
| 3, 7, 11 | copy owned permanently (start set, script `AbilityAdd`); copied to bits 0 / 4 / 8 at every rebuild | 0x140E98CF0, 0x140E87AA0 (start value 0xF) |
| 12..14 | copies owned through victory bonuses (recounted from the flags) | 0x140E86B30, 0x140E90EE0 |
| 15..17 | copies owned through levels (recounted from the EXP) | 0x140E90E00 |
| 18..24 | copies supplied by worn equipment / the weapon's form (recounted, cap 99) | 0x140E87590, 0x140E87610 |

Rebuild order (0x140E90EE0 and 0x140E90E00): clear the counter, `Normalize` 0x140E98CF0 (owned
bits = permanent bits, then one more owned bit per counted copy, lowest free first, up to three),
count again, and finally `FixEquipped` 0x140E95FC0 when more copies are equipped than owned
(equipped bits are cleared and set again for the first n owned copies).

Queries:
- `Count(record, id)` 0x140E8AAB0 [I]: `(bits0-1 == 3) + (bits4-5 == 3) + (bits8-9 == 3) + bits18..24`; ids >= 0x200 give 0.
- `Count(bp, id)` 0x140580810 [I] = the above + 1 when `bp[0x138]` exists and `[meal+0x30] == id`;
  `Has(bp, id)` 0x140588690 = count > 0.
- Pawn `AbilityCount` 0x1405A03C0 = `Count(pawn[0x4D0], id)`.

So an ability works when **owned and equipped**, or worn, or eaten. Owned-only copies do nothing.

[?] The functions that set and clear the equipped bits from the camp menu, and the AP test they
apply, were not found (the menu class is `UTresUIP_CampMenuAbility`). What the data and the status
code establish: max AP = 2.7; each equipped copy costs the row's `m_UseAP` [A]; equipment copies
cost nothing [C]; the help text of the three reprisals says only one can be installed [A].
[?] `m_MaxEquip` (1..3 in `TresAbilityData`) is not a cap on owned copies: Sora owns 2 or 3 copies
of five abilities whose row says 1 (CRITICAL_HALF, COMBO_PLUS, AIRCOMBO_PLUS, ATTRACTION_TIME,
MAGIC_TIME). [A]

### 5.2 Player gates [C] 0x1414358B0, 0x141424130, 0x1414242F0

The player overrides first test the script action flags `GW+0x95B0`, set per map by
`BP_SetAbility2ndJump`, `...AirSlide`, `...ShootFlow` etc.:

| ability | flag bit that must be set |
|---|---|
| DOUBLEFLIGHT (17) | bit 0 |
| AIRSLIDE (23), AIRDODGE (24) | bit 1 |
| FOCUS_ASPIR (46) | bit 4 |
| SUPERJUMP (18), SUPERSLIDE (19) | bit 7 |

`HasAbility` then asks an override object of the player controller (`FUN_140EA1550`, a per-id forced
on / off used by tutorials and debug) and finally the base count. A free test level must present
all bits as set. Party-wide variants: 0x140EABAB0 (any member has it) and 0x140EA88E0 (sum of counts
over the player and six party slots); the player pawn's `vt+0x28E0` is the summed count used by
prizes.

### 5.3 Data [A]

`sheets/entities/Load/Tres/TresAbilityData.json` (`TresAbilityDataTable`, 312 rows): `m_KeyName`,
`m_Category` (ACTION / MOVE / SUPPORT), `m_EquipType` (all NORMAL), `m_UseAP`, `m_UseMP` (party
command abilities), `m_MaxEquip`, `m_UIPriority` (menu order), text ids. Rows `A00001..A00243`
are the 183 enum abilities (ids 1..112, 170..186, 190..243 without 239); rows `A00257..A00386`
(`TEXT_*`, enum `ETresTextAbilityKind`) are display entries for shotlock, formchange and keyblade
action names and are not abilities. Ids 199..243 are party-member command abilities.

Sora's abilities are ids 1..112, 170..186 (meal bonuses and two keyblade abilities) and 190..198:
**138**. With the start set, level 99 and every Sora victory bonus he owns **73 abilities in 88
copies**; equipping all of them would cost 250 AP against 180 (210 in Critical) [D].

### 5.4 Catalogue

`sora-abilities` has one row per ability with the effect text, every hook function, every
constant-id call site (`function@site:kind`, 281 sites in 147 functions, decoded on each run of
the tool), a status and a test idea. Status counts for the 138:

| system | abilities | full | partial | hook only | no reference |
|---|---|---|---|---|---|
| movement (jump, slides, glide, dodge, flowmotion moves) | 12 | 4 | 8 | 0 | 0 |
| guard | 1 | 0 | 1 | 0 | 0 |
| counter / reprisal | 7 | 0 | 6 | 0 | 1 |
| combo (modifiers, length, replacements) | 11 | 2 | 9 | 0 | 0 |
| finisher | 4 | 0 | 4 | 0 | 0 |
| flowmotion attack | 4 | 0 | 4 | 0 | 0 |
| damage boosts | 3 | 3 | 0 | 0 | 0 |
| magic | 9 | 7 | 2 | 0 | 0 |
| MP | 17 | 17 | 0 | 0 | 0 |
| survival | 6 | 5 | 1 | 0 | 0 |
| status protection | 6 | 6 | 0 | 0 | 0 |
| prize | 9 | 6 | 1 | 2 | 0 |
| item / EXP | 2 | 2 | 0 | 0 | 0 |
| situation commands (formchange, attraction, link timers and gauges) | 9 | 6 | 2 | 1 | 0 |
| focus | 2 | 0 | 1 | 0 | 1 |
| team (finishers with Donald / Goofy) | 5 | 0 | 5 | 0 | 0 |
| party-member abilities | 8 | 5 | 3 | 0 | 0 |
| meal bonuses (cuisine) | 16 | 5 | 8 | 2 | 1 |
| Critical-mode | 3 | 1 | 2 | 0 | 0 |
| HUD / system | 4 | 0 | 2 | 2 | 0 |
| **total** | **138** | **69** | **59** | **7** | **3** |

"partial" = the hook and what it switches are read, but the state, animation or roll it belongs to
is specified elsewhere (movement, actions, combat, magic specs) or was not read to the end.
No reference: RD_COUNTER (9) and FOCUS_BOOST (42) have neither a name text nor a source (unused);
LUNCH_TIME (170, meal duration) is real but no query was found [?].

The effects with numbers (all in the sheet; evidence there):

**Movement.** HIGHJUMP: held jump launches at `MyJumpPowerHighLv3` 1950 cm/s instead of 1480
(`vt+0x1AA0` 0x141434320 returns the Lv3 property; Lv1 / Lv2 are not read) [I]. DOUBLEFLIGHT: jump
kind 3 at 2300. SUPERJUMP: kind 2 at 2500. Selection 0x141434010. SUPERSLIDE: duration 0.2 / 0.35 /
0.5 s for 1 / 2 / 3 copies (constructor 0x1413E7E8A..E9E) at 4000 cm/s. AIRSLIDE: speed scale
0.725 / 1.075 / 1.425. GLIDE: 1500 cm/s, gravity scale 0.001. The other movement abilities are
request gates of states specified in the movement and actions specs.

**Combo.** Max combo (0x141433570) = 4 + COMBO_PLUS copies on the ground, 4 + AIRCOMBO_PLUS copies
in the air. Attack-kind replacements at step selection (0x14134ACA0, 0x1413378B0): Quick Slash
1 → 9 and 0x10 → 0x18, Flash Step 2 → 10 and 0x11 → 0x19, Radial Blaster 3 → 11 and 0x12 → 0x1A,
Aerial Sweep 8 → 0x1F, Aerial Dive 0xF → 0x20; Falling Slash 0xE when the target is >= 250 cm
below, <= 400 cm away and grounded. Finisher pool (0x141433A60) = 1 + (Speed Slash or Triple Rush)
+ (Magic Flash or Last Charge).

**Damage** (section 7). **MP** (magic spec; the sheet repeats the formulas).

**Survival.** Second Chance, Withstand Combo (6.1), Damage Control (7), Defender (2.7), Leaf Bracer
(damage-invalid bit during Cure), HP Walker / MP Walker (6.4), Damage Syphon and the attribute
syphons (6.4), status protections: `ResistStatus` 0x140589610 maps bad status 17 → BURN_GUARD,
15 → CLOUD_GUARD, 13 → SNEEZE_GUARD, 7 → FREEZE_GUARD, 16 → DISCHARGE_GUARD, 12 → STUN_GUARD, then
falls back to the base row's resist bits.

**Prizes.** Treasure Magnet radius 300 / 600 / 900 cm for 0 / 1 / 2+ copies (table 0x1469AB6D8,
squared) [I]; Master Treasure Magnet unlimited; converters and Rags to Riches remap the prize kind
(0x141472DC0; kinds: 0 / 1 HP small / large, 2 / 3 MP, 4 / 5 / 6 munny 1 / 5 / 10, 7 / 8 focus);
prize values 6.3.

**Situation commands.** New command timer (0x14140DA40): formchange x(1 + 0.2 FORM_TIME), grand
magic x(1 + 0.2 MAGIC_TIME), attraction x(1 + 0.2 ATTRACTION_TIME), team attack x(1 + 0.5
COMBI_TIME of the friend); cap 99.9 s [I]. Situation gauge gain +0.2 with FORM_UP, 1.3 rate while
recharging MP with CRITICAL_CHARGE (0x1413E9000, property `CriticalChargeStylePointAddRate`),
Cure Converter: `(15n + 20) * MP / maxMP` gauge per Cure. Link timer x0.8 speed with LINK_BOOST.
Attraction marker chance `0.2 + 0.05 n`.

**Critical-mode abilities.** Critical Counter: a block inside `CriticalModeJustGuardTime` 0.1333 s
(constructor 0x141423C49) marks the reprisal as just-guard; such hits deal x2 (section 7). Critical
Recharge: above. Critical Converter: no attraction markers; a full gauge always yields a formchange
command.

### 5.5 Mechanisms other than a constant-id query

- one call site, id computed: the damage function (0x1406328BC: `0x1F + aerial bit`; 0x1406329BA:
  0x21..0x25 by attribute), the MP-on-damage function (0x54..0x59 by attribute), the status resist
  switch, the team-finisher sum (0xBE..0xC2), the guard request (`lea edx,[rsi+2]` at 0x1414295C2).
- party-wide sums: LUCK_UP (39), HARVEST (78), PRIZE_FEEVER (175) (5.2).
- data-driven: abilities reach the record from the level table, the bonus table, equipment rows,
  upgrade rows, the meal list and the start data; no table maps commands to required abilities.
- blueprint calls of `BP_IsAbilityEnable` / `BP_AbilityGetEquipNum` exist as natives
  (0x141D82720, 0x141D81700); the cooked blueprints were not searched for them [?] (a full-text
  search of `assets_converted/packages` takes about ten minutes).

---

## 6. HP / MP / focus runtime rules (outside magic)

### 6.1 Damage application: `ApplyHpDamage` 0x1405A2A00(pawn, attackerActor, info, damage, bArmor, bForce) [C], constants [I]

1. Return when `damage <= 0` (unless forced) or HP < 1. `d = floor(damage)`; `new = HP - d`.
2. Not forced, `new < 1`: `new = 1` when the pawn's `vt+0x2090` is true, or `vt+0x20A0` is true and
   this is not an armor hit ([?] the two tests; candidates are the reflected flags `m_bIsHpLimitOne`
   and `m_bIsHpLimitOneExceptPlayerAttack`), or by the kill rule of the hit (`info+0x66`:
   1 always leaves 1; 2 leaves 1 for actor kind 1; 3 for kinds 1 and 10; 4 for kind 2).
3. **Second Chance** (LAST_LEAVE 44): `HasAbility` and `HP > 1` and `new < 1` → `new = 1`. [I] 0x1405A2B2C
4. **Withstand Combo** (COMBO_LEAVE 45): `HasAbility` and (`pawn+0x848 > 0` or the state queue's
   current or next state is 19 `TSID_STATE_DAMAGE`) and `new < 1` → `new = 1`. [I] 0x1405A2B4C, 0x1405A2B6F
5. `pawn+0x848 = 0.6` after every non-forced hit [I] 0x1405A2BA2. The tick 0x1405AF5B0 counts it
   down only while COMBO_LEAVE is counted, else holds it at 0. So: the first lethal hit is survived
   by Second Chance (needs HP >= 2); further lethal hits within 0.6 s of the previous hit, or while
   still in the damage state, are survived by Withstand Combo, also from HP 1. Neither depends on
   the difficulty.
6. `new < 1`: a player pawn survives at 1 when `vt+0xA48` is true (invincible / cinematic); an enemy
   when GI flag `+0x610` bit 5 is set.
7. `SetHitPoint(new)`; damage number to the HUD (0x141584130); `vt+0x1198(lost, info)` = damaged
   notify (MP gain, 6.4); armor path for attraction HP; the attacker's notify interface.

Death is not handled here: the callers start the die state when HP is 0 (`TresPlayerState_Die`, id 59;
combat / actions specs). The magic-damage twin 0x1405BC2C0 applies rules 3 and 4 for the kill test
of area effects (without the timer clause). Percent damage 0x1405A3970 (ratio of max HP for kind 1,
of current HP for kind 2) applies Damage Control and then calls 0x1405A2A00.

### 6.2 Low-HP state [I] 0x1405AD600

`IsPinch` (pawn `vt+0x2538`) = `HP <= maxHP * 25 / 100` (integer arithmetic). The cockpit
(0x1415849A0) starts the looping HP alert while it holds (`UTresCockpit.m_HPAlertAudio`, system SE
index 9, volume 0.8, started through 0x141515B30) and stops it otherwise [C]. The same 25%
threshold (as HP percent <= 25.0) gates Damage Control and the first Defender copy.
[?] the gauge blink of the HUD (HUD spec).

### 6.3 Heal, set and prizes [C]

- `SetHitPoint(v)` 0x1405B9700: `HP = clamp(v, 0, maxHP)`, then the status listener
  `(pawn+0x448)->vt+0x220(oldHP)`.
- `Recover(hp, mp, focus, kind, flag)` 0x1405B57D0: `HP += hp`; MP: with kind 1 while charging the
  charge timer is reduced by `mp` seconds, otherwise `MP += mp`; `focus += focus`; kind != 0 shows the
  recover effect. Clamps are those of the setters.
- `FullRecover` 0x1405B5700: HP, MP, focus, attraction HP = max; charge flag and timer cleared.
- Prizes `ApplyPrize` 0x1405AB950 (player only): kind 0 HP +1, 1 HP +5, 2 MP +3 (charging: timer
  -2 s), 3 MP +5 (charging: -6 s), 4 / 5 / 6 munny 1 / 5 / 10, 7 focus +3, 8 focus +5.
- Item Boost: healing items x(1 + 0.5 n) [I] 0x14126398C.

### 6.4 Passive gains [C], constants [I]

- Walk regeneration 0x1405AF5B0 (pawn tick): while the movement speed squared is above 1 (0.25 s
  grace after stopping) a timer runs up to 10 s; from 5 s on HP Walker gives +1 HP every
  `1 / (n + 1)` s and MP Walker +1 MP every `1 / (n + 1)` s (not while charging). Standing still
  resets the timer.
- Damaged 0x1405B58D0(lost, info): not while charging or dead. Damage Syphon:
  `mp = ceil(lost / maxHP * 100 * n)`; attribute syphons (attribute 1..6 → ids 84..89):
  `mp += ceil(p * 3 * n)`, p = attack power of the hit (1 for percent damage); `Recover(0, mp, 0, 0)`.
- Block Replenisher: `Recover(0, 3, 0, 0)` on a successful block (0x141331AF0).
- Wizard's Ruse: 1 in `8 / (n + 1)` casts returns the MP spent as HP (0x1405BCDC0).

### 6.5 MP and focus

MP storage, cost, charge (50 s, 70 s in Critical; rate `1 + 0.1 a + 0.2 b + 0.3 c` for MP Haste /
Hastera / Hastega copies [I] 0x1405AB4D0), MP Safety and Extra Cast: `docs/sora-magic-spec.md` part C.
Focus: max 100 (2.7); gauge behaviour, shotlock cost and Focus Syphon in part D of the magic spec;
the pawn properties are `MyFocusPointConvertRate` 0.05, `MyFocusPointNeedAF` 10,
`MyFocusAspirRateLv1..3` 1.4 / 1.9 / 2.4 (blueprint), focus prizes +3 / +5. [A]+[I] 0x141423D52..D7A

---

## 7. The damage formula: `CalcDamage` 0x140632240 [I] (listing `build/sora_tmp/stats/dis_140632240.txt`)

Arguments: power (f32), attacker actor (abilities), stat-source actor, defender actor, damage info
(`FTresDamageInfo`, fields in the magic spec 4.1), two flags. `S` = Strength of the stat source, or
Magic when `info+0x50` bit 0; `D` = defender Defense; all as floats.

1. Zero result when power <= 0 or the info says no damage (0x14064E770).
2. `info+0x65 == 1`: result = `ceil(power * defender maxHP)`; `== 2`: `ceil(power * actor value)`. Done.
3. JOIN_FORCE (100) counted on the attacker (not for the flagged call) and the defender found in
   the target list reached through the defender's world object (`+0x44E0` → array `+0x4A0`; by the
   help text "the same target as Sora"): `S += 5`. [I] 0x140632542
4. `base = max(S - D, 1)`.
5. Defender limits `min = m_DamageMin`, `max = max(m_DamageMax, min)` (`bp+0xA4`, `bp+0xA8`):
   - `min <= 0`: for a defender of kind 1 or 5 (the enemy kinds, 0x140589180)
     `min = 1 + ceil(0.1 * battleLevel)`; for the others `min = 13` at battle level > 70, else 1;
   - `base = max(base, min)`;
   - `max > 0` and `base > max`: `t = floor((max + S - D) * 0.5)`, `t = max(t, min)` when `min > 0`;
     `base = min(base, t)`;
   - battle level > 70 and defender kind 1 / 5: `m2 = max * 2` (`max * 6` with option B0); if
     `base > m2 > min`: `base = m2`;
   - defender kind 0, 3 or 4 (the player and friends, 0x1405891A0): `base = min(base, 80)` unless
     `min >= 80`. [I] 0x1406326B5
   For Sora as defender (kind 0, min 0, max 255): `min` is 1 (13 above battle level 70) and the
   attack-minus-defence term never exceeds 80.
6. `dmg = ceil((base - 1e-6) * (power - 1e-6))`.
7. Defender: `g = bp[0x114]` ([?] which row field) > 0 and `info+0x51` bit 6: `dmg = ceil((100 - min(g, 100)) / 100 * dmg)`;
   attribute resist r (2.7): `dmg = ceil(clamp(100 - r, 0, 200) / 100 * dmg)`; `info+0x50` bit 4:
   second resist `bp[0xDC]` with clamp 0..100; `bp[0x89]` bit 0: `dmg = ceil(0.25 * dmg)`.
8. `dmg = ceil(dmg * DifficultyFactor(attacker))` (section 3).
9. Attacker abilities, in this order (n = count of the id; c = `info+0x54`, the combo value of the
   hit, i8, used only when > 0 and the last argument flag is clear):
   - physical: id `0x1F + (info+0x50 bit 7)` (COMBO_UP / AIRCOMBO_UP): `dmg += ceil(0.05 * n * c * dmg)`;
     magic: MAGIC_COMBO_UP (0x39): `dmg += ceil(0.1 * n * c * dmg)`;
   - `info+0x51` bit 1 (counter): COUNTER_UP (0x45): `dmg += ceil(0.5 * n * dmg)`;
   - `(info+0x51 & 6) == 6` (counter + just guard): CRITICAL_COUNTER (0x68) counted: `dmg *= 2`;
   - attribute 1..5: FIRE_UP .. AERO_UP (0x21..0x25): `dmg += ceil(0.2 * n * dmg)`;
   - HEARTLESS_BUSTER (0x65) and defender bio type 1: `dmg = ceil((1 + 0.3 n) * dmg)`;
     NOBODY_BUSTER (0x66) and bio type 2: the same;
   - attacker actor `vt+0x98` true: `dmg = ceil(1.5 * dmg)` [?] meaning (a pawn buff state);
   - `info+0x51` bit 4 (full MP): FULLMP_BURST (0x4D): `dmg += ceil(0.5 * n * dmg)`.
10. Defender CRITICAL_HALF (0x33) counted and HP percent <= 25: `dmg = ceil(0.5 * dmg)` with one
    copy, `ceil(dmg / 3)` with two or more.
11. A result of exactly 0 is returned as 0.1 (floored to 0 HP by 6.1).

Boosts are applied one after another, each on the running value (they compound). The combat spec owns
the hit records (`info`), the guard and the reactions.

---

## 8. Save-data shape of the player

The game keeps, for Sora:

| what | where | type | level-99 value |
|---|---|---|---|
| game level | `GW+0x7DF0` | i32 | 1 (Standard) |
| total EXP | `GW+0x7E00` | i32 | 1,527,640 |
| growth type / ability path | `GW+0x7E0C` / `+0x7E0D` | u8 / u8 | 2 / 0 (Balance, path A) |
| battle level | `GW+0x7E09` | u8 | per area |
| game flags | `GW+0x13280` | set | all Sora victory-bonus flags set |
| inventory | `GW+0x86B4` | per-item count bytes | items, keyblades |
| action flags | `GW+0x95B0` | u8 | all set |
| save record | `GW+0x9640 + i*0x9C0` | 0x9C0 bytes | below |

Save record (0x9C0 bytes) [C] 0x140E92280, 0x1405751B0, 0x140E87710:

| offset | content |
|---|---|
| +0 / +1 | kind (1 = playable character) / character id |
| +3 / +4 / +5 | start slot counts armour / accessory / item (1 / 1 / 3) |
| +6 / +7 | index of the current keyblade slot / current weapon form |
| +8..+0x67 | three 0x20-byte blocks (one per keyblade slot; upgrade / form state) [?] fields |
| +0x80 | 3 keyblade slots x 8 bytes: i32 item id, u8 flags (bit 0 open, bit 1 locked) |
| +0x98 | 8 armour slots (same layout) |
| +0xD8 | 8 accessory slots |
| +0x118 | 8 item slots |
| +0x15A..+0x15C | flag bytes [?] |
| +0x160 | u32[0x200] ability words (5.1) |
| +0x960 + kind | u8[32] victory-bonus counters (recomputed) |
| +0x980..+0x983 | u8 Strength / Magic / Defense / AP boosts |
| +0x9A0.. | [?] |

Current HP, MP and focus are properties of the pawn (`m_HitPoint` etc.) and are copied through the
status snapshot; [?] where a save file keeps them (not needed for a constructed state: full recover).

**Constructing "level 99 with everything" from the sheets** (what the port's own state needs):
1. EXP = `sora-levels` row 99 `exp_total`; level, Strength, Magic, Defense, AP from the row.
2. Bonus counters from the chosen set of victory-bonus rows (`sora-loadout-99` `hp.*` / `mp.*`
   give the sums); max HP / MP = base (halved in Critical) + sums.
3. Ability words: for every row of `sora-abilities` with `owner = sora` set `copies` owned copies;
   set the equipped bit for the copies listed in `abilities.equipped`; add equipment copies for
   the worn items (`sora-equipment.abilities`; Kingdom Key: PRIZE_DRAW).
4. Slots: keyblade slot 0 = 0x301; armour / accessory slots open per `slot.*` and empty; item slots
   per `item_slots` with the items of the loadout.
5. Magic tiers: 3 per element (`magic.known`); shotlocks and formchange of the worn keyblade.
6. HP = max HP, MP = max MP, focus = max focus, no MP charge.

---

## 9. Level-99 numbers (`sora-loadout-99`)

Level 99, total EXP 1,527,640. Strength 62, Magic 64, Defense 59, AP 180 (Critical 210).
With the Kingdom Key (+4 / +3): Strength 66, Magic 67 (fully upgraded +9 / +8: 71 / 72).
Max focus 100. The ability path does not change any number at level 99.

Max HP / max MP with every story bonus and the Re Mind MP bonuses:

| first choice | Standard / Beginner / Proud HP | MP | Critical HP | Critical MP |
|---|---|---|---|---|
| Vitality (0) | 280 | 159 | 139 | 72 |
| Balance (1) | 265 | 169 | 131 | 77 |
| Wisdom (2) | 250 | 179 | 124 | 82 |

Without the Re Mind rows MP is 24 lower (8 lower in Critical); with no story bonus at all:
120 / 100, 105 / 110, 90 / 120 (Critical 60 / 50, 52 / 55, 45 / 60). [D] from [A]+[C]

Abilities: 73 owned (88 copies, 250 AP). The default equipped set of the sheet uses exactly 180 AP:
everything except Zero EXP, MP Safety, the two extra reprisals (Counter Impact, Counter Kick), the
attraction / link / team / formchange-timer and grand-magic-timer abilities, Master Treasure Magnet,
two of three Focus Syphon copies and one of two Treasure Magnet copies. These exclusions are a
choice of the test configuration, documented in the row, not a game rule (except "one reprisal").
Magic: all six elements at tier 3. Shotlocks: King of Hearts, Ragnarok. Item slots 8 (6 without
Re Mind).

---

## 10. Cross-checks with the other specs

- `docs/sora-magic-spec.md` part C agrees on the base-param offsets, max MP formula, bonus tables,
  Critical halving, MP charge and the MP abilities (independently read; same addresses).
- **Disagreement**: its max-MP table adds both opening-choice rows (`Vbonus_080` MP_UP10 and
  `Vbonus_081` MP_UP20) to the "base game" total (165 / 189). The level blueprint awards exactly one
  of the three choice rows (2.3), so the totals are 135 / 145 / 155 (+24 Re Mind) by first choice.
- Magic spec 4.1 gives the `FTresDamageInfo` field names used in section 7 (flags +0x50 / +0x51).
- Movement / actions specs own the states gated by the movement abilities; the jump powers and slide
  values quoted in 5.4 are the same properties those specs use.

---

## 11. Open questions

1. Ability menu code: the AP test and what `m_MaxEquip` means. (Settled since: the equipped bits are
   written by 0x140E95E90 from the camp menu only, bits 2 / 6 / 10 are the NEW mark, and a newly granted
   ability is owned but not equipped: section 14.3 and docs/sora-ui-spec.md 10.4.)
2. Critical start: which code grants the abilities of the 13 bonus rows flagged
   `m_InitAbilityEquipOnCriticalMode` (2.4).
3. Boost items (Strength / Magic / Defense / AP Boost): the incrementing code and caps.
4. Keyblade upgrades: the function that builds the upgraded equipment row.
5. `bp+0x88` (x5 Strength, 0..3): which state supplies it (`(pawn+0x460)->vt+0xA8`).
6. The attacker `vt+0x98` x1.5 step of the damage function.
7. LUCK_UP drop-rate formula; PRIZE_FEEVER and HARVEST amounts; LUNCH_TIME (no query found).
8. Reprisal selection in 0x1414145C0: which of Counter Slash / Impact / Kick maps to reprisal kind
   3 / 4 (ids at the sites are known, the branch bodies were not read).
9. Level-up presentation: sound, voice, panel timing (HUD spec).
10. Whether blueprints call `BP_IsAbilityEnable` / `BP_AbilityGetEquipNum` for Sora.
11. The names of the code-menu options A0, A1, A4, B0.

## 12. Runtime probe requests (read-only probe, `docs/sora-runtime-probe.md`)

| object | offset | type | purpose |
|---|---|---|---|
| GW (= GI + 0x870) | +0x7DF0, +0x7E00, +0x7E09, +0x7E0C, +0x7E0D | i32, i32, u8, u8, u8 | confirm game level, EXP, battle level, choices against a known save |
| Sora save record (GW+0x9640 + i*0x9C0) | +0x160 + 4*id for ids 23, 28, 46, 51, 104 | u32 | confirm the bit layout: watch a word while equipping / unequipping in the menu (question 1) |
| same | +0x960..+0x97F, +0x980..+0x983 | u8[] | bonus counters and boosts after a known number of bonuses / boost items |
| same | +0x80, +0x98, +0xD8, +0x118 | 8-byte slots | slot layout and item ids |
| Sora pawn | +0x5FC, +0x604, +0x614, +0x848 | i32, i32, i32, f32 | HP / MP / focus and the Withstand Combo timer during a combo |
| Sora base param (pawn+0x4D0) | +0x40..+0x70, +0x88, +0x160..+0x170 | i32 / i16 / u8 | level, stats, bonuses at a known level; `+0x88` during buffs (question 5) |
| GW | +0x95B0 | u8 | action flags in a normal field map |

## 13. Function index (verdict from `sheets/impl/progress`; "C" = decompiled only)

| address | role | verdict |
|---|---|---|
| 0x140571980 | `UTresChrBaseParam` constructor | same |
| 0x140575C80 | player base-param init (base row, Critical halving, level, bonuses) | same-partial |
| 0x1405751B0 | flat status struct init (menu twin; shows slot → row mapping) | differs |
| 0x140582FD0 | level from EXP | differs |
| 0x1405832D0 / 0x140576E70 | read level row / apply level | differs / same-partial |
| 0x140583470 | level-up deltas | same |
| 0x140576130 | enemy base-param init (EXP reward, Critical +5 attack) | same-partial |
| 0x1405864C0 / 0x140E8CE20 / 0x140E90EE0 | bonus amount / totals / recount from flags | same-partial / no verdict / no verdict |
| 0x140586280 | decode a bonus row for a character (Critical rule) | C |
| 0x140E90E00 / 0x140E98CF0 / 0x140E95FC0 | level abilities / normalize word / fix equipped | no verdict / same / same-partial |
| 0x140E8AAB0 / 0x140580810 / 0x140588690 | ability count (record) / count / has (base param) | same / same / same |
| 0x1414358B0 / 0x141424130 / 0x1414242F0 | player HasAbility / count / level | no verdict / same-partial / same |
| 0x140E86B30 / 0x140E87590 / 0x140E87610 | grant bonus ability / equipment abilities / form abilities | same / differs / differs |
| 0x140E92280 / 0x140E87AA0 | save-record init from `TresChrInitData` / all records | same-partial / differs |
| 0x140E97490 | store growth type and ability path | same |
| 0x140E864B0 / 0x140E960E0 | add / set EXP | same / same-partial |
| 0x140E9E830 / 0x140EBE400 / 0x140632CB0 | GiveExp / CheckLevelUp / AdjustExp | same-partial / no verdict / same-partial |
| 0x140632240 / 0x140642820 | CalcDamage / difficulty factor | differs (read in disassembly) / differs (constants in disassembly) |
| 0x140583990 / 0x140583B40 / 0x1405838E0 | max HP / max MP / max focus | differs / same / same |
| 0x140580E70 / 0x1405836E0 / 0x140581830 | Strength / Magic / Defense | differs / differs / differs (thresholds in disassembly) |
| 0x1405808B0 / 0x1405850B0 | max AP / attribute resist | same / differs |
| 0x1405A2A00 / 0x1405BC2C0 / 0x1405A3970 | ApplyHpDamage / kill test twin / percent damage | differs (constants in disassembly) / same-partial / differs |
| 0x1405B9700 / 0x1405B57D0 / 0x1405B5700 / 0x1405AB950 | SetHitPoint / Recover / FullRecover / ApplyPrize | same-partial / same-partial / C / same-partial |
| 0x1405AF5B0 / 0x1405B58D0 | walk regeneration tick / damaged MP gain | differs / same-partial (constants in disassembly) |
| 0x1405AD600 / 0x1415849A0 | pinch test / cockpit HP alert | same / same-partial |
| 0x140589610 | status resist by ability | same-partial |
| 0x141433570 / 0x141434010 / 0x141434320 | max combo / jump kind and power / high-jump power | same-partial / differs / read in disassembly |
| 0x141432EB0 / 0x1414354B0 / 0x141433C90 | air-slide scale / superslide time / focus-syphon rate by count | differs / differs / differs |
| 0x14140DA40 / 0x1414794C0 / 0x141472DC0 / 0x141432610 | command timers / prize magnet / prize conversion / attraction chance | same-partial / same-partial / same-partial / same |
| 0x1405AA250 | status snapshot for menu and HUD | same |
| 0x142223CF0 → 0x140E4B180 | `BP_TresSetPlayerGrowthType` | C |

Functions with verdict "differs" were used through their decompiled C; where a number matters it was
read in the disassembly and is tagged [I] at its place.

---

## 14. A new game (added 2026-10-12; sheet `sheets/port/sora-newgame.json`)

What Sora has when a new game begins, before any reward of the story, and how the port builds it.
Certainty: **A** read in an asset or script, **C** decompiled C, **I** instruction level, **D** derived
from tagged values, **?** not determined. Nothing here was checked in the running game.

### 14.1 The opening and its two questions [A]

The first playable level is `Levels/ew/ew_01` (the Dive to the Heart). Its scripts
(`ew_01_gameflow`, `ew_01_camera_ms010`) ask twice, each time with three mirrors Sora walks to:

| | left mirror | centre mirror | right mirror |
|---|---|---|---|
| question 1 `kh3_level.ew.text_0020` "What do you desire?" | `text_0120` "Wisdom... A sharp mind that will grant you mastery of wondrous power." | `text_0110` "Vitality... A rugged body that will endure even the toughest of challenges." | `text_0130` "Balance... A body and mind of equal merit and equal potential." |
| stored `1stChoice` | 2 | 0 | 1 |
| effect | `Vbonus_081` MP_UP20 | `Vbonus_079` HP_UP30 | `Vbonus_080` HP_UP15 + MP_UP10 |
| question 2 `text_0030` "What power do you seek?" | `text_0230` "The power of the guardian... Kindness to aid friends." | `text_0210` "The power of the warrior... Invincible courage." | `text_0220` "The power of the mystic... Inner strength." |
| stored `2ndChoice` = ability path (GW+0x7E0D) | 2 = path C (`m_AbilityKind3`) | 0 = path A (`m_AbilityKind1`) | 1 = path B (`m_AbilityKind2`) |

Evidence: `ew_01_camera_ms010` sets `Select_Center` / `Select_Right` / `Select_Left` with the text key
(ubergraph 32755..33134) and raises `ew_01_REvt_ms030a..f_choice` by them (3099..4128); `ew_01_gameflow`
sets `1stChoice` 0 / 1 / 2 for a / b / c and `2ndChoice` 0 / 1 / 2 for d / e / f (59764..62698) and then
calls `BP_TresSetPlayerGrowthType(growth, path)` and awards the plan row whose `VBonusName` byte is 77 /
78 / 79 (`gameflow_VBONUS_079_Select_HP` / `080_Select_NT` / `081_Select_MP`; the enum's byte values are
the `names` list of `MacroLib_TresPlanLevelUtility_VBonusName`, not the `NewEnumeratorN` numbers).
This settles the [?] of 2.2: warrior = path A, mystic = path B, guardian = path C.

The sequence on screen: the question by `TresUI_SetCutsceneText` (movie `92_message/CutsceneText`;
centred at the top, LineFade over 2 s, kept 2 s, faded over 1 s; the mirrors work after 5 s); at a
mirror its text (left-aligned at the top, LineFade over 3 s), after 4 s `text_0040` "Is this what you
desire?" (question 2: `text_0035` "Is this the power you seek?") in its place and 2 s later
`TresUI_OpenChoice` (movie `92_message/Dialog`, no text, colour DH, at 0.5 / 0.8 of the screen) with the
entries [`choice_0080` "No", `choice_0070` "Yes"] in this order, default index 0; "No" gives the mirrors
back. After both: `text_0240` "Is this who you are?" and the same choice; Yes applies the pair, No
starts the mirrors again (`ew_01_gameflow_REvt_ms040_NO`). The strings `choice_0010` .. `0060` ("Choose
Vitality / Wisdom / Balance / the Warrior / the Mystic / the Guardian") are in the table but no script
of `ew_01` uses them. [?] whether the game draws the "No" entry above "Yes" as the array orders them
(the Dialog movie lays its buttons out in array order, 35 pixels apart: [A] `Dialog.setText`).

What the two choices change, completely [A + I]: the first one max HP / MP through its bonus row and
nothing else (the growth type it also stores at GW+0x7E0C has no reader); the second one the column of
the level table and nothing else. Strength, Magic, Defense, AP, the start abilities, magic, items and
equipment are the same for all nine pairs. All three columns hold the same 28 copies of 26 abilities by
level 52; they differ in WHEN each arrives (first grants at level 2: Combo Plus / Magic Combo Thrift /
Damage Control).

### 14.2 The start table

| what | value at the start | evidence | certainty |
|---|---|---|---|
| level, EXP | 1, 0 (next level at 80) | level table row 1; the level is derived from the EXP (0x140582FD0) | A + C |
| Strength / Magic / Defense | 11 / 12 / 8 from the level row; with the Kingdom Key (+4 / +3) 15 / 15 / 8 | `p_ex001_LevelUpData` row 1, `TresEquipItemData` I03001 | A |
| AP | 28 (Critical 58) | level row 1; `m_CriticalUseAPUpNum` 30 | A |
| max HP / MP, Vitality | 120 / 100 (Critical 60 / 50) | base row 90 / 100 + `Vbonus_079` | A + C |
| max HP / MP, Balance | 105 / 110 (Critical 52 / 55) | + `Vbonus_080` | A + C |
| max HP / MP, Wisdom | 90 / 120 (Critical 45 / 60) | + `Vbonus_081` | A + C |
| max focus | 100 | base row `m_MaxFocusPoint`; no bonus source exists | A + C |
| abilities owned AND equipped (16, costing exactly 28 AP) | Scan (LIBRA), Dodge Roll (DODGE), Air Slide (AIRSLIDE), Aerial Dodge (AIRDODGE), Block (REFLECT_GUARD), Superslide (SUPERSLIDE), Assist Friends (FRIEND_AID), Sonic Slash, Sonic Cleave (SONIC_DOWN), Buzz Saw (TURN_CUTTER), Somersault (SUMMERSALT), Pole Spin, Pole Swing, Wall Kick, Damage Control (CRITICAL_HALF), Magic Lock-On (AUTO_LOCK_MAGIC) | `TresChrInitData.m_PlayerSora.m_EquipAbility`; the record initialiser gives each the equipped bit (0x140E93CA0) | A + C |
| abilities owned, not equipped | Zero EXP (EXPZERO) | `m_HaveAbility` | A |
| Critical only | Critical Counter and Critical Recharge equipped, Critical Converter owned | `m_CriticalEquipAbility`, `m_CriticalHaveAbility` | A |
| Keyblade | Kingdom Key (`WEP_KEYBLADE00`), not upgraded, in slot 0; slots 1 and 2 open and empty | `m_Weapons`; 0x140E92280 | A + C |
| the Kingdom Key's own ability | Treasure Magnet (PRIZE_DRAW), no AP | equipment row `m_AppendAbility` | A |
| armour, accessories | none; one slot of each | `m_PlayerSora` lists none; `UTresChrInitData` constructor 0x140571C26 / C32 | A + C |
| items, pouch | none; 3 item slots, stock empty | `m_Items` empty; constructor 0x140571C41 | A + C |
| shortcuts | all 12 slots empty | magic spec J 3.1 | A |
| munny | 0 | [?] no start value was looked for; the port starts at 0 | ? |
| magic at the very first frame | none | every `MELEM_*` tier is a victory bonus (`TresVBonusData`) | A |
| magic after the prologue (before the first world) | Fire and Water, tier 1, in this slot order | `Vbonus_082` `gameflow_VBONUS_082_ew_001` = MELEM_FIRE + MELEM_WATER, awarded by `ew_01_gameflow` (ubergraph 111841..112950: plan row byte 80, "the first Darkside fight") | A |
| later magic | Cure `Vbonus_014` (Twilight Town), Blizzard 016, Thunder 021 (Toy Box), Aero 024 (Corona); second tiers 034 Fire, 037 Water, 040 Cure, 043 Thunder, 047 Blizzard, 057 Aero; third tiers 054 Fire, 061 Thunder, 064 Water, 067 Blizzard, 068 Aero, 070 Cure | `TresVBonusData` Sora columns | A |
| shotlocks | King of Hearts, Ragnarok (the Kingdom Key's) | `TresKeybladeAbilityData` WEP_KEYBLADE00 `m_ShootflowAbilitys` | A |
| formchange | Second Form (`TRES_WEAR_FORM_05`) through the Kingdom Key; its attacks and finishers are the form's own ability set (`p_ex001_FormAbility`), worn while the form lasts, never learned | `TresKeybladeAbilityData`; combat spec F 10.5 | A |
| script switches GW+0x95B0 (2nd jump, air slide, magic, formchange, shotlock, airstep, attraction, flowmotion, wall run, shortcut menu, attack) | all ON: the word is initialised to 0x7FF | 0x140E84635, 0x140E8D066, 0x140E92EDA | I |
| MP gauge and focus gauge on the HUD | shown: they follow bits 2 and 4 of that word | docs/sora-ui-spec.md, HudStatus parts `pcMp` / `pcEx` | C |
| what switches them off | only level scripts, for tutorials: `ew_01` (the Dive: shortcut menu, shotlock, magic, formchange, attraction off; attack off until its tutorial) and the first visit of Olympus `he_01` .. `he_03` (airstep, attraction, magic, shotlock, formchange, wall run: off at first, on after each tutorial). 2nd jump, air slide and flowmotion are never switched by a script | the cooked blueprints call `BP_SetAbility*` only there | A |

Movement and action abilities he does NOT start with, and where the game gives them [A, `TresVBonusData`
+ the plan table's Japanese row captions]:

| ability | source |
|---|---|
| Aerial Recovery (AIR_RECOVERY) | `Vbonus_017`, Toy Box arrival fight |
| High Jump | `Vbonus_020`, Toy Box (Angelic Amber) |
| Superjump | `Vbonus_027`, Kingdom of Corona |
| Doubleflight | `Vbonus_048`, Arendelle (the snow slide's ice dragon) |
| Air Slide, copies 2 and 3 | `Vbonus_049` Arendelle, `Vbonus_060` The Caribbean |
| Superslide, copies 2 and 3 | `Vbonus_050` Arendelle boss, `Vbonus_055` San Fransokyo |
| Glide | `Vbonus_076`, The Final World (before the heart-collecting mission) |
| Counter Slash (GUARD_COUNTER) | `Vbonus_026`, Corona; Counter Impact 032 Monstropolis; Counter Kick 058 The Caribbean; Final Blow 069 Keyblade Graveyard |
| Rising Spiral (SLASH_UPPER) 028, Groundbreaker (AIR_ROLL_BEAT) 041, Falling Slash (AIR_DOWN) 023, Speed Slash (TRIPPLE_SLASH) 019, Triple Rush (CHARGE_THRUST) 039, Magic Flash 063, Payback Strike (BLOW_COUNTER) 066, Risk Dodge 045, Combo Master 036, MP Safety 014 | the story rows named |
| Focus Syphon | `Vbonus_052` San Fransokyo + two minigame rows |
| the 26 level abilities (28 copies) | the level table, by the chosen column (2.2; `sora-levels`) |
| Quick Slash, Flash Step, Radial Blaster, Aerial Sweep, Aerial Dive, Last Charge | Re Mind rows `VBonus_DLC_debug_Init_01..03` |

Dodge Roll, Air Slide (one copy), Aerial Dodge, Block and Superslide (one copy) ARE equipped from the
first frame; none of them is switched off by a script.

### 14.3 What a newly learned ability gets [C]

Level abilities (`SetLevelAbilities` 0x140E90E00) and victory-bonus abilities (0x140E86B30, called with
its last argument 1 from 0x140E9F040) set OWNED bits only; the bonus path also sets the copy's NEW mark
(bit 2 / 6 / 10, cleared by 0x140E95F10 when the menu's cursor reaches the row). The equipped bit is
written by `SetEquipped` 0x140E95E90, whose only callers are the camp menu's Abilities screen
(0x1415C50C0, 0x1415C59B0), and by `FixEquipped`. **A learned ability is therefore not installed
automatically: the player installs it when the AP allow.** The level-up panel names it
(`LevelUpDelta` kind 7); the victory-bonus panel `HudRightGetBonus` (0x1415874F0) gets the entries
5 max HP + amount, 6 max MP + amount, 9 / 10 / 11 armour / accessory / item slot, 8 the magic command
learned (also put into a magic slot, 0x14159EE70), 7 the ability; texts `kh3_system.general.bonus_0010`
.. `0070`, title `window_0040` "BONUS!", static SE 38.

### 14.4 The port

- `sheets/port/sora-newgame.json` (authored; loadout rows) is the start of 14.2 taken AFTER the prologue:
  with Fire and Water, since that is what he has when the first world begins and the bonus cannot be
  missed. A host that wants the very first frame's "no magic" empties the rows `magic.known` and
  `bonus.counts`; the command menu then shows Magic dark and refuses to open it, as the game does without
  a valid magic slot (magic spec J 2.1).
- `kh_host::rig::loadout::new_game(base, choice)` builds it (the sheet is compiled in);
  `kh_host::save::new_game_loadout_for(base, choice)` / `new_game_loadout(base, _)` (the pair of the base
  sheet: balance + path A) are what a host calls; the test levels' level 1 (`sora-test-levels`
  `new_game` 1, `--level`-toggle, `sora.level = 1`) is the same start with the fixed pair balance + A.
  The level-20 and level-99 test loadouts are unchanged.
- The pair is save data (`SoraSave.desire`, `.ability_path`, file version 2) and comes from the new-game
  choice menu (`kh_ui::newgame`, `kh_host::hud::Hud::open_newgame_choice`), which asks the two questions
  with the game's texts, movies and timings (its header lists what is composed: the options are a Dialog
  list instead of mirrors).
- Levelling: `Status::give_exp` -> `rebuild` grants the row's numbers and the column's abilities (owned,
  not equipped); verified level by level against the game's data table and at level 99 against
  `sora-loadout-99` restricted to the sources "start" and "level" (`kh-host` test
  `levelling_grants_what_the_level_table_grants`).
- Other means: `Sora::grant(Reward)` (`kh_core::status::reward`): ability copies, a magic tier, a
  victory-bonus kind of max HP / MP or a slot, AP, a shotlock, an item, with the bonus panel of 14.3.
