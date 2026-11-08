# KH3 weapon data: Keyblades, Forge, Workshop (synthesis), materials, Moogle shop

Status: DATA (2026-11-03). Written by the KH3 tables agent ("WD" in documentation/KH3_WEAPONS_PLAN.md).
Generator: `node tools/kh3_weapon_sheets.js`. It writes 13 sheets under `sheets/port/` and registers them in
`tools/lib/sheet_defs.js` and `tools/lib/port_gen_defs.js`. It touches only its own `(weapons)` entries, replacing them
in place on a re-run. After a run: `node tools/extract.js`, `node tools/preflight.js --summary` (0 errors),
`node tools/gen_port.js`. That writes `build/gen/rust/kh3_*.rs`, which pass `cargo clippy -D warnings` when included.
No crate includes them yet.

The sheets are raw KH3. Each row carries its own `source`, and `ref_conf` where a value is not read straight from a
table. The plan's own implementation sheets (`sora-keyblades`, `sora-keyblade-forms`, `kh-workshop`,
`kh-weapon-sources`, `kh-moogle`) are left to the implementers. They can be built from these sheets.

## 1. Sheets

| Sheet | Rows | One row = | Built from |
|---|---|---|---|
| `kh3-keyblades` | 17 | a Sora Keyblade: item id, `ETresItemDefWeapon`, name / help, Str / Mag, ability, max / initial Forge level, shotlocks, wear forms, formchange commands, icons, equip data, base weapon blueprint, mesh, projectile set, how KH3 gives it | `TresItemData`, `TresEquipItemData`, `TresItemWeaponEnhanceData`, `TresItemWeaponIconData`, `TresKeybladeAbilityData`, `w_soNNN_Equip` |
| `kh3-keyblade-levels` | 170 | a Forge level (+1..+10 of each Keyblade): +Str, +Mag, the added ability, cumulative Str / Mag, materials and counts, munny (0), save flag index, `m_bInitAchieved` | `ItemSynthesis/TresItemWeaponEnhanceData` |
| `kh3-keyblade-forms` | 47 | a weapon form x grow level of a Keyblade's `m_EquipMap`. Holds: the formchange command (`CHANGE_*`), form level, form time, command lifetime, finisher, UI banner assets, weapon units with attach part and socket, transform part, replacement anim / magic / projectile sets, the moves the Abilities screen lists, the wear form's abilities, Sora's costume texture swaps, the charge effect | `TresEquipKeybladeData`, `Load/Common/TresCmdData`, `TresKeybladeAbilityData`, `p_ex001_WearformSet`, `p_ex001_FormAbility`, `UI/Data/Weapon/UIDataAssetWeapon*` |
| `kh3-weapon-assets` | 164 | a weapon blueprint (60 Sora Keyblade / form parts, 14 Donald, 14 Goofy) or a shotlock projectile blueprint (76). Lists its mesh, mesh materials, skeleton, physics, bonamik (chain), anim blueprint, anim sets, effects, sound sets, projectiles and other references. **The asset agent's packing list.** | imports + property references of each decoded package; the mesh and anim-blueprint packages |
| `kh3-party-weapons` | 28 | a Donald staff (14) or Goofy shield (14): stats, abilities, buy / sell, shop and Gummi levels, equip data, weapon blueprint, mesh, how KH3 gives it | `TresItemData`, `TresEquipItemData`, `Blueprints/Npc/n_ex00x/w_xx###_Equip` |
| `kh3-synthesis` | 88 | a Workshop recipe: reward, kind, carry cap, condition (`NONE` / `COLLECT_LIST` / `MOTIF_LIST` + row), materials and counts | `ItemSynthesis/TresItemSynthesisData` |
| `kh3-collect-goals` | 78 | a Collector's Goal: condition (N / X / Y / rank), reward kind and item, list text, the recipes and buyable items it unlocks | `ItemSynthesis/TresItemCollectData` |
| `kh3-photo-missions` | 20 | a Photo Mission: name / help, story gate (game flag >= label), seven-princess flag, previous mission, recipes it unlocks | `ItemSynthesis/TresItemMotifData` |
| `kh3-materials` | 60 | a material: rank / grade, name, help, icon, prices, carry cap, when it becomes buyable, enemy drops, chests, goal rewards, recipes that make or use it, Lucky Emblem milestones | `TresItemData`, enemy tables, `TresTreasureData*`, `TresLuckyMarkMilestoneRewardData` |
| `kh3-material-drops` | 253 | an item slot of an enemy base table: enemy code + English name, drop id, material, rate (% per kill). 252 are materials | `DataTable/Enemy/Base/*_BaseDataTable*` (`TresChrBaseParamDataTable`), `UI/Data/MobilePortal/DictionaryEnemyData` |
| `kh3-shop-stock` | 184 | an item any KH3 shop can stock: Moogle shop level, Gummi shop level, M.O.G. Card, unlock flag / goal, price, sell, sold-out rule | `TresItemData` |
| `kh3-weapon-sources` | 45 | one way KH3 gives a weapon (one per weapon in KH3) | chests, shop levels and recipes from the tables; events, minigames and entitlements from the research run |
| `kh3-workshop-rules` | 98 | a rule of the Forge, shop, Workshop, Collector's Goals or Photo Missions, with its function address (parameter sheet) | decompiled exe (`re/decomp`), level bytecode, UI strings; research rows in `build/sora_tmp/kh3_weapons/` |

Counts:
- 17 Keyblades, 14 Donald staves, 14 Goofy shields.
- 170 Forge levels: 10 per Keyblade.
- 47 form rows (30 are formchanges).
- 88 recipes:
  - by unlock: 1 always, 67 by a Collector's Goal, 20 by a Photo Mission;
  - by output: 13 weapons, 37 armour, 15 accessories, 11 materials, 8 battle items, 4 camp items.
- 78 Collector's Goals: 24 give an item, 24 unlock recipes, 30 make a material buyable.
- 20 Photo Missions.
- 60 materials: 13 shards, 13 stones, 13 gems, 21 crystals / ores.

## 2. Where it comes from

**Paks.** Every `Content/Load/Tres/**` table wins from `pakchunk0_s6-WindowsNoEditor.pak` (`build/pak_winners.json`). No
`_0_P` patch pak carries a `Load/Tres` file, so these are final. The tables are read from their decoded JSON under
`sheets/entities/Load/...`.

Structs:
- `TresItemDataTable`
- `TresEquipItemData...` (TresEquipItemData)
- `TresItemWeaponEnhanceDataTable`
- `TresItemWeaponEnhanceIconDataTable`
- `TresItemSynthesisDataTable`
- `TresItemCollectDataTable`
- `TresItemMotifDataTable`
- set: `TresItemSynthesisDataTableSet`
- `TresKeybladeAbilityDataAsset` (map `m_Datas`)
- `TresCommandKindData` (TresCmdData)
- `TresTreasureDataTable`
- `TresLuckyMarkMilestoneRewardDataTable`
- `TresChrBaseParamDataTable` (enemy base tables, packages in `pakchunk1_s*`)

**Packages** come from `assets_converted/packages/KINGDOM HEARTS III/Content/...uasset.json`:
- `Blueprints/Player/p_ex001/w_soNNN_Equip` (`TresEquipKeybladeData`)
- `..._Wep` / `..._ProjSet` (`TresProjectileSet`)
- `p_ex001_WearformSet` (`TresWearformSet`)
- `p_ex001_FormAbility` (`TresFormAbilitySet`)
- `Blueprints/Npc/n_ex001|n_ex002/w_do|w_gu###_Equip`
- `UI/Data/Weapon/UIDataAssetWeapon*` (`TresUIDataAsset`)
- `UI/Data/MobilePortal/DictionaryEnemyData`

**Strings:** English locres in `build/sora_pack/ui/strings/en` (`kh3_item`, `kh3_system`, `kh3_menu`, `kh3_char_name`).

**Weapon ids.** The value v of `ETresItemDefWeapon` is the item index: row `I03<v>`, item id `0x300 + v`. The enum names
`WEP_KEYBLADEnn` are in a different order from the item keys `WEP_KEYBLADE_SO_nn`. For example `WEP_KEYBLADE02` (value 2)
is Hero's Origin, `WEP_KEYBLADE01` (value 3) is Shooting Star, and `WEP_KEYBLADE09` (value 6) is Happy Gear. The evidence:
- the upgrade code lists ids 0x301..0x313;
- all 19 `TresItemWeaponIconData` rows agree (icon `keybladeNN` = item icon `weapon0NN`);
- the formchange contents agree (Shooting Star = Double Arrowguns).

`kh3-keyblades.weapon_def` gives the pairing. Use it, never the number in the name.

**Formchange commands.** Weapon form n is TresCmdData row `CHANGE_*`. The tool resolves it in two ways:
1. by the form's UI data asset code (`UIDataAssetWeaponHM` → `CHANGE_HM`);
2. by the formchange names in the Keyblade's own help text (`detail_kb_NN` "Formchanges: ...") matched to the commands'
   LocKey texts.

Both agree for 25 of the 30 formchange rows (`confirmed`). Five rows are by name only (`inferred`):
- Dead of Night (UI asset `L2` but help "Second Form D" = `CHANGE_L5`);
- Oathkeeper / Oblivion Light / Dark Form (`CHANGE_LF` / `CHANGE_DF`);
- their Double Form (`CHANGE_TF` / `CHANGE_TF2`), one row each.

Command fields (docs/sora-combat-spec.md 10.1): `Param1` = form level, `Param3` = form time in s, `Param0` = how long
the command stays offered. The generic gauge, decay and extension rules (FORM_TIME +20 % per copy, capped at 99.9 s) are
in that spec and in `sora-combat-form-fields`. They are the same for every form.

## 3. Rules found in the code (`kh3-workshop-rules`; `inferred` = decompiled C only)

### Keyblade Forge
- **Cost.** The Forge costs **no munny**: the check (0x141128CE0) and the commit (0x14112A3C0) only touch the six
  material pairs. Materials are taken from the Workshop stock first (cap 999 each), then from the bag.
- **Order.** Levels go strictly in order. Only the row at current level + 1 is offered. Max level = the highest row
  (10 for all).
- **`m_bInitAchieved`.** These levels count as done live; they are never saved. So Ultima Weapon arrives at +10, Favorite
  Deputy and Ever After at +1, Happy Gear / Crystal Snow / Grand Chef at +2, and Hunny Spout / Nano Gear / Wheel of Fate
  at +3.
- **Re Mind exception.** Init-achieved levels are ignored for Keyblades carried over into the Re Mind start (player
  record +0x9A0).
- **Stats.** Stats are rebuilt from base + every done row. Each row's ability is added to the weapon.
- **Which weapons.** The Forge lists only Sora Keyblades the player owns or has equipped (0x301..0x313). Donald and Goofy
  have no Forge rows.
- **Unlock.** The Keyblade Forge entry appears at gameflow_GM >= GM_EVENT2.

### Moogle shop
- **Level.** The shop level is the byte save+0x7E0A, clamped 1..99 and only ever raised. Every level script sets it on
  load from `global_GameShopLV`. The flag is raised at these story points:

  | Level | Map | Story point |
  |---|---|---|
  | 2 | yt_50 | YT_EVENT2, cutscene yt154 |
  | 3 | sf_50 | cutscene sf231 |
  | 4 | rg_50 | RG_EVENT2, cutscene rg331 |
  | 5 | kg_51 | KG_INTERVAL_02, kg433 |
  | 6 | gm_50 | GM_EVENT7, cutscene gm571 |
  | 7 | ew_27 | EW_MISSION_END |

  No item needs level 7.
- **Stock.** An item is stocked when it is not sold out and any of these holds:
  - 1 <= `m_ShopLevel` <= the shop level;
  - its `m_UnlockFlagIndex` bit is set (by a Collector's Goal with ENABLE_BUY);
  - `m_bNeedMogCard` is set and the M.O.G. Card (KEY_ITEM05) is held.

  Every Moogle sells the same list.
- **Sold out.** An item is sold out when `m_bCanbeSoldOut` is set and the held count >= `m_MaxNum`.
- **Sell.** Every held item with `m_bCanSell` can be sold, at `m_SellPrice`.
- **Gummi shop.** Huey, Dewey and Louie use `m_GumiShopLevel`. Their level (wm_01 `InitGumiShopLevel`) is:
  1 at the start; 2 after RA or TS; 3 after MI or FZ; 4 after BX or CA; 5 after EW_MISSION_END.
- **Menus.** The shop top menu is Buy / Sell / Workshop (`kh3_menu.shop.menu_0010..0030`). The Workshop has 5 entries:
  1. Synthesize Items
  2. Keyblade Forge (GM_EVENT2)
  3. Photo Missions (TT_TRM_EVENT03)
  4. Collector's Goals
  5. Material List

  Locked entries are hidden. KH3 has no Free Development and no Material Exchange.

### Synthesis
- **When a recipe is listed:**
  - `NONE`: always.
  - `COLLECT_LIST`: once the named goal is achieved.
  - `MOTIF_LIST`: once the named Photo Mission is done.

  Nothing else gates a recipe. A newly unlocked recipe gets a "new" mark.
- **Making an item.**
  - The player picks a count. It is capped by `m_MaxNum` − held and by the materials in the Workshop stock.
  - The step consumes count x each material and gives count x the reward. It costs no munny.
  - It sets a "made once" bit (the SYNTH LOG counts these out of 88) and adds to a per-recipe count.
  - Recipes repeat without limit. Keyblades are capped at 1 (Ultima Weapon, `m_MaxNum` 1; making it unlocks
    achievement 0x2C).
  - There are no "synthesize N items" rewards.
- **Workshop visit.** Each visit moves every carried material into the Workshop stock. It then tests every open goal in
  table order, with no chaining, and then the Photo Missions.

Goal conditions:
- `KIND_NUM`: N kinds ever obtained.
- `TOTAL_NUM`: the total obtained (stock + used + carried) >= N.
- `KIND_ALL`: every material X..Y obtained.
- `SPKIND_NUM`: material X obtained >= N times.
- `GET_RANK`: every material of rank R obtained.

Goal rewards:
- `GET_ITEM`: 1 item, once.
- `ENABLE_SYNTHESIS`: unlocks recipes.
- `ENABLE_BUY`: makes the material buyable at its `m_BuyPrice` (30 materials). RANK00 materials are never buyable.

### Photo Missions
- **Feature unlock:** at gameflow_TT >= TT_TRM_EVENT03.
- **When a mission is listed:** when its `m_GameFlagName` >= `m_GameFlagLabel`.
- **Completion:** a photo containing an actor whose `TresPhotoMissionComponent` names the mission. The check runs right
  after a phone photo and again on each Workshop visit.
- **Seven princess mission (IM_6):** needs all 12 teammates across the album.
- **Reward:** a mission only unlocks its recipe.

## 4. How KH3 gives each weapon (`kh3-weapon-sources`)

| Weapon | How | Where |
|---|---|---|
| Kingdom Key | start | `TresChrInitData` m_PlayerSora.m_Weapons |
| Hero's Origin | event | Olympus he_01 after he118 |
| Shooting Star | event | Twilight Town tt_01 after tt211 |
| Favorite Deputy | event | Toy Box ts_02 after ts271 |
| Ever After | event | Kingdom of Corona ra_01 after ra325 |
| Happy Gear | event | Monstropolis mi_01 after mi374 |
| Crystal Snow | event | Arendelle fz_05 after fz471c |
| Hunny Spout | event | 100 Acre Wood po_01 after po505 |
| Nano Gear | event | San Fransokyo bx_02 after bx564 |
| Wheel of Fate | event | The Caribbean ca_03 after ca420b |
| Grand Chef | minigame | Le Grand Bistrot final milestone (tt_01 after tt212; `TresGameModeLoad.m_finalMilestoneReward`) |
| Classic Tone | minigame | Classic Kingdom: all LSI games have a record (`MobilePortalDataAsset` LSIGamePlayRewardItemName); gate inferred |
| Oathkeeper | event + Moogle | ending ss941 (ss_50) gives Proof of Promises if >= 90 Lucky Emblems; the Moogle shop exchanges it (gate / swap inferred) |
| Oblivion | event + Moogle | the same; Proof of Times Past if the game level > Proud (Critical) |
| Ultima Weapon | synthesis | IS_58 (7 Orichalcum+, 2 Wellspring / Lucid / Pulsing Crystal), unlocked by IC_24 (58 material kinds) |
| Starlight | event (+ entitlement) | Keyblade Graveyard kg_01 after kg853; also the "UnionCrossKeyblade" entitlement |
| Dead of Night | entitlement | always owned on this PC build (owned check 0x14066EA00 hard-wires index 3) |
| Mage's Staff / Knight's Shield | start | `TresChrInitData` m_FriendDonald / m_FriendGoofy |
| Knight's Shield+ | event | Olympus he_01 after mission ms100 |
| Mage's Staff+ / Magician's Wand+ / Nirvana+ | chest | TS_SBOX_012 / FZ_SBOX_025 / BX_SBOX_018 |
| Star Shield+ / Storm Anchor+ | chest | MI_SBOX_017 / CA_SBOX_035 |
| Warhammer, Clockwork Shield | shop | level 2, 400 munny |
| Magician's Wand, Star Shield | shop | level 4, 600 |
| Nirvana, Aegis Shield | shop | level 5, 800 |
| Astrolabe, Storm Anchor | shop | level 6, 1200 |
| Warhammer+, Clockwork Shield+ | synthesis | IS_12 / IS_13 (IC_8) |
| Aegis Shield+, Astrolabe+ | synthesis | IS_19 / IS_33 (IC_18) |
| Heartless Maul, Nobody Guard | synthesis | IS_48 / IS_49 (IC_20) |
| Heartless Maul+, Nobody Guard+ | synthesis | IS_53 / IS_54 (IC_22) |
| Save the Queen, Save the King | synthesis | IS_55 / IS_56 (IC_23) |
| Save the Queen+, Save the King+ | synthesis | IS_59 / IS_60 (IC_24) |

Synthesis-only: Ultima Weapon plus 12 friend weapons (all six "+" or late recipes above). No Keyblade is in a chest, and
none is sold.

## 5. DLC content in the installed files

Re Mind is installed. It sits inside the normal pak chunks; there is no separately named DLC pak (the `_DLC_` folder
holds only voice paks).

**Levels:**
- `Levels/kg_DLC`: Re Mind episode (kg_04..08, kg_52).
- `Levels/rg_DLC`: rg_01, rg_03, rg_10..rg_22. The rg_10..22 boss maps are inferred to be Limit Cut.
- `Levels/ss_DLC`: ss_01, ss_51 (the secret episode, inferred).
- `Levels/bt_DLC`, `dp_DLC`, `dw_DLC`, `ew_DLC`, `he_DLC`.
- `Levels/holo_DLC`: 31 hologram spots.

**Other folders:**
- `Blueprints/ReMind/{LgRx,MiRx,SoKc}`
- `Blueprints/Level/VBonus_DLC`
- `Blueprints/Gimmick/*_DLC`
- `Effects/dlc01`, `Effects/dlc02`
- `Sound/_DLC`
- `DLCMat`
- `TresWorldLoad{BT,DW,Holo,KG,RG,SP}_DLC`
- PS4 / Xbox `TresDLCIdentifierTable` (no Windows table)
- the native entitlement table: SIE 0x310, MS 0x311, UnionCross 0x312, Seven 0x313

**What this means for the weapons:**
- No Keyblade is DLC-only. Oathkeeper and Oblivion live in the base `ss_50`.
- Re Mind weapon-relevant rewards are accessories / armour only (rg_DLC boss deaths, ss_DLC ss_01).
- Elemental Encoder (`kb_15` text, `w_so120_Equip`) and the console bonus ids 0x310 / 0x311 have **no item row on PC**:
  not included. Neither are the item-less `w_so100` / `w_so160` / `w_so180` equip data.

## 6. Gaps and open points

1. **Finishers.** Only forms with a `FINISH_<code>` command have a finisher in the sheet: SH, BZ, DR, WA, CL, YO, SK, GM,
   FL, SW, TF, TF2.
   - The first-stage forms without one (DW, HM, CL2, DW2, BZ2, HL, SH2, HM2, DR2, LF, DF) and the Second Form variants
     (L1, L2, L5) show `-`.
   - Second Form's finishers are its LM_* moves (Stun Impact, Sonic Blade, Ars Arcanum; Ancient Light on Starlight); see
     the `moves` column.
   - How a first-stage form's finish leads to the second stage was not traced in code.
2. **Which `CHANGE_L*`.** For the Second Form Keyblades (Kingdom Key L1, Starlight L2, Dead of Night L5) the command was
   matched by name. The code that picks it was not read.
3. **Grow levels.** Counter Shield and Frying Pan have 3 grow levels with different weapon parts (`ETresWeaponGrowLvl`).
   What raises the grow level was not traced.
4. **Grand Magic.** It is the generic element-point roll of the form gauge (`sora-combat-form-fields` +0xB98). There is
   no per-Keyblade data.
5. **Per-move effects and sounds.** These are in the notifies of the replacement anim sets (`kh3-keyblade-forms.anim_sets`,
   e.g. `p_ex001_AnimSet3`) and the projectile blueprints (`kh3-weapon-assets` kind `projectile`). They are not expanded
   here; the asset agent follows those lists.
6. **Orichalcum+.** Ultima Weapon needs 7. The tables show 3: chests CA_SBOX_018 and EW_LBOX_001, and Lucky Emblems 80.
   The other KH3 sources (minigame / Flantastic Seven / other event grants passed as variables) were not resolved.
   - The other ores (Mythril, Fluorite, Damascus, Adamantite, Electrum) have no enemy drop; they are in chests
     (`kh3-materials.chests`) and recipes.
   - The crystals Betwixt, Twilight, Sinister, Evanescent and Illusory come only from drops.
7. **Drop to material mapping.** `ETresDropItemID::ITEM_MaterialNN` = `MAT_ITEMNN` is by name and order (`inferred`).
   11 drop slots of the shared minigame tables (`e_AirdroidCommon`, `e_BouncyPetsCommon`, `e_PatchworkAnimalsCommon`)
   and `e_ex012` have no dictionary name.
8. **Weapon sources not settled at instruction level:**
   - Grand Chef: the milestone that triggers tt212 was not traced.
   - Classic Tone: what each LSI record slot means.
   - Oathkeeper / Oblivion: the 90-emblem and Critical gates, and the Moogle swap (`TresUIP_Shop` slot 90 → 0x141745FE0).
   - Starlight / Dead of Night: the entitlement sync 0x140EA0E30.
   - Whether Re Mind is entitlement-gated at run time on PC.
9. **Shop:**
   - where `global_GameShopLV` is set to level 1 (the code treats 1 as the floor);
   - the numeric order of the gameflow labels;
   - the munny add / subtract code of Buy and Sell;
   - which menu entries the Gummi shop shows.
10. **Text:**
    - `kh3_mobile` strings are not in the string pack (the Photo Mission camera message).
    - The use of `kh3_menu.shop.dialog_0310` "Traded {0} for {1}" was not located. It is probably the Proof → Keyblade
      exchange.
11. **Photo Missions.** The subjects (actors carrying `TresPhotoMissionComponent` with each `IM_n`) were not enumerated.
12. **Confidence.** Code findings are decompiled C (`inferred`), not instruction-level, except where a row says otherwise.

Evidence of the research runs (scripts and raw outputs): `build/sora_tmp/kh3_weapons/`, in particular `upgrade_shop.json`,
`synthesis_rules.json`, `acquisition.json` and their `*_progress.md`.
