# Sora, as he is now (FINAL10 build)

This page describes Sora in the KH3 Sora → Elden Ring mod **as deployed now**: the FINAL10 build (`build/sora_tmp/er/final10/kh_er_af5718317bd7.dll`, deployed 2026-10-18 22:39). It was built from `build/sora_tmp/er/final10/ws`, and that folder's sheets and code are the source for everything below.

This page is documentation only. No code, sheet, build or game file was changed, and nothing was run to write it.

**Labels used on this page**

| Label | Meaning |
|---|---|
| **Implemented** | In FINAL10's sheets and code. Many things are still **untested in game**; section 9 lists them. |
| **Planned** | Decided (by you, or approved in a design note) but not in the build. |
| **Potential (suggestion)** | Not decided anywhere. It is my suggestion, tied to Elden Ring progression, for you to accept or change. |
| *[user]* | A reward you picked yourself. |
| *[XS]* | One of the 16 new Sora abilities (XS design, approved 2026-10-16). |
| *proposal* | Built and active, but marked in the sheet as "a proposal, not confirmed by the user". |

Short source names: `sheets/` = `final10/ws/sheets/port/`, `notes/` = `build/sora_tmp/party/` or `build/sora_tmp/er/`.

---

## 1. Overview

**Who he is.** Sora is the KH3 Sora, his combat ported from the game's own code and data. He runs inside Elden Ring in place of the Tarnished:
- He is drawn by the mod's own renderer.
- He hits Elden Ring enemies with KH3 combos, magic and juggles.
- He saves at KH3 save points, which replace the sites of grace.

He is one of three playable characters, with Donald and Goofy (section 8).

**Choosing who to start as (Implemented).** The new game asks "Whose journey will you begin?" (Sora / Donald / Goofy; `sheets/party-er-unlocks.json`).
- If you pick Sora, he starts at level 1.
- If you pick someone else, Sora is found fighting Margit and joins when Margit is beaten (`ER_PHASE_NOTES.md` U2).

**The opening's two KH3 questions (Implemented).** These are KH3's own (`kh-core/src/status/newgame.rs`). The default pair, for a host that does not ask, is Balance + Warrior.

| Question | Choices | What it changes |
|---|---|---|
| "What do you desire?" | Vitality / Balance / Wisdom | Start HP / MP (Standard): 120/100, 105/110, 90/120 |
| "What power do you seek?" | Warrior / Mystic / Guardian | The level-up ability path: A / B / C (section 4.2) |

**Controls (Implemented).** These are KH3's layout on an Xbox pad (`sheets/sora-input-layout.json`); the keyboard is an extra path.

| Pad | Action |
|---|---|
| Left stick / right stick | Move / camera (R3 resets the camera) |
| **A** | Attack; confirm in menus; advances dialogue while talking, except in combat |
| **B** | Jump (hold for Glide once he has it); cancel |
| **X** | Block; with the stick, Dodge Roll; in the air, Aerial Dodge / Air Slide |
| **Y** | Situation command: Second Form, Grand Magic, team attacks, finishers, reprisals |
| **LB + A/B/X/Y** | Shortcuts: spells and items, set in Customize (3 sets of 4) |
| **RB** | Tap: lock on. Hold: Shotlock aim |
| **RT** | Tap: change target |
| **LT** | Next situation command |
| **D-pad** | Command menu: Attack / Magic / Items. Link is disabled: Sora has no Links in the port. |
| **Menu** | Pause (the KH3 menu) |

**Difficulty (Implemented)** (`IMPL_DF_STATE.md` revision of 2026-10-16; `sheets/sora-er-host.json` row `difficulty.new_game` = `STANDARD`).
- **Standard is the default** for new games. On Standard, everything is as before the difficulty work:
  - spell MP costs ×1.5 (`magic.mp_cost_scale`);
  - MP Charge 50 s;
  - no Critical abilities at the start.
- **Critical is an option.** Set `difficulty = critical` in the ctl file before starting a new game. Critical gives:
  - halved base HP / MP (Balance: HP 52 / MP 55);
  - AP +30 (58 at level 1);
  - Elden Ring hits on him ×4 of the Standard share;
  - EXP ×0.75;
  - MP Charge 70 s;
  - KH3's normal spell MP cost (×1.0).
- **No Critical Counter at the start**, even on Critical (your decision dec032). He starts on Critical with Critical Recharge and Critical Converter, plus KH3's 13 Critical start abilities. 54 of his 58 AP are in use.
- Existing characters keep their difficulty. A switch exists only as a ctl key, off by default (`difficulty.existing = switch`).
- **Up to 5 enemies may attack at once.** This applies on every difficulty, through Elden Ring's own group-attack limiter (`enemy.max_attackers` = 5; `IMPL_ENG_STATE.md`, `kh-er/src/attackers.rs`).

---

## 2. Stats

**Where his stats come from** (`sheets/sora-levels.json`, `sora-newgame.json`, `sora-equipment.json`):
- **Strength, Magic, Defense, AP** come from the level table, plus the Keyblade and gear. The Kingdom Key adds Strength +4 and Magic +3.
- **Max HP and MP do NOT grow with levels** (as in KH3). They grow only from boss rewards, some converted Elden Ring items and roster boosts (section 7).

**Key levels, Standard** (AP on Critical = +30). Strength and Magic include the Kingdom Key's +4 / +3; Defense is with no armour.

| Level | Total EXP | Strength | Magic | Defense | AP |
|---|---|---|---|---|---|
| 1 | 0 | 15 | 15 | 8 | 28 |
| 10 | 5,410 | 19 | 20 | 12 | 40 |
| 20 | 29,440 | 24 | 25 | 18 | 62 |
| 30 | 87,850 | 29 | 31 | 23 | 76 |
| 40 | 197,560 | 35 | 36 | 28 | 90 |
| 50 | 376,030 | 40 | 41 | 34 | 118 |
| 60 | 599,450 | 45 | 47 | 39 | 134 |
| 70 | 828,820 | 50 | 52 | 44 | 148 |
| 80 | 1,064,140 | 56 | 57 | 49 | 162 |
| 90 | 1,305,410 | 61 | 62 | 55 | 174 |
| 99 | 1,527,640 | 66 | 67 | 59 | 180 |

**HP / MP by bosses beaten** (Balance start; Standard / Critical). These are the DF note's offline numbers, from the boss table's HP / MP steps (`IMPL_DF_STATE.md` table 4).

| Bosses beaten | Max HP | Max MP |
|---|---|---|
| 0 | 105 / 52 | 110 / 55 |
| 9 | 135 / 67 | 120 / 59 |
| 24 | 185 / 91 | 135 / 65 |
| all 48 | 265 / 131 | 174 / 79 |

The other fixed numbers: Focus 100 on every difficulty. At the start: 3 item slots, 1 armour slot, 1 accessory slot.

**How EXP and levels work here (Implemented).**
- A defeated Elden Ring enemy gives `ceil(its max HP × 0.03)` KH3 EXP (`exp.per_max_hp`). On Critical it is ×0.75. KH3's cap of 999 per kill applies (`IMPL_DF_STATE.md`).
- EXP goes into **one shared party total**. Each character reads his level from that total with his own level table (`kh-partyhost/src/progression.rs`).
- **Zero EXP belongs to the party leader**, whoever leads. Its on/off state carries over when you change leader (your rule).
- Level-ups give Strength / Magic / Defense / AP and the path's ability (section 4.2).
- Elden Ring's own rune levelling is not part of Sora's progression. Runes become munny (section 7).

---

## 3. Combat kit

| Part | What is in FINAL10 | Status |
|---|---|---|
| **Ground combo** | KH3's Keyblade combo: normal, approach (far) and crowd hits, then the finisher. Combo Plus copies lengthen it. | Implemented (`sheets/sora-combat-attacks.json` kinds 1-4) |
| **Air combo** | KH3's air combo: normal, approach, crowd, falling-attack and finisher hits. Air Combo Plus lengthens it. | Implemented (kinds 0x0F-0x13) |
| **Finishers** (ground and air) | Basic finisher. With the abilities: Speed Slash, Triple Rush, Magic Flash, Last Charge, Falling Slash. XS finishers: Spiral Rush, Zantetsuken, Event Horizon (replaces Magic Flash), Giant Stamp, Crimson Blast (Riskcharge). | Implemented |
| **Re Mind combo swaps** | Quick Slash, Flash Step, Radial Blaster, Aerial Sweep and Aerial Dive replace combo steps when equipped | Implemented (DLC bosses give them) |
| **Launchers** | Rising Spiral (ground), Groundbreaker (air). Air juggles on Elden Ring enemies are limited by the "not infinitely stunnable" rule: 3 reactions in 10 s, then 8 s with none (`sora-er-host.json` `react.chain.*`). | Implemented |
| **Guard** | Block (X). Reprisals after a block: Counter Slash, Counter Impact, Counter Kick, Final Blow. Payback Strike after Aerial Recovery. | Implemented |
| **Critical Counter** | A block in the last 0.1333 s before a hit makes the reprisal deal ×2. Not given at the start (section 4). | Implemented |
| **Dodge** | Dodge Roll, Aerial Dodge, Air Slide, Superslide; Risk Dodge with its four counters | Implemented |
| **Movement** | High Jump, Doubleflight, Superjump and Glide, all boss rewards. Ladders (LADDER2, 2026-10-25): press Y at a ladder's Climb prompt and the party jumps up the whole ladder in one leap, with Torrent's wind sound, and lands where the Tarnished would step off at the top, facing the same way. There is no climbing down: just drop off the ladder (the Descend prompt is disabled). No fall damage (XS notes: "already the mod's rule"). | Implemented |
| **Flowmotion / Athletic Flow** | The states are in the shared core: airstep travel, flow attacks Sonic Slash / Sonic Cleave / Buzz Saw / Somersault, Wall Kick, wall run, rail slide, pole spin and pole swing. Elden Ring's world has no KH3 flow markers (`SQEX_bEnableWallRun` etc.), so wall run, rail and pole moves have not occurred in Elden Ring (`IMPL_ER7`, `IMPL_ER58`). Whether airsteps to enemies trigger in Elden Ring: **unknown**. | Ported in core; in Elden Ring effectively absent / unknown |
| **Shotlocks** (hold RB) | Kingdom Key's two (sheet names: "King of Hearts", "Ragnarok"). Spectral Rays *[XS]* is picked in Customize "Shotlocks". Your nerf applies: damage ×0.5, at most 40 % of the target's max HP per shotlock, 3 s gap. | Implemented |
| **Situation commands** (Y) | Second Form, Grand Magic, team attacks, form finishers, Riskcharge, Ancient Light | Implemented |
| **Second Form** (formchange) | The Kingdom Key's only formchange. The situation gauge fills from hits; when it fills, KH3's `RollGrandMagic` rule decides whether Second Form is offered beside Grand Magic. Critical Converter makes it always offered. Form finishers: Stun Impact / Sonic Blade / Ars Arcanum; form combo finishers: Explosion, Stride Break, Ripple Drive, Hurricane Period, Aerial Finish, Magnet Splash. Formchange Extender lengthens it. Rage Form (anti form) is **not** in the port. | Implemented |
| **Grand Magic** | Offered on Y when the gauge fills from magic hits. The "-za" tiers need the matching ability (Firaza, Blizzaza, Thundaza, Waterza, Aeroza from the Cufflink accessories). No MP cost. | Implemented |
| **Team attacks: Donald** | Unison Fire (level 8). Unison Blizzard and Unison Thunder: **no source**, see section 9. Donald Meteor, Flare Force. | Partly implemented |
| **Team attacks: Goofy** | Fusion Spin (level 10), Fusion Rocket (Leonine Misbegotten), Goofy Bombardier, Trinity Guard (the group Shield Guardian; you kept it) | Implemented |
| **Links** (summons) | None. The Link entry of the command menu is disabled (`kh-core/src/command/mod.rs`). | Not in the port |
| **Attraction Flow** | **Excluded by your decision.** Attraction Extender and Attraction Enhancer were removed and their level slots refilled (X1; `IMPL_X1_STATE.md`). | Excluded |

---

## 4. Abilities: when and how he gets each

AP is the sheet's cost to equip. A boss reward or a level gives one **owned** copy; you equip it on the Abilities screen while your AP allows.

Sources: `sheets/sora-abilities.json`, `sora-levels.json`, `sora-newgame.json`, `sora-er-boss-rewards.json`, `sora-equipment.json`, `sora-er-items.json`. Boss rewards are once per character.

### 4.1 At the start (Implemented)

| Ability | AP | Notes |
|---|---|---|
| Scan, Dodge Roll, Air Slide, Aerial Dodge, Block, Superslide, Assist Friends | 1, 3, 1, 3, 2, 1, 1 | Equipped |
| Sonic Slash, Sonic Cleave, Buzz Saw, Somersault (flow attacks) | 2 each | Equipped |
| Pole Spin, Pole Swing, Wall Kick | 1 each | Equipped |
| Damage Control, Magic Lock-On | 4, 1 | Equipped |
| Zero EXP | 0 | Owned. It belongs to whoever leads. |
| Treasure Magnet | 0 | From the Kingdom Key itself |
| Critical only: Critical Recharge (3) equipped; Critical Converter (1) owned; KH3's 13 Critical start abilities (Aerial Recovery, Counter Slash, Rising Spiral, Counter Impact, Combo Master, Groundbreaker, Risk Dodge, Air Slide ×2, Superslide ×2, Counter Kick, Final Blow) | - | **No Critical Counter** (dec032) |

### 4.2 Level-ups, by path (Implemented)

A = Warrior, B = Mystic, C = Guardian (`sheets/sora-levels.json`). The ★ rows are the X1 slots refilled with XS abilities.

| Level | A (Warrior) | B (Mystic) | C (Guardian) |
|---|---|---|---|
| 2 | Combo Plus | Magic Combo Thrift | Damage Control |
| 3 | Treasure Magnet | Treasure Magnet | Treasure Magnet |
| 4 | Air Combo Plus | MP Haste | Withstand Combo |
| 5 | Auto-Finish | Auto-Finish | Auto-Finish |
| 6 | MP Haste | Combo Plus | Combo Plus |
| 8 | Unison Fire | Unison Fire | Unison Fire |
| 9 | Combo Plus | Fire Boost | Block Replenisher |
| 10 | Fusion Spin | Fusion Spin | Fusion Spin |
| 12 ★ | **Spiral Rush** *[XS]* | Water Boost | MP Haste |
| 14 | Fire Boost | Leaf Bracer | Air Combo Plus |
| 16 | Air Combo Plus | Air Combo Plus | Fire Boost |
| 18 ★ | Water Boost | Blizzard Boost | **Critical Counter** *[XS]* |
| 20 | Combo Boost | Grand Magic Extender | Reprisal Boost |
| 22 ★ | **Diving Strike** *[XS]* | **Diving Strike** *[XS]* | **Diving Strike** *[XS]* |
| 24 | Air Combo Boost | Reprisal Boost | Combo Plus |
| 26 | Withstand Combo | Magic Galvanizer | Leaf Bracer |
| 28 | Leaf Bracer | Withstand Combo | Water Boost |
| 30 | Blizzard Boost | Combo Plus | Second Chance |
| 32 | Link Extender | Damage Control | Air Combo Plus |
| 34 | Thunder Boost | Thunder Boost | Link Extender |
| 36 | Block Replenisher | Block Replenisher | Combo Boost |
| 38 | Aero Boost | Aero Boost | Blizzard Boost |
| 40 | Reprisal Boost | Air Combo Plus | Thunder Boost |
| 42 | Grand Magic Extender | Second Chance | Air Combo Boost |
| 44 ★ | Magic Combo Thrift | **Light Wave** *[XS]* | Grand Magic Extender |
| 46 | Magic Galvanizer | Combo Boost | Magic Combo Thrift |
| 48 | Second Chance | Link Extender | Aero Boost |
| 50 | Damage Control | Air Combo Boost | Magic Galvanizer |

Nothing is learned after level 50. Each path holds 28 level abilities.

### 4.3 Boss rewards: abilities, magic, HP / MP, slots (Implemented)

**The rules** (`kh-host/src/bossrewards.rs`):
- Each row gives once per character when the boss's defeat flag is seen. Only you fixed the *[user]* items; every other item is a *proposal* (built and active, not confirmed by you).
- **Magic gives "one tier more of that element"**, whatever tier the row names. Beat the bosses in any order and you still go up one tier at a time.
- A *[XS]* ability gives nothing if he already owns it from his level path ("skip if owned").
- The "Great Rune order" slots go: item, accessory, armour, item, accessory, armour, item. The Nth such boss gives the Nth slot.
- **The flag ids are guesses** (section 9). `boss.grant = <row>` in the ctl gives a row by hand.

| # | Boss | Region | Path | Sora's reward |
|---|---|---|---|---|
| 1 | Tree Sentinel | Limgrave | optional | Max HP +10; Spiral Rush (3 AP) *[XS]* |
| 2 | Flying Dragon Agheel | Limgrave | optional | Max HP +10; Sonic Dash (3 AP) *[XS]* |
| 3 | Margit, the Fell Omen | Limgrave (Stormveil gate) | main | **Thunder** *[user]*; MP Safety (0 AP) |
| 4 | Leonine Misbegotten | Weeping Peninsula (Castle Morne) | optional | Speed Slash (3 AP); Max HP +10; Fusion Rocket (3 AP) |
| 5 | Godrick the Grafted | Limgrave (Stormveil Castle) | main | **Cure** *[user]*; Counter Slash (4 AP); slot (Great Rune order) |
| 6 | Royal Knight Loretta | Liurnia (Caria Manor) | optional | **Blizzard**; Rising Spiral (3 AP) |
| 7 | Glintstone Dragon Smarag | Liurnia | optional | Grand Magic Extender (3 AP); Max MP +5; Spectral Rays (3 AP) *[XS]* |
| 8 | Red Wolf of Radagon | Liurnia (Raya Lucaria) | main | Aerial Recovery (2 AP); Max MP +5 |
| 9 | Rennala, Queen of the Full Moon | Liurnia (Raya Lucaria) | main | **Blizzara**; High Jump (2 AP); slot (Great Rune order) |
| 10 | Magma Wyrm Makar | Liurnia (Ruin-Strewn Precipice) | optional | Falling Slash (2 AP); Max HP +10 |
| 11 | Ancestor Spirit | Siofra River | optional | **Watera**; Treasure Magnet (1 AP) |
| 12 | Dragonkin Soldier of Nokstella | Ainsel River | optional | Groundbreaker (3 AP); Max MP +5; Break Boost (4 AP) *[XS]* |
| 13 | Starscourge Radahn | Caelid | main | **Aero**; Superjump (2 AP); Max MP +5; slot (Great Rune order) |
| 14 | Mimic Tear | Nokron | optional | Formchange Extender (3 AP); Max HP +5; Hidden Potential (5 AP) *[XS]* |
| 15 | Regal Ancestor Spirit | Nokron | optional | Focus Syphon (3 AP); Max HP +5 |
| 16 | Valiant Gargoyles | Siofra Aqueduct | optional | **Waterga**; Air Slide (1 AP) |
| 17 | Astel, Naturalborn of the Void | Lake of Rot (Grand Cloister) | optional | Magic Flash (5 AP); Max MP +5; Light Wave (4 AP) *[XS]* |
| 18 | Lichdragon Fortissax | Deeproot Depths | optional | **Thundaga**; Max HP +10 |
| 19 | Elemer of the Briar | Altus Plateau (Shaded Castle) | optional | Master Treasure Magnet (5 AP); Giant Stamp (5 AP) *[XS]* |
| 20 | Godskin Noble | Mt. Gelmir (Volcano Manor) | optional | Risk Dodge (3 AP); Max HP +10; Critical Counter (5 AP) *[XS]* |
| 21 | Rykard, Lord of Blasphemy | Mt. Gelmir (Volcano Manor) | optional | **Fira**; Triple Rush (4 AP); slot (Great Rune order) |
| 22 | Draconic Tree Sentinel | Capital Outskirts | main | **Thundara**; Counter Impact (4 AP) |
| 23 | Godfrey (golden shade) | Leyndell | required | **Cura**; Doubleflight (2 AP) |
| 24 | Morgott, the Omen King | Leyndell | required | Final Blow (5 AP); Max HP +10; slot (Great Rune order); Ancient Light (6 AP) *[XS]* |
| 25 | Mohg, the Omen | Leyndell (Shunning-Grounds) | optional | Combo Master (4 AP); Max HP +10; Rage Raid (4 AP) *[XS]* |
| 26 | Commander Niall | Mountaintops (Castle Sol) | optional | **Blizzaga**; Superslide (1 AP) |
| 27 | Fire Giant | Mountaintops | required | **Firaga**; Counter Kick (4 AP); Max HP +10 |
| 28 | Mohg, Lord of Blood | Mohgwyn Palace | optional | Max HP +10; slot (Great Rune order); Riskcharge (5 AP) *[XS]* (replaced his Focus Syphon) |
| 29 | Loretta, Knight of the Haligtree | Haligtree | optional | Superslide (1 AP); Max MP +5; Chandelier (3 AP) *[XS]* |
| 30 | Malenia, Blade of Miquella | Haligtree (Elphael) | optional | Focus Syphon (3 AP); Air Slide (1 AP); Max HP +10; slot (Great Rune order) |
| 31 | Godskin Duo | Farum Azula | required | **Aerora**; Payback Strike (5 AP) |
| 32 | Dragonlord Placidusax | Farum Azula | optional | **Aeroga**; Grand Magic Extender (3 AP); Max HP +10 |
| 33 | Maliketh, the Black Blade | Farum Azula | required | **Glide** *[user]*; **Curaga** |
| 34 | Sir Gideon Ofnir | Leyndell, Ashen Capital | required | Max HP +10; Max MP +5 |
| 35 | Godfrey / Hoarah Loux | Leyndell, Ashen Capital | required | Max HP +10; Max MP +5 |
| 36 | Radagon / Elden Beast | Erdtree | required | Max HP +10 |
| 37 | Divine Beast Dancing Lion | DLC (Belurat) | main | Flash Step (3 AP); Max MP +3 |
| 38 | Rellana, Twin Moon Knight | DLC (Castle Ensis) | main | Quick Slash (3 AP); Max MP +3; Zantetsuken (4 AP) *[XS]* |
| 39 | Golden Hippopotamus | DLC (Shadow Keep) | main | Max MP +3 |
| 40 | Messmer the Impaler | DLC (Shadow Keep) | required | Last Charge (7 AP); item slot +1 |
| 41 | Commander Gaius | DLC | optional | Radial Blaster (3 AP) |
| 42 | Putrescent Knight | DLC (Stone Coffin Fissure) | optional | Max MP +3 |
| 43 | Scadutree Avatar | DLC | optional | Max MP +3 |
| 44 | Metyr, Mother of Fingers | DLC | optional | Max MP +3; Event Horizon (4 AP) *[XS]* |
| 45 | Midra, Lord of Frenzied Flame | DLC (Midra's Manse) | optional | Max MP +3 |
| 46 | Romina, Saint of the Bud | DLC | required | Aerial Sweep (2 AP); Max MP +3 |
| 47 | Bayle the Dread | DLC (Jagged Peak) | optional | Aerial Dive (2 AP) |
| 48 | Promised Consort Radahn | DLC (Enir-Ilim) | required | item slot +1; Myriad (7 AP) *[XS]* |

On Critical, a boss ability that Critical already gave at the start is skipped; the boss's other rewards still come.

**Small bosses** (any boss-bar enemy not in the table above) each give one capped reward, once (`small.*` rows, proposal):
- every 3rd: a permanent +1 to Strength, Magic, Defense and AP in turn, for the whole roster;
- the others alternate between a recovery item (Hi-Potion up to Megalixir by tier) and an armour piece or accessory;
- the tier is capped by region.

### 4.4 The 16 new abilities (XSI, Implemented)

Design: `XS_SORA_ABILITIES.md`. Build state: `IMPL_XSI_STATE.md`, Elden Ring hookup `IMPL_ER90_STATE.md`. All 16 are in FINAL10; the form weapons and effects are drawn in Elden Ring.

| Ability | AP | How he gets it | What it is (pad) |
|---|---|---|---|
| Diving Strike | 2 | Level 22, every path | KH3 plunge: A while falling, with ≥ 6 m of drop and an enemy below. Damage grows with the fall. |
| Sonic Dash | 3 | Flying Dragon Agheel | In the air, locked on 4-15 m: X + stick at the target. Dashes through; A after the 2nd dash launches. |
| Spiral Rush | 3 | Level A 12, or Tree Sentinel | Light Form drill charge finisher, when the target is 5-12 m away |
| Zantetsuken | 4 | Rellana (DLC) | Hold A on the finisher step: a charged cut (up to ×1.6) |
| Event Horizon | 4 | Metyr (DLC) | Light-ring magic finisher (replaces Magic Flash, scales with Magic) |
| Giant Stamp | 5 | Elemer of the Briar | Hyper Hammer slam finisher against big foes. The hammer is shown for the move. |
| Chandelier | 3 | Loretta, Knight of the Haligtree | Blizzard finish becomes Blizzard Blades' ice chandelier |
| Spectral Rays | 3 | Glintstone Dragon Smarag | A second shotlock: refracting lasers. Pick it in Customize. |
| Light Wave | 4 | Level B 44, or Astel | During MP Charge: tap for a piercing wave, hold for a 6-wave volley |
| Critical Counter | 5 | Level C 18, or Godskin Noble | The just-block (×2 reprisal). **Not** at the start on any difficulty. |
| Rage Raid | 4 | Mohg, the Omen | Counter when a boss breaks out of your juggle (its revenge) |
| Riskcharge | 5 | Mohg, Lord of Blood | Situation command: pay HP, then Crimson Blast |
| Ancient Light | 6 | Morgott | Second Form finisher outside the form (capped at 25 % of the target's max HP) |
| Myriad | 7 | Promised Consort Radahn (DLC) | Second Form ends in Ultimate Form's finale (25 % cap) |
| Hidden Potential | 5 | Mimic Tear | Passive: more Strength / Magic against stronger foes (by region band) |
| Break Boost | 4 | Dragonkin Soldier of Nokstella | Passive: breaks a boss's guard faster and keeps it open longer |

### 4.5 Abilities that come only from gear (Implemented, through loot)

Armour and accessories drop from enemies, corpses / world pickups, chests and merchants, by tier and region (`sheets/sora-er-items.json`). The 60 variant necklaces and medals (`ACC_ITEM51`-`110`) never drop.

| Ability | Worn on (droppable pieces) |
|---|---|
| Fire / Blizzard / Thunder / Water / Aero Boost (also level-ups) | Cufflinks, Celestriad, Yin-Yang Cufflink, Master / Star Medal |
| Firaza, Blizzaza, Thundaza, Waterza, Aeroza (the Grand Magic -za tiers) | The matching Cufflink |
| MP Haste / Hastera / Hastega | Sorcerer's Ring, Necklaces / Flanniversary Badge / Mickey Clasp, Crystal Regalia |
| MP Thrift, Extra Cast, Full MP Blast, Endless Magic | Flanniversary Badge, Rune Ring, Force Ring, Mickey Clasp |
| Damage Syphon | Chains, Aqua Chaplet, Aero Armlet, Moon Amulet, Star Charm, Cosmic Arts |
| Element Syphons (Fire, Blizzard, Thunder, Water, Aero, Dark) | Bangles, Chokers, Trinkets, Bandannas, Fans, Anklets |
| Burn / Cloud / Sneeze / Freeze / Electric Protection | The Rosettes |
| Defender | Soldier's / Fencer's Earring |
| Lucky Strike, Wizard's Ruse, Item Boost | Lucky Ring, Wisdom Ring, Hero's Glove |
| Magic Treasure Magnet, Master Treasure Magnet (also a boss) | Mage's / Slayer's Earring, Draw Ring |
| MP Walker, HP Walker | Laughter Pin, Forest Clasp |
| HP / MP / Munny Converter | Bronze / Silver / Gold Amulet |
| Cure Converter | Buster, Valor, Phantom, Orichalcum Ring, Breakthrough |
| Harvester, Chef Extraordinaire | Gourmand's Ring |
| Frontline Photographer | Hero's Belt |

### 4.6 Abilities with no source in the mod now

| Ability | Why there is no source |
|---|---|
| Unison Blizzard, Unison Thunder | Decided (XD decision 3) for Royal Knight Loretta and the Draconic Tree Sentinel, but **missing from the boss sheet** (section 9) |
| Focus Converter, Stun Protection, Situation Boost, Magic Roulette | Only on Keyblades the mod cannot give (section 6) |
| Critical Recharge, Critical Converter | Critical start only; nothing gives them on Standard |
| Berserk Charge, Curaza and the KH3 cuisine abilities (Hearty Meal etc.) | Cooking is not in the mod; their accessory rows are excluded variants |
| Attraction Extender, Attraction Enhancer | **Excluded** (your decision; X1) |

---

## 5. Magic

**The spells.** Sora has six elements, each with three tiers (`kh-core/src/magic/spells.rs`; names from KH3's strings): Fire / Fira / Firaga, Blizzard / Blizzara / Blizzaga, Thunder / Thundara / Thundaga, Water / Watera / Waterga, Aero / Aerora / Aeroga, Cure / Cura / Curaga.
- Grand Magic adds the -za versions (Firaza ... Aeroza) as situation commands.
- MP costs are KH3's: Fire costs 10, per the DF note. On Standard they are ×1.5. MP Charge after running dry lasts 50 s (Critical: 70 s, ×1.0 costs).

**When each tier unlocks (Implemented).**
- Each boss below gives **one tier more** of its element, whichever of that element's bosses you beat first (`kh-host/src/bossrewards.rs`).
- KH3's prologue gives Fire and Water at tier 1 (`sheets/sora-newgame.json` `magic.known`).

| Element | Tier 1 | Tier 2 | Tier 3 | The element's bosses | Extra boss (Planned, shared with Donald) |
|---|---|---|---|---|---|
| Fire | start | 1st boss | 2nd boss | Rykard, Fire Giant | Magma Wyrm Makar |
| Water | start | 1st boss | 2nd boss | Ancestor Spirit, Valiant Gargoyles | Dragonkin Soldier of Nokstella |
| Thunder | 1st boss | 2nd boss | 3rd boss | Margit *[user]*, Draconic Tree Sentinel, Lichdragon Fortissax | Ancient Dragon Lansseax |
| Cure | 1st boss | 2nd boss | 3rd boss | Godrick *[user]*, Godfrey (golden shade), Maliketh | Morgott |
| Blizzard | 1st boss | 2nd boss | 3rd boss | Royal Knight Loretta, Rennala, Commander Niall | Borealis the Freezing Fog |
| Aero | 1st boss | 2nd boss | 3rd boss | Starscourge Radahn, Godskin Duo, Dragonlord Placidusax | Red Wolf of Radagon |

The table's own order gives Fira at Rykard, Waterga at the Gargoyles and so on, but beating them out of order never skips a tier.

**Planned (user decision, with the Donald overhaul; not implemented yet):** each element gets the **extra boss** in the last column, shared with Donald (`documentation/DONALD_OVERHAUL.md`, section 9). Sora takes his next tier from **any** of that element's bosses. If he **already has the -ga tier** of that element, the boss gives him **Max MP +3** instead. Sora **only** gets something from a boss of his own magic: bosses that give Donald something Sora doesn't have (Flare and the other line-2 spells, the ultimates, the Arcane Y upgrades, Zettaflare) give Sora nothing.

**Grand Magic (Implemented).** Offered when the situation gauge fills from magic hits (`kh-core/src/magic/grand.rs`, KH3's `RollGrandMagic`). Two of its tiers need something extra:
- **-za tiers**: they need the element's Cufflink ability (section 4.5).
- **Curaza**: it needs Cure Converter, and Curaza has no source (section 4.6).

**Donald (Implemented).** Donald uses the same spells and tiers as Sora while you play as Donald (your decision; stream X2).

---

## 6. Keyblades

### 6.1 What is implemented now

**Only the Kingdom Key can be held** (`sheets/sora-newgame.json`, `sora-equipment.json`).

| Keyblade | Strength / Magic | Ability | Formchange | Shotlocks | How he gets it |
|---|---|---|---|---|---|
| **Kingdom Key** | +4 / +3 | Treasure Magnet | **Second Form** | "King of Hearts", "Ragnarok" | Start weapon (keyblade slot 0; slots 1 and 2 empty) |

What else exists:
- **Kingdom Key +1 to +10** exist as upgrade rows (+10: Strength 9 / Magic 8, adds Formchange Extender from +4). The mod has **no upgrade or synthesis system**: materials are "kept, nothing uses them" (`kh-host/src/items.rs`). So the upgrades cannot be reached.
- **Form weapons shown briefly (XS moves, not equippable):**
  - Hyper Hammer (Favorite Deputy's form) during Giant Stamp;
  - Blizzard Blades (Crystal Snow's form) during Chandelier;
  - the Ultimate Form weapon (Ultima Weapon) during Myriad.

  The Kingdom Key is hidden meanwhile (`IMPL_ER90_STATE.md`).
- **Elden Ring weapons never become Keyblades.** A picked-up weapon becomes munny, a synthesis material or an accessory (`sora-er-items.json` rules `r2.7xx`). By contrast, staves / seals and shields can become Donald's and Goofy's weapons (`party.weapons`).

### 6.2 What is planned or decided

**Nothing is decided for Keyblades:**
- `docs/sora-er-boss-rewards.md` open question 4: "Keyblades are still not assigned to any boss."
- The XS design dropped Double Form (two Keyblades at once) as "a later proposal if wanted".

### 6.3 The other Keyblades in the data, with a potential source

These 16 have full stat rows in `sora-equipment.json` and models / form variants in `sora-weapon.json`, but **no way to get them**.
- Formchange names are given only where this project's notes name them; "unknown" means the data holds the form but its name is not mapped here.
- The KH3 Formchange Extender upgrades and the keyblade-only abilities above would come with these.

| Keyblade | Str / Mag | Ability | Formchange | **Potential (suggestion):** where to get it |
|---|---|---|---|---|
| Hero's Origin | 5 / 2 | Defender | unknown | Godrick the Grafted: the first demigod, the hero's first great wall |
| Shooting Star | 3 / 5 | Magic Treasure Magnet | unknown | Rennala, Queen of the Full Moon: stars and sorcery, early magic Keyblade |
| Starlight | 4 / 4 | MP Haste | unknown (has a form-05 variant) | Early and cheap: sold by Merchant Kalé (Church of Elleh) |
| Favorite Deputy | 5 / 3 | Lucky Strike | Hyper Hammer / Drill Punch | Elemer of the Briar, so it matches Giant Stamp's hammer from the same boss |
| Ever After | 2 / 6 | Leaf Bracer | unknown | Glintstone Dragon Smarag, so it matches Spectral Rays ("Ever After's lasers") |
| Happy Gear | 6 / 2 | Focus Converter | unknown | Royal Knight Loretta (Caria Manor) |
| Hunny Spout | 4 / 4 | Harvester | unknown | Ancestor Spirit (Siofra River) |
| Nano Gear | 5 / 4 | Stun Protection | unknown | Valiant Gargoyles (Siofra Aqueduct): constructs for a tech Keyblade |
| Wheel of Fate | 6 / 4 | Waterza | unknown | Dragonkin Soldier of Nokstella (Ainsel River, water) |
| Crystal Snow | 3 / 6 | Freeze Protection | Blizzard Blades / Blizzard Claws | Commander Niall (snowy Mountaintops) |
| Grand Chef | 4 / 6 | Wizard's Ruse | unknown | A merchant: the Twin Maiden Husks (Roundtable Hold) |
| Classic Tone | 6 / 9 | MP Haste | unknown | Astel, Naturalborn of the Void: a late magic Keyblade |
| Oathkeeper | 7 / 7 | MP Converter | Light Form (Re Mind); Double Form with Oblivion | Morgott, the Omen King, beside Ancient Light |
| Oblivion | 8 / 6 | HP Converter | Dark Form (Re Mind); Double Form with Oathkeeper | Maliketh, the Black Blade |
| Ultima Weapon | 8 / 3 | Situation Boost, Combo Boost, Air Combo Boost | Ultimate Form | Radagon / Elden Beast (the last boss), or Malenia as the hardest optional fight |
| Dead of Night | 4 / 4 | Lucky Strike, Magic Roulette | unknown (has a form-05 variant) | DLC: Midra, Lord of Frenzied Flame |

**Potential (suggestion), building on these:**
- **Equipping:**
  - only Second Form is ported, so start each new Keyblade with its stats and ability only, without its formchange;
  - add each formchange later as its own job;
  - turn on keyblade slots 1-2 so you can switch Keyblades with the D-pad.
- **Kingdom Key upgrades:** let a smith take munny plus the materials the mod already converts Smithing Stones into: Fluorite, Damascus, Adamantite, Electrum, Orichalcum, Orichalcum+ (`sora-er-items.json` `r2.30x`). Smithing Master Hewg at the Roundtable fits.
- **Elden Ring weapons:** a rare chance for some Elden Ring weapons to give a Keyblade, the way staves and shields already give the friends' weapons.

---

## 7. Items, prizes and progression

| Thing | How it works now | Status / source |
|---|---|---|
| **Munny** | Runes become munny, 1 munny per rune. Enemies drop munny orbs (≈ 0.4 munny per enemy max HP, a guess). Merchants are priced in munny. | Implemented (`sora-prizes.json` `er.*`, `sora-er-items.json` `r2.100`, `shop.kh`) |
| **HP / MP / Focus orbs** | KH3 prizes drop from enemies. HP orbs are raised for Elden Ring: +4 small, +20 large. MP +3 / +5, Focus +3 / +5. Treasure Magnet widens the pull (300-900 cm). | Implemented (`sora-prizes.json`, `sora-er-host.json` `prize.*`) |
| **Usable items** (Potion … Megalixir) | Only from enemies, corpses / world pickups, chests, NPCs, bosses and shops. Plants and animals give **materials only**. | Implemented (your 2026-10-18 rules; `source.usable`) |
| **Flasks** | Crimson / Cerulean Flasks give **nothing** | Implemented (your decision) |
| **Golden Seed / Sacred Tear** | Megalixir | Implemented (your decision) |
| **Synthesis materials** | Elden Ring crafting and gathering items become KH3 materials, Smithing Stones become ores. They are **kept but unused**: no synthesis in the mod. | Materials Implemented; synthesis not in the mod |
| **Stat boosts** | Permanent boosts go to **the whole roster**, unlocked or not, and a late joiner arrives with them. Sources: every 3rd small boss (+1 Str / Mag / Def / AP in turn), Whetblades (AP +1 each), Memory Stones (Max MP +5 each). | Implemented (your 2026-10-17 rule; `kh-partyhost/src/boosts.rs`; the item conversions are *proposals*) |
| **Slots** | Boss slots (Great Rune order, Messmer, Promised Consort). Talisman Pouch → accessory slot, then armour slot, then Megalixir. Larval Tear → armour slot (while under three), then Orichalcum+. | Implemented (Talisman / Larval rules are *proposals*) |
| **Save points** | A KH3 save point stands at every site of grace; the grace itself is removed and can never be used. Touching one unlocks it for fast travel. Menu: Save, **Load**, Select an Area, Pass Time, Respawn Enemies, Party Formation, Title Screen. **Delete** a slot from the save list, with a Yes/No confirm. Saving heals the party and cures Elden Ring statuses on every member. Disabled in combat. | Implemented (`kh-host/src/savepoint_menu.rs`, your decisions) |

---

## 8. Party (brief)

- **Starting characters:** Sora, Donald or Goofy, chosen at the new game; the starter is level 1.
- **Recruiting** (Implemented; `sheets/party-er-unlocks.json`):

  | Friend | Where he is | How he joins |
  |---|---|---|
  | Goofy | Fighting Leonine Misbegotten | Beat the boss |
  | Donald | Angry at the Finger Reader crone in Limgrave | Talk to him; six lines you approved |
  | Sora (if not the starter) | Fighting Margit | Beat Margit |

  Until you enter the arena, the staged fight does no damage on either side. A new friend joins at the level the shared EXP gives him.
- **Leader swap:** Party Formation at a save point picks who is in the party (up to 3) and who leads.
  - The leader holds Zero EXP.
  - Customize edits the leader's own shortcuts.
  - Menus list only the active party.
  - AI allies are targetable by enemies everywhere, and anyone can pick up items into the shared inventory.
- **Friends' boss rewards:** Donald and Goofy get KH3's friend bonuses from the boss table's `party.*` rows. A friend who joins later is caught up. Two kit unlocks are tied to Sora's bosses: Red Wolf gives Donald's Aero Float, and Maliketh gives Goofy's Tornado Copter.

---

## 9. Known gaps, untested items and open questions

**Surprises found while writing this:**
1. **Unison Blizzard and Unison Thunder have no source** (Planned, not built).
   - XD decision 3 (approved 2026-10-15) gives them to Royal Knight Loretta and the Draconic Tree Sentinel (`XD_PLAYABLE_KITS.md` lines 114-115, 357).
   - Only Fusion Rocket at Leonine was added (`IMPL_X3G_STATE.md`).
   - Donald's kit uses only the Unisons Sora owns, so Donald can't use them either.
2. **Link Extender is still on all three level paths** (A 32, B 48, C 34), but the port has no Links. It is useless, like the Attraction abilities you had removed.
3. **No Keyblade but the Kingdom Key can be obtained**, and the Kingdom Key cannot be upgraded. The friends get weapons from Elden Ring loot; Sora doesn't.

**Not verified:**
- **All 48 boss defeat flags are guesses** from memory; only Margit's character id was seen in your game. A wrong flag means a boss gives a small-boss item instead of its row, until `boss.grant` is used.
- Small bosses count only when killed by **Sora's own hit**, per the code's header comment (`kh-er/src/bossreward.rs`). Whether a Donald or Goofy kill counts: **unknown**.
- **Untested in game**, by the stream notes: Critical difficulty, the 5-attacker limiter, most XS moves in Elden Ring, the item conversions, the roster boosts.

**Built but not confirmed by you:** most boss rewards (everything not marked *[user]*), the small-boss rules, the Great Rune slot order, and the Memory Stone / Whetblade / Talisman Pouch / Larval Tear conversions.

**Open questions:**
- Where should the Keyblades come from (section 6.3)?
- Should the DLC keep KH3's six Re Mind abilities, or should they move to base-game bosses (`docs/sora-er-boss-rewards.md` Q2)?
- Is one stat boost per three small bosses the right amount (Q1)?
- Flowmotion in Elden Ring: there are no KH3 flow markers, so wall run, rail and pole moves don't come up. Should they get Elden Ring triggers?
