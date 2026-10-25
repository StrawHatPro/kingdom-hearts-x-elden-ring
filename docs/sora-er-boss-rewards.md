# Sora in Elden Ring: which boss gives what (implemented)

Date 2026-10-13 (IMPL_ER31). This replaces the draft of 2026-10-12. **Built, unit-tested, seen in the test app; NOT RUN IN
ELDEN RING.** The table is the sheet `sheets/port/sora-er-boss-rewards.json` (edit it there; the game reads the file when
Sora starts). Switch: `boss.rewards` (on by default; `boss.rewards = 0` in kh-er.ini turns everything here off).

## 1. What you decided, and what is still a proposal

- **Yours:** Margit, the Fell Omen gives **Thunder**. Godrick the Grafted gives **Cure** ("heal"). **Glide** is at Maliketh.
  Small bosses give a meaningful item (section 5), and not too much too fast.
- **Proposals (everything else):** the other 45 rows, and the extra rewards beside yours on those three rows (MP Safety at
  Margit; Counter Slash and a slot at Godrick; the last Cure tier at Maliketh). Change any row in the sheet.

## 2. What a new-game Sora owns (so nothing is given twice)

Level 1, HP 105 / MP 110 (Balance), AP 28. Abilities: Scan, Dodge Roll, Air Slide, Aerial Dodge, Block, Superslide, Assist
Friends, Sonic Slash, Sonic Cleave, Buzz Saw, Somersault, Pole Spin, Pole Swing, Wall Kick, Damage Control, Magic Lock-On,
Zero EXP (owned, not worn), Treasure Magnet from the Kingdom Key. **Magic: Fire and Water, first tier.** No Cure, Blizzard,
Thunder or Aero. Three item slots, one armour slot, one accessory slot. Levelling gives 28 more ability copies and all
Strength / Magic / Defense / AP; it never gives max HP, max MP, magic or the abilities below.

## 3. The rules

- **Once per character.** Each boss's reward is recorded in Sora's save file (`.khsave`, a list `records`). A boss met
  again, a second session, Margit's second appearance outside the capital, Morgott: none of them gives Margit's reward again.
- **Magic is "one tier more of that spell".** The table names the tier you get on the usual route. If you beat a later
  source first you get the first tier there and the higher tier at the earlier boss: no order wastes a reward. Tiers
  replace each other as in KH3 (Thunder, then Thundara, then Thundaga in the same slot).
- **An ability arrives owned, not worn**, as KH3's victory bonuses do: put it on in the camp menu's Abilities screen.
- **Great Rune bosses give a slot by count:** your 1st gives an item slot, then accessory, armour, item, accessory, armour, item.
- **Shown** with KH3's own "BONUS!" panel (two lines a panel, its sound), once the boss finish is over and no cutscene or
  menu is up. The spell is in the command menu's Magic list at once.

## 4. The table

Path: required = cannot be skipped; main = on the usual route; optional = off it. Bold = magic or an ability.
The last three columns are for whoever reads the log: the row id (`boss.grant = <row id>`), the defeat flag and chr id the
mod looks for. **Every flag is from memory, not yet verified in the game** (section 7).

| # | Boss | Region | Path | Reward | Whose | Row id | Defeat flag (guess) | Chr id |
|---|---|---|---|---|---|---|---|---|
| 1 | Tree Sentinel | Limgrave | optional | Max HP +10 | proposal | `tree_sentinel` | 1042360800 | - |
| 2 | Flying Dragon Agheel | Limgrave | optional | Max HP +10 | proposal | `agheel` | 1043360800 | - |
| 3 | Margit, the Fell Omen | Limgrave (Stormveil gate) | main | **Thunder** **[USER]**; **MP Safety** | user + proposal | `margit` | 10000850 | c2130 in map area 10 (verified) |
| 4 | Leonine Misbegotten | Weeping Peninsula (Castle Morne) | optional | **Speed Slash**; Max HP +10 | proposal | `leonine` | 1043300800 | - |
| 5 | Godrick the Grafted | Limgrave (Stormveil Castle) | main | **Cure** **[USER]**; **Counter Slash**; Slot (Great Rune rule) | user + proposal | `godrick` | 10000800 | c4750 in map area 10 (guess) |
| 6 | Royal Knight Loretta | Liurnia (Caria Manor) | optional | **Blizzard**; **Rising Spiral** | proposal | `loretta_caria` | 1035500800 | - |
| 7 | Glintstone Dragon Smarag | Liurnia | optional | **Grand Magic Extender**; Max MP +5 | proposal | `smarag` | 1034450800 | - |
| 8 | Red Wolf of Radagon | Liurnia (Raya Lucaria) | main | **Aerial Recovery**; Max MP +5 | proposal | `red_wolf` | 14000850 | - |
| 9 | Rennala, Queen of the Full Moon | Liurnia (Raya Lucaria) | main | **Blizzara**; **High Jump**; Slot (Great Rune rule) | proposal | `rennala` | 14000800 | - |
| 10 | Magma Wyrm Makar | Liurnia (Ruin-Strewn Precipice) | optional | **Falling Slash**; Max HP +10 | proposal | `makar` | 39200800 | - |
| 11 | Ancestor Spirit | Siofra River | optional | **Watera**; **Treasure Magnet** | proposal | `ancestor_spirit` | 12080800 | - |
| 12 | Dragonkin Soldier of Nokstella | Ainsel River | optional | **Groundbreaker**; Max MP +5 | proposal | `dragonkin_nokstella` | 12010800 | - |
| 13 | Starscourge Radahn | Caelid | main | **Aero**; **Superjump**; Max MP +5; Slot (Great Rune rule) | proposal | `radahn` | 1252380800 | - |
| 14 | Mimic Tear | Nokron | optional | **Formchange Extender**; Max HP +5 | proposal | `mimic_tear` | 12020850 | - |
| 15 | Regal Ancestor Spirit | Nokron | optional | **Focus Syphon**; Max HP +5 | proposal | `regal_ancestor` | 12090800 | - |
| 16 | Valiant Gargoyles | Siofra Aqueduct | optional | **Waterga**; **Air Slide (extra copy)** | proposal | `valiant_gargoyles` | 12020800 | - |
| 17 | Astel, Naturalborn of the Void | Lake of Rot (Grand Cloister) | optional | **Magic Flash**; Max MP +5 | proposal | `astel` | 12040800 | - |
| 18 | Lichdragon Fortissax | Deeproot Depths | optional | **Thundaga**; Max HP +10 | proposal | `fortissax` | 12030850 | - |
| 19 | Elemer of the Briar | Altus Plateau (Shaded Castle) | optional | **Master Treasure Magnet** | proposal | `elemer` | 1039540800 | - |
| 20 | Godskin Noble | Mt. Gelmir (Volcano Manor) | optional | **Risk Dodge**; Max HP +10 | proposal | `godskin_noble` | 16000850 | - |
| 21 | Rykard, Lord of Blasphemy | Mt. Gelmir (Volcano Manor) | optional | **Fira**; **Triple Rush**; Slot (Great Rune rule) | proposal | `rykard` | 16000800 | - |
| 22 | Draconic Tree Sentinel | Capital Outskirts | main | **Thundara**; **Counter Impact** | proposal | `draconic_tree_sentinel` | 1045520800 | - |
| 23 | Godfrey, First Elden Lord (golden shade) | Leyndell | required | **Cura**; **Doubleflight** | proposal | `godfrey_shade` | 11000850 | - |
| 24 | Morgott, the Omen King | Leyndell | required | **Final Blow**; Max HP +10; Slot (Great Rune rule) | proposal | `morgott` | 11000800 | - |
| 25 | Mohg, the Omen | Leyndell (Shunning-Grounds) | optional | **Combo Master**; Max HP +10 | proposal | `mohg_omen` | 35000800 | - |
| 26 | Commander Niall | Mountaintops (Castle Sol) | optional | **Blizzaga**; **Superslide (extra copy)** | proposal | `niall` | 1051570800 | - |
| 27 | Fire Giant | Mountaintops | required | **Firaga**; **Counter Kick**; Max HP +10 | proposal | `fire_giant` | 1252520800 | - |
| 28 | Mohg, Lord of Blood | Mohgwyn Palace | optional | **Focus Syphon**; Max HP +10; Slot (Great Rune rule) | proposal | `mohg_lord` | 12050800 | - |
| 29 | Loretta, Knight of the Haligtree | Haligtree | optional | **Superslide (extra copy)**; Max MP +5 | proposal | `loretta_haligtree` | 15000850 | - |
| 30 | Malenia, Blade of Miquella | Haligtree (Elphael) | optional | **Focus Syphon**; **Air Slide (extra copy)**; Max HP +10; Slot (Great Rune rule) | proposal | `malenia` | 15000800 | - |
| 31 | Godskin Duo | Farum Azula | required | **Aerora**; **Payback Strike** | proposal | `godskin_duo` | 13000850 | - |
| 32 | Dragonlord Placidusax | Farum Azula | optional | **Aeroga**; **Grand Magic Extender**; Max HP +10 | proposal | `placidusax` | 13000830 | - |
| 33 | Maliketh, the Black Blade | Farum Azula | required | **Glide** **[USER]**; **Curaga** | user + proposal | `maliketh` | 13000800 | - |
| 34 | Sir Gideon Ofnir, the All-Knowing | Leyndell, Ashen Capital | required | Max HP +10; Max MP +5 | proposal | `gideon` | 11050850 | - |
| 35 | Godfrey, First Elden Lord / Hoarah Loux | Leyndell, Ashen Capital | required | Max HP +10; Max MP +5 | proposal | `hoarah_loux` | 11050800 | - |
| 36 | Radagon of the Golden Order / Elden Beast | Erdtree | required | Max HP +10 | proposal | `elden_beast` | 19000800 | - |
| 37 | Divine Beast Dancing Lion | Shadow of the Erdtree (Belurat) | main | **Flash Step**; Max MP +3 | proposal | `dancing_lion` | 20000800 | - |
| 38 | Rellana, Twin Moon Knight | Shadow of the Erdtree (Castle Ensis) | main | **Quick Slash**; Max MP +3 | proposal | `rellana` | 2048440800 | - |
| 39 | Golden Hippopotamus | Shadow of the Erdtree (Shadow Keep) | main | Max MP +3 | proposal | `golden_hippo` | 21000850 | - |
| 40 | Messmer the Impaler | Shadow of the Erdtree (Shadow Keep) | required | **Last Charge**; Item slot +1 | proposal | `messmer` | 21010800 | - |
| 41 | Commander Gaius | Shadow of the Erdtree | optional | **Radial Blaster** | proposal | `gaius` | 2049480800 | - |
| 42 | Putrescent Knight | Shadow of the Erdtree (Stone Coffin Fissure) | optional | Max MP +3 | proposal | `putrescent` | 22000800 | - |
| 43 | Scadutree Avatar | Shadow of the Erdtree | optional | Max MP +3 | proposal | `scadutree_avatar` | 2050480800 | - |
| 44 | Metyr, Mother of Fingers | Shadow of the Erdtree | optional | Max MP +3 | proposal | `metyr` | 25000800 | - |
| 45 | Midra, Lord of Frenzied Flame | Shadow of the Erdtree (Midra's Manse) | optional | Max MP +3 | proposal | `midra` | 28000800 | - |
| 46 | Romina, Saint of the Bud | Shadow of the Erdtree | required | **Aerial Sweep**; Max MP +3 | proposal | `romina` | 2044450800 | - |
| 47 | Bayle the Dread | Shadow of the Erdtree (Jagged Peak) | optional | **Aerial Dive** | proposal | `bayle` | 2054390800 | - |
| 48 | Promised Consort Radahn | Shadow of the Erdtree (Enir-Ilim) | required | Item slot +1 | proposal | `consort_radahn` | 20010800 | - |

Where each spell comes from (a star = required or main path):

| Spell | Tiers to give | Sources in route order |
|---|---|---|
| Thunder | 3 | Margit, the Fell Omen*, Lichdragon Fortissax, Draconic Tree Sentinel* |
| Cure | 3 | Godrick the Grafted*, Godfrey, First Elden Lord (golden shade)*, Maliketh, the Black Blade* |
| Blizzard | 3 | Royal Knight Loretta, Rennala, Queen of the Full Moon*, Commander Niall |
| Aero | 3 | Starscourge Radahn*, Godskin Duo*, Dragonlord Placidusax |
| Fire | 2 (he starts with Fire) | Rykard, Lord of Blasphemy, Fire Giant* |
| Water | 2 (he starts with Water) | Ancestor Spirit, Valiant Gargoyles |

Changes against the draft: Thunder moved to Margit and Cure to Godrick (yours); Aero went from Godrick to Radahn; the
Dragonkin Soldier of Nokstella lost its Thunder and has Groundbreaker with Max MP +5 (one MP step more than KH3 hands out:
+64 instead of +59 in all; max HP is KH3's +160); Glide moved from Morgott to Maliketh (yours) and Maliketh's Final Blow to
Morgott. Everything else is the draft's.

## 5. Small bosses (every boss that is not in the table)

Caves, catacombs, hero's graves, evergaols, field bosses, dragons, Erdtree Avatars and the like.

- **One guaranteed reward each, once per boss per character.** Either an armour piece or an accessory, picked by the item
  tier model the loot code already uses (`docs/sora-er-items.md`), or a **stat boost** (+1 Strength, Magic, Defense or AP).
- **Not too strong too fast:** the item's tier is capped by region: Limgrave and the Weeping Peninsula tier 2, Liurnia 3,
  Caelid / Altus / the capital / underground 4, Mountaintops / Farum Azula / Haligtree 5, the DLC 6. Inside the cap the
  tier rises with the boss's maximum HP (from 1,500 HP tier 2, 4,000 tier 3, 9,000 tier 4, 16,000 tier 5, 30,000 tier 6).
  You never get a piece you already hold two of (the item code's `equip.dup_max`); if every piece up to the cap is held
  twice you get an Elixir.
- **Stat boosts are limited:** exactly every third small boss you beat gives one, in the fixed turn Strength, Magic,
  Defense, AP, so no stat runs ahead. The boost is applied at once (KH3's boost items cannot be used from a menu in the
  mod yet), with a panel naming it.
- **Which piece** a boss gives depends only on that boss (the same boss, the same class of piece). **Which bosses give the
  boosts** depends on your order: it is your 3rd, 6th, 9th... small boss. That is what makes the limit exact.
- **Named bosses** (section 4) give their table reward and **no** item by this rule. They still drop what the item code
  gives any boss (its consumable drop).
- **Power budget** (boss counts are rough, from memory of the game): clearing every small boss of Limgrave and the Weeping
  Peninsula (about 25) gives about 8 boosts (+2 to each of the four stats) and about 17 pieces of tier 1 or 2. Liurnia
  (about 30): 10 boosts, 20 pieces up to tier 3. The whole base game (about 130): about 43 boosts, so about +11 to each
  stat by the end, against Sora's own 15 / 15 / 8 at level 1 and 66 / 67 / 59 at level 99. If that is too much, raise
  `small.boost_every` (5 gives about +6 each).
- Sheet rows: `small.on`, `small.tier_cap`, `small.hp_steps`, `small.boost_every`, `small.wait`. All proposals.

How a small boss is recognised, exactly: a character one of Elden Ring's three boss bars pointed at, killed by Sora's own
hit, the last such character alive, still dead two frames later (the boss finish's own test), and not claimed by a table
row within 12 seconds. **It misses** a boss that dies by anything but Sora's hit, a boss whose death starts a cutscene at
once, every boss while the boss finish is switched off, and a boss the game gives no bar. **It may wrongly include** a
table boss whose flag number in the sheet is wrong (it then gives an item instead of its table reward; the log says so
and `boss.grant` gives the row), and any non-boss the game shows a boss bar for. There is no catch-up for small bosses
beaten before this build.

## 6. Movement (closed)

Aerial Recovery at Red Wolf of Radagon, High Jump at Rennala, Superjump at Radahn, Doubleflight at Godfrey's golden shade,
**Glide at Maliketh (your decision)**: as late as KH3 gives it. He flies in the ashen capital, the DLC and after the ending.

## 7. An existing character, and what is not verified

- **Catch-up.** One second after Sora starts the mod reads every row's defeat flag in your Elden Ring save. Rows whose
  flag is set and whose reward is not recorded are given together, three seconds in, with one panel ("Rewards of N
  bosses obtained") and one log line per boss. It needs the flag numbers to be right.
- **Not verified:** all 48 flag numbers (from memory of public lists) and Godrick's chr id. Margit's chr id (c2130) was
  seen in your own session. When any boss with a bar dies, the log names the flag of that map that became set: that line
  confirms or corrects the sheet.
- **By hand:** `boss.grant = <row id>` in kh-er.ini gives one row (refused if already recorded); `boss.grant = list`
  writes every row, its flag's state and whether it is rewarded into the log.
- **Save format:** Sora's file is now version 3. An older build of the mod refuses a version-3 file (it starts a level-1
  Sora in memory and writes nothing), so do not go back to an older DLL with a character saved by this one.

## 8. Open questions

1. Are the stat boosts (one per three small bosses) the right amount?
2. Should the DLC keep KH3's six Re Mind abilities, or should they move to late base-game bosses?
3. Grand Magic Extender and Formchange Extender are handed out although their use depends on features that may not be in yet.
4. Keyblades are still not assigned to any boss.

## 9. For whoever builds on this

Table, record and grant: `crates/kh-host/src/bossrewards.rs`. Elden Ring side (flags, chr rule, catch-up, small bosses,
keys, log): `crates/kh-er/src/bossreward.rs`. Sheet writer: `build/sora_tmp/er/er31/make_sheet.js` (it rewrites the whole
sheet). State note: `build/sora_tmp/er/IMPL_ER31_STATE.md`. The 2026-10-12 draft's inventory of what KH3 hands out (94
rewards) is in `build/sora_tmp/er/boss_rewards_proposal.json`.

## 10. The friends: Donald's and Goofy's rewards (ER party S6, 2026-10-15): A DRAFT FOR YOU TO EDIT

Your answer to U3 was "yes, using a draft table the user can edit". The draft uses KH3's OWN friend victory bonuses
(`TresVBonusData`: 13 rows give Donald something, 14 give Goofy something; the two debug rows `vbonus_debug_status_*` are
left out), in KH3's story order, spread evenly over the base game's bosses in this sheet's order. The DLC's bosses give
the friends nothing (their KH3 set is used up by then).

How it works in the game:
- A friend IN THE PARTY gets the rewards of a boss this character has beaten, once (record `party:<boss>:<friend>` in
  Sora's save, next to `boss:<boss>`). The bonus panel names him ("Donald / Max HP +10, Fire Boost").
- A friend who joins later (unlocked later, or slotted in again at a save point) gets the rewards of every boss beaten
  before, at once: he is "where he should be".
- `bonus:HP_UP10` = his max HP +10; `bonus:ITEM_SLOT_UP1` / `DEF_SLOT_UP1` / `ACC_SLOT_UP1` = one more item / armour /
  accessory slot (at most 4); `ability:<name>` = one more copy of that ability, equipped while his AP allows.
- `party.rewards = 0` in kh-er.ini switches the friends' rewards off.

To edit: in `sheets/port/sora-er-boss-rewards.json`, the rows `party.<boss row id>` have `donald` and `goofy` lists; move
tokens between rows, add or remove them. NOTE: the sheet writer of section 9 rewrites the whole sheet; after running it,
run `node build/sora_tmp/er/party/s6/make_party_boss_rows.js --write` to put the friends' rows back.

| boss (row) | Donald | Goofy | KH3 source rows |
|---|---|---|---|
| Tree Sentinel (party.tree_sentinel) | MP_HASTE | LUCK_UP | donald vbonus_002, goofy vbonus_008 |
| Leonine Misbegotten (party.leonine) | HP_UP10 + FIRE_UP | HP_UP10 | donald vbonus_007, goofy vbonus_011 |
| Royal Knight Loretta (party.loretta_caria) | - | COMBI_TIME | goofy vbonus_018 |
| Glintstone Dragon Smarag (party.smarag) | COMBI_TIME | - | donald vbonus_015 |
| Rennala, Queen of the Full Moon (party.rennala) | - | HP_UP10 + LAST_LEAVE | goofy vbonus_025 |
| Magma Wyrm Makar (party.makar) | ITEM_SLOT_UP1 | - | donald vbonus_018 |
| Dragonkin Soldier of Nokstella (party.dragonkin_nokstella) | - | ACC_SLOT_UP1 | goofy vbonus_030 |
| Starscourge Radahn (party.radahn) | DAMAGE_ASPIR | - | donald vbonus_028 |
| Mimic Tear (party.mimic_tear) | - | SHARE_PRIZE | goofy vbonus_033 |
| Valiant Gargoyles (party.valiant_gargoyles) | HP_UP10 | - | donald vbonus_029 |
| Astel, Naturalborn of the Void (party.astel) | - | HP_UP10 | goofy vbonus_042 |
| Elemer of the Briar (party.elemer) | DEF_SLOT_UP1 + THUNDER_UP | - | donald vbonus_035 |
| Godskin Noble (party.godskin_noble) | - | HYPER_HEALING | goofy vbonus_044 |
| Rykard, Lord of Blasphemy (party.rykard) | HP_UP10 | - | donald vbonus_044 |
| Godfrey, First Elden Lord (golden shade) (party.godfrey_shade) | - | ITEM_SLOT_UP1 | goofy vbonus_045 |
| Morgott, the Omen King (party.morgott) | BLIZZARD_UP | - | donald vbonus_048 |
| Mohg, the Omen (party.mohg_omen) | - | DEF_SLOT_UP1 | goofy vbonus_051 |
| Fire Giant (party.fire_giant) | HYPER_HEALING | - | donald vbonus_051 |
| Mohg, Lord of Blood (party.mohg_lord) | - | CRITICAL_HALF | goofy vbonus_053 |
| Malenia, Blade of Miquella (party.malenia) | HP_UP10 | - | donald vbonus_053 |
| Godskin Duo (party.godskin_duo) | - | COMBO_LEAVE | goofy vbonus_056 |
| Maliketh, the Black Blade (party.maliketh) | ACC_SLOT_UP1 + COMBI_UP | COMBI_UP | donald vbonus_059, goofy vbonus_062 |
| Radagon of the Golden Order / Elden Beast (party.elden_beast) | AERO_UP | HP_UP10 | donald vbonus_065, goofy vbonus_065 |

