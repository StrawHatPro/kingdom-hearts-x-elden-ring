# Sora in Elden Ring: items, loot and rarity

2026-10-13 (IMPL_ER25, reworked in IMPL_ER28). The design behind `sheets/port/sora-er-items.json` (the data: edit it; the
DLL reads the file when Sora starts), `crates/kh-er/src/lootmap.rs` (the rules, the value model and the roll, unit-tested)
and `crates/kh-er/src/loot.rs` (the game side; EVERYTHING OFF BY DEFAULT; NEVER RUN IN THE GAME). KH3's own item drop is
`docs/sora-prize-spec.md` section 11.

## 0. Your decisions, and what is still open

Decided (2026-10-13, your words in quotes):

| question | decision | where |
|---|---|---|
| which sources | "items and equipment to replace non-essential items in chests as well"; "the items that you can pick up from set dead bodies"; by the same rule ground items, gathered plants, gifts and rewards. Key / progression items stay Elden Ring's | the gate (section 5); `loot.gate` |
| things Sora cannot use (weapons, shields, spells, Ashes of War, "the summons") | "replaced with equipment and such", "comparable to what you'd receive in Elden Ring of similar rarities"; "Munny, equipment, useable items, what you think makes sense" | the value model (section 4), rules `r2.5xx` .. `r2.9xx` |
| a visible rarity cue | "No": none, no new art. Rarity only decides WHICH KH item you get | - |
| shops | "Sell KH items that make sense, no elden ring items" | `shop.kh` (section 6): PARTLY done, see there |
| Elden Ring items already held | "Remove them" | `loot.purge` (section 7) |
| Elden Ring key items | "make it possible for Sora to see Elden Ring key items in the KH item menu" | `items.er_keys` (section 7) |

Still open (short):

1. **The progression items to convert** (section 3.4): the table is a proposal row by row; say which rows to change. One
   switch turns them all off (`progression.convert` = 0).
2. **Chest against corpse.** The game hands both over through the same function with the same data; they are only told apart
   by WHICH CODE called it. The log prints the call sites; after one session with a chest, a corpse and a gift the sheet row
   `gate.callers` can name them. Until then a chest counts as a fixed world pickup (half a point less).
3. **The shop window** still shows Elden Ring's names and pictures (section 6). Two routes: a message / icon file mod, or
   KH3's own Moogle shop screen in the place of the game's window. Which?
4. **Corpses the mod cannot take over.** An enemy type whose loot table holds a key item, an unknown item or a one-time drop
   keeps Elden Ring's own lootable corpse; with the gate on, picking it up still turns the non-key items into KH items.
5. **Reusable tools** (the whistle, the lantern, the flasks, multiplayer fingers) pass through because some are needed.
   Which of them should go?
6. **Status cures and buff consumables**: boluses cannot be told from other consumables by the fields read; until their ids
   are named in the sheet they become synthesis materials, not Panaceas. KH3 has no temporary buff item.
7. **Crafting**: with no crafting materials arriving the menu has nothing to craft from; removing the entry is separate work.
8. **Remembrances** pass (they are key items by the rule) although Sora cannot use what they buy. Convert them too?

## 1. Elden Ring's side (what the rules read)

Field names are those of the pinned SDK's param structs; their MEANING, the goods-type and weapon-type numbers, the map
tiles and every item id below come from PUBLIC DOCUMENTATION / memory and were not checked in the game here. Every log
line prints the fields (and the item's NAME from the game's message tables when readable), so a wrong reading shows at once.

- **Tables**: weapons (`EquipParamWeapon`: also shields, staves, seals, bows, arrows, bolts), armour (`EquipParamProtector`),
  talismans (`EquipParamAccessory`), ashes of war (`EquipParamGem`), everything else (`EquipParamGoods`). An item id carries
  its table in the top four bits (0 weapon, 1 protector, 2 accessory, 4 goods, 8 gem) [SDK `ItemCategory`].
- **`rarity`** (all five tables): 0 .. 3 (common / uncommon / rare / legendary; the game uses it for the pickup light).
- **`goodsType`** (goods): 0 normal item, 1 key item, 2 crafting material, 3 remembrance, 5 sorcery, 7 / 8 spirit ashes,
  9 Wondrous Physick, 10 physick tear, 11 flask upgrade material, 12 info item, 14 reinforcement material, 15 great rune,
  16 incantation, 17 / 18 self-buff spells.
- **`isConsume`**, **`sellValue`**, **`refId_default`** (goods): the use effect is a `SpEffectParam` row; a negative HP / FP
  change = a restore, `soul` > 0 = it gives runes.
- **Weapons**: `wepType` (57 staff, 61 seal, 65 / 67 / 69 shields, 50 .. 56 bows, 81 .. 86 ammunition: sheet row
  `weapon.kinds`), `materialSetId` (2200 = somber: `weapon.somber_material_sets`), `attackBaseMagic / Fire / Thunder / Dark`
  (its element). A weapon id's last four digits are affinity and reinforcement level: the same armament.
- **Armour**: the four elemental damage cut rates (1 = no protection, lower = better).
- **Loot tables**: `NpcParam.itemLotId_enemy` -> `ItemLotParam_enemy`; `ItemLotParam_map` is the same struct for everything
  placed in the world (chests, corpses, ground items, plants, event rewards). Rows with consecutive ids are a chain, each
  rolled by itself; a row has eight slots (item, category, base points, count, `enableLuck`, `getItemFlagId`), a
  `lotItem_Rarity` (-1 = the item's own), a row flag, a pity counter (NOT modelled).
- **The map**: the block the player stands in, `mAA_XX_ZZ_00`; the overworld is `m60` with XX / ZZ the tile.
- **Shops**: `ShopLineupParam` (`equipId`, `equipType` 0 weapon / 1 armour / 2 talisman / 3 goods / 4 ash, `value` the price).

## 2. KH3's side (what can be given)

The 240 items of `sheets/port/sora-item-table.json` (from the game's `TresItemData`; every stock maximum is 99): 11 battle
items (Potion 40 munny, Hi-Potion 100, Mega-Potion 600, Ether 200, Hi-Ether 800, Mega-Ether 1200, Elixir 4000, Megalixir 8000,
Refocuser, Hi-Refocuser, Panacea), 5 camp items (Tent and four permanent Boosts), 60 synthesis materials (13 families in
four grades Shard < Stone < Gem < Crystal, and eight single ores), 52 armour pieces, 112 accessories (52 named pieces and 60
variant rows of Necklaces and Medals with a second ability).

What the camp menu does with them today: battle items can be put in the pouch and used; **armour and accessories can be
worn** (checked in the test app in IMPL_ER28: section 8); **materials and camp items are only kept and listed** (no
synthesis, no boosts). KH3 has no weapon drops, no arrows, no throwables, no temporary buff item.

## 3. The mapping rules

`r2.<nnn>.<name>` rows, lowest number first; the first whose conditions all hold decides. `id.<table>.<id>` rows override
everything for one item. Fail-safe: an item whose row is not found, whose table is unknown or that no rule names PASSES (it
stays Elden Ring's) and is logged as `unknown`. (The rows `rule.*` are the first generation of IMPL_ER25: the build
4B6C2CFEF72E reads only those, the build of IMPL_ER28 only `r2.*`. Leave them while the older build is in use.)

A rule's conditions: `cat`, `goods_type`, `rarity`, `effect`, `consumable`, `ids` (ranges), and new: `source`, `tier`,
`wep_kind`. Its result (`give`): `pass`, `nothing`, `munny:sell`, `munny:soul`, `munny:<n>`, `item:<KEY>`,
`bytier:<six keys>`, `material` (family by the item's id, grade by tier; `material:<n>` = that grade at least), `equip`,
`equip:armour`, `equip:accessory` (section 4.3; `equip:accessory:magic` forces the kind), `reward:acc_slot`,
`reward:def_slot`, `reward:max_mp:<3|5|10|20>`, `reward:max_hp:<3|5|10|15|30>`, `reward:ap:<n>`; several separated by `|` =
the first that has something to give. `"as_tier": n` fixes the tier, `"tier_min": n` raises it, `"count": "one"` gives one
for a whole stack, `"proposal": true` marks a row of section 3.4.

### 3.1 By category

| Elden Ring | rule | KH3 | why |
|---|---|---|---|
| Stonesword Key | 010 | **pass** | it opens things |
| progression items Sora cannot use | 020 - 027, 060, 061 | section 3.4 | proposals |
| Golden Seed, Sacred Tear | 030 | Megalixir | your earlier decision |
| key items, remembrances, info items, great runes (goods type 1, 3, 12, 15) | 040 | **pass**: untouched, Elden Ring's popup | key items stay |
| reusable tools (goods type 0, not consumed) | 050 | pass | some are needed |
| rune items (also the glowing skulls' Golden Runes) | 100 | munny of exactly that many runes | |
| HP-restoring consumable | 200 | Potion / Hi-Potion / Mega-Potion / Mega-Potion / Elixir / Elixir by tier | |
| FP-restoring consumable | 201 | Ether / Hi-Ether / Mega-Ether / Mega-Ether / Elixir / Elixir by tier | |
| both | 202 | Elixir; tier 5, 6: Megalixir | |
| status cure (once named) | 203 | Panacea | |
| Smithing Stones | 300 - 305 | Fluorite / Damascus / Adamantite / Electrum / Orichalcum / Orichalcum+ by the stone's own number | the stone's grade is its value |
| other reinforcement materials (gloveworts) | 309 | the same ore line by tier | |
| crafting materials (goods type 2) | 400 | **30 % a Potion-line item, 15 % an Ether-line item** (by tier, as rules 200 / 201), else a synthesis material: family by the item's id (always the same one), grade by tier: Shard / Stone / Gem / Crystal | "potions and ethers should be way more common"; a chest's or a late region's material is a higher grade |
| sorceries, incantations | 500 | a MAGIC accessory of the tier | he cannot cast them |
| spirit ashes | 510 | an accessory of the tier with a SUPPORT ability | "the summons" |
| any other consumable (greases, buff foods, throwables, unnamed boluses) | 600 | **35 % a Potion-line item, 20 % an Ether-line item**, else a synthesis material of the tier's grade | KH3 has no temporary boost |
| ammunition (arrows, bolts) | 700 | sell value as munny, per piece | stacks of shop goods: equipment makes no sense |
| a tier-1 or tier-2 armament from an enemy's repeatable drop | 710 | sell value as munny (a Shard when it has none) | low-tier junk: every soldier drops his sword (your log: they all have rarity 1, so tier 2 in Limgrave) |
| shields | 720 | a KH ARMOUR piece of the tier | protection |
| every other armament | 730 | a KH ACCESSORY of the tier: Strength; a staff / seal Magic; an elemental one its element's | RESERVED: Keyblades later |
| Ashes of War | 800 | an AP accessory of the tier (else Strength) | a skill; AP is KH3's currency of skills |
| armour pieces | 900 | a KH armour piece of the tier, of the element it guards best against | |
| talismans | 910 | a KH accessory of the tier, picked by its id | name single ones with `id.accessory.<id>` rows |

### 3.2 Armour and its element
The element is the one of magic / fire / lightning / holy a piece guards best against, when it beats the mean of the other
three by `armour.family_margin`; an armament's is its largest elemental base attack. Elden Ring magic -> KH blizzard, holy ->
dark, lightning -> thunder (KH3 has no "magic" and no light element on equipment: marked choices). So a rare fire-guarding
piece from a Caelid chest is a Firaza Bangle, a common one from a Limgrave soldier a Fire Bangle.

### 3.3 Talismans by effect (not done: needs ids)
Talismans map by tier only. A table by effect (attack up -> Power / Buster / Valor / Phantom Ring; FP -> the Necklaces;
discovery -> Lucky Ring; rune gain -> Gold Amulet; ...) needs the talisman ids, which the log now prints WITH THEIR NAMES:
after a session they can be written as `id.accessory.<id>` = `item:<KEY>` rows.

### 3.4 The progression items: TO CONFIRM
Elden Ring build-progression that Sora cannot use. Each row is a PROPOSAL (`"proposal": true`); `progression.convert` = 0
switches all of them off (the items then stay Elden Ring's). The ids are from memory: the log line of each pickup shows the
name next to the rule that took it; a wrong id would show as a wrong name there.

| Elden Ring item (id) | in the game | rule | proposal | reason |
|---|---|---|---|---|
| Stonesword Key (8000) | opens imp seals | 010 | **PASS** | it opens things |
| every other key item: keys, medallions, maps, quest items | progression | 040 | **PASS** | needed |
| **Bell bearings** (key items) | hand to the Twin Maiden Husks: unlock shop stock | 040 | **PASS** | they unlock merchant stock, and merchants sell KH items under `shop.kh`. I know of no quest that needs one handed to anyone but the Husks; they are not converted in any case |
| Great Runes, remembrances | boss rewards | 040 | **PASS** | key items by type (open question 8) |
| Talisman Pouch (10040; 3) | +1 talisman slot | 020 | **+1 accessory slot** (the BONUS! panel); with three accessory slots an armour slot; then a Megalixir | the same thing in KH3. THIS IS HOW SORA GETS HIS SLOTS (section 8) |
| Larval Tear (8185; 18) | rebirth | 024 | **+1 armour slot** while he has fewer than three; then an accessory slot; then Orichalcum+ | a rare, limited item; Orichalcum+ is KH3's |
| Memory Stone (10030; 8) | +1 spell slot | 021 | **maximum MP +5** | KH3's own step |
| Whetstone Knife (8590), Whetblades (8970 - 8974) | unlock Ash of War affinities | 022 | **AP +1** each | skill customisation = AP |
| Lost Ashes of War (10070) | duplicates an Ash of War | 027 | an AP accessory, tier 3 | as an Ash of War |
| Dragon Heart (10060) | buys dragon incantations | 025 | a Magic accessory, tier 4 | what it would have bought |
| Deathroot (2090; 9) | buys Gurranq's rewards | 026 | a Strength accessory, tier 4 | what it would have bought |
| Cookbooks (9300 - 9499) | crafting recipes | 023 | a synthesis material, a Gem at least | crafting is gone |
| Crystal Tears (goods type 10) | physick ingredients | 061 | Mega-Ether / Elixir / Megalixir by tier | KH3's strongest usable items |
| Flask of Wondrous Physick (goods type 9) | a flask | 060 | an Elixir | HP and MP, as a mixed flask |
| Golden Seed, Sacred Tear | flask upgrades | 030 | Megalixir | decided earlier |
| Stonesword Keys bought, Rune Arcs, Starlight Shards ... | consumables | 600 | a synthesis material | by the consumable rule |

## 4. The value model ("comparable")

### 4.1 An Elden Ring item's tier
An item is worth POINTS; `tier = 1 + floor(points)`, at most 6. Every number is a sheet row.

| part | points | row |
|---|---|---|
| rarity 0 / 1 / 2 / 3 (the lot row's rarity when it sets one) | 0 / 1 / 2 / 3 | `tier.rarity_points` |
| source: an enemy's repeatable drop, a gathered plant, a shop | 0 | `tier.source_points` |
| source: a fixed world pickup (a placed corpse, a ground item), unknown | 0.5 | |
| source: a chest, a gift / event reward | 1 | (needs `gate.callers`: open question 2) |
| source: a boss | 2 | (only through `gate.callers` or an `id.*` row) |
| region: Limgrave, Weeping / Liurnia, Stormveil, Raya Lucaria / Caelid, Altus, Gelmir, Leyndell, underground / Mountaintops, Farum Azula, Haligtree / the DLC | 0 / 0.5 / 1 / 1.5 / 2 | `tier.region_points`, `region.*` |
| sell value from 1000 / from 5000 | 0.5 / 1 | `tier.sell_points` |
| a somber (unique) armament | 0.5 | `tier.somber_points` |

The region is the map block Sora stands in when the item is given (the overworld tile through `region.overworld`, a legacy
dungeon through `region.areas`; inside a cave or catacomb the last placed block counts).

### 4.2 KH3's equipment in the same tiers
A piece's SCORE = 10 x Strength + 10 x Magic + 10 x Defense + 1 x AP + 0.25 x (sum of its resistance percents) + its
abilities (8 each; MP Hastega, Endless Magic 20, Lucky Strike 25, MP Hastera, Master Treasure Magnet 14, Damage Syphon 10):
rows `score.weights`, `score.abilities`. Tier by score: accessories from 0 / 29 / 50 / 65 / 85 / 110, armour from 0 / 25 / 40 /
55 / 65 / 80 (`tier.accessory_min`, `tier.armour_min`; a single piece: `kh.tier.<KEY>`). The 60 variant rows (Necklaces and
Medals without stats that share their names with the named pieces) are left out (`pool.exclude`).

| tier | accessories | armour |
|---|---|---|
| 1 | Ability Ring, Power Ring, Magic Ring, Junior Medal | Shield Belt, Defense Belt |
| 2 | Ability Ring+, Technician's Ring, Expert's Ring, Buster Ring, Rune Ring, Bronze Necklace, Bronze / Silver / Gold Amulet, Master Medal, Star Medal, the five Cufflinks, Gourmand's Ring | Guardian's Belt, Fire Bangle, Blizzard Choker, Thunder Trinket, Shadow Anklet, Elven Bandanna, Wind Fan, the six Chains / Chaplet / Armlet, Aegis Chain, Abas Chain, the five Rosettes, Power Weight, Magic Weight |
| 3 | Laughter Pin, Technician's Ring+, Skill Ring, Master's Ring, Valor Ring, Force Ring, Silver Necklace, Soldier's Earring, Mage's Earring, Moon Amulet, Draw Ring, Lucky Ring | Hero's Belt, Power Band, Buster Band, Fira Bangle, Blizzara Choker, Thundara Trinket, Dark Anklet, Petite Ribbon |
| 4 | Forest Clasp, Skill Ring+, Phantom Ring, Orichalcum Ring, Sorcerer's Ring, Wisdom Ring, Master's Necklace, Fencer's Earring, Slayer's Earring, Star Charm, Celestriad, Yin-Yang Cufflink | Hero's Glove, Buster Band+, Cosmic Belt, Firaga Bangle, Blizzaga Choker, Thundaga Trinket, Midnight Anklet, Divine Bandanna, Storm Fan, Acrisius |
| 5 | Cosmic Arts, Flanniversary Badge, Breakthrough | Cosmic Belt+, Firaza Bangle, Blizzaza Choker, Thundaza Trinket, Chaos Anklet, Cosmic Chain, Ribbon, Acrisius+, Master Belt |
| 6 | Cosmic Ring, Mickey Clasp, Crystal Regalia, Crystal Regalia+ | Royal Ribbon |

Materials: tier 1 Shard, 2 Stone, 3 Gem, 4 and above Crystal. Usable items: the `bytier` lists of section 3.1.

### 4.3 Which piece
The pool is the class's pieces of the tier (an empty tier: the next lower, then the next higher). Each piece has kinds
(tags): `strength` / `magic` (which stat it raises more; `balanced` when alike), `ap` (20 AP or more), `support` (an ability
of `affinity.support`: regeneration, draw, luck, the converters, Defender), its element; armour: its element, `defence` (no
resistance), `all` (three or more elements). The Elden Ring item asks for kinds in order: an armour piece or shield its
element, then `defence`; a staff or seal `magic`; an elemental armament its element, then `magic`; another armament
`strength`; an Ash of War `ap`, then `strength`; a sorcery or incantation `magic`; a spirit ash `support`; a talisman
nothing. Among the pieces of the first kind the tier has (else the whole tier), a STABLE HASH of the Elden Ring id picks one:
the same Elden Ring item is always the same KH item, at every reinforcement level and affinity.

**No flood of duplicates**: he never gets a piece he already holds `equip.dup_max` (2) of (stock + worn): the next piece of
the list is given; when the kind is used up, any piece of the tier; when the whole tier is, `equip.exhausted`: a synthesis
material of the tier's grade, else the sell value as munny.

### 4.4 Worked examples (unit test `worked_examples` prints them; the rarities are ILLUSTRATIVE: your log prints the real ones)

| Elden Ring item | points | tier | KH item | by |
|---|---|---|---|---|
| a Limgrave soldier's common crafting material | 0 | 1 | Betwixt Shard | 400 |
| the same material from a Limgrave chest | chest 1 | 2 | Betwixt Stone | 400 |
| a talisman (rarity 1) from a Limgrave chest | 1 + 1 | 3 | Soldier's Earring | 910 |
| a legendary somber armament from a Mountaintops chest | 3 + 1 + 1.5 + 0.5 | 6 | Cosmic Ring (Strength +3, AP +80) | 730 |
| a spirit ash (rarity 1) on a Limgrave corpse | 1 + 0.5 | 2 | Gold Amulet (a support piece) | 510 |
| an Ash of War (rarity 1) from a Limgrave scarab | 1 | 2 | Expert's Ring (AP +20) | 800 |
| a soldier's sword (rarity 1, as your log shows them) dropped in Limgrave | 1 | 2 | its sell value as munny (100) | 710 |
| an uncommon sword on a Liurnia corpse | 1 + 0.5 + 0.5 | 3 | Valor Ring (Strength +4) | 730 |
| a staff (rarity 1) on a Liurnia corpse | 1 + 0.5 + 0.5 | 3 | Force Ring (Magic +4) | 730 |
| a fire-guarding armour piece (rarity 2) from a Caelid chest | 2 + 1 + 1 | 5 | Firaza Bangle | 900 |
| a medium shield (rarity 1) on a Limgrave corpse | 1 + 0.5 | 2 | Guardian's Belt | 720 |
| a sorcery (rarity 2) from an Altus chest | 2 + 1 + 1 | 5 | Flanniversary Badge (Str +4, Mag +4; tier 5 has no pure Magic piece) | 500 |
| a Stonesword Key, a bell bearing | - | - | PASS | 010, 040 |
| a Talisman Pouch | - | - | +1 accessory slot | 020 |
| a stack of arrows | - | - | sell value x count as munny | 700 |

### 4.5 The base consumable drop (2026-10-13, after your first session)
**What your log showed** (build 4B6C2CFEF72E, `loot.kh` + `loot.gate`): 17 enemy types examined, 16 taken over. 26 kills
rolled 174 lot rows: 164 "nothing", 10 drops: 5 synthesis Shards, 1 Fluorite, 3 Defense Belts, 150 munny for arrows and a
bow. **No Elden Ring enemy lot seen holds a recovery item at all**: they hold crafting materials (bones, feathers, hide: 8 -
66 %), a Smithing Stone [1] (4 %), the soldiers' own armaments and armour (1.5 - 2 % each), arrows (5 - 10 %). So no rule
could have produced a Potion. (KH3 itself gives recovery from enemies as HP / MP orbs, which this mod has; its enemy rows
hold materials in their item slots, one Potion in 271 rows: there is no KH3 rate to copy. The rates below are a host choice.)

**The base drop**: with `loot.kh`, EVERY kill rolls the lines `base.drop.*` of the sheet, each by itself, with KH3's own
item roll and Lucky Strike (rate x (1 + 0.3 per copy), rounded), on top of whatever the enemy's own lot gives, and also for
an enemy whose lot was not taken over or that has none. The items burst out as KH3 item boxes.

| line (`base.drop.<name>`) | ordinary | big (max HP from 700) | boss (from 2500) |
|---|---|---|---|
| potion: Potion, Potion, Hi-Potion, Hi-Potion, Mega-Potion, Mega-Potion | 15 % | 35 % | always |
| ether: Ether, Ether, Hi-Ether, Hi-Ether, Mega-Ether, Mega-Ether | 8 % | 20 % | always |
| elixir: Elixir x5, Megalixir | 0 | 3 % | 25 % |

The item of a line is the one at level = region depth (0 Limgrave .. 4 the DLC) + 0 / 1 / 2 by class (`base.class_level`):
a Limgrave soldier drops Potions and Ethers, a Limgrave boss a Hi-Potion and a Hi-Ether for sure, an ordinary enemy of
Caelid or Altus Hi-Potions, of the DLC Mega-Potions. With one Lucky Strike 15 % becomes 20 %, with three 29 %. Edit the
`rate` arrays to taste; `base.on` = 0 (or the key `loot.base = 0`) switches it off; `base.class_hp` moves the classes.
And the lots and pickups lean the same way: rules 400 and 600 above.

### 4.6 The kill roll (unchanged from IMPL_ER25)
**Elden Ring decides whether and which**: every row of a killed enemy's loot chain is one weighted pick among its slots by
base points; an empty slot is "nothing". **KH3 decides the luck**: where Elden Ring multiplies the points of its
`enableLuck` slots by discovery / 100, the mod multiplies them by `lot.base_discovery / 100 x (1 + 0.3 x Lucky Strike
copies)`, rounded (KH3's own rule). Examples (unit test `the_lot_roll_is_elden_rings_pick_with_kh3s_luck`): a soldier's row
nothing 920 / a material 50 (luck) / a consumable 30 (luck): 5 % and 3 %; with one Lucky Strike 6.3 % and 3.8 %; with three
14.2 % together. A guaranteed slot stays guaranteed. A row with a pity counter is rolled with its base points only.

## 5. How the items appear

| source | what happens | state |
|---|---|---|
| enemy drops | rolled by the mod at the kill, thrown as KH3 item boxes; the game's own drop for that enemy type switched off | `loot.kh = 1`, UNTESTED IN GAME |
| chests, items on placed corpses, ground items, plants, gifts, quest and boss rewards | **Elden Ring's own interaction stays** (opening the chest, picking up from the body, the glow and its colour, the prompt). The swap happens when the game GIVES the item (a detour on its item-give function): the Elden Ring item never enters the inventory and its Elden Ring popup does not appear; the KH item is given with KH3's item popup (picture and name), a slot / MP / AP reward with KH3's BONUS! panel. No box (it is a pickup, not a kill) | `loot.gate = 1`, UNTESTED IN GAME |
| key items | pass untouched with Elden Ring's own popup | |

- **The popup**: KH3's HUD movie has ONE "obtained" clip (`HudLeftGetItem.setParam(icon, name, stock full)`): the same for
  an accessory as for a Potion; there is no equipment variant. KH3's treasure-chest window (movies `MultiItemGet`, `KeyItem`
  of the UI pack) is not ported: it could replace the corner popup for chests later.
- **Not done / cannot be done yet**: the pickup light's colour and the chest's own animation stay Elden Ring's (the light is
  the Elden Ring item's rarity colour); a pickup of several items shows one KH popup after another; nothing is shown for an
  item that maps to nothing.
- **Where an item came from** is read off the lot tables at the gate (an item only enemies drop / an item of a flagged map
  lot = fixed pickup / of unflagged map lots = gathered); an item that both enemies and the world hold counts as an enemy's
  (the lower value; the log says so). Chest, gift and boss need the call site (open question 2).

## 6. Shops (`shop.kh`): partly done

With `shop.kh = 1` every merchant line (`ShopLineupParam`) is read when Sora starts:

- a **key item's** line (Stonesword Keys, maps, bell-bearing stock of key items ...) stays Elden Ring's;
- a line whose item **maps to a KH item** gets that item's price in munny written into the row (the rune counter is Sora's
  munny): KH3's own shop price where KH3 sells the item (Potion 40, Ether 200, Fire Bangle 1000, a Shard 100 ...), else the
  price of its tier (`shop.tier_price`: 300 / 900 / 1500 / 2200 / 3000 / 5000). The prices are written back when Sora stops;
- a line that maps to **munny, a reward or nothing** (ammunition, a Memory Stone) is "dropped": told in the log, BUT STILL
  OFFERED (hiding a line needs the shop window);
- what is **bought** becomes its KH item at the gate if purchases go through the item-give function (NOT VERIFIED: the log
  will show a `loot: gate:` line at a purchase or not); `shop.kh = 2` also watches the inventory once a second and converts
  whatever arrives there by another path (it uses the hand-written removal of section 7).

**Said plainly: the shop window still shows Elden Ring's item names and pictures.** The game draws them from the item's own
row; a line's `nameMsgId` / `iconId` can point elsewhere, but only at text and icons that exist in the game's files, and code
alone cannot add a string to a message table (an existing one can be overwritten only at its own length). Two routes:
(a) a message / icon file mod (KH names as new message entries, the lines pointed at them; KH pictures would need the game's
icon atlas rebuilt); (b) **KH3's own shop screen** (the Moogle shop, a movie of KH3's menus) opened in the place of Elden
Ring's window, fed from the same `ShopLineupParam` lists and this mapping: the line list, names, pictures, prices and
"dropped" lines are then all ours. (b) is the route I propose; it is a milestone of its own.

## 7. Elden Ring items already held; key items in the KH menu

**`loot.purge`** (with `loot.gate = 1`; once per character: a file `<character>.purged.txt` with the list is written
beside Sora's save): the non-key Elden Ring items in the normal inventory are removed. IRREVERSIBLE once the game saves:
**copy your save first**. 1 = only TELLS what it would remove (start here); 2 = removes goods (consumables, materials, spells,
spirit ashes, tears: things without an instance record); 3 = also armaments, armour, talismans and Ashes of War. Never
removed: whatever the rules pass (key items, tools, unknowns), what the character wears, what is in a quick slot or the
pouch. Nothing is given for what is removed. HOW: the pinned SDK has no "remove item" function, so the entry is emptied by
hand as the SDK lays the inventory out (quantity, the "no item" id, the id's place in the lookup table); with 3 the
instance records of the removed equipment stay behind in the game's instance table (NOT VERIFIED harmless). The storage
chest is not touched.

**`items.er_keys = 1`**: the camp menu's Items screen has KH3's own **Key Items** tab; it now lists the Elden Ring key items
the character holds (name from the game's message tables, count), read once a second, read-only, reached like every tab
(L2 / R2 = LT / RT on the Items screen). A name the tables do not give is shown as "Elden Ring goods <id>".

(IMPL_ER42, user: "show elden ring icon and description") The item pane of the highlighted key item shows the item's
**Elden Ring description**: the message files of sheet row `msg.texts` (GoodsCaption 24 / 321 first, then GoodsInfo 20 / 320;
numbers from public documentation, UNVERIFIED in the game), markup tags removed, lines run on as one text and cut with "..."
to the three lines the pane's help field holds (`kh_ui::render::fit_text`). The first listing of each item logs
`loot: key item goods <id> "<name>": icon id <iconId> (texture MENU_ItemIcon_<iconId>...), description <n> characters from
message file <f>: "<start>"`.

(IMPL_ER43, user: "Find a way to make the icons load, the simplest way is the best") **The picture**: Elden Ring's own
icon, taken OFFLINE from the user's install into the UI pack like every KH3 asset: `node tools/er_icons.js` (read-only;
never regulation.bin) decrypts the archive headers with the RSA public keys in eldenring.exe, takes
`/menu/hi/01_common.tpf.dcx` (the item icon atlases SB_Icon_00 .. SB_Icon_08_dlc, BC7, base game and DLC) and
`/menu/hi/01_common.sblytbnd.dcx` (their layouts: `MENU_ItemIcon_<iconId>` = x, y, 160 x 160) out of Data0.bdt, undoes
DCX with a copy of the game's Oodle DLL, and writes `build/sora_pack/ui/er_icons/<iconId:05>.png` (2976 icons) and the
index `data/er_icons.json`. The HUD lists a key item as `HOSTKEY_n.icon<iconId>`; `kh_ui::item_art` loads its picture on
first show (fade-in as KH3 items). An icon number not in the index: text only, as before. Run the tool again after a game
update that adds items. (`menutpfbnd:/00_Solo` = `/menu/hi/00_solo.tpfbhd`, 1.4 GB: the large `MENU_Knowledge_*`
pictures of Elden Ring's item detail; not used.)

## 8. Is received equipment usable?

Yes (test app, IMPL_ER28, through the pad path: `build/sora_tmp/er/er28/equip_sheet.png`): a Valor Ring and a Fira Bangle
arrive through the same door the loot uses (`kh_host::items::give`), each with the popup; the Equipment screen lists them
in the stock with the others; equipped, the Stats screen goes from Strength 15 / Defense 8 / AP 28 / fire 0 % to Strength
19 / Defense 11 / AP 34 / fire 25 % (`Status::rebuild`).

**Slots**: a new-game Sora has ONE armour slot and ONE accessory slot (KH3's start); KH3's Sora ends with three of each,
from victory bonuses, which this host does not hand out yet. So without anything else all found equipment competes for two
slots. The proposal of section 3.4 closes the gap from Elden Ring's own progression: the three Talisman Pouches give
accessory slots 2 and 3 and one armour slot, the first Larval Tear the last armour slot. The Equipment screen opens one slot
more per slot bonus (code read; the reward seam's `grant slot accessory` word exists in the test app).

## 9. Keys (er\mods\kh-er.ini; since 2026-10-13 loot.kh, loot.gate, shop.kh and items.er_keys default to 1; the rest 0)

- `loot.log = 1`: change nothing; log every enemy type's loot chain with the mapping of each slot, and every item the game
  hands the player (name, fields, source, block and depth, tier arithmetic, call sites). Start with this.
- `loot.kh = 1`: enemy drops as KH3 boxes (needs `prize.kh`, which is on by default); `loot.base = 0` switches the base
  consumable drop off (section 4.5).
- `loot.gate = 1`: the gate (chests, corpses, pickups, gifts).
- `shop.kh = 1` / `2`: section 6.
- `loot.purge = 1` / `2` / `3`: section 7.
- `items.er_keys = 1`: section 7.

## 10. The friends' weapons (ER party S6, 2026-10-15; the plan's U4 default)

Donald's 14 staves and Goofy's 14 shields come from the same loot as Sora's gear (row `party.weapons` of
`sheets/port/sora-er-items.json`; builds before the party do not read it):
- an Elden Ring staff or sacred seal can become one of Donald's staves, a shield one of Goofy's shields: `share` percent
  of those items (50: decided by a stable hash of the Elden Ring id, so the same item always gives the same thing); the
  rest stay what the model made of them (a Magic accessory, an armour piece);
- the weapon is of the item's value tier (tiers 1..6 over the friend's 14 weapons in KH3's order, weakest first); never
  more than `dup_max` (1) of one weapon; not sold in shops; only for a friend the party has unlocked;
- it goes into the camp menu's stock (the item popup names it) and the friend's Equipment screen lists it;
- the friends' armour and accessories are the shared KH3 pieces the rules above already give (they wear what Sora
  wears).
Keys: `party.weapons = 0` in kh-er.ini stops the friends' weapons dropping (`party.er = 0` stops the whole party).

## 11. Where usable items may come from, the flasks, pickup by every character (PU, 2026-10-18)

Your rules (2026-10-18): "make it impossible for usable items to be obtainable from plants and animals, only crafting
materials can be found on them"; "anything that isn't a dead body or an enemy or chest should not drop usable items,
equipment or accessories unless I've specifically allowed it"; "remove the two items you get replacing the flask of
crimson and cerulean tears. You get nothing instead"; every character picks up items into the shared inventory.

- **Sheet row `source.usable`** = enemy, chest, npc (gifts, quest and event awards), boss, shop. From every other source
  (gather = plants, animals, nodes; world = one-time pickups; unknown) a usable item, armour, an accessory, a friend's
  weapon or munny becomes **a synthesis material of the same tier** (`LootRules::restrict`). Kept from every source: the
  `id.*` rows and rules marked `any_source` (Golden Seed / Sacred Tear -> Megalixir), rewards (slots, max HP / MP, AP),
  key items and tools (pass).
- **How the source is read at the gate** (`LootRules::gate_source`): (1) the body played a chest animation within
  `loot.chest_window` s (default 3) = a chest; (2) `gate.callers`, now filled from your logs: 0x6e8590 gathering,
  0x56db3d an event's award, 0xea8f99 a talk's gift, 0x55c97e the treasure pickup (a corpse or a ground item); (3) a window
  of Elden Ring's has the pad = a purchase (shop); (4) the lot tables, where "enemies and the world both hold it" now
  counts as the world (the safe side; before it counted as the enemy's, which is how a gathered Rowa Fruit gave an Ether).
- **Not told apart: a placed corpse and a plain ground item** (one call site, one lot table). Both are "world" and give
  materials only. Add `"world"` to `source.usable` to let both give usable items again.
- **The flasks** (`r2.015.flask`, goods 1000 .. 1099 of goods type 0): nothing, no popup; still kept out of the inventory.
- **KH item boxes** (kill drops, the base drop) are collected by whichever party member reaches them, leader or AI ally,
  any character (kh-host `member_takes`); the item goes to the one shared stock with the item popup. Elden Ring's own
  pickups stay the leader's (the game's prompt at the hidden body, which follows the leader).
