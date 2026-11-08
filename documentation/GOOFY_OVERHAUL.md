# Goofy — Playable Kit Overhaul (design)

Status: **built** (2026-11-02), tested in Elden Ring on the test profile; the 2026-11-02 parry redesign (no counters, section 16) is not tested in game yet. The numbers below are the game's as of that date; the sheets under `sheets/port` win where they differ. Section 16 lists what was changed from the original design and what no one has seen in game yet.

This document **replaces Goofy's whole former kit**. That includes:
- the GK tank rework (combo strings, Combo Plus, finishers, Bombardier on Y, Brace, Perfect Guard, the old Counter Bash, the lunge rules);
- BAL's Goofy ratios;
- the XD_PLAYABLE_KITS Goofy sections.

Section 14 lists what stays (the GUARD bar, shared systems, team attacks).

Pad layout (KH3 on an Xbox pad):

| Button | Action |
|---|---|
| A | Attack / confirm |
| B | Jump |
| X | Guard / deflect / dodge hop (sections 4.3, 10) |
| Y | Situation command, Goofy Breaker |
| LB | Shortcuts: LB + A and LB + B Shield Arts, LB + X Taunt, LB + Y items |
| RB | Lock-on |
| D-pad | Command menu |

Keyboard is extra, never required.

---

## 1. Who Goofy is

From the user: Goofy is a **parry fighter**. He is all about **parrying**, with **different kinds of parries depending on the move**, unlocked through progression. His basic attacks are fairly weak and unimpressive, but he is **good at defense and at drawing aggro away from his friends**. He should be as mechanically deep as Donald's rework.

Since 2026-11-02 the parries feed the Breaker: no technique opens a counter. Each parry takes 0 damage and fills the enemy's Stagger, and the Breaker is his big hit (sections 6, 16).

Pillars:

1. **Read, deflect, break, finish.** Goofy's damage comes from the Breaker, not from his combo. His parries take 0 damage and fill the enemy's Stagger until it breaks.
2. **Every attack type has an answer.** Thrusts, sweeps, grabs, projectiles, magic, heavy blows and combo strings each have their own parry technique. Choosing the right one, at the right time, is the skill.
3. **Posture war.** Each successful parry fills the enemy's **Stagger** gauge. Each sloppy block drains Goofy's **GUARD**. Whoever breaks first loses.
4. **He's the shield of the party.** He pulls enemies onto himself and can step in front of a friend to parry for them.
5. **Deep in the hands, not the menus.** (user: as deep as Donald, but not menu-heavy; focused on the skills of timing and reading) His command menu is just Attack / Items. Everything else is buttons, timing and reading the enemy.

---

## 2. Stats (overrides BAL for Goofy)

Status-screen values, with his starting shield. Sora's are shown for comparison only.

| | Level 1 | Level 50 | Level 99 | Sora | Notes |
|---|---|---|---|---|---|
| Strength | **11** | **28** | **44** | 15 / 40 / 66 | Weak basic attacks, by design (2026-10-30, the user: "Goofy 0.7x Sora's strength" as the level-1 reference). Donald's: 5 / 12 / 19. |
| Defense | **10** | **48** | **86** | 8 / 34 / 59 | Highest in the party (2026-11-01, the user: "Goofy's defence should be 86 at level 99"). |
| Magic | **0** | **0** | **0** | 15 / 41 / 67 | He has no spells, and his Shield Arts are physical (section 8). |

| | Start | With every reward | Sora (Balance start) |
|---|---|---|---|
| Max HP | **142** | **366**, plus Max HP +10 from Deflect's third boss (11): **376** | 105 → 265 |
| Max MP | **88** | **136** | 110 → 174 |

| Reward | Amount | Bosses |
|---|---|---|
| Max HP | **+14** each | Tree Sentinel, Leonine Misbegotten, Valiant Gargoyles, Onyx Lord (Liurnia), Lichdragon Fortissax, Onyx Lord, Morgott, the Omen King, Mohg, the Omen, Fire Giant, Mohg, Lord of Blood, Dragonlord Placidusax, Sir Gideon Ofnir, the All-Knowing, Radagon of the Golden Order / Elden Beast, Commander Gaius, Promised Consort Radahn |
| Max HP | **+7** each | Mimic Tear, Regal Ancestor Spirit |
| Max MP | **+4** each | Glintstone Dragon Smarag, Red Wolf of Radagon, Dragonkin Soldier of Nokstella, Starscourge Radahn, Astel Naturalborn of the Void, Loretta Knight of the Haligtree, Sir Gideon Ofnir, Hoarah Loux |
| Max MP | **+2** each | Divine Beast Dancing Lion, Rellana, Golden Hippopotamus, Putrescent Knight, Scadutree Avatar, Metyr, Midra, Romina |

**Equipment and item slots.** A tank's spread: the most armour in the party, with fewer accessories.

| Slots | Start | With every reward | Bosses | Sora |
|---|---|---|---|---|
| Item | 3 | **5** | Godfrey (golden shade), Starscourge Radahn | 3 → 8 |
| Armour | 2 | **5** | Margit, Mohg the Omen, Bayle the Dread | 1 → 3 |
| Accessory | 1 | **3** | Dragonkin Soldier of Nokstella, Promised Consort Radahn | 1 → 3 |

The party's limit is 5 of each slot.

MP is only used for Shield Arts (section 8). MP Charge follows KH3's rules, plus the MP of perfect parries (section 6.1).

| | Value | Notes |
|---|---|---|
| GUARD | His posture | Silver bar above MP: **100 at level 1**, +5 every 5 levels, 150 at level 50, 195 at level 99 (kept from GG). |
| Run speed | 560 (sprint 800) | Heavier than Sora (680 / 980) and Donald (680 / 900). |

**His own stats, independent of Sora** (2026-11-01, the user: "Donald and Goofy need to have independent stats from Sora, those multipliers are just for reference from level 1"): nothing here follows Sora. His stats don't change with Sora's rewards or Sora's opening choice. The old ×Sora ratios only picked his level-1 values.

- **Growth:** each stat grows steadily from level 1 to 99 at Goofy's own rate, as KH3's own party tables do.
- **HP and MP:** fixed starting values, plus his own boss rewards below. The bosses are the ones that give Sora HP or MP, with his own amounts. These replace his four old Max HP +10 rewards. HP, MP and slots are worked out from the record of what bosses gave (`friend_growth`), never added to saved values, so nothing counts twice.
- **Damage taken:** one rule for every friend, leading or not. An Elden Ring hit is first turned into a share of a sheet curve (105 / 185 / 265 HP at levels 1 / 50 / 99), then multiplied by a reference Defense (8 / 34 / 59) over his status-screen Defense, held between ×0.5 and ×2. So his Defense and his armour count, and more Max HP makes him properly tougher. The old 24-damage cap on a friend leader is gone, so big hits hurt more than they used to. Sora's own rule didn't change.
- **Existing saves:** he gets the rewards for bosses already beaten, once. His totals may shift a little on the first load.

How tough that makes the party, counted in hits of 10 % of Sora's bar:

| Hits to KO | Level 1 | Level 99, no rewards | Level 99, every reward |
|---|---|---|---|
| Goofy | 16.9 | 7.8 | about 20.7 |
| Donald | 6.4 | 2.5 | about 6.5 |
| Sora | 10 | 10 | 10 |

**One set of stats, leading or not** (ONESTAT, 2026-10-27, the user: "there should be no party ally/leader seperation, their stats should be their stats same for Goofy and every KH character added later"): the stats above are his whether he leads or follows as an AI ally.

Damage reaches Elden Ring through the party's one conversion, **×8** (`damage.out.scale`, 2026-10-28, the user: "lower the scale to 8"), the same as Sora's and Donald's.

Every stat is a sheet row (`sheets/port/party-balance.json`, the level table `party-balance-levels.json` built from it by `tools/party_balance_sheet.js`, GUARD in `party-guard-levels.json`), so it can be tuned without touching the code.

---

## 3. Basic attack: weak on purpose

- **Ground combo (A, A, A):** shield shove, shield swipe, two-handed shield push. Low damage (0.6 / 0.6 / 0.9), short reach, no lunge.
  - **Combo Plus is not in his kit.** His string never grows; his depth comes from the parry system.
  - Basic hits fill the enemy's **Stagger** gauge only slightly (section 5).
- **Air:** one downward shield bonk. He is a ground fighter.
- **What the string is for:**
  - poking to keep pressure;
  - **Taunt Hits**: every basic hit adds threat (aggro), so his weak string also keeps enemies looking at him (section 9);
  - **opening into a parry:** any step can be cancelled into Guard (X) at any time, with no recovery. The attack-then-deflect rhythm is the point. (As built: the bash that ends the string has an end lag, like Sora's last finisher, and X cancels it only from 0.2 s after its hit; see section 16.)

---

## 4. Guard, Deflect and GUARD (his posture)

### 4.1 Guard and Deflect

| Input | Name | Window | GUARD cost | Result |
|---|---|---|---|---|
| Hold X | **Guard** | Always | **High** (scaled by the hit's power) | Blocks the hit |
| Tap X just before the hit | **Deflect** | 0.20 s at rank 1 (wider with ranks and Resolve, section 7) | **None**, and refills a bit on success (the user, 2026-11-02) | Clang plus sparks; the enemy's Stagger gauge rises a lot (section 6.1) |
| Mashing X | — | Each tap within 0.25 s of the previous one **shrinks** the window by 30 % | — | Spamming is weaker than timing |

**Bounce off the shield** (the user, 2026-11-02: "make non-boss enemies bounce off of Goofy's shield if they're not doing a long combo string when Goofy successfully performs a perfect deflect"). A **perfect deflect** is the right answer to the attack, in its window: Deflect of a normal attack, Pinning Guard of a thrust, and In the Zone's deflect of a sweep. A plain deflect (an attack that has its own technique) doesn't count, and neither does a late tap that Second Deflect forgave. Revenge Bash's deflect does count (11.3). When a perfect deflect meets a melee swing (normal, thrust or sweep), the attacker plays Elden Ring's own attack rebound: its swing is knocked back, its attack ends, and a short opening follows. This comes on top of the Stagger. A perfect deflect also gives MP back (section 6.1). It never happens:
- to a boss (the game's boss bar);
- to an attacker in the middle of a string: Rhythm Deflect's reading, a deflect of the same attacker's blow within 1.5 s before. The first blow of a string can't be known ahead of time;
- for Bull Brace (no guard turns an unblockable heavy), Shield Reflect or Spell Ward (nothing is swung at the shield), Hop or Slip Counter (evasions), or Ally Guard and Interpose.

The rows are `deflect.bounce` (on/off) and `deflect.bounce_wait` (in `party-kit-goofy`).

### 4.2 GUARD = Goofy's posture

- **How it fills and drains:** it drains only on guarded hits and failed parries, and refills slowly when he isn't guarding. Holding the guard up costs nothing, and a timed block (deflect) costs nothing and refills it slightly (the user, 2026-11-02: "Goofy's guard should not deplete while holding it, only from getting hit").
- **At 0, Guard Break:** Goofy staggers for 1.5 s (KH3's stun: `BadStatStunStart`, then its loop; in the air the air stun) and takes full damage. This replaces the GG "broken" state.
- **Level growth stays as GG built it.** The bar's look stays too: the silver bar, red loss and broken style.

### 4.3 Reading attacks: one danger cue, the rest is your eyes

There is **one** warning only: a single red KH3-style **"!"** over the enemy for a **perilous** attack (thrust, sweep, grab or unblockable heavy). **On its own it doesn't say which**; the ability **Danger Sense** adds a word under it that names the kind (below). Without it, the player must read the animation and pick the right answer:

| The enemy does… | The answer (an input, not a menu) |
|---|---|
| A normal attack | **Deflect**: tap X on the hit, stick neutral |
| A thrust / lunge (perilous) | **Pinning Guard**: tap X **while pushing the stick toward the enemy** as the thrust lands |
| A sweep / low swing (perilous) | **Hop Counter**: jump (B) over it |
| A grab (perilous) | **Slip Counter**: the sideways hop (X + stick sideways) as the grab closes |
| An arrow / projectile | **Shield Reflect**: tap X with the stick aimed at the shooter: the shot stops on his shield |
| Magic / an incantation | **Spell Ward**: hold X through the cast, then release and press X again on impact |
| An unblockable heavy / charge (perilous) | **Bull Brace**: hold X and the stick **away** from the enemy, set your feet before impact |
| A long combo string | **Rhythm Deflect**: deflect every hit in time; from the string's second deflect on, each one fills more Stagger |

- **No cues for normal attacks, projectiles or magic.** Recognising Elden Ring's wind-ups is the skill.
- **The "!"** rises about 0.5 s before the hit, and only for attacks aimed at someone in the party. If the enemy is off screen it sits at the edge of the screen, and a short cue sound plays.
- **Without the right technique learned,** the attack can only be guarded (expensive) or deflected plainly: half Stagger.
- **Spell Ward's press** doesn't count as mashing once X has been held for 0.25 s.
- **Danger Sense** (ability) **names the kind of attack** in a word under the red "!" (the user, 2026-11-02: "Danger Sense ability should indicate what kind of attack it will be"). It does **not** make the "!" come earlier: the user, 2026-11-05: "Danger Sense needs to be 8 AP and should not make the cue come earlier" (`danger.lead` is 0; it was 0.15 s). The word is in KH3's menu font (the font of the HUD's "STAGGER" label), with the "!"'s dark outline and a colour per kind. It is drawn in the "!"'s own clip, so it pops, fades and goes exactly with it. Off screen it sits on the screen's edge with the "!", and the pair is moved in far enough that the word stays on screen. The words are new text (COMPOSED; KH3 has none) and are sheet rows (`party-guard` `hud.peril_word_*`), as are their colours, height and gap:

  | Class read | Word | Colour | Answer |
  |---|---|---|---|
  | Thrust | THRUST | yellow | Pinning Guard |
  | Sweep | SWEEP | green | Hop Counter |
  | Grab | GRAB | pink | Slip Counter |
  | Heavy | HEAVY | orange | Bull Brace |
  | Projectile | PROJECTILE | cyan | Shield Reflect |
  | Magic | MAGIC | blue-violet | Spell Ward |
  | Normal | STRIKE | white | Deflect |

  In Elden Ring only thrusts, sweeps, grabs and heavies are perilous, so only those four words appear there. The other three exist so that every class the kit reads has a word, if a host ever marks one perilous. The class is the generic sensor's (table below), so this works on every enemy.

**Where the attack types come from.** Everything is read from the running game, for any enemy, with no per-enemy table:

| Class | How it's told |
|---|---|
| Grab | The attack row's throw id or throw flag |
| Heavy | Guard is disabled, or its guard rate is 999 |
| Thrust | Attack attribute 2, at hit level 3 or more (lighter thrusts are Normal) |
| Sweep | Any other melee attack of level 3 or more. Elden Ring has no sweep field, so this is by elimination |
| Projectile | A bullet, or an arrow attack |
| Magic | A bullet whose element isn't physical |
| Normal | Everything else |

Rows that deal no damage at all (Margit's footsteps, for example) are ignored, except grabs. Timing comes from the event data of the animation the enemy is playing (`CS::TaeDat`, read in memory), then the game's own animation files, then what the mod has learned during play.

---

## 5. Stagger: the enemy's posture

- A **Stagger gauge** appears under the locked-on enemy's KH3 HP gauge, in the same KH3 style. It is built from the HUD's unused armour gauge, so it isn't tied to Donald's Scan.
- **Size:** 100, 140 on a large enemy, 240 on a boss.
- **Filled by:**
  - deflects (25; a plain deflect of an attack that has its own technique, 12.5);
  - the right parry technique against its attack type (30 to 45, Rhythm Deflect up to 46; section 6.1);
  - Shield Arts (their own Stagger, section 8);
  - basic hits (3).
- **Drains** 4 a second, starting 5 s after he stops pressuring (the user, 2026-11-02: "The enemy's stagger gauge should not begin to deplete so fast"; rows `stagger.drain_delay`, `stagger.drain_rate`). The low-HP rule: the rate falls with the enemy's HP share, down to a quarter at no HP (`stagger.drain_low_share` 0.25). A full gauge left untouched lasts about 30 s on a normal enemy, 40 s on a large one and 65 s on a boss, at full HP (it used to be 16.3 s, 22 s and 36.3 s), and longer at low HP.
- **When full, the enemy is BROKEN for 2.5 s.** Y offers **Goofy Breaker** (section 6.3).
- **Elden Ring link:** a full Stagger makes the enemy play Elden Ring's own stance-break kneel (`W_SABreak`), cut short with `W_Idle` to fit the window. Elden Ring's critical attack isn't used, because it is a paired throw. The kneel is the Elden Ring reaction under our Breaker.
- **Bosses:** a boss breaks into a shorter 1.5 s window, after which the gauge resets to half.
- **No endless stuns:** an enemy still recovering from a break refuses the next one, and its gauge falls back to half. The existing chain cap still applies.
- **Refused outright:** a boss in a no-death state, and any enemy while Zettaflare is running. No boss phase is ever skipped.
- **On a kneeling enemy,** friends' hits take HP but start no reaction, so the kneel holds until the slam.
- **Protect the slam** (the user, 2026-11-03): once Goofy Breaker has begun on an enemy, every other party hit on it leaves it at least 1 HP: Sora's, Donald's, the leader's or an AI's, spells and our summons alike. The party's hate and Over Here! bullets (1 HP each) aren't fired at it then. So the slam always lands, and its own defeat rule (6.3) finishes the enemy. Elden Ring's own spirit ashes hit through the game, not through the party's hits, so this doesn't cover them.

---

## 6. Stagger and the Breaker: where Goofy's damage lives

### 6.1 What each technique gives

**The loop:** parry → the enemy's Stagger fills → **BROKEN** → Y: **Goofy Breaker** (6.3). Every parry technique takes **0 damage**: no HP and no GUARD lost (a timed parry refills GUARD a little, section 4.2). Its whole job is to fill the enemy's Stagger gauge. His big damage comes only from the Breaker. His basic string (0.6 / 0.6 / 0.9) and his Shield Arts keep their damage: the Arts are utility with modest damage, and Goofy Bash (1.6 / 2.0 / 2.4) is the biggest of them, well under the Breaker.

Stagger at rank 1 (×1.25 at rank 2, ×1.5 at rank 3). The rows are `stagger.*` and `rhythm.*` in `sheets/port/party-kit-goofy.json`.

| Technique | Against | Stagger | Hits to break a normal enemy (100) |
|---|---|---|---|
| Deflect | A normal attack | **25** (`stagger.deflect`) | 4 |
| Plain deflect | An attack that has its own technique, deflected plainly | **12.5**, half of Deflect | 8 |
| Shield Reflect | A projectile | **30** (`stagger.reflect`) | 4 |
| Spell Ward | Magic | **34** (`stagger.ward`) | 3 |
| Hop Counter | A sweep | **36** (`stagger.hop`) | 3 |
| Pinning Guard | A thrust | **40** (`stagger.pin`) | 3 |
| Slip Counter | A grab | **40** (`stagger.slip`) | 3 |
| Bull Brace | An unblockable heavy | **45** (`stagger.brace`) | 3 |
| Rhythm Deflect | A long string | The first deflect of a string is a plain Deflect (25). The second in time gives **28** (`rhythm.stagger`), and each further deflect in a row **+6** (`rhythm.stagger_per`), up to **46** from the fifth on (`rhythm.stagger_max`) | A 5-hit string fills 173 (25 + 28 + 34 + 40 + 46), so a normal or large enemy breaks inside one string |

Rank 3's ×1.5 is the row `stagger.rank3_mult`.

- **Other multipliers:** an aggressive deflect (stick toward the enemy) gives ×1.3 Stagger with a tighter window (section 7). Resolve's first pip adds +10 % Stagger on parries (`resolve.pip1_stagger`). The ability **Stagger Boost** adds ×1.2 (`stagger.boost_mult`, section 11.3).
- **Unchanged:** the gauges (100, 140 large, 240 boss), basic hits (3) and the Arts' own Stagger.

**MP from perfect parries** (the user, 2026-11-02: "MP back on a perfect parry is good"). A **perfect parry** gives **4 MP** back (`parry.mp`), or during MP Charge takes 0.5 s off it (`parry.mp_charge`). A perfect parry is the right answer to the attack, in its window, by the same rule as the shield bounce (4.1): Deflect of a normal attack, or the right technique for the attack's class (Hop and Slip Counter's evasions, Shield Reflect, Spell Ward, Bull Brace, and Rhythm Deflect's deflects). In the Zone's deflects, Revenge Bash's deflect, and Ally Guard's or Interpose's parry for a friend count as well. A plain deflect of an attack that has its own technique doesn't, and neither does a late tap that Second Deflect forgave. So about four (Goofy Charge, 15 MP) to seven (Interpose, 26 MP) perfect parries pay for one Art. KH3's own MP rules (MP Charge and the rest) stay as they are.

**Jumps and hops in Elden Ring.** The hit is judged on the game's hidden body, which knows nothing of his jump. So being off the ground for the technique's minimum time when a sweep arrives counts as clearing it. A hop, or a grab closing on one, sets the body's no-hit flags for those frames, so a throw can't take him.

### 6.2 No counters

Since 2026-11-02 no technique opens an A counter. The counter chain and the free Art after a counter went with them (section 16).

### 6.3 Goofy Breaker (Y on a BROKEN enemy)

- **The finisher:** his finishing blow. A big comic shield sequence (wind-up, spin, slam) for **8.0×** damage (`breaker.power`). It is his only big damage, so one break cycle is worth the Breaker's 8× (it was 5.0× before the 2026-11-02 redesign; section 16 says why).
- **Elden Ring link:** it plays over the stance-break kneel (section 5). If Y comes late in the window, the kneel plays slower so the enemy doesn't stand up before the slam.
- **Defeating with it:** on non-boss enemies with less than 40 % HP left it **defeats them outright**, through Elden Ring's own death, with its runes and our EXP and prizes. It never kills a boss.
- **Breaker variants** come from abilities (section 11):

| Variant | What it adds |
|---|---|
| **Party Breaker** | Nearby friends join in, a mini team attack |
| **Launch Breaker** | Launches the enemy for air juggles by Sora and Donald |
| **Rally Breaker** | Heals the party 10 % |

---

## 7. Momentum: depth from execution, not menus

Goofy is **not menu-heavy**. His command menu is just **Attack / Items**, plus his two equipped Shield Arts on button presses (section 8). His depth is in timing, reading, risk, and managing two bars at once.

**Resolve**, a 3-pip meter beside GUARD, fills only from **clean play**:
- a full pip for the right answer to a perilous attack;
- a third of a pip for a perfect deflect;
- nothing for guarding.

It empties if he gets hit.

| Resolve | Effect, automatic (no button) |
|---|---|
| 1 pip | Deflect window +0.03 s, and parries fill +10 % Stagger |
| 2 pips | Deflects also refill GUARD a little more, and his threat doubles (enemies prefer him) |
| 3 pips | **In the Zone**: every deflect counts as the right technique (it fills that technique's Stagger and gives its MP), and Breakers do +50 %. Lasts until he's hit. |

So the reward for playing perfectly is playing even better, and one mistake drops him back down. That is the skill loop, not menu switching.

- **Risk:** **aggressive deflects** make it harder. If he deflects while **moving toward** the enemy (stick forward), Stagger gain is ×1.3 but the window is slightly tighter.
- **Two bars at once** (his HP against his GUARD): it's often right to take a small hit (HP) rather than guard (GUARD), to avoid a Guard Break before the enemy breaks.

---

## 8. Shield Arts: two equipped, on buttons

Goofy has **no magic and no Magic menu**. He **equips two Shield Arts** in the camp menu (Customize), outside combat. In combat they're fired directly:
- **LB + A:** the first Art;
- **LB + B:** the second.

Taunt has its own button, **LB + X**. Arts cost **MP**, and MP comes back from perfect parries (sections 2, 6.1). Each Art except Taunt has a **cooldown** (section 8.1). No menu is opened in combat.

| Art | Effect | Notes |
|---|---|---|
| **Goofy Charge** | Shield-first rush that pushes enemies along | Closes distance; ends in a guard stance |
| **Goofy Tornado** | Spinning shield whirl | Hits around him; deflects projectiles while spinning |
| **Goofy Bash** | Overhead slam | Big Stagger |
| **Goofy Turbo** | Rides the shield forward like a sled | Mobility; can be cancelled into Guard |
| **Taunt** | Goofy hollers and bangs the shield | Enemies within 10 m turn on him for 5 s (section 9) |
| **Interpose** | Dashes in front of the nearest threatened friend and auto-deflects the next hit aimed at them | The bodyguard tool |
| **Shield Toss** | Throws the shield, which returns | Ranged poke; while it's out he can only guard bare-handed: half GUARD, no parry, shove, Breaker or other Art. Taunt still works |

Arts gain **ranks** from progression (section 11). Items stay on KH3's shortcut palettes (LB + Y), the same as every character.

| Art | MP (rank 1 / 2 / 3) | Cooldown | Power | Stagger | Other numbers | Rank 3 adds |
|---|---|---|---|---|---|---|
| Goofy Charge | 15 / 14 / 12 | 6 s | 1.0 / 1.25 / 1.5 | 10 / 12 / 14 | | The guard stance opens as a fresh tap |
| Goofy Tornado | 20 / 18 / 16 | 10 s | 0.5 / 0.6 / 0.7 per hit | 4 / 5 / 6 | A hit every 0.3 s for 1.4 s; each deflect +8 Stagger | A deflected bullet flies back |
| Goofy Bash | 22 / 20 / 18 | 8 s | 1.6 / 2.0 / 2.4 | 30 / 36 / 42 | | Rings out 380 cm |
| Goofy Turbo | 16 / 15 / 13 | 10 s | 0.6 / 0.75 / 0.9 | 5 / 6 / 7 | Ride 2.0 / 2.4 / 2.4 s | Untouchable for the first 0.4 s |
| Taunt | 3 / 3 / 2 | none | | | 5 / 6.5 / 8 s, radius 1000 | Refills 20 GUARD |
| Interpose | 26 / 24 / 22 | 15 s | | | Cover 3 / 4 / 5 s, hits 1 / 1 / 2 | |
| Shield Toss | 15 / 14 / 12 | 8 s | 0.9 / 1.1 / 1.3 | 6 / 8 / 10 | | Bounces once to the nearest enemy within 900 cm |

### 8.1 Cost and cooldown

(2026-11-02, the user: "Goofy's abilities (except taunt) should cost more MP, and introduce a cooldown with a visible timer, they are way too spammable")

**MP.** The old prices (2 to 6 MP) let a full bar of 88 buy 14 to 44 Arts in a row. Every Art except Taunt now costs about four to five times as much, so a full bar buys a few, not a stream:

| Art (rank 1) | Was | Now | A full bar at the start (88 MP) | A full bar with every reward (136 MP), rank 3 |
|---|---|---|---|---|
| Goofy Charge | 3 | 15 | 5 | 11 |
| Goofy Tornado | 4 | 20 | 4 | 8 |
| Goofy Bash | 4 | 22 | 4 | 7 |
| Goofy Turbo | 3 | 16 | 5 | 10 |
| Interpose | 6 | 26 | 3 | 6 |
| Shield Toss | 3 | 15 | 5 | 11 |

His MP doesn't grow with level (section 2), so level 1 and level 99 have the same bar; only boss rewards add to it. A perfect parry gives 4 MP back (section 6.1), so about four (Goofy Charge) to seven (Interpose) perfect parries pay for one Art.

**The last MP and MP Charge.** (2026-11-02, the user: "let Goofy use his last MP and go into MP reserve, make sure Donald and his spells (including Zettaflare) have this as well") MP reserve is KH3's MP Charge. Goofy follows the party's MP rule (`crates/kh-party/src/mp_rule.rs`), the same one Donald and any later character use, built on Sora's:
- Any MP above 0 buys an Art, Taunt included. With 8 MP he can use a 15-MP Art.
- An Art that costs as much as he has left, or more, takes all of it: his MP goes to 0 and MP Charge starts at once.
- In MP Charge his MP stays at 0 for his own `mp_charge_time` row (30 s, sped up by MP Haste), then the bar is full again. No Art starts meanwhile, and the panel greys them.
- So an Art is refused with the refused sound only in MP Charge or at 0 MP. A cooldown still refuses as above. (There are no free Arts any more: the free Art after a counter went with the counters, 2026-11-02.)
- AI Goofy follows the same rule. He isn't offered an Art in MP Charge. He may spend his last MP on an Art, but while a friend is low enough for Interpose (`ai.interpose_low`) and his Interpose is learned and ready, he holds back any Art that would empty his MP, because that MP buys Interpose. Interpose itself may still take the last of it.
- The HUD shows MP Charge the way KH3 does: the pink charge gauge refills over the MP bar on his leader bar and on his small party gauge.

**Cooldown.** Each Art except Taunt waits its cooldown from the moment it's used before it can be used again. The longer and stronger the Art, the longer the wait: Charge 6 s, Bash and Shield Toss 8 s, Tornado and Turbo 10 s, Interpose 15 s.
- A press while it waits is **refused with the refused sound**, the same as short MP, and nothing is spent.
- Each Art waits on its own: using Bash doesn't hold up Charge.
- It holds for whoever plays him. AI Goofy isn't offered an Art that is waiting.
- **Taunt is unchanged**: same price, no cooldown.
- Customize's help line for each Art gives both, from the same rows, for example "MP Cost: 15  Cooldown: 6 s  Rank 1". Taunt's gives its MP only.

**How it shows.** On the LB panel, a waiting Art's entry shows **KH3's own situation command timer**: the gauge that drains under a situation command's name (`actGgAnm`), in its violet "grand magic" colour, laid on the entry's plate under the name. It drains from full to empty as the wait runs out. The seconds left stand in the entry's number field, where an item's "x3" goes, counted up to the whole second ("6" just used, "1" in the last second). The name stays lit so the white text and the count read on the gauge. Whatever else greys an Art, such as MP Charge, still greys it, and its gauge still runs. When the wait is over, the gauge and the number go. His Arts are only on the LB panel: his command menu is Attack and Items (PLAN.md R19), so there's no other entry to mark. The panel shows only while LB is held, as KH3's shortcuts do.

Pictures from the test (`crates/kh-ui/tests/gof_recharge.rs`): `build/sora_tmp/hud/t_gof_recharge_running_zoom.png`, `t_gof_recharge_back_zoom.png`.

---

## 9. Aggro: drawing attention away from his friends

- **Threat:** every guard, deflect, basic hit and Taunt raises Goofy's **threat** on that enemy, halving every 12 s. Threat feeds the party's aggro director, which keeps its weighted roll and its holds, so enemies lean toward the member with the most threat but can still pick someone else.
- **Base:** his threat counts ×1.5 compared with a friend's (×3 at Resolve 2 or more).
- **Taunt** (LB + X) is a strong refreshed pull, not a guarantee, because Elden Ring has no way to force a target. It uses two routes, proven separately in game: a target-priority value on Goofy's body (12 while taunting, 0.3 normally, 0.6 at Resolve 2 or more) and hits of pure hate that make enemies turn. Donald drops to −12 during Zettaflare.
- **Interpose** (Art) and the **Ally Guard** ability let him parry for a friend.
- **Aggro director:** it runs in the game for the first time with this overhaul, for every leader. Elden Ring's attacker limit is per target, so up to 15 enemies can now be attacking across three party members.
- **Friends take real hits now:** no enemy attack had ever landed on an AI friend in Elden Ring. The attack sensor now judges a swing or bullet aimed at a friend (rows `body.swing.*`, off with `party.body_swings = 0`), so AI friends can be hurt and can die.
- **Synergy:**
  - with Donald's aggro abilities (Taunt Pass etc.: when Goofy guards, enemies near Donald switch to Goofy);
  - with Sora's combo pressure.
- **Solo:** the aggro tools matter less, but Taunt still pulls packs into one spot for his Tornado.

---

## 10. Movement and dodge

- **Dodge (X + stick sideways, or any stick with no enemy near):** a clumsy hop with a **1.0 s cooldown** and short invincibility. Goofy is meant to parry, not dodge. There is no back hop clip, so a backward stick hops to the side.
- **B is always jump.** Jump is low, but it's the input for **Hop Counter** over sweeps.
- **No air dodge and no air guard.**
- **Which way the stick points** is judged against the lock-on target, then the attacker of the nearest attack on its way, then the nearest enemy the lock-on accepts.
- **Swim and ladders:** shared rules (Y at a ladder jumps him up to its top).

---

## 11. Progression: how he gets his parries and Arts

Same model as Sora and Donald:
- **Levels** give core abilities.
- **Bosses** give the **next rank** of a technique or Shield Art tied to that boss. A boss gives "Pinning Guard", meaning the next rank of it, never a fixed rank, so the order bosses are beaten in never matters.

**Saved ranks** (2026-11-01, the user: "save it, detail this in the docs"): a boss's rank is never lost, even if Goofy hasn't learned that technique or Art yet.
- **Beaten too early:** the rank is saved. The popup says it is waiting, for example "Pinning Guard: Rank 2 ready". It gives nothing to use yet.
- **When he learns it:** every saved rank applies at once. Example: he beats a Crucible Knight at level 10, then reaches level 20 and learns Pinning Guard at **rank 2** straight away. With both of its bosses beaten, he learns it at rank 3.
- **Levels never give ranks 2–3,** and a boss never teaches rank 1. A saved rank doesn't bring the move forward, so level pacing holds.
- **Past rank 3:** a boss step gives **Max HP +10** (2026-11-01, the user: "do Max HP +10 for him"), the size of Sora's usual HP reward. Deflect is the usual case: it has three bosses for its two ranks.
- **Before he joins:** Goofy gets every step even before he joins the party, and joins with them, as Donald does (kept in the roster's record, saved with the party).
- **Several copies of one boss:** Crucible Knights and Erdtree Avatars each appear more than once. Only the first one beaten counts as that boss's rank, so the other copies stay small bosses.
- **Older saves:** bosses already beaten count once when the overhaul arrives, with the same catch-up Donald's steps use.

### 11.1 Parry techniques and ranks

All eight are learned by level 40 (the user, 2026-11-02: "make Goofy learn all of the techniques by Level 40"; rows `n_ex002.learn.*` in `sheets/port/party-balance.json`). They are never equipped: once learned, a technique always works.

Each technique has 3 ranks:
- **Rank 1:** learned.
- **Rank 2:** a wider window (0.25 s instead of 0.20 s) and ×1.25 Stagger.
- **Rank 3:** ×1.5 Stagger in all (`stagger.rank3_mult`).

| Technique | Learned at | Ranks 2–3 from bosses whose style is… |
|---|---|---|
| Deflect | Start | Duelists: Margit, Morgott, Malenia |
| Shield Reflect | Level 5 | Archers and casters: Starscourge Radahn, Red Wolf of Radagon |
| Hop Counter | Level 10 | Sweeping giants: Godrick, Fire Giant |
| Pinning Guard | Level 15 | Spear and thrust users: Crucible Knights, Draconic Tree Sentinel |
| Slip Counter | Level 20 | Grabbers: Godskin Duo, Mohg |
| Bull Brace | Level 26 | Heavy chargers: Rykard, Erdtree Avatars |
| Spell Ward | Level 33 | Mages: Astel Stars of Darkness, Loretta Knight of the Haligtree |
| Rhythm Deflect | Level 40 | Long-string bosses: Malenia, Maliketh |

**In the camp menu: the Parry Techniques page** (the user, 2026-11-02: "Can we have a way to visually show in the camp menu what parry techniques Goofy currently has, when to use them and how to perform them? (preferably using symbols from Kingdom Hearts' controller icons)").

- **Where:** Menu → **Abilities** → **Goofy** → **Y** ("Show Next List"). It works whether Goofy leads or is an AI member, and only for him: Sora's and Donald's Abilities screens don't offer it. If Goofy isn't in the party, he isn't in the character list, so the page can't be reached. KH3's own Abilities screen uses Y for exactly this kind of list (the Keyblade's Form Actions and Shotlocks: moves a character simply has, not ones he equips), and the button guide uses KH3's own words.
- **On the pad:** the D-pad or stick moves between rows. **Y** or **B** goes back to his abilities list, and B again goes back to the character list. **LT / RT** switch to another friend. A does nothing, because the page only shows information. The menu's own sounds play (cursor, list change, back).
- **Each row** (all eight, in the order he learns them):
  - **Technique:** KH3's ability icon. A learned technique shows the "set" icon; a locked one shows the plain icon and its row is greyed.
  - **Rank:** a learned technique shows "Rank N" and three pips. A locked one shows "Learned at Lv. N", taken from the sheet. If a boss has already saved a rank, the row also says "Rank N ready".
  - **The enemy does:** the attack it answers. A red **!** marks the perilous kinds (thrust, sweep, grab, unblockable heavy).
  - **How to perform it:** KH3's own Xbox button icons and left-stick icons. "Toward the enemy" is drawn as the stick pushed up, "away" as down and "sideways" as left / right, and the words say what's meant.
  - **Stagger:** the Stagger it adds at his current rank, as "+25" (Rhythm Deflect: "+28 to +46"), with one to three bars: 1 bar under 30, 2 bars from 30 to 39, 3 bars at 40 and up. A locked technique shows its rank-1 amount, greyed.
- **Under the table,** a footer line (COMPOSED): "Every technique takes 0 damage and fills the enemy's Stagger gauge. When it is full the enemy is broken: press <Y icon> for Goofy Breaker, his big hit."
- **The help band** at the bottom gives the focused row's whole description (what it answers, how, "0 damage" and its Stagger), then its state: "Rank 2 of 3", "Learned at Lv. 15", or that a saved rank is waiting.
- **Live:** the page is rebuilt from Goofy's real level and saved ranks every time the camp menu reads the party, so it is right at any point in a playthrough.

| Technique | The enemy does | How (the page's icons) | Stagger (rank 1) |
|---|---|---|---|
| Deflect | A normal attack | Tap X as the hit lands | +25 |
| Shield Reflect | An arrow or a projectile | Stick at the shooter + tap X | +30 |
| Hop Counter | A sweep or a low swing (!) | B: jump over it | +36 |
| Pinning Guard | A thrust or a lunge (!) | Stick toward the enemy + tap X as it lands | +40 |
| Slip Counter | A grab (!) | Stick sideways + X: hop aside | +40 |
| Bull Brace | An unblockable heavy or a charge (!) | Hold X + stick away, before impact | +45 |
| Spell Ward | Magic or an incantation | Hold X through the cast, let go, tap X on impact | +34 |
| Rhythm Deflect | A long combo string | Tap X on every hit, in time | +28 to +46 |

Pictures (offline): `build/sora_tmp/party/GOOFY/shots/ui_parry_lv1.png`, `ui_parry_lv40.png`, `hud_parry_*.png`.

The page's wording is ours, not KH3's. It lives in `crates/kh-party/src/goofy/guide.rs`; the column headings and rank lines are in `crates/kh-ui/src/camp_parry.rs`.

### 11.2 Shield Arts ranks

- **From the start:** Goofy Charge, Taunt.
- **By level:** Tornado 8, Bash 15, Turbo 25, Interpose 35, Shield Toss 50.
- **Ranks 2–3:** each Art has two bosses of its own, each giving its next rank, with the saved-rank rules above. Ranks improve damage, cost, and an extra effect per Art.

| Art | Learned at | Ranks 2–3 from |
|---|---|---|
| Goofy Charge | Start | Leonine Misbegotten, Elemer of the Briar |
| Taunt | Start | Flying Dragon Agheel, Commander Niall |
| Goofy Tornado | Level 8 | Magma Wyrm Makar, Dragonlord Placidusax |
| Goofy Bash | Level 15 | Godskin Noble, Hoarah Loux |
| Goofy Turbo | Level 25 | Fallingstar Beast, Full-Grown Fallingstar Beast |
| Interpose | Level 35 | Ancestor Spirit, Regal Ancestor Spirit |
| Shield Toss | Level 50 | Glintstone Dragon Adula, Messmer the Impaler |

Each boss here has a working defeat flag, and none of them is a technique boss in 11.1. A boss's Goofy rank is its own record, separate from what Sora and Donald get from it.

### 11.3 Abilities (as many as Sora's; with AP costs, names and descriptions)

**Parry**

| Ability | AP | Level | What it does |
|---|---|---|---|
| Danger Sense | 8 | 10 | The "!" names the kind of attack (4.3); the cue does not come earlier (the user, 2026-11-05) |
| Steady Guard | 3 | 6 | Guard costs −20 % GUARD |
| Iron Posture | 3 | 16 | GUARD refills faster |
| Stagger Boost | 4 | 30 | Parries fill the enemy's Stagger 20 % more. Old saves keep it: their `GOF_COUNTER_CHAIN_PLUS` entry is read as Stagger Boost, for the same AP |
| Second Deflect | 3 | 24 | One mistimed tap per string is forgiven |
| Revenge Bash | 3 | 34 | After a Guard Break, his next deflect counts as perfect, so it also gives the bounce and the MP |
| Ally Guard | 4 | 38 | Deflects for a friend within 2 m automatically, once per 10 s |

**Breaker**

| Ability | AP | Level | What it does |
|---|---|---|---|
| Launch Breaker | 3 | 42 | The Breaker launches the enemy for juggles |
| Party Breaker | 4 | 46 | Nearby friends join the Breaker |
| Rally Breaker | 4 | 52 | The Breaker heals the party 10 % |
| Deep Breaker | 5 | 60 | +30 % Breaker damage |

**Resolve**

| Ability | AP | Level | What it does |
|---|---|---|---|
| Forward Deflect+ | 2 | 18 | Aggressive deflects lose less window |
| Resolve Keeper | 4 | 40 | One hit only removes one Resolve pip, instead of all |
| Zone Extender | 5 | 65 | In the Zone lasts through one hit |

**Aggro and support**

| Ability | AP | Level | What it does |
|---|---|---|---|
| Loud Mouth | 2 | 14 | +25 % threat |
| Second Wind | 3 | 22 | Restores half his GUARD when HP drops under 25 % |
| Bodyguard | 3 | 48 | Interpose costs no MP when a friend is under 30 % HP |
| Lucky Strike, Treasure Magnet, Item Boost (level 26), Second Chance, Withstand Combo | KH3's | | KH3 standard |

**Stats:** Max GUARD increases through levels (GG).

### 11.4 Goofy's boss rewards, boss by boss

**Built** (2026-11-01, the user: "Donald and Goofy both should get more equipment and item and accessory slots, but they don't need to be equal make them suit the character's playstyle. Create the detailed Sora-styled boss reward lists"). The rows are `gof.*` in `sheets/port/sora-er-boss-rewards.json`, with once-records `gof:<row>`. Each row gives once per character when the boss's defeat flag is seen. (2026-11-01, the user: "Donald should unlock Zettaflare from Elden Beast, move next banish level to another boss and spread out Donald and Goofy's rewards across other bosses including DLC, and give Sora more stat rewards from the DLC. Make sure it ALL follows natural progression")

- **Technique and Art ranks** follow the saved-rank rules above: a rank waits if he hasn't learned that move yet.
- **Past rank 3,** a step gives Max HP +10 instead.
- **Abilities** are given once.
- **Slots** suit a tank: the most armour in the party, with fewer accessories (section 2).
- **Natural progression:** each technique's or Art's bosses come around or after the point he learns it, so ranks rarely wait long. Shield Toss is learned late, so its last rank comes from the DLC, which also gives HP, MP and two slots.
- **No Goofy reward:** none.

| # | Boss | Region | Path | Goofy's reward |
|---|---|---|---|---|
| 1 | Tree Sentinel | Limgrave | optional | Lucky Strike (5 AP); Max HP +14 |
| 2 | Flying Dragon Agheel | Limgrave | optional | **Taunt**, next rank |
| 3 | Margit, the Fell Omen | Limgrave (Stormveil gate) | main | **Deflect**, next rank; Armour slot +1 |
| 4 | Leonine Misbegotten | Weeping Peninsula (Castle Morne) | optional | **Goofy Charge**, next rank; Max HP +14 |
| 5 | Godrick the Grafted | Limgrave (Stormveil Castle) | main | **Hop Counter**, next rank |
| 6 | Royal Knight Loretta | Liurnia (Caria Manor) | optional | Team Attack Extender (3 AP) |
| 7 | Glintstone Dragon Smarag | Liurnia | optional | Max MP +4 |
| 8 | Red Wolf of Radagon | Liurnia (Raya Lucaria) | main | **Shield Reflect**, next rank; Max MP +4 |
| 9 | Rennala, Queen of the Full Moon | Liurnia (Raya Lucaria) | main | Second Chance (4 AP) |
| 10 | Magma Wyrm Makar | Liurnia (Ruin-Strewn Precipice) | optional | **Goofy Tornado**, next rank |
| 11 | Ancestor Spirit | Siofra River | optional | **Interpose**, next rank |
| 12 | Dragonkin Soldier of Nokstella | Ainsel River | optional | Max MP +4; Accessory slot +1 |
| 13 | Starscourge Radahn | Caelid | main | **Shield Reflect**, next rank; Max MP +4; Item slot +1 |
| 14 | Mimic Tear | Nokron | optional | Share Prizes (4 AP); Max HP +7 |
| 15 | Regal Ancestor Spirit | Nokron | optional | **Interpose**, next rank; Max HP +7 |
| 16 | Valiant Gargoyles | Siofra Aqueduct | optional | Max HP +14 |
| 17 | Astel, Naturalborn of the Void | Lake of Rot (Grand Cloister) | optional | Max MP +4 |
| 18 | Lichdragon Fortissax | Deeproot Depths | optional | Max HP +14 |
| 19 | Elemer of the Briar | Altus Plateau (Shaded Castle) | optional | **Goofy Charge**, next rank |
| 20 | Godskin Noble | Mt. Gelmir (Volcano Manor) | optional | **Goofy Bash**, next rank; Hyper Healing (3 AP) |
| 21 | Rykard, Lord of Blasphemy | Mt. Gelmir (Volcano Manor) | optional | **Bull Brace**, next rank |
| 22 | Draconic Tree Sentinel | Capital Outskirts | main | **Pinning Guard**, next rank |
| 23 | Godfrey, First Elden Lord (golden shade) | Leyndell | required | Item slot +1 |
| 24 | Morgott, the Omen King | Leyndell | required | **Deflect**, next rank; Max HP +14 |
| 25 | Mohg, the Omen | Leyndell (Shunning-Grounds) | optional | Max HP +14; Armour slot +1 |
| 26 | Commander Niall | Mountaintops (Castle Sol) | optional | **Taunt**, next rank |
| 27 | Fire Giant | Mountaintops | required | **Hop Counter**, next rank; Max HP +14 |
| 28 | Mohg, Lord of Blood | Mohgwyn Palace | optional | **Slip Counter**, next rank; Damage Control (4 AP); Max HP +14 |
| 29 | Loretta, Knight of the Haligtree | Haligtree | optional | **Spell Ward**, next rank; Max MP +4 |
| 30 | Malenia, Blade of Miquella | Haligtree (Elphael) | optional | **Deflect**, next rank; **Rhythm Deflect**, next rank |
| 31 | Godskin Duo | Farum Azula | required | **Slip Counter**, next rank; Withstand Combo (4 AP) |
| 32 | Dragonlord Placidusax | Farum Azula | optional | **Goofy Tornado**, next rank; Max HP +14 |
| 33 | Maliketh, the Black Blade | Farum Azula | required | **Rhythm Deflect**, next rank; More Team Attacks (5 AP) |
| 34 | Sir Gideon Ofnir, the All-Knowing | Leyndell, Ashen Capital | required | Max HP +14; Max MP +4 |
| 35 | Godfrey, First Elden Lord / Hoarah Loux | Leyndell, Ashen Capital | required | **Goofy Bash**, next rank; Max MP +4 |
| 36 | Radagon of the Golden Order / Elden Beast | Erdtree | required | Max HP +14 |
| 37 | Divine Beast Dancing Lion | Shadow of the Erdtree (Belurat) | main | Max MP +2 |
| 38 | Rellana, Twin Moon Knight | Shadow of the Erdtree (Castle Ensis) | main | Max MP +2 |
| 39 | Golden Hippopotamus | Shadow of the Erdtree (Shadow Keep) | main | Max MP +2 |
| 40 | Messmer the Impaler | Shadow of the Erdtree (Shadow Keep) | required | **Shield Toss**, next rank |
| 41 | Commander Gaius | Shadow of the Erdtree | optional | Max HP +14 |
| 42 | Putrescent Knight | Shadow of the Erdtree (Stone Coffin Fissure) | optional | Max MP +2 |
| 43 | Scadutree Avatar | Shadow of the Erdtree | optional | Max MP +2 |
| 44 | Metyr, Mother of Fingers | Shadow of the Erdtree | optional | Max MP +2 |
| 45 | Midra, Lord of Frenzied Flame | Shadow of the Erdtree (Midra's Manse) | optional | Max MP +2 |
| 46 | Romina, Saint of the Bud | Shadow of the Erdtree | required | Max MP +2 |
| 47 | Bayle the Dread | Shadow of the Erdtree (Jagged Peak) | optional | Armour slot +1 |
| 48 | Promised Consort Radahn | Shadow of the Erdtree (Enir-Ilim) | required | Max HP +14; Accessory slot +1 |
| 49 | Fallingstar Beast | Altus Plateau (South Altus crater) | optional | **Goofy Turbo**, next rank |
| 50 | Onyx Lord | Altus Plateau (Sealed Tunnel) | optional | Max HP +14 |
| 51 | Onyx Lord (Liurnia) | Liurnia | optional | Max HP +14 |
| 52 | Full-Grown Fallingstar Beast | Mt. Gelmir | optional | **Goofy Turbo**, next rank |
| 53 | Astel, Stars of Darkness | Consecrated Snowfield (Yelough Anix Tunnel) | optional | **Spell Ward**, next rank |
| 54 | Glintstone Dragon Adula | Liurnia (Three Sisters / Cathedral of Manus Celes) | optional | **Shield Toss**, next rank |
| 55 | Crucible Knights (the first one beaten) | Limgrave, Caelid, Altus Plateau, Deeproot Depths | optional | **Pinning Guard**, next rank |
| 59 | Erdtree Avatars (the first one beaten) | Weeping Peninsula, Liurnia | optional | **Bull Brace**, next rank |

---

## 12. AI Goofy (when he isn't the leader)

(The user, 2026-11-02: "Make sure Goofy as an AI party member uses his new tools properly".)

- **Same moves:** AI Goofy presses the same inputs through the same kit as a leading Goofy, so every parry, Breaker and Art follows the same rules and ranks. He obeys the combo's end lag like a player.
- **Staying close:** he holds a post behind the leader, within an 8 m leash.
- **Parries:** he reads each attack once and rolls its timing by level: 40 % at level 1 to 85 % at level 99 on the sheet. He answers with the technique for its class, among those he has learned by his level (Deflect 0, Shield Reflect 5, Hop Counter 10, Pinning Guard 15, Slip Counter 20, Bull Brace 26, Spell Ward 33, Rhythm Deflect 40). Without it he deflects plainly (a blockable attack) or takes the dodge hop (an unblockable one). A failed roll is the shield up early, a guarded block, while GUARD can pay for it; else the hop; else he takes the small hit.
- **Low GUARD:** under 20 % GUARD he never blocks. He dodges, starts no strings and keeps his distance until GUARD refills.
- **Pushing Stagger:** there are no counters to wait for. After a perfect deflect bounces an enemy (4.1), he strikes into the opening at once.
- **The Breaker:** when an enemy is BROKEN he takes Goofy Breaker on it after a short beat (0.2 s). He moves in to reach it when it kneels within 6 m of him, with enough of its window left. This holds in battle or not, and the leader's leash doesn't cut the walk: a gauge fills only from the party's own blows, so a broken enemy is one the party is fighting even when the leader's battle flag is down (2026-11-03, the in-game test: a soldier broke twice with Sora leading and AI Goofy stood still). While he walks to it he counts as in battle, so he runs at his own speed rather than the standing leader's follow speed, and his KH3 tree's own moves wait until the walk ends. He never takes it on a body the lock-on refuses. He always takes it, because only Goofy can cash a break: Sora and Donald can't. (In KH3, friends land finishing blows too.) With Party Breaker the others join in. The broken enemy doesn't have to be his own target: any broken enemy within the 6 m counts, and the friends' hits can't kill it once his slam has begun (section 5, Protect the slam). If he is in the middle of an Art when an enemy breaks, he takes no Breaker on it, because an Art can't be cancelled (the user, 2026-11-03: accepted).
- **Arts:** only his two equipped ones, and never one still cooling down. The Customize "Abilities" AI setting decides how freely:
  - **Go Wild:** uses them whenever they pay off and he has the MP;
  - **Watch MP:** keeps a reserve of 30 % of his MP for Interpose and Taunt;
  - **Conserve:** uses only Taunt and Interpose.
- **When an Art pays off:**
  - **Taunt** when a friend has two or more enemies on them, at most once every 8 s;
  - **Interpose** when the leader is under 35 % HP with a danger near them;
  - **Goofy Bash** on a nearby enemy whose Stagger is at 60 % or more, to break it;
  - **Goofy Tornado** when three or more enemies are within reach;
  - **Shield Toss** and the rushes (**Charge**, **Turbo**) never on his own, because they leave him bare-handed or carry him off the leader. An Art his KH3 tree asks for is still taken when it is equipped and ready.
- **Customize "Combat Style":** Stick By Sora and Be Yourself play as above. Hands Off never starts strings and never chases a Breaker (he takes one only in reach). Finish the Job strikes the most-staggered enemy near the party and chases Breakers further (9 m).
- **Aggro:** his aggro rules apply as an ally too, so playing as Sora or Donald with AI Goofy feels protected.
- **Friendly NPCs** (the user, 2026-11-03: "the party leader can hit friendly NPCs, but not AI allies"): as an AI member his blows, Arts and shots pass through friendly NPCs. While he leads, he hits them as the Tarnished would, with Elden Ring's own consequences. The same rule holds for every character: the player's leader may hit a friendly NPC, and an AI never does (Donald, AI Sora, Mimic Sora and summons included).

---

## 13. Feel targets

- **Mashing A:** a player who mashes A with Goofy does poor damage and dies to perilous attacks.
- **Parrying:** a player who learns to read each attack and answer it with the right input wins fights by breaking enemies and finishing them with Breakers, without taking damage. Against a boss, Goofy should feel like the best duelist in the party once mastered.
- **Every parry sells the hit:** a loud KH3 clang, sparks, a short hit-stop and a camera nudge. A perfect deflect is the most satisfying sound in his kit.

---

## 14. What this replaces / keeps

**Removed:**
- the GK combo strings (Push / Sweep / Shield Bash / Shove / Shield Dash);
- Combo Plus, Air Combo Plus, and the finisher abilities (Shield Quake, Shield Upper, Fusion finishers);
- Bombardier on Y;
- the GK block (Perfect Guard, Brace, Counter Bash as designed there);
- the counters after a parry (Counter Bash, Pin Down, Goofy Stomp, Pratfall Toss, Return Fire, Ward Burst, Shield Shove, Crescendo), the counter chain and the free Art after a counter (2026-11-02);
- the shield-bash lunge rules;
- Tornado Copter, air guard, and Discus Barrage with its tiers;
- KH3's ability seeds that drove his old AI trees (dropped from old saves, which frees their AP);
- BAL's Goofy ratios;
- the XD_PLAYABLE_KITS Goofy kit.

**Kept:**
- **The GUARD bar** (GG), reused as his posture: same look, level growth and save field.
- The shared systems: hit reactions, items, menus, conversations, saves, pickup, death flow, rendering.
- His team attacks with friends: Trinity Guard and the others on Y when friends are near.
- The HR hit reactions; Guard Break uses his own stagger clip.
- Goofy's look and voice.

---

## 15. Decisions

Settled by the user on 2026-11-02, when the build started:

1. **Stagger and Elden Ring's stance break:** linked, as described in section 5.
2. **Defeating with a Breaker:** a Breaker defeats non-boss enemies under 40 % HP outright (section 6.3).
3. **The single red "!"** for perilous attacks stays (section 4.3).
4. **Donald's own stats and boss rewards** are built at the same time as Goofy's, so both friends share one stat system.

Not asked, kept as written: **Resolve / In the Zone** (section 7) and **Shield Toss** leaving him bare-handed (section 8). Both are sheet values.

---

## 16. As built

### Changed from the original design

| Design said | Built | Why |
|---|---|---|
| Dodge on B or X + stick | Dodge only on X; B is always jump | One button can't be both jump and dodge |
| Slip Counter on stick + B | The sideways X hop | Follows from the line above |
| Perilous = thrust, sweep, grab | Unblockable heavies count too | They have the same stakes |
| Sweeps are read from the game | Sweeps are found by elimination | Elden Ring has no sweep field |
| Elden Ring's critical reaction plays under the Breaker | The stance-break kneel plays instead | The critical is a paired throw |
| Taunt forces targets | Taunt is a strong pull | Elden Ring has no forcing route |
| A counter (A) after every parry: Counter Bash, Pin Down, Goofy Stomp, Pratfall Toss, Return Fire, Ward Burst, Shield Shove, Crescendo | No counters: each technique takes 0 damage and fills Stagger by its own row (6.1); the Breaker is the only big damage | The user (2026-11-02): "Remove countering with 'A' after a parry technique, Goofy's big damage should solely come from his guard break attacks from 6.3. The parries are there to make the enemy's stagger go up fast while taking 0 damage yourself. Redesign the menu for this as well and also remove any reference to [the game he was first modelled on] in any of the descriptions and the overhaul document. The enemy's stagger gauge should not begin to deplete so fast." |
| Stagger per technique: deflect 18, the right technique 32, plus the counter's 12 to 28 | Deflect 25 (plain 12.5), Shield Reflect 30, Spell Ward 34, Hop Counter 36, Pinning Guard 40, Slip Counter 40, Bull Brace 45, Rhythm Deflect 28 to 46 in a string | The parries alone must fill the gauge fast, now that the counters' Stagger is gone (same quote) |
| Crescendo after the last hit of a string | Rhythm Deflect's Stagger grows from the string's second deflect: 28, then +6 a deflect, up to 46 | No counter; the game can't know a string's last hit ahead of time |
| Stagger drains 7 a second, 2 s after the last pressure | 4 a second, 5 s after; the low-HP rule stays | Same quote: "The enemy's stagger gauge should not begin to deplete so fast" |
| Goofy Breaker 5.0× | 8.0× | It is now his only big damage. The counters gave about 2 to 3× per parry, so one break cycle on a normal enemy was about 4 counters × 2.0 + Breaker 5.0 = 13×; now it is the Breaker's 8× |
| Counters give 4 MP back | A perfect parry gives 4 MP back (or 0.5 s off MP Charge) | The user (2026-11-02): "MP back on a perfect parry is good" |
| Counter Chain+ (the counter chain stacks to 5) | Stagger Boost: parries fill Stagger 20 % more; old saves' entry reads as it, same AP | The counter chain is gone |
| Rank 3: a free follow-up (Pin Down launching, Return Fire splitting into three) | Rank 3: ×1.5 Stagger in all | The follow-ups were counters |
| Resolve 1 pip: counters +10 % | Parries fill +10 % Stagger | The counters are gone |
| In the Zone: a longer counter window | No counter window; its deflects fill the right technique's Stagger and give its MP | The counters are gone |
| Spell Ward's Ward Burst refilled 25 GUARD; Pin Down paused the enemy's AI for 0.8 s | Both gone with their counters | They were parts of the counters |
| The camp page's "Then (A)" column | A Stagger column (amount and one to three bars) and a footer that names the Breaker as his big hit (11.1) | Same quote: "Redesign the menu for this as well" |
| AI Goofy cashes counters and Breakers | AI Goofy's new plan (section 12): techniques by level, low-GUARD retreat, strikes into a bounce, chases Breakers, Arts by need and by the Customize settings | The user (2026-11-02): "Make sure Goofy as an AI party member uses his new tools properly" |
| AI Goofy walks to a broken enemy only in battle, near the party | In battle or not, within 6 m of him; the leash doesn't cut the walk. He counts as in battle while he walks or slams, and his tree's moves are refused meanwhile. A walk of his tree that is already running is turned toward the enemy, not aborted | In game (2026-11-03) he stood still while a soldier broke twice. Out of battle, a walk took the standing leader's follow speed (none), and an abort followed by a new walk in the same step left his locomotion standing |
| Hop and Slip are judged by the hit missing | Hop and Slip are judged from the attack's timing, and the hop sets the body's no-hit flags | The game's hit and throw never ask the kit |
| Ground combo: no lunge | Each step moves him forward: 81 cm and 117 cm along KH3's own root tracks of the two shoves, 60 cm for the bash. He stops short of an enemy's body and at any drop deeper than his step height. The wind-up before a hit plays 1.5x faster, the stick takes him out sooner, and the clips' TurnToTarget windows keep him facing his target | The user (2026-11-02): "Goofy's combat also feels EXTREMELY stilted" and "His combo should move him a little bit" |
| Any step cancels into Guard at any time, and the string starts again at once | The bash, the end of the string, has an end lag of 0.55 s after its hit (`combo.end_lag`). In it, A can't start a new string on the ground or in the air, and the stick, B, Arts and the Breaker do nothing. A press made late in the lag waits in the pad's 0.3 s buffer and fires when the lag ends. The hop and the guard (X) may cancel the lag from 0.2 s after the hit (`combo.end_lag_cancel`), but the lag's clock keeps running, so no string starts out of them either. The lag ends just before the clip does, and the clip hands back for its last frames. Sora's rule decides this: his last finisher (kind 4, clip 1304) can't attack, move or jump until its clip ends (0.83 s after its hit), and he can guard or dodge only from its TresEnableChange (0.23 s after the hit; 0.20 s at Goofy's 1.15x rate). KH3's own SGuardShieldBash runs out 0.595 s after its hit. The two shoves before the bash keep their chain and their guard cancel at any time, and AI Goofy has the same lag. The air bonk has no lag: there is one bonk per jump, and it ends when he lands | The user (2026-11-02): "He needs some end lag at the end of his combo so you can't spam it, like Sora." Before this, the bash's link point (0.62 clip s) started a new string 0.15 s after its hit |
| Air bonk: a big sphere ahead of him | KH3's own 50 cm sphere follows the shield along the clip's socket track, so it reaches about 1.4 m from him (it used to reach 2.9 m) | The user (2026-11-02): "His aerial hit's hitbox also extends way too far away from him" |
| Danger Sense never says which kind | With Danger Sense the "!" names the kind in a coloured word under it; without it the "!" is still plain | The user asked for it on 2026-11-02 |
| Arts cost cheap MP (2 to 6) and have no cooldown | Each Art except Taunt costs about four to five times as much and has a cooldown, shown on its LB panel entry by KH3's situation command timer gauge and a seconds count (section 8.1) | The user (2026-11-02): "Goofy's abilities (except taunt) should cost more MP, and introduce a cooldown with a visible timer, they are way too spammable" |

**Guarded blows and blood (the user, 2026-11-02).** A block, deflect or parry of an Elden Ring blow plays only his own clang, from his kit's sheet rows. It makes no hurt sound and gives no blood. If he is leading, the hidden Elden Ring body's own sounds are switched off for 0.6 s after the blow, which is untested in game. Blood now builds up four times slower on every character. See `crates/kh-er/src/guardblow.rs` and the `guard_sound.*` and `blood.*` rows of `sheets/port/sora-er-host.json`.

**Enemy shields (the user, 2026-11-02: "make the character who attacks a shield bounce off playing their animation from KH3").** An Elden Ring enemy whose own guard is up guards his hits from its front, by KH3's guard rules (the same as Sora's and Donald's): KH3's guard spark `vs_com_haj00`, no hit sound, and no combo landing or Stagger from it. A swing of his own (his shoves, the air bonk and Arts that ride his body; the KH3 AI's Push / Sweep) bounces him off with his own KH3 recoil, `AttackReflect` (`AttackAirReflect` in the air), after a 0.05 s hold, and the combo ends. Shots and the thrown shield never bounce him. From behind it is a plain hit. Damage and guard pressure are Elden Ring's (its guard's cut, stamina, guard break). See `crates/kh-party/src/repel.rs`, `pad/goofy_shots.rs` `step_areas`, the `repel.*` rows of `sheets/port/party-hurt.json`, `crates/kh-er/src/shieldguard.rs`. Untested in game.

Clips marked as stand-ins in `build/sora_tmp/party/GOOFY/CLIPS.md` are still stand-ins, because new clips would need a player rebuild.

### Where it lives

| Part | Files |
|---|---|
| Rules, abilities, Stagger book, threat, AI | `crates/kh-party/src/goofy/` |
| Moves on the pad | `crates/kh-party/src/pad/goofy*.rs` (`goofy_counters.rs` now holds only the batted shot that Goofy Tornado's rank 3 sends back; the counters are gone) |
| Boss rewards | `crates/kh-host/src/bossrewards.rs`, `crates/kh-party/src/growth.rs` |
| Attack sensor in Elden Ring | `crates/kh-er/src/taebook.rs`, `telegraph.rs`, `attackread.rs` |
| Kneel, AI pause, priority | `crates/kh-er/src/stance.rs`, `aipause.rs`, `party/prio.rs` |
| Bounce off the shield (4.1) | Rule: `crates/kh-party/src/goofy/parry.rs` (`perfect`, `bounces`), `core.rs` (`mid_string`), `seams.rs` (`on_bounce`). Elden Ring's rebound: `crates/kh-er/src/rebound.rs`, asked by `targets.rs` (`StaggerAsk::Rebound`) |
| HUD | `crates/kh-ui/src/hud_goofy.rs`, `hud_guard.rs`; the Arts' cooldown on the LB panel: `crates/kh-ui/src/command.rs` (`shortcut_rows`), `hud_command.rs` (`set_shortcut_recharge`) |
| Camp menu's Parry Techniques page (11.1) | Rows and words: `crates/kh-party/src/goofy/guide.rs`. Feed: `crates/kh-partyhost/src/parry_page.rs` (`PartyMenuMember::parry`). Page: `crates/kh-ui/src/camp_parry.rs`, shown by `camp.rs` / `camp_menu.rs`. Host: `crates/kh-host/src/hud.rs` (`refresh_camp`) |
| Numbers | `sheets/port/party-kit-goofy.json`, `party-guard.json`, `party-er-aggro.json`, `party-er-bodies.json`, `party-balance.json` |

The in-game results are in `build/sora_tmp/party/GOOFY/INGAME_NOTES.md`.

### Not seen in game yet

Unit tests cover these, but nobody has watched them happen in Elden Ring:

- Interpose, leading or as AI.
- The bounce off the shield (4.1). It fires Elden Ring's rebound events `W_Repelled_Large`, then `W_Repelled_Small`, then `W_Repelled_Wall`, 0.2 s apart, until the swing ends. If none of them works, a light flinch is asked instead. Which of these an enemy answers has never been tried; the log's `rebound:` lines say which step worked.
- AI Goofy's Hop and Slip Counter.
- The new Stagger numbers and the slower drain in a real fight: how fast an enemy breaks, and whether 5 s and 4 a second feel right.
- Goofy Breaker at 8.0×.
- MP from perfect parries.
- AI Goofy's new plan in Elden Ring: chasing Breakers, Bash at high Stagger, Tornado on packs, the Customize AI settings and the low-GUARD retreat.
- The off-screen "!" and its sound.
- Danger Sense's word under the "!" over a real enemy: whether it reads at a glance, and whether it covers the enemy's head too much. The size and gap are sheet rows. It is checked only in the offline HUD pictures `build/sora_tmp/party/GOOFY/shots_ui/36_danger_sense_every_kind.png` and `37_danger_sense_on_the_edge.png`.
- Armour lowering damage on a friend.
- The Arts' new MP prices and cooldowns in a real fight, and the cooldown gauge and count on the LB panel. The panel is checked only in the offline pictures `build/sora_tmp/hud/t_gof_recharge_running_zoom.png` and `t_gof_recharge_back_zoom.png`.
- A spear thrust watched on screen. Thrust rows are read correctly in the logs.
- Hit-stop and the clang judged by a person, or the kit played by hand on a real pad.
- The bash's end lag (0.55 s, cancelled by the hop or the guard from 0.2 s) on a real pad: whether it stops the spam without feeling sluggish, and whether the clip's last frames read as a recovery pose.
- Chests, ladders and a story cutscene with this build. Those routes weren't changed.
- The camp menu's Parry Techniques page (11.1) in Elden Ring: Y on Goofy's Abilities screen, with Goofy leading and as an AI member; whether the small stick icons read at the game's resolution; the new Stagger column and the footer. It is checked only in the offline pictures `build/sora_tmp/party/GOOFY/shots/ui_parry_lv1.png`, `ui_parry_lv40.png` and `hud_parry_*.png`.
