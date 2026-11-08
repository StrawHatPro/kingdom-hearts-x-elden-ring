# KH3 weapons, Synthesis Moogle, Keyblade Forge, blacksmith trim: the plan

Status: PLAN (2026-11-03). Nothing here is built yet. Written by the planning agent (read-only), saved and extended by the coordinator (Orichalcum section 3.1, outfit form colours in 1.3 / P5b).

The user's request (2026-11-02): "Add all of the keyblades from KH3 including the DLC and all of their form changes and sound effects and visual effects and everything else. Add KH3's synthesis from a Moogle at the roundtable hold (put it in a spot that makes sense and no other NPC uses) and add KH3's keyblade upgrade system accessible from any instance Elden Ring's blacksmithing menu would appear. Synthesis and upgrading needs to work and behave exactly like KH3 with every option and function from KH3. They need to be 1:1 with KH3. Remove all options from blacksmiths' dialogue options related to ashes of war and other stuff like that, their only mechanical use to the player is upgrades. Also add Donald and Goofy's weapons. Any weapon in KH3 (including Donald and Goofy) that is only obtainable via synthesis needs to be the same in the mod with all of the same material requirements. Some of Donald and Goofy's weapons can be bought from shops where it makes sense, others are rare enemy drops from specific enemies and others can be found in chests. For keyblades, most are obtained from beating bosses but some should be found in chests where it makes sense. There is only 1 of each keyblade in the game (no duplicates) but Donald and Goofy can get duplicates. Try to stick as close to KH3 as possible when it comes to how to obtain a weapon for Sora, Donald and Goofy. Obtaining a weapon should give the same pop up as obtaining a map."

Later additions (2026-11-03): "for all of the keyblade forms, make sure KH1 and KH2 Sora get unique colours for each form change that suits the colours of the form change from KH3 Sora"; "make sure there is enough orichalcum spread throughout the game for synthesis (some bosses can drop it, can be found in chests, etc, but orichalcum must be a fixed item drop (never random) and there should be enough to craft everything".

"WD" = `documentation/KH3_WEAPON_DATA.md` (the KH3 tables agent), "WA" = `documentation/KH3_WEAPON_ASSETS.md` (the asset agent).

## 0. What the files already establish

KH3 weapons (`TresItemData` I030xx, names from `kh3_item` locres):
- 17 Sora Keyblades: SO_00 Kingdom Key, SO_01 Hero's Origin, SO_02 Shooting Star, SO_03 Favorite Deputy, SO_04 Ever After, SO_05 Happy Gear, SO_06 Crystal Snow, SO_07 Hunny Spout, SO_08 Nano Gear, SO_09 Wheel of Fate, SO_10 Grand Chef, SO_11 Classic Tone, SO_12 Oathkeeper, SO_13 Oblivion, SO_14 Ultima Weapon, SO_17 Starlight, SO_18 Dead of Night. `m_MaxNum` 1, none sellable. w_so100 / w_so120 / w_so160 / w_so180 have no item row: excluded.
- 14 Donald staves (WEP_DONALD_00..13): Mage's Staff(+), Warhammer(+), Magician's Wand(+), Nirvana(+), Astrolabe(+), Heartless Maul(+), Save the Queen(+).
- 14 Goofy shields (WEP_GOOFY_00..13): Knight's Shield(+), Clockwork Shield(+), Star Shield(+), Aegis Shield(+), Storm Anchor(+), Nobody Guard(+), Save the King(+). Party weapons `m_MaxNum` 99: duplicates possible.

Synthesis (`sheets/entities/Load/Tres/ItemSynthesis/*`):
- `TresItemSynthesisData`: 88 recipes (FlagIndex, reward, up to 6 materials, condition): 67 unlocked by a Collector's Goal (`TISC_COLLECT_LIST IC_n`), 20 by a Photo Mission (`TISC_MOTIF_LIST IM_n`), 1 always open (IS_0 Ether).
- `TresItemCollectData`: 78 Collector's Goals; conditions KIND_NUM, TOTAL_NUM, GET_RANK, KIND_ALL (range), SPKIND_NUM; rewards GET_ITEM, ENABLE_SYNTHESIS, ENABLE_BUY (30 goals make materials buyable).
- `TresItemMotifData`: 20 Photo Missions, each gated by a KH3 gameflow flag.
- `TresItemWeaponEnhanceData` (Keyblade Forge): 170 rows, 10 levels x 17 Keyblades: +Str / +Mag, an added ability (e.g. FORM_TIME at +4), up to 6 materials, `m_bInitAchieved` (levels a Keyblade has when obtained; Ultima Weapon has all 10).
- KH3 texts are in the live UI pack (`kh3_menu.shop.*`, `kh3_menu.synthesis.*`). KH3's movie `41_shop/Shop` is in `build/sora_pack/ui` with every Workshop part (`McMenuTop`, `McMenuMixTop`, `McMenuSynthesis`/`CoSynthesisList`, `McMenuKeyb`/`McPickerKeyb*`/`McEffKeybEnhance`, `McMenuCollection`, `McMenuPhoto`/`CoMissionList`, `McMenuMaterial`/`CoMaterialList`, SYNTH LOG, `CoImgChara`).

Synthesis-only weapons (exact recipes kept):

| Weapon | Recipe | Unlock |
|---|---|---|
| IS_12 Warhammer+ | Mythril Stone 2, Fluorite 1, Pulsing Gem 1, Hungry Shard 1 | IC_8 |
| IS_13 Clockwork Shield+ | Mythril Stone 2, Fluorite 1, Lucid Gem 1, Hungry Shard 1 | IC_8 |
| IS_19 Aegis Shield+ | Mythril Gem 2, Adamantite 1, Frost Gem 2, Pulsing Gem 2, Hungry Stone 1 | IC_18 |
| IS_33 Astrolabe+ | Mythril Crystal 2, Electrum 1, Lightning Gem 2, Pulsing Gem 2, Hungry Stone 1 | IC_18 |
| IS_48 Heartless Maul | Illusory Crystal 1, Damascus 2, Writhing Crystal 1, Writhing Gem 2 | IC_20 |
| IS_49 Nobody Guard | Evanescent Crystal 1, Damascus 2, Betwixt Gem 2, Twilight Gem 2 | IC_20 |
| IS_53 / IS_54 Heartless Maul+ / Nobody Guard+ | the same + Hungry Gem 2 | IC_22 |
| IS_55 Save the Queen | Orichalcum 1, Frost / Lucid / Soothing Crystal 1 each | IC_23 |
| IS_56 Save the King | Orichalcum 1, Blazing / Lightning / Pulsing Crystal 1 each | IC_23 |
| IS_59 / IS_60 Save the Queen+ / King+ | the same + Hungry Crystal 1 | IC_24 |
| IS_58 Ultima Weapon | Orichalcum+ 7, Wellspring Crystal 2, Lucid Crystal 2, Pulsing Crystal 2 | IC_24 (58 kinds) |

Other KH3 sources: chests (`TresTreasureData*`): Mage's Staff+ (Toy Box TS_SBOX_012), Magician's Wand+ (Arendelle FZ_SBOX_025), Nirvana+ (San Fransokyo BX_SBOX_018), Star Shield+ (Monstropolis MI_SBOX_017), Storm Anchor+ (Caribbean CA_SBOX_035); no Keyblade is in a chest. Moogle shop (`m_ShopLevel`): Warhammer / Clockwork Shield 2, Magician's Wand / Star Shield 4, Nirvana / Aegis Shield 5, Astrolabe / Storm Anchor 6. Start: Mage's Staff, Knight's Shield. Knight's Shield+ has no source in the extracted tables (WD). Keyblade story rewards are in event scripts, not tables (verify with WD).

The mod today:
- Only the Kingdom Key is held; its +1..+10 rows exist but are unreachable.
- `kh-host/src/hud_obtained.rs` has `ObtainedKind::Keyblade { key }` for the KeyItem "OBTAINED" popup maps use.
- Friends' weapons are items (`kh-host/src/items.rs` `party_weapon_key`/`_id`), today from ER staves / seals / shields (`sora-er-items.json` `party.weapons`, 50 % share).
- Only Second Form is ported (`kh-core/src/combat/form.rs`, `style.rs`, `states/style_change.rs`, `style_finish.rs`), on KH3's generic StartStyle / CHANGE_* commands (`sora-combat-form-commands.json`: CHANGE_KB, CHANGE_L1..L5, CHANGE_AN). Per-Keyblade form data: `sora-weapon.json` `keyblade.w_soNNN.formchange_variants`. 22 forms besides Second Form; 9 wear forms (Sora's costume colours: 01, 02, 03, 04, 05, 06, 08, 09, 0A).
- Smiths: talk command 24 (OpenEnhanceShop) in exactly 4 scripts (none DLC; not Gostoc):
  - Hewg t213001110 (m11_10) and t213006000: 1 "Strengthen armament" (22130001) -> 24(0); 2 "Ashes of War" (22130003) -> 143(112000, 112150); 4 "Sell" (20000011) -> 46; 99 Leave.
  - Iji t224006000: 20 "Purchase" (22240020) -> 22; 1 "Strengthen armaments" (22240000) -> 24(0); 99 Leave.
  - Smithing Table t602006000: 1 "Strengthen armament" (26020000) -> 24(1); 9 Leave.
  - Today `npc.smith_talk = 0` refuses every smith's talk (`talkkh/shop.rs`): retired by this plan. Pattern: Boc (`talkkh/alter.rs` + `boc.rs`).

## 1. Architecture

### 1.1 Data (new sheets; declared once in `tools/lib/sheet_defs.js` by P1; validated by `node tools/extract.js` + `node tools/preflight.js --summary`)

| Sheet | Owner | Keys |
|---|---|---|
| `sheets/port/sora-keyblades.json` | P2 | `kb.<WEP_KEYBLADE_SO_NN>`: item id, row, name / help ids, icon, mesh / blueprint / equip asset, base Str/Mag, abilities, shotlocks (`m_ShootflowAbilitys`), forms, init level; `kb.<KEY>.lv<1..10>`: +Str, +Mag, ability, materials; `kb.<KEY>.fx.*` / `.sound.*` / `.anim.*` (generalising `kingdom_key.*` of `sora-weapon.json`) |
| `sheets/port/sora-keyblade-forms.json` | P3 | `form.<FORM_xx>`: Keyblade(s), command, level chain, time, finishers, wear form, form abilities, replacement anim set, transform unit, weapon parts + attach points |
| `sheets/port/kh-workshop.json` | P6a | `syn.IS_<n>`, `goal.IC_<n>`, `photo.IM_<n>`, `moogle.level.<1..6>`, `moogle.stock`, `forge.*`, `workshop.*` |
| `sheets/port/kh-weapon-sources.json` | P9a / P9b | `chest.<KEY>`, `drop.<KEY>.<n>`, `shop.<KEY>`, `catchup.*`, and (3.1) `orichalcum.*` fixed placements |
| `sora-er-boss-rewards.json` new `kb.<boss>` rows | P9b | `{"add":["weapon:WEP_KEYBLADE_SO_NN"]}` (and `{"add":["item:ORICHALCUM"]}` style rows for 3.1) |
| `sheets/port/kh-moogle.json` | P8 | spot, model, anims, sounds, ranges, prompt |
| `sora-er-shop.json` rows `smith.forge`, `smith.trim` | P7 | via its generator `build/sora_tmp/party/SHOP/make_shop_sheet.js` (it rewrites the sheet) |
| `sora-er-items.json` | P10 | Evanescent / Illusory Crystal rules; `party.weapons` share (Q5) |
| `sora-item-table.json` | P1 | new `weapon` kind: 17 Keyblades + 28 friend weapons (via `tools/sora_item_table.js`) |

### 1.2 Inventory and save (P1)
- Weapons become `Class::Weapon` item rows; `items::give` accepts them.
- New `kh-host/src/weapons.rs`: a held Keyblade is refused ("already owned: no duplicate", source still marked done); friend weapons stack to 99; Keyblade levels (from `m_bInitAchieved`), Sora's 3 Keyblade slots, current slot.
- Save version 6 (additive): `weapons` block (Keyblade levels, slots, current slot; Workshop state: goals, photos, "new" recipes seen, created counts, Items Created, materials ever obtained by kind, total obtained, ranks seen, buy unlocks; Moogle level). Migration from v5: Kingdom Key level from `weapon_upgrade`; "ever obtained" back-filled from stock; goals / recipes evaluated on load; boss / chest catch-up records `kb:<boss>`, `wsrc:<flag>`. v6 refused by older builds (`Loaded::Refused`).
- Material gains feed the Workshop through one hook in `items.rs` (`on_material_gain(key, n)`); only KH3's 60 materials count for goals (not gloveworts).

### 1.3 Keyblades and form changes (P2, P3, F-packages)
- kh-core status: 3 Keyblade slots + current; level per Keyblade; stats = base + level rows; abilities + level abilities; new `kh-core/src/status/keyblade.rs`.
- Switching: KH3's Keyblade change (CHANGE_KB; the pad binding from `sora-input-layout.json` / the exe), with the new weapon's appear / disappear effect.
- Form framework (P3): Second Form re-expressed as data (`sora-keyblade-forms.json`); `form.rs` / `style.rs` read the equipped Keyblade's forms. Regression: Second Form exactly as today (existing tests unchanged + a golden trace). Hooks for F-packages: per-form state set, replacement attack / anim sets, weapon parts, transform clip, projectile kinds.
- F-packages: one per form family (`kh-core/src/combat/forms/<name>.rs` + `sheets/port/sora-form-<name>.json`).
- Shotlocks: 2 per Keyblade (~30 new): SHOT packages.
- Keyblade-only abilities now reachable: Focus Converter, Stun Protection, Situation Boost, Magic Roulette, Harvester, Wizard's Ruse; Charisma Chef shown, no effect (no cooking).
- OUTFIT x FORM COLOURS (P5b, the user 2026-11-03): on form start Sora's outfit material switches to the (current outfit, current wear form) variant, back on form end. Existing path for wear form 5: `Form.wear_form` -> `kh_er_render::set_wear_form` (`kh-er/src/sora.rs`) -> `kh-look/src/library.rs` `step_wear` (`wear_forms` / `direct`, from `wear_extra.json` by `tools/sora_outfit_forms.js`) -> `kh-er/src/boc.rs` `effective(..., wear_form, form_fallback)`. Generalise to all 9 wear forms: KH3 default from `p_ex001_WearformSet` (boc.rs's `"kh3" wear_forms [5]` becomes all); KH1 / KH2 from WA's (outfit, form) manifest as `direct` replacements (no fallback to the KH3 outfit); other outfits: their own KH3 wear forms, else a WA variant, else `form_fallback`. XS moves that show forms use the same path. Changing outfit at Boc during a form re-applies the wear form at once.

### 1.4 Equipment (P11)
`kh-ui/src/camp_equip.rs` + `kh-host/src/hud_eq.rs`: Sora's 3 Keyblade slots, KH3's Keyblade status-pane upgrade rows, the friends' weapon lists (stats independent of Sora's, `party-weapons.json`). Boost items usable from camp Items on one character (Collector's Goals IC_44..47 give them).

### 1.5 The Synthesis Moogle at Roundtable Hold (P8a, P8, P6b)
- Drawn by our renderer like Donald at the Finger Reader (`kh-er/src/unlock/host.rs`): KH3's Moogle (`Blueprints/Gimmick/ex/g_ex_mogli_01/02`; WA resolves the mesh), Bonamik on pom-pom / wings, within `show_range`.
- Talked to with the KH "Talk" prompt (Y) within `talk_range` while facing it; KH3's Moogle sound (`se00000_moogle*`); world paused (as the KH shop); KH3's `menuTop` (Buy / Sell / Workshop) with the `chara` slot (render-to-texture if possible, else hidden).
- KH3's Moogle menu 1:1: Buy (stock by Moogle level 1..6, the 8 shop weapons, goal-unlocked materials), Sell, Workshop (Synthesize Items with NEW marks, "? ? ?", Qty / Stock, confirmation; Keyblade Forge; Photo Missions; Collector's Goals with its three reward kinds and dialogs; Material List; SYNTH LOG; dialog_0010), KH3 sounds / effects.
- Spot (P8a): read `m11_10_00_00.msb` (read-only), list every NPC part; spot >= 5 m from every NPC part, outside every ER script's talk range, not a doorway, on navmesh, visible from the hall. Candidates: A (recommended) the corridor wall outside Hewg's forge, across from his door; B an empty side room at the far end of the east hall; C by the Table of Lost Grace (Enia, likely fails). P8a writes `moogle.spot` + a top-down plot picture for the user to approve. The Moogle has no collision: a nook keeps the player from walking through it.

### 1.6 Keyblade Forge from every blacksmith (P7)
- New `kh-er/src/talkkh/smith.rs` (rules, like `alter.rs`): command 24 in any character script (talk id >= 100000) is not run; a Forge request is left; query 59 reports the reinforcement menu open until the Forge closes. Recognises the command, not the NPC.
- New `kh-er/src/forgekh.rs` (the `boc.rs` / `spirittune.rs` pattern) opens KH3's Shop movie straight on the Keyblade Forge; B returns to the smith's talk list.
- Trim to upgrades only: AddTalkList (19 / 149) entries whose message is in `smith.trim` are not added: Hewg 22130003 Ashes of War, 20000011 Sell; Iji 22240020 Purchase (Q9). Leave and story dialogue stay. Safety net: commands 143 / 22 / 46 in those scripts swallowed. P7 also scans command 149 and conditional lists.
- Retire `npc.smith_talk` (`ShopTalk::smith_block`) and the query-56 skip for these scripts. ER's own reinforcement never opens; Roderika's tuning stays (already a KH3 screen).

### 1.7 The obtain popup (P9b)
`ObtainedKind::Keyblade` -> `Weapon { key }` (Keyblades and friend weapons): KH3's KeyItem "OBTAINED" popup as for maps (TresItemData icon, `kh3_item.weapon.*` name / help, static SE 10, A closes). Fires for boss, chest, drop, Collector's Goal, catch-up; purchases / synthesis per Q4.

## 2. Where each weapon comes from

All ER sources verified by P9a: boss flags against the event scripts (`sora-er-boss-rewards.json` / `ER_BOSS_FLAGS.md`), chests by lot (ItemLotParam_map, getItemFlagId) + MSB treasure part + the flag set by the chest event, drops by NpcParam ids from `build/sora_tmp/er/conv/game/reg/regulation.bin`.

### 2.1 Keyblades (one of each; boss Keyblades once per save, given even before Sora joins)

| Keyblade | KH3 source (verify, WD) | Elden Ring source | Why |
|---|---|---|---|
| Kingdom Key | start | start | |
| Hero's Origin | Olympus, Rock Titan | Margit (`boss.margit`, 10000850) | the first great boss |
| Favorite Deputy | Toy Box, King of Toys | Godrick (10000800) | a grafted giant, like a giant of toys |
| Shooting Star | Twilight Town | Rennala (14000800) (alt.: Radahn) | stars and sorcery |
| Ever After | Corona, Grim Guardianess | Royal Knight Loretta, Caria Manor (1035500800) | a forest manor by a tower; illusion magic |
| Happy Gear | Monstropolis, Lump of Horror | Red Wolf of Radagon (14000850) | Agile Claws, a clawed beast |
| Wheel of Fate | Caribbean, Davy Jones | Dragonkin Soldier of Nokstella (12010800) | underground river; Waterza |
| Nano Gear | San Fransokyo, Dark Baymax | Valiant Gargoyles (12020800) | animated constructs |
| Crystal Snow | Arendelle, Skoll | Commander Niall, Castle Sol (1051570800) | snow castle and ice |
| Oathkeeper | Re Mind | Morgott (11000800) | bound by oath to the Erdtree; Light Form |
| Oblivion | Re Mind | Maliketh (13000800) | the Black Blade; Dark Form; Double Form with Oathkeeper |
| Hunny Spout | 100 Acre Wood | chest, Mistwood (Limgrave) | no mini-games; early peaceful forest |
| Grand Chef | Le Grand Bistrot | chest: the Altus-band chest holding a Cookbook (goods 9300-9499), picked by P9a | ER's cooking stands in for KH3's |
| Classic Tone | Classic Kingdom | chest, Nokron, Eternal City | an ancient "classic" city |
| Starlight | Union chi link bonus | chest, Gatefront Ruins carriage chest (m60_42_37) | a start bonus on the main road |
| Dead of Night | bonus / tie-in (WD) | chest, Midra's Manse (DLC) | dark and frenzied; follow WD if a story source exists |
| Ultima Weapon | synthesis IS_58 | synthesis IS_58, IC_24 | |

10 bosses, 5 chests. Tokens are additive; a held Keyblade's token does nothing. Catch-up for saves past a boss / chest: Q8.

### 2.2 Donald and Goofy (duplicates allowed)
- Shops: the Moogle sells the 8 KH3 shop weapons by Moogle level (2 / 4 / 5 / 6). ER merchants also stock them by area tier (KH3 level 2 -> tier 1, 4 -> 2, 5 -> 3, 6 -> 4) where it makes sense, generically: a merchant whose own ShopLineupParam range holds a staff / seal sells Donald's staves, one with a shield sells Goofy's shields (P9a lists them; if none, the Moogle only). Price: KH3 buy price x `price.kh_factor` 10 x tier markup.
- Chests and rare drops (~2 %, one ER enemy kind each; an explicit user exception to the generic-enemy rule):

| Weapon | KH3 | ER chest | ER rare drop |
|---|---|---|---|
| Mage's Staff / Knight's Shield | start | | |
| Mage's Staff+ | Toy Box chest | Stormveil Castle chest | Raya Lucaria / glintstone sorcerers |
| Knight's Shield+ | unknown (WD) | follow WD; else a Stormveil chest | Godrick Knights / Banished Knights |
| Warhammer, Clockwork Shield | shop 2 | | |
| Magician's Wand, Star Shield | shop 4 | | |
| Nirvana, Aegis Shield | shop 5 | | |
| Astrolabe, Storm Anchor | shop 6 | | |
| Magician's Wand+ | Arendelle chest | Castle Sol chest | Twinsage sorcerers / battlemages |
| Star Shield+ | Monstropolis chest | Raya Lucaria Academy chest | Nox swordstress / monks |
| Nirvana+ | San Fransokyo chest | Leyndell chest | Oracle Envoys |
| Storm Anchor+ | Caribbean chest | Crumbling Farum Azula chest | Beastmen of Farum Azula |
| Warhammer+, Clockwork Shield+, Aegis Shield+, Astrolabe+, Heartless Maul(+), Nobody Guard(+), Save the Queen(+), Save the King(+) | synthesis only | synthesis only, exact recipes | |

Drop hook: an enemy's death (prize.rs kill handling) looks up its NpcParam / chr id in `drop.*`; only real deaths (respawns / reloads: `LOAD_EPOCH`); a KH prize box; the popup on.

## 3. Materials economy (P10)
Flowing today: 52 elemental materials (13 families x 4 grades; family from a stable hash of the ER item id, grade from the tier); ores from Smithing Stones (Fluorite [1-2], Damascus [3-4], Adamantite [5-6], Electrum [7-8]); Orichalcum from the Ancient Dragon Smithing Stone; Orichalcum+ from the Somber Ancient stone; Larval Tears after the slots.

Missing: Evanescent and Illusory Crystal (Q6); the 30 goal-unlocked Moogle purchases; the KH shop reselling ER's Smithing Stone bell-bearing lines as ores (extends the glovewort mechanism).

Budget tool `tools/kh_material_budget.js` (read-only regulation reader): counts lots converting into each KH3 material by tier / source and compares with requirements (all recipes once, e.g. Mythril Crystal 30, Orichalcum 15, Orichalcum+ 7; all 17 Keyblades to +10, e.g. Fluorite 51, Damascus 51, Adamantite 51, Electrum 17, Wellspring Shard 137 / Stone 69 / Gem 69 / Crystal 28; goal thresholds: IC_24 needs 58 kinds, KIND_ALL every family). A short family gets its hash share weighted (an economy rule, not an enemy table).

### 3.1 Orichalcum: fixed placements only (the user, 2026-11-03)

Planner's count (from `sheets/entities/Load/Tres/ItemSynthesis/*`; re-check against WD): Orichalcum 15 (14 recipes: IS_46, IS_47, IS_55, IS_56, IS_57, IS_59, IS_60 and the photo-mission recipes IS_69, 70, 71, 75, 76, 79 x1, IS_80 Petite Ribbon x2); Orichalcum+ 7 (all for IS_58 Ultima Weapon, held at once). No Forge level costs either; no recipe makes them; KH3 sells neither. Proposed placements (package P13 ORE, sheet `sheets/port/kh-ore-placements.json`, test `crates/kh-host/tests/ore_budget.rs`):
- Orichalcum 19 (15 + 4): mid game Starscourge Radahn, Rykard, Lichdragon Fortissax, Ancient Dragon Lansseax, Astel Naturalborn of the Void, Elemer of the Briar (6); late Morgott, Mohg the Omen, Fire Giant, Commander Niall, Godskin Duo, Maliketh, Loretta Knight of the Haligtree, Sir Gideon Ofnir (8); a chest in Leyndell and one in Crumbling Farum Azula (2); DLC Rellana, Messmer, Romina (3). Base game alone 16 >= 15.
- Orichalcum+ 10 (7 + 3): Malenia, Mohg Lord of Blood, Dragonlord Placidusax, Godfrey / Hoarah Loux, Astel Stars of Darkness, Borealis, Full-Grown Fallingstar Beast, a chest in Elphael (8); DLC Bayle, Promised Consort Radahn (2). Base game alone 8 >= 7.
- Never random: MAT_ITEM56 / 57 removed from the `bytier` tail of `r2.309.reinforce_other` (top tiers give Electrum); enemy lots with Ancient Dragon / Somber Ancient stones give Electrum; ER's own FLAGGED world pickups of those stones stay Orichalcum / Orichalcum+ (fixed: extra margin only); the Larval Tear rule adds margin only.
- The test fails if base-game placements fall short of the recomputed need, a placement is unverified / duplicate-flagged, the ore can still come at random, or fewer than 7 Orichalcum come before Morgott.
- Note: the Keyblade redo (6a) may move bosses; P13 keeps these ore rows additive and independent of Keyblade rows.
- Count every Orichalcum and Orichalcum+ that crafting EVERYTHING needs: every synthesis recipe using them (incl. Ultima Weapon, Save the Queen / King and their +) plus every Keyblade Forge level that costs them, from WD / the Forge rows.
- Place exactly that many plus a stated small margin as FIXED, never random, sources: one-time guaranteed boss rewards (by defeat flag, additive rows in `sora-er-boss-rewards.json`, not clashing with existing rewards), chests / world pickups at named places (`kh-weapon-sources.json` `orichalcum.*`), optionally a fixed shop quantity if KH3 sells any; spread by natural progression, late-game heavy as KH3. Remove Orichalcum / Orichalcum+ from any random conversion (the Ancient Dragon / Somber Ancient stone rules become fixed placements or other ores, decided by P10 with reasons).
- A test sums placed Orichalcum / Orichalcum+ against the totals needed and fails if short.

## 4. Work packages (strict ownership)

Only INT edits the integration files (`kh-host/src/hud.rs`, `kh-host/src/lib.rs`, `kh-er/src/lib.rs`, `kh-er/src/sora.rs`, `kh-er/src/talkkh/hooks.rs`, `kh-er/src/talkkh/mod.rs`, `kh-ui/src/lib.rs`); other packages hand INT the exact lines. Every package: Clippy 0 with no `#[allow]` / `#[expect]` (kh-er both feature states), tests green, preflight 0 errors, `build/sora_tmp/weapons/<PKG>/STATE.md`, pictures under `build/sora_tmp/weapons/<PKG>/shots/`. In-game testing per the current permission (test profile rules); the coordinator deploys.

| Pkg | Wave | Owns | Reads | Depends | Tests |
|---|---|---|---|---|---|
| WD / WA | 0 | the two docs, pack inputs | | | |
| P1 ITEMS + SAVE | 1 | `kh-host/src/items.rs`, new `weapons.rs`, `save.rs` (v6), `tools/sora_item_table.js`, `sora-item-table.json`, `tools/lib/sheet_defs.js` | | WD | no Keyblade duplicates, stacking, v5 -> v6 round trip on real saves, back-fill |
| P2 KEYBLADE DATA + EQUIP | 1 | `sora-keyblades.json` + generator, `kh-core/src/status/keyblade.rs`, `status/loadout.rs`, Keyblade parts of `status/mod.rs`, switch command | | WD | stats per level, init levels, abilities, switching |
| P3 FORM FRAMEWORK | 1 | `kh-core/src/combat/{form.rs, style.rs}`, formchange parts of `situation.rs`, `states/{style_change.rs, style_finish.rs}`, `sora-keyblade-forms.json` | | P2 | Second Form golden trace unchanged; data-driven chain |
| P5 WEAPON DRAW | 2 | weapon paths of `kh-er-render`, `kh-er/src/partydraw.rs`, `tools/sora_weapon_pack.js` | WA | P2, WA | pictures of all 17 Keyblades and form parts |
| P5b OUTFIT x FORM COLOURS | 2 | `kh-look/src/library.rs`, `kh-er/src/boc.rs` wear-form lists, `tools/sora_outfit_forms.js`, `set_wear_form` | WA manifest | P3 | outfit x 9 wear forms grid, switch on / off, Boc during a form |
| F1..F13 FORM MOVESETS | 2-3 | `kh-core/src/combat/forms/<name>.rs` + `sora-form-<name>.json` | KH3 decomp, WA | P3, P5 | traces per form, pictures |
| SHOT1..n | 2-3 | `kh-core/src/magic/shotlocks/<name>.rs` + rows | | P2 | per shotlock |
| P6a WORKSHOP RULES | 1 | `kh-host/src/workshop.rs`, `kh-workshop.json` + generator | ItemSynthesis/*, P1 | P1 | every goal kind, every unlock, Forge levels, Ultima at IC_24 |
| P6b WORKSHOP SCREENS | 2 | `kh-ui/src/shop_moogle.rs`, `kh-ui/src/workshop/*`, `kh-host/src/hud_workshop.rs` | Shop.as3, strings | P6a | every screen, pad flows, KH3 texts / sounds, pictures |
| P7 SMITHS | 2 | `kh-er/src/talkkh/smith.rs`, `kh-er/src/forgekh.rs`, `talkkh/shop.rs`, `smith.*` via the SHOP generator | talk scripts | P6b (stub ok) | 4 scripts: 24 swallowed, trims, list returns |
| P8a MOOGLE SPOT | 1 | `kh-moogle.json` spot rows, MSB reader in `build/sora_tmp/weapons/P8a/` | m11_10 MSB | | plot picture, distance table |
| P8 MOOGLE NPC | 3 | `kh-er/src/moogle.rs`, rest of `kh-moogle.json` | `unlock/host.rs` | P8a, P6b, WA | range / prompt / open / close; picture |
| P9a SOURCE DATA | 1 | `kh-weapon-sources.json` | regulation, MSB, event scripts | WD | verification table |
| P9b ACQUISITION | 2 | `kh-host/src/bossrewards.rs` (`weapon:` / item tokens, `kb.*`, catch-up), `kb.*` / Orichalcum rows in `sora-er-boss-rewards.json`, `kh-er/src/weaponsrc.rs`, `hud_obtained.rs` (`Weapon`), weapon stock in `kh-host/src/shop.rs` | | P1, P9a | tokens, catch-up, mocked flags, drops, popup |
| P10 MATERIALS | 1-2 | `sora-er-items.json` (crystals, `party.weapons`, Orichalcum rules), material rules in `lootmap.rs`, ER line resale in `shopkh.rs`, `tools/kh_material_budget.js`, Orichalcum fixed placements data (3.1) | regulation | P6a | budget report; Orichalcum sum test |
| P11 EQUIP + BOOSTS | 2 | `kh-ui/src/camp_equip.rs`, `kh-host/src/hud_eq.rs`, boost use | | P1, P2 | pictures |
| P12 PHOTO MISSIONS (Q1) | 3 | `kh-er/src/photo.rs`, camera view, `photo.*` rows | | P6a/b | motif tests |
| INT | each wave | integration files | all | each | workspace Clippy / tests |
| DOC | last | `documentation/KH3_WEAPONS_CURRENT.md`, `SORA_CURRENT.md` | | all | |

Order: wave 1 (P1, P2, P3, P6a, P8a, P9a, P10) needs only WD; then wave 2; then 3. F / SHOT packages run in parallel once P3 + P5 exist. A useful first release: Keyblades with stats and upgrades, Forge, Workshop, sources, popup; forms other than Second Form marked "form pending" until their F-package lands.

## 5. Risks
- ER hooks: swallowing command 24 and AddTalkList entries (new, untested in game); the talk-list index after removals; chest catch-up depends on the right getItemFlagId; kills counted once (respawns, reloads, `LOAD_EPOCH`); the Moogle's prompt vs ER prompts nearby.
- Renderer: multi-part weapons (Crystal Snow form 07 has 4 parts; Hero's Origin w_so037 has an "UNKNOWN" attach point), transform clips, weapon Bonamik, Double Form's two Keyblades, gun-form projectiles, the Moogle in `CoImgChara` (render-to-texture), 9 wear forms x outfits (texture memory).
- Size: 22 forms and ~30 shotlocks, each about the Second Form port's size: ship incrementally.
- Save: v6 refused by older builds; back-fill rules; old saves keep weapons got from staves / shields.
- Pack / release: pack is 2.5 GB; +1-2 GB expected; load per equipped Keyblade; pack-epoch bump; new sheets / pack folders into both Windows and Linux setup manifests (coordinator); `sora-er-shop.json` via its generator only.

## 6a. The user's decisions (2026-11-03)
- KEYBLADE SOURCES: "Hero's Origin from Margit is good and so is Shooting Star from Rennala, but spread out the keyblades in the bosses and chests more, add some of the stronger ones into the DLC and lategame. you should be obtaining new Keyblades for the duration of a playthrough." So table 2.1 is REDONE (P9a, with WD's real strengths): Margit -> Hero's Origin and Rennala -> Shooting Star stay; the other Keyblades are spread across the WHOLE playthrough, base game and Shadow of the Erdtree, one or two per progression band, in order of strength (WD's base Str / Mag at +0 and +10), the strongest (e.g. Oathkeeper, Oblivion, Dead of Night and other high-tier ones) in the late base game and the DLC; a mix of bosses and chests across the bands; no two consecutive main bosses both giving one; Ultima Weapon stays synthesis. The redone table goes to the user before P9b builds it.
- PHOTO MISSIONS: "Unlock them another way" (no camera). The 20 photo-mission recipes unlock through Elden Ring progress milestones instead (each IM_n mapped to a boss defeat or a region reached that mirrors its KH3 world's place in the story; P6a proposes the list). The Workshop's Photo Missions entry shows those milestones in KH3's screen (title / list / COMPLETE marks as KH3), with COMPOSED mission texts. P12 is cancelled.
- ER STAVES / SEALS / SHIELDS: stop turning them into friend weapons (weapons already held are kept).
- THE REST: accepted as recommended, except the Moogle shop levels: 1 at the start, 2 at GODRICK (not Margit), 3 at the second Great Rune boss, 4 at Morgott, 5 at Maliketh, 6 at a later boss after Maliketh (Godfrey, First Elden Lord / Hoarah Loux in the ashen capital, the last boss before the ending; KH3 has 6 levels).

- MOOGLE = SYNTHESIS ONLY (the user, 2026-11-03: "Do we need M.O.G card? We already have all of the Elden Ring merchants converted to KH3 shops. The moogle in the round table is only for synthesis"): no Buy / Sell / M.O.G. Card at the Moogle; it opens on KH3's Workshop list. Shop-only items (staves / shields per P9a, Elixir, bangles; Astrolabe at the Twin Maiden Husks, Moogle level 6) and goal-unlocked materials are sold by the converted ER merchants, gated by the Moogle level (6c), which survives as a progression gate only. Dishes / ingredients stay out.
- PHOTO-MISSION MILESTONES: P6a's 20 boss-defeat milestones (Agheel … all 7 Great Rune holders) APPROVED by the user, 2026-11-03.
- MOOGLE SPOT A APPROVED by the user, 2026-11-03: m11_10_00_00 (-290.5, -22.61, -306.5), rot_y 150, corridor wall across from Hewg (P8a).
- BOOST ITEMS (the user, 2026-11-03): "Whole party" — one Strength / Magic / Defense / AP Boost raises Sora, Donald and Goofy together (not KH3's one-target use).
- KH1 / KH2 FINAL FORM (the user, 2026-11-03): "Match KH3 Ultimate" — white shorts with KH3-placed dark panels, not dark shorts.
- Coordinator decisions: bangles sold from Moogle level 7; smith Forge shows only the Forge (Workshop dialogs at the Moogle); friend weapons stay unsellable; KH1 Final pockets stay as KH3's data votes (white); Wellspring family weight 4 (P10) accepted; Forge / Photo entries shown from the start as KH3.

### 6b. APPROVED Keyblade sources (the user, 2026-11-03; replaces table 2.1) — strength at +10 from `sheets/port/kh3-keyblade-levels.json`

| # | Band | Keyblade (+10 Str/Mag) | Source |
|---|---|---|---|
| - | start | Kingdom Key (9/8) | start |
| 1 | Limgrave | Starlight (9/9) | chest, Gatefront Ruins carriage (m60_42_37) |
| 2 | Limgrave | Hero's Origin (10/7) | boss: Margit |
| 3 | Limgrave | Hunny Spout (9/9) | chest, Mistwood |
| 4 | Liurnia | Shooting Star (8/10) | boss: Rennala |
| 5 | Liurnia | Ever After (7/11) | chest, Caria Manor |
| 6 | Caelid | Favorite Deputy (10/8) | chest in Caelid (P9a picks it) |
| 7 | Siofra / Nokron | Dead of Night (9/9) | chest, Nokron, Eternal City |
| 8 | Siofra | Happy Gear (11/7) | boss: Valiant Gargoyles (12020800) |
| 9 | Mt. Gelmir | Nano Gear (10/9) | boss: Rykard (16000800) |
| 10 | Altus | Grand Chef (9/11) | the Altus chest holding a Cookbook (P9a picks it) |
| 11 | Mountaintops | Crystal Snow (8/11) | boss: Commander Niall, Castle Sol (1051570800) |
| 12 | Crumbling Farum Azula | Wheel of Fate (11/9) | chest (P9a picks it) |
| 13 | DLC | Oathkeeper (12/12) | boss: Rellana, Twin Moon Knight (2048440800) |
| 14 | DLC | Oblivion (13/11) | boss: Messmer the Impaler (21010800) |
| 15 | DLC endgame | Classic Tone (11/14) | chest, Enir-Ilim (P9a picks it) |
| - | any | Ultima Weapon | synthesis IS_58 (IC_24) |

7 bosses, 8 chests. KH3 init levels from the data apply on obtaining (e.g. Favorite Deputy +1, Hunny Spout +3).

### 6c. Moogle shop levels (the user, 2026-11-03): KH3 has 7 levels (6 raises, `kh3-workshop-rules`)
1 start; 2 Godrick; 3 the second Great Rune boss; 4 the third Great Rune boss; 5 Morgott; 6 Maliketh; 7 Godfrey, First Elden Lord / Hoarah Loux.

### 6e. RAGE FORM (the user asked 2026-11-03 "will rage form be implemented as well?"; part of "all of their form changes")
Rage Form is not tied to a Keyblade, so table 0 / WD's keyblade forms don't list it. It is KH3's own formchange (form command CHANGE_AN in `sora-combat-form-commands.json`, wear form 7 "Anti", anim set p_ex001_AnimSet13: packed by WA in `keyblades/sora_forms/`, outfit colour variants made for KH1 / KH2). Already in the mod: its moves OUTSIDE the form as XS abilities (`kh-core/src/abilities/xs/rage.rs`: Rage Raid, Riskcharge, the red aura `vp_ant_bdy_ora00`). NEW PACKAGE F-RAGE (wave 2-3, after P3): trace KH3's own rule for when the Rage Form situation command is offered (its trigger, HP / damage conditions, how it is learned, its gauge / time, its levels, its finishers e.g. Rage's counters / Riskcharge inside the form, how it ends, any HP cost), port the form's moveset from AnimSet13 and KH3's rows like the other F-packages (`kh-core/src/combat/forms/rage.rs` + `sheets/port/sora-form-rage.json`), with the aura, sounds, wear form 7 on every outfit, and the existing XS Rage moves kept consistent (no double offers). Where it is learned in Elden Ring follows KH3's rule; if KH3 grants it by a story event, P9a proposes an ER boss / progression point and the user approves. LOOK ON EVERY OUTFIT (the user 2026-11-03: "make sure KH1 and KH2 Sora can look like rage form as well"): KH1 and KH2 Sora get Rage Form's real look (KH3's point pattern p_ex001_7f00 / material treatment / red eyes / aura reproduced on their meshes), not a darkened palette; the WA agent documents any runtime shader part the renderer must apply. DONE BY WA (2026-11-03): baked KH1 / KH2 wear form 7 (`tools/lib/rage_bake.js`); the renderer work F-RAGE must do is in `documentation/KH3_WEAPON_ASSETS.md` section 6b (KH3 Sora: drive `AntiVolume` 0 -> 1 / 1 -> 0 on every slot; KH1 / KH2: wear form 7 + the scrolling glow mask as emission + red iris override + hair darkening through the same formula + the same `AntiVolume` fade; all: the aura `ead_p_ant_com`). OPEN: KH3's post-darkness colour step wasn't traced, so the dark-blue shade must be checked against the game.

### 6d. Data notes from WD (`documentation/KH3_WEAPON_DATA.md`)
- The Keyblade Forge costs NO munny (materials only).
- `WEP_KEYBLADEnn` enum numbers don't match item keys: pair via `kh3-keyblades.weapon_def`.
- KH3 has no Free Development / Material Exchange; recipes repeatable (Keyblades capped at 1), no munny, no "synthesize N" rewards.
- The data sheets are prefixed `kh3-*` (13 sheets); implementers build the plan's `sora-keyblades` / `kh-workshop` etc. from them.

## 6. Open questions for the user (recommendation first) — answered, see 6a / 6b / 6c
1. Photo Missions: build a KH3-style camera with 20 ER subjects mirroring KH3's (P12), new Moogle-voiced texts; until then those 20 recipes show locked.
2. Moogle prices: KH3's x the mod's factor 10, no region markup.
3. Moogle shop level 1..6: start, Margit, Godrick, the 2nd Great Rune boss, Morgott, Maliketh.
4. Popup for purchases / synthesis: popup for every source except purchases; synthesis shows it after KH3's own result.
5. Stop turning ER staves / seals / shields into friend weapons: yes (held ones kept).
6. Evanescent / Illusory Crystal: Lost Ashes of War -> Evanescent, Dragon Heart -> Illusory.
7. Approve the Keyblade table (Shooting Star at Rennala or Radahn; Happy Gear at Red Wolf; Grand Chef by the Cookbook-chest rule).
8. Catch-up for existing saves (bosses beaten / chests opened before): yes, on load, with the popup.
9. Also remove Hewg's "Sell" and Iji's "Purchase": yes.
