# Donald Duck — Playable Kit Overhaul (design)

Status: **built** (streams DON-KIT, DON-MAGIC, DON-PROG, DON-UI, ZETTA and later, from 2026-10-22); the numbers below are the game's as of 2026-10-28 (the sheets under `sheets/port` win where they differ). This document **replaces Donald's whole former kit**: the XD_PLAYABLE_KITS Donald sections, the X2/X3D work, BAL's Donald ratios and anything else already in game. Section 13 lists what gets removed.

Pad layout used throughout: KH3 on an Xbox pad. **A** attack/confirm, **B** jump/cancel, **X** dodge/block, **Y** situation command, **LB** shortcuts, **RB** lock-on, **D-pad** command menu. Keyboard is extra, never required.

---

## 1. Who Donald is

> "Donald Duck is THE mage of Kingdom Hearts, so he should play like it with a very involved magic system that allows for deep player expression. Donald is a ranged glass cannon, he can deal fantastic damage from a distance but really struggles when enemies get in his face. Donald relies on his allies to keep aggro off of himself, with supporting abilities that he earns via levelling up that reduces the chances of enemies aggroing to him when he is in a party with at least one other character. Donald has a unique elemental attunement system that changes multiple parts of his kit." (user)

Design pillars:

1. **The command menu is his weapon.** Sora can play well by mashing A; Donald can't. His damage, his safety and his MP all come from choosing the right spell, in the right element, at the right moment, from the menu and the shortcuts.
2. **Range is life.** Every tool rewards staying at mid to long range. Up close he is weak, slow to recover and easy to punish.
3. **Attunement changes everything.** The element he is attuned to rewrites his basic attack, his dodge, his finisher, his spell costs and the party buff he gives. Switching attunement at the right time is the skill ceiling.
4. **He needs a party.** Solo Donald is fragile. In a party, his abilities pull enemy attention onto his friends.

---

## 2. Stats (overrides BAL for Donald)

| | vs Sora | Notes |
|---|---|---|
| Magic | **1.2×** | Highest in the game: **18 at level 1** on the status screen (his row 14 + the starting staff's 4), 49 at level 50, 80 at level 99 (2026-10-27, the user: "make Donald's starting magic 18 at level 1"; was 1.25×, 19 at level 1). KH3's growth curve kept. |
| Max MP | **1.25×** | 1.25× **Sora's max MP now**, plus his own MP victory bonuses and his Max MP +3 boss steps (9). Sora's MP does not grow with levels, only from bosses, so Donald's grows with it. Sora's opening choice (100 / 110 / 120 MP for Vitality / Balance / Wisdom) gives Donald **125 / 138 / 150** at the start (2026-10-27, the user: "make Donald have x1.25 MP"; was 1.5×). |
| Strength | **0.667×** | About **10 at level 1** on the status screen with the starting staff (2026-10-27, the user: "lower Donald's physical damage as well, it should be something like 10 at level 1", "I mean the KH3 attack stat in the status screen"; was 0.55×). Only matters for his close-range Wand Bonk (5.3); his missiles are magic. |
| Max HP | **0.85×** | 0.85× Sora's max HP now, plus his own HP bonuses. Glass cannon. |
| Defense | **0.80×** | Glass cannon. |
| MP regen / MP Charge | KH3 rules, plus attunement refunds (4.4) | MP Charge length scales with Magic, as in KH3. |

**One set of stats, leading or not** (ONESTAT, 2026-10-27, the user: "there should be no party ally/leader seperation, their stats should be their stats same for Goofy and every KH character added later"): the ratios above are his stats whether he leads or follows as an AI ally.

Every stat is a sheet row (`sheets/port/party-balance.json`, the level table `party-balance-levels.json` built from it by `tools/party_balance_sheet.js`), so it can be tuned without touching the code.

---

## 3. Basic attack: magic missiles

> "Donald's basic attack is not a melee hit, but a homing magic missile that he shoots from his wand. His initial three-hit combo string is him waving his wand to the left, then to the right, then slamming the ground with it which causes double magic missiles to burst from his wand. For the final hit in the combo string, the doubled magic missiles deal x0.75 the normal damage." (user)

### 3.1 Ground combo (A, A, A)

| Step | Motion | Shot | Damage per missile |
|---|---|---|---|
| 1 | Wand wave to the **left** | 1 homing missile | 1.0 |
| 2 | Wand wave to the **right** | 1 homing missile | 1.0 |
| 3 (finisher) | **Ground slam** with the wand | **2 missiles** burst out | **0.75 each** (1.5 total) |

- **Missile:** about 18 m/s, homes on the lock-on target (else the soft target), soft homing (turn rate capped, so a sidestep can dodge it), 30 m range, then fades out.
- **Element:** missiles are tinted and behave by his current attunement (4.2). With no attunement they are plain arcane missiles.
- **Range sweet spot:** full damage from 6 to 25 m. Under 6 m, damage drops to 0.6×; under 2 m, A becomes the Wand Bonk instead (5.3). **With no target at all** (nothing soft-targeted and nothing locked on), A is always the Wand Bonk: missiles are only fired at a target.
- **Cancels:** any step can cancel into a spell, a shortcut or a dodge after its missile leaves the wand, so missiles weave into spellcasting. That weaving is the core rhythm.
- **Moving while casting the string:** with the stick held, Donald **strafes** (stick left / right) or **hops back or forward** (stick back / forward) in the window between projectiles (an input buffer catches it), always **facing the targeted enemy** while he moves. The final ground slam is the exception: he stands still for it.
- **Combo Plus** abilities add steps (3 → 4 → 5), each a single missile, before the slam.
- **Finisher abilities** replace the slam (section 9).
- Animation: his own KH3 staff clips (`M_1301` Beat and `M_1302` Swing for the waves), plus a ground-slam clip, picked when it's built.

### 3.2 Air combo (A in the air)

Two quick downward missiles, then he floats for 0.5 s. **No air finisher by default**; abilities add one. Air time is short: he is a ground mage who hops, not an aerial fighter.

### 3.3 Holding A: Charged Missile

Hold A for 0.8 s and release to fire one large missile at 2.5×. It pierces, and its attunement effect is doubled. He is never frozen while charging: he walks slowly in any direction (with a walk animation), facing his target. It costs no MP, so it's how he fights when MP is out.

---

## 4. Attunement — Donald's signature system

**Command menu: Attack / Magic / Attunement / Items.** Attunement is its own top-level command, between Magic and Items, with the elements in its submenu. Any element can also be put on a shortcut (LB + face button).

### 4.1 The basics

- He is attuned to **one element at a time**: **Fire, Blizzard, Thunder, Water, Aero** or **Light** (Light covers healing and holy magic). **An element's attunement unlocks as soon as he has any spell of that element**, from any of its lines (so Banish from the Tree Sentinel unlocks Light, section 9).
- **Resonance** is a 3-pip meter on his HUD, in the element's colour around his MP bar. It fills by:
  - casting spells of the attuned element (large gain);
  - landing missiles (small gain);
  - landing the right Elemental Reaction (4.5).
- Resonance drains slowly out of combat. It doesn't drain in combat unless he is hit hard. A heavy hit takes away one pip, so getting caught up close costs him his build-up.
- **Attuning takes a 1 s cast** and can be interrupted; it is the price of switching. The ability **Swift Attune** halves it, and **Instant Attune** removes it.
- **Unattune:** the last entry of the Attunement submenu (also shortcut-able) clears his attunement (both elements with Dual) and returns him to plain arcane missiles, the plain dodge and slam, and no aura. It is cast like attuning; at Resonance 3 it Releases the element, as switching away does.

### 4.2 What attunement changes

Each element rewrites **six** parts of his kit:

| Element | Basic missiles | Dodge (X + stick) | Combo finisher | Spell cost | Party aura (when in a party) | Resonance 3 bonus |
|---|---|---|---|---|---|---|
| **Fire** | Explode on hit (small splash), apply **Burn** | **Flame Step**: dash that leaves a fire trail | Slam becomes a 3-missile fireball spread | Fire spells at the attuned price (6.2) | Party attack +10 % | Firaza and **Inferno** castable; missiles ignite the ground |
| **Blizzard** | Apply **Chill** (stacks slow the target; 5 stacks = **Frozen** for 2 s) | **Ice Glide**: long low slide, enemies hit are Chilled | Slam fires an ice lance that pierces | Blizzard spells at the attuned price (6.2) | Enemies near Donald −15 % speed | Blizzaza and **Diamond Dust** castable; Frozen lasts 4 s |
| **Thunder** | Missiles **chain** to a second target within 8 m | **Blink**: short teleport in the stick's direction, invincible | Slam calls a bolt from above on the target | Thunder spells at the attuned price (6.2) | Party movement +10 % | Thundaza and **Judgment Bolt** castable; chains hit 3 targets |
| **Water** | Missiles **pierce** and apply **Wet** | **Bubble Hop**: hop inside a bubble that absorbs one hit | Slam fires a water wave that pushes enemies away | Water spells at the attuned price (6.2) | Party MP regen +25 % | Waterza and **Tsunami** castable; Wet lasts twice as long |
| **Aero** | Missiles curve around cover; on a third hit they **launch** small enemies (juggles) | **Air Step**: a mid-air dash, and he can dodge twice in the air | Slam makes a tornado that lifts enemies | Aero spells at the attuned price (6.2) | Party evasion and recovery speed up | Aeroza and **Aerial Blast** castable; launches work on medium enemies |
| **Light** | Missiles that pass an ally **heal** them a little; hits **Reveal** enemies (more damage taken) | **Ward Step**: a short dodge that leaves a barrier for allies | Slam makes a healing pulse around Donald | Light spells at the attuned price (6.2) | Party Regen | Curaza and **Holy** castable |

### 4.3 Release: the switch itself is a move

**When he switches attunement with Resonance at 3**, the old element is **Released**: an instant burst around the lock-on target, free of MP.

| Released element | Burst |
|---|---|
| Fire | **Firestorm**: a fire pillar. |
| Blizzard | **Shatter**: an ice burst that breaks Frozen enemies for big damage. |
| Thunder | **Overload**: lightning on every enemy within 10 m. |
| Water | **Deluge**: a flood wave that applies Wet to everything around. |
| Aero | **Cyclone**: a whirlwind that gathers enemies into one spot. |
| Light | **Sanctuary**: heals the party and grants a short invincibility. |

A Release also starts the new element at 1 pip instead of 0. So the expert loop is: build to 3, **switch** (Release), then build again in the new element, chaining Releases and Reactions (4.5). That loop is what makes his damage spike.

### 4.4 MP and attunement

- Spells of the attuned element use the attuned price in the MP table (6.2). The -za tier and the line-3 ultimate can only be cast at **full-power attunement**: attuned to their element with Resonance at 3. The ultimate can be cast **once per attunement period** (6.1).
- Each Resonance pip **refunds 2 % of max MP** when it fills.
- **Off-element casting** (a spell not of the attuned element) costs full MP and gives no Resonance, but it is how Reactions are set up.
- **MP Charge:** KH3 rules, and Donald keeps firing missiles during it. Landing missiles during MP Charge shortens it.

### 4.5 Elemental Reactions (why the menu matters)

Statuses from one element, hit by another element, react. Reactions do bonus damage and fill Resonance by a full pip.

| Status on target | Hit with | Reaction | Effect |
|---|---|---|---|
| **Wet** | Thunder | **Conduct** | 2× Thunder damage, chains to every Wet enemy |
| **Wet** | Blizzard | **Flash Freeze** | Instant Frozen, regardless of stacks |
| **Chill / Frozen** | Fire | **Steam Burst** | Big damage plus **Blind** (see below) |
| **Burn** | Aero | **Wildfire** | The Burn spreads to every enemy near the target |
| **Frozen** | Aero or Thunder | **Shatter** | Huge single-target damage; ends Frozen |
| **Revealed** | any | **Exposed** | +25 % damage from everyone in the party |
| **Wet** | Fire | **Fizzle** | Removes Wet, weak damage. A *bad* combo: knowing what not to do matters too. |


**Blind** (from Steam Burst and Scald): a Blind enemy loses track of the party. It keeps attacking **the spot where it last saw its target at the moment it was blinded**, swinging at empty air there, until Blind wears off. Anyone who stays out of that spot is safe from it; anyone standing in it still gets hit. It never redirects a hit onto someone else.
Example (Wet → Conduct → Release → Flash Freeze → Shatter):

1. Attune Water, hit a group with Watera.
2. Switch to Thunder (the Release floods them, Deluge).
3. Thundara for Conduct on everyone.
4. Switch to Blizzard (the Release is Overload).
5. Blizzara for Flash Freeze.
6. Charged Missile for Shatter.

That is the "mastery of the active menu" fantasy.

### 4.6 Dual Attunement (late game)

The ability **Dual Attunement** (level ~80) lets him hold **two elements at once**. Each gets its own Resonance meter, and missiles alternate between them. Every pair of elements gains a hybrid spell while both are attuned (section 6.4).

---

## 5. Defense and survival — the weak side, on purpose

### 5.1 Dodge (X + stick)

The dodge is set by attunement (4.2). Without attunement it's a short hop (300 cm) with brief invincibility. The dodge has a **cooldown of 0.9 s**, so it can't be spammed.

### 5.2 Block (X, neutral)

**Magic Block** is a frontal barrier with a short window that blocks one hit. A perfect block, in the first 0.15 s, **reflects projectiles** and fills one Resonance pip. A missed block leaves him open.

### 5.3 Up close

- **Wand Bonk:** when A is pressed with an enemy within 2 m, **or with no target at all** (no soft target and no lock-on), he swings his staff instead of shooting. It is weak, but it **knocks the enemy back 3 m** to make room. It can't be spammed: he can only Bonk again once the Bonk animation has finished. It doesn't count toward the combo.
- Getting hit while casting interrupts the spell; the MP is spent and lost. Abilities can reduce this (section 9).
- **Heavy hits knock him down for longer than Sora**, using the shared HR hit-reaction rules with his own timings.

### 5.4 Aggro (being targeted by enemies)

When he is in a party with at least one other character, enemies pick him as their target less often. This is earned through levelling up:

| Ability | How he gets it | Effect |
|---|---|---|
| **Low Profile** | Level ~10 | −20 % chance enemies target him |
| **Hide Behind** | Level ~25 | −40 % while an ally is within 8 m of him |
| **Distraction** | Level ~40 | When an enemy targets him, 30 % chance it switches to the nearest ally |
| **Taunt Pass** | Level ~55 | When an ally blocks or guards (Goofy's GUARD), enemies near Donald switch to that ally |
| **Out of Sight** | Level ~70 | Enemies lose interest in him while he stands still and casts at over 15 m |

When he plays alone (no party), none of these apply.

---

## 6. Spells

**Rule:** Donald gets magic **higher than Sora's**, up to the **"-za" tier**. Each element has **three spell lines from Final Fantasy**:

- the main line has strength tiers (base → -ra → -ga → -za);
- the second spell has three levels;
- the third is that element's ultimate, which comes with the element's -za tier (section 9) and is cast only at full-power attunement.

### 6.1 Spell list

| Element | Line 1 (tiers) | Line 2 (3 levels) | Line 3 (ultimate) |
|---|---|---|---|
| **Fire** | Fire → Fira → Firaga → **Firaza** | **Flare**: his single-target nuke. A small white-hot star leaves the wand after a 0.8 s cast (the longest of line 2, so getting hit cancels it), flies at the lock-on target, shrinks for 0.5 s on arrival, then detonates for heavy damage with a 2 m splash and a big stagger. On a **Burned** target it consumes the Burn for +50 % damage. Lv 2: 0.6 s cast, more damage, 3 m splash; Lv 3: 0.4 s cast, and an after-flare blast follows 1 s later at half damage | **Inferno**: a fire field that burns everything in it |
| **Blizzard** | Blizzard → Blizzara → Blizzaga → **Blizzaza** | **Freeze**: an ice prison. Frost gathers on the lock-on target for 1 s (a visible warning), then encases it: **Frozen** for 3 s with light damage. A Frozen enemy can't act; other hits shorten the ice, and Aero, Thunder or a Charged Missile breaks it for **Shatter** (4.5). Large enemies and bosses don't freeze: they take 3 Chill stacks and move 30 % slower for 4 s. Lv 2: freezes every small and medium enemy within 3 m of the target, for 4 s; Lv 3: 5 m and 5 s, and each enemy freezing throws ice shards that Chill its neighbours | **Diamond Dust**: a blizzard over a large area that freezes all |
| **Thunder** | Thunder → Thundara → Thundaga → **Thundaza** | **Ball Lightning**: 3 crackling orbs circle Donald or **any party member** (6.1) for 5 s. They are real **balls of lightning orbiting the wearer**, and they only affect an enemy that **touches a ball**: that ball deals Thunder damage, knocks the enemy back and is **used up** (it disappears), so all of them can be spent before the time runs out. On Donald they guard his weak close range; on a melee friend they add hits around him. Casting it again on the same member while orbs remain hurls them all as homing shots at the lock-on target, and an orb that hits a Wet enemy triggers Conduct. One set of orbs per member. Lv 2: 4 orbs for 12 s, recharging one at a time; Lv 3: 5 orbs for 20 s, and a ball that is touched also arcs to a second enemy nearby | **Judgment Bolt**: a storm of bolts on every enemy around |
| **Water** | Water → Watera → Waterga → **Waterza** | **Flood**: his "get away from me" spell. A fast 0.3 s cast: Donald slams the wand down, and a wave rolls out in front of him in a 90° arc for 8 m. It deals moderate damage, applies **Wet**, pushes enemies 5 m back and interrupts small and medium enemies' attacks. Enemies pushed into a wall take extra damage and are stunned for 1 s. The wave leaves a puddle for 6 s that keeps enemies in it Wet. Lv 2: a 120° arc, 12 m, 7 m push; Lv 3: a full ring around him, 15 m, and it knocks small enemies down | **Tsunami**: a huge wave that sweeps the area |
| **Aero** | Aero → Aerora → Aeroga → **Aeroza** | **Aero Barrier**: KH1's Aero, a wind barrier that cuts the damage its wearer takes. Lv 2 (KH1's Aerora) is stronger and lasts longer; Lv 3 (KH1's Aeroga) also hurts enemies that touch it. Castable on Donald or **any party member** (6.1) | **Aerial Blast**: a wind cannon that launches everything hit |
| **Light** | Cure → Cura → Curaga → **Curaza** (heals) | **Banish** (Final Fantasy XI's holy spell; a ray of holy light on the target that deals light damage and Reveals it; Lv 2–3, Banish II and III, fire more rays that also strike nearby enemies) | **Holy**: big light damage on one target |

- **No walking while casting:** during any spell cast (menu, shortcut, Y, ultimates, Zettaflare, attuning) Donald never walks; his cast animation always takes priority. Only his basic attacks move him (the strafe / hop between missiles, the slow walk while charging).
- **Full-power attunement:** attuned to an element with Resonance at 3 (4.2). It is needed for both that element's **-za tier** and its **line-3 ultimate**. Otherwise the menu casts the -ga tier, and the ultimate shows greyed. This ties the strongest magic to the attunement game.
- **Ultimates: once per attunement period.** An attunement period runs from attuning to an element until he switches away. In it he can cast that element's ultimate **once**. To cast an ultimate again he must switch attunement and power the new element up to Resonance 3, which opens a new period (and that element's ultimate). Switching back to an element later also starts a fresh period for it. The -za tier has no such limit while at full power.
- **Ultimates' presentation:** every ultimate is cast like Zettaflare (6.5): the same casting circle and Donald's channel and cast animation, rooted and interruptible, but **no cinematic camera** and a **3 s** cast (Zettaflare keeps 5 s). The circle and every effect tied to it take **the element's colour**.
- **On the ground only:** the ultimates, Ultima and Zettaflare can't be cast while Donald is in the air; in the air they show greyed ("On the ground only").
- **Casting on allies (KH1 style), for Aero Barrier and Ball Lightning:** after choosing the spell, a small target list opens with the party members' portraits and HP: Donald, then each friend in the party. D-pad up/down picks, A casts, B goes back. Its default is the last target. From a shortcut, the same list opens, so casting on a friend is one extra A press. The barrier shows as a swirl of wind around whoever wears it, and its HUD icon sits by that member's portrait. Recasting renews it. **A member can wear only one of these ally spells at a time** (Aero Barrier or Ball Lightning): casting the other replaces it. Its icon sits beside a friend's portrait, or between the leader's MP and HP bars.
- **Per element:** each element's three spells sit in its own sub-list: Magic → Fire → Fire / Flare / Inferno.
- **Weak spells stay useful:** base-tier spells are cheap and fast (good for setting up Reactions). Higher tiers hit harder and take a little longer to cast.

### 6.2 MP costs: fixed MP, like Sora

**Spells cost a fixed amount of MP, exactly like Sora's.** The Y spells (6.3) cost set amounts too. Line 1's first three tiers cost what Sora's KH3 spells cost; attuned, about a quarter less. There is no difficulty select: like Sora's, every cost below is multiplied by **1.5**, which is simply the game's base cost (user, 2026-10-22). As his max MP grows from bosses, more casts fit in a bar, as with Sora.

The -za tier is the expensive one: **41 MP** a cast (27 × 1.5), with no discount. At his starting max MP (138 with Sora's Balance start; 125 / 150 with Vitality / Wisdom) **a full MP bar holds three -za casts**. (It was four at the old 1.5× start of 165; with the MP ratio at 1.25× the user chose to keep the -za cost, 2026-10-27.) For the line-3 ultimates, reaching Resonance 3 is the real price, not the MP.

| Spell (MP, before the ×1.5) | Not attuned | Attuned |
|---|---|---|
| Fire / Fira / Firaga | 10 / 14 / 20 | 8 / 11 / 15 |
| Blizzard / Blizzara / Blizzaga | 12 / 17 / 24 | 9 / 13 / 18 |
| Thunder / Thundara / Thundaga | 14 / 20 / 28 | 11 / 15 / 21 |
| Water / Watera / Waterga | 12 / 15 / 22 | 9 / 11 / 17 |
| Aero / Aerora / Aeroga | 13 / 18 / 26 | 10 / 14 / 20 |
| Cure / Cura / Curaga | 13 / 17 / 23 | 10 / 12 / 17 |
| **-za tier** (full-power attunement only; Curaza too) | — | **27** (41 after ×1.5: three per starting bar of 125 / 138 / 150) |
| Line 2 (Flare, Freeze, Ball Lightning, Flood, Aero Barrier, Banish), Lv 1 / 2 / 3 | 10 / 13 / 17 | 7 / 10 / 12 |
| **Line 3 ultimates** (full-power attunement, once per attunement period) | — | **20**, and only 13 if cast within 3 s of reaching Resonance 3 |
| Hybrids (6.4) | — | 15 |

Each Resonance pip also refunds 2 % of max MP (4.4), so a good attunement loop wins back some of the MP.

### 6.3 Non-elemental magic on Y (replaces Resonance Magic)

KH3's Grand Magic slot becomes Donald's **non-elemental Arcane magic**, cast with **Y** as a situation command, each with **its own set MP cost**, like the rest of his magic. Y is only offered while he has at least that much MP; below it the Y command shows greyed with the MP it needs. The charges still run out on their normal timer, so he has to manage MP to be ready.

**The trigger is Arcane Flow**, a **4-charge** meter. It fills from **playing the attunement system well**, not from spamming one spell:

| Action | Arcane Flow |
|---|---|
| Landing an Elemental Reaction (4.5) | +1 |
| A Release (switching at Resonance 3, 4.3) | +1 |
| Casting spells of 3 different elements within 10 s | +1 |
| A perfect Magic Block (5.2) | +1 |

- **Decay:** charges last 20 s in combat and empty out of combat.
- **What Y offers: every Y spell he can afford, all at once.** Each spell has a **cost in charges**. Y lists every learned spell whose cost is at or below his current charges, stacked like KH3's situation commands and cycled the same way. Nothing overwrites anything: the player selects which one to cast.
- **Casting spends only that spell's cost.** With 2 charges, casting Gravity (1) leaves 1 charge; with 3, casting Comet (2) leaves 1 (user, 2026-10-23: "make gravity cost 1 star and comet cost 2 stars"). So he chooses between spending a little now or saving up for Meteor or Ultima (which also needs full-power attunement).
- **MP:** each Y spell has a set MP cost (before the ×1.5 of 6.2): Comet 40, Comet II 45; Gravity 45, Gravira 55, Graviga 65; Meteor 55 / 60 / 65; Ultima 70. Zettaflare: **a FULL MP bar, all of it spent; he goes into MP Charge (the red bar, as after Cure), recharging 30 % slower** (`zetta.regen_mult` 0.7; user, 2026-10-23, revised: "it needs all of your current MP to be useable ... unlike heal needs all of your MP and recharge takes 30% longer", confirmed as "Requires a full MP bar"). The Y list offers it only at full MP. The Y list shows only what he can cast right now (charges, MP, on the ground): each row is just the name and its star cost, with no reasons or MP shown.

| Cost (charges) | Y spell | Levels | Effect |
|---|---|---|---|
| 1 | **Gravity** | Gravity → **Gravira** → **Graviga** | Crushes and pulls enemies together inside a gravitational field, then they stay close (perfect for area spells). **Damage: a share of each enemy's CURRENT HP**, so it hits big, healthy enemies hardest and every repeat cast takes less as the bar shrinks. The share grows with Donald's **Magic**: base share for the tier + Magic × a sheet factor, capped (sheet rows). **Bosses resist it by 90 %** (they lose a tenth of the normal share). Higher tiers: a bigger field, a bigger share and a higher MP cost. Lv 1: the target plus enemies within 4 m, held 2 s; Lv 2: 6 m, more damage, held 3 s; Lv 3: 9 m, the most damage, held 4 s. **Look:** a dark KH1-style sphere around the target, swirling blue-violet inside, a jagged violet lightning band across its middle and a glowing pink ring on the ground. **Motion:** the ball squishes down while sinking toward the floor, pressing onto the enemies, and fades away once it is thin enough. **Flatten:** that descending ball is what squashes every non-boss enemy flat; they stay flat while held and spring back to normal 0.2 s after the spell ends; **bosses are never flattened**. |
| 2 | **Comet** | Comet → **Comet II** | A falling star on the lock-on target: big single-target damage, small blast. Comet II drops two stars. |
| 3 | **Meteor** | Meteor Lv 1 → 2 → 3 | A rain of meteors around the target. Lv 1: 3 meteors over 6 m; Lv 2: 6 meteors over 9 m; Lv 3: 10 meteors over 12 m, each knocking enemies down. |
| **4** + full-power attunement (any element, Resonance 3) | **Ultima** | one level | Its own spell: a huge sphere of pure magic bursts on the target, 10 m wide, ignoring elemental resistances. It takes a full Arcane Flow meter, and is listed only while he is at full-power attunement in some element. |
| 4 + the Zettaflare condition | **Zettaflare** | one level | 6.5 |

- **Learning them:** **Gravity (Lv 1) is known from the start**, so he always has a basic Y attack. Comet, Meteor and Ultima are learned from Arcane bosses, and Comet, Gravity and Meteor gain their levels from them too, **with the same rules as every other spell** (section 9). Until then they simply don't appear in the Y list.
- **Sharing Y:** team attacks with friends also use Y. When both are waiting, they stack as KH3's situation commands do, so the player picks.

### 6.4 Hybrid spells (with Dual Attunement)

Every pair of elements has its own hybrid spell (15 in all). A hybrid appears in a **Hybrid** sub-list at the bottom of Magic, under Light, while both elements of its pair are attuned.
- **Cost:** 15 MP (6.2).
- **Resonance:** it fills both meters.
- **Reactions:** it counts as both elements, so it can set up or trigger a Reaction (4.5) from either side.
- **Light hybrids never heal.** Light's own part in a hybrid is holy damage plus **Reveal** (4.2), which sets up the Exposed reaction (4.5). Healing stays with the Cure line and Light attunement (its missiles, slam and party Regen aura).

| Attuned pair | Hybrid spell | Effect |
|---|---|---|
| Fire + Blizzard | **Thermal Shock** | Hot and cold strike the target at once. Big damage that ignores part of its defence; breaks guards and shields. |
| Fire + Thunder | **Plasma** | A slow ball of plasma that detonates on contact. Burns everything in the blast, and the Burn arcs to one more enemy each second. |
| Fire + Water | **Scald** | A cloud of steam: Burn over time plus Blind. The one way to mix Fire and Water without a Fizzle. |
| Fire + Aero | **Fire Whirl** | A fire tornado that wanders after the target, pulling enemies in and burning them. |
| Fire + Light | **Sunburst** | A small sun hangs over the target for 6 s, burning and Revealing every enemy beneath it. Enemies both Burned and Revealed take extra fire damage. |
| Blizzard + Thunder | **Hailstorm** | Hail and lightning over an area. Hail adds Chill stacks; lightning chains between Chilled enemies. |
| Blizzard + Water | **Glacier** | A wall of ice rises in a line. Enemies caught are Frozen, and the wall blocks projectiles for 5 s. |
| Blizzard + Aero | **Whiteout** | A blizzard around Donald. Enemies inside are Chilled each second and their attacks miss more often. |
| Blizzard + Light | **Prism Frost** | A cone of glittering ice shards. Each shard adds a Chill stack and Reveals; an enemy that becomes Frozen stays Revealed until it thaws. |
| Thunder + Water | **Thunderstorm** | A storm cloud that chains lightning forever between Wet enemies. |
| Thunder + Aero | **Tempest** | A moving storm front. Bolts strike as it passes, and its wind launches small enemies into juggles. |
| Thunder + Light | **Divine Spark** | A holy bolt that leaps from enemy to enemy, Revealing each one it hits. It jumps once more for every enemy already Revealed. |
| Water + Aero | **Maelstrom** | A whirlpool that drags a group to its centre and leaves them Wet (perfect before Thunder). |
| Water + Light | **Shining Rain** | A glittering rain over an area. Every enemy under it is Wet and Revealed, setting up Conduct and Exposed together. |
| Aero + Light | **Radiant Gale** | A wind of light feathers that knocks enemies back and Reveals them. Enemies it launches stay Revealed while airborne. |

### 6.5 The capstone: Zettaflare (on Y)

**Zettaflare** is his ultimate magic, from a boss mid-way into the DLC (section 9), and the top of the Y Arcane magic. Y lists it alongside the other Y spells when he has **4 Arcane Flow charges** (a full meter) and **every element he has learned has reached Resonance 3 at least once in the current fight**. Small HUD icons track the elements. It needs **a FULL MP bar** (MP = max MP) and spends all of it: he goes into MP Charge (the red bar, as after Cure), which recharges **30 % slower** (`zetta.regen_mult` 0.7) (user, 2026-10-23, revised: "Zettaflare needs to use up all of your magic (it needs all of your current MP to be useable) and then put you into the red MP bar mode with a 30% slower recharge rate"; confirmed as "Requires a full MP bar"). The F7 / L3+R3 test aid tops MP up to full, and it empties his Resonance and Arcane Flow. **Presentation:** it plays KH3's own Zettaflare cutscene (Keyblade Graveyard, kg702b) aimed at the target: a 5 s channel, his cast and "Zettaflare!", with a cinematic camera that mimics the cutscene's shots. While it is cast (channel to hit), **the target is frozen in place and turns to look at Donald**, then **reacts to the beam exactly as Terra-Xehanort does in the cutscene**, held for the whole beam, and **immersed in the white light**: **if Zettaflare kills it, it becomes a black silhouette that crumbles away leaving only the light** (any enemy); if it survives, it is **held in place until the wide shot of the beam shrinking, then thrown straight back along the beam's direction** (user, 2026-10-23); the return to gameplay is seamless; **Donald is invincible for the whole sequence** and **no enemy targets him**, so nothing can interrupt it; **every enemy caught in the blast takes damage**; its damage **scales with Donald's Magic** (its power × his Magic, nothing else; user, 2026-10-23: "make sure Zettaflare scales with your magic stat", "make level 99 Zettaflare do 20,000 damage"): at first **20,000 Elden Ring HP at level 99** (Magic 71, the x7 conversion); since 2026-10-27 the host's x12 (the user: "allow Zettaflare to be scaled up") and his balanced Magic (Magic 1.2×, 80 at level 99) make it **about 38,640 at level 99**, about 23,700 at level 50 and 8,700 at level 1 (its power unchanged), landing in full; it **ignores all armour, defence and resistances and overrides everything** (super armour, poise, boss states) so it always works; and **AI allies attack other enemies and keep away from Zettaflare's target and the beam line**. The beam and the camera are aimed **at the target**.

(ZETTA, 2026-10-23) **The beam and its framing.** Every enemy shot is the user's PERFECT frame (the test level's Sora in the beam) scaled about the target: the body's full bounds (height and width with reach) fill the reference's region, 45 % of the height and 63 % of the width (sheet `zetta.frame_body_*`, `zetta.frame_share_*`), never cropped; the beam fills that frame. **The beam always starts at Donald's wand and never reaches behind him** (the user: "the beam MUST start at Donald's wand, and NEVER show the end of it"; "the beam must NEVER extend behind Donald"): the ending beam grows only across its line; its far end is out of every shot's frame from the hit on. **The one exemption: the launch.** Its head travels from the wand to the target before the hit, with its end in view (the user: "keep the travelling head"); the tests (`kh-host/tests/zetta_beam_lines.rs`, kh-party `the_ending_beams_far_end_is_never_in_the_long_shot`) check the far end from the hit on only.

(ZETTA, 2026-10-23) **After Zettaflare, the down.** At the hand-off Donald plays KH3's own collapse from the cutscene (kg702c n_ex001_702101w and 702102w): he staggers, his staff flies from his hand, and he falls on his back. He lies there `zetta.down_time` (2 s), with no control but the camera free, then gets up with KH3's get-up off his back (M_0213). He cries out as he falls (his damage voice) and says his RecoveryHP line as he gets up. **His staff stays where it fell.** At the get-up it dissolves in his staff's own appear / disappear light (vw_don_app00) and appears in his hand in the same light, with the Keyblade's appear sound (se02001_010). He can be hurt throughout: a hit ends the down at once and his usual reaction plays. The player has him back when it ends; the staff stays on the ground until then and is recalled the moment he is the player's.

### 6.6 Team attacks (Y, with friends)

The existing team attacks stay with Y when the right friends are near (as KH3): Donald's with Sora and with Goofy. They gain a bonus when Donald is at Resonance 3.

---

## 7. Command menu and shortcuts

Donald's command menu:

```
Attack
Magic ▸       Fire     ▸ Fire(-ra/-ga/-za) / Flare / Inferno
              Blizzard ▸ ...
              Thunder  ▸ ...
              Water    ▸ ...
              Aero     ▸ Aero(-ra/-ga/-za) / Aero Barrier ▸ target / Aerial Blast
              Light    ▸ Cure(-ra/-ga/-za) / Banish / Holy
              Hybrid   ▸ the hybrid of the two attuned elements (only with Dual Attunement, 6.4)
Attunement ▸  Fire / Blizzard / Thunder / Water / Aero / Light / Unattune   (the current one is marked)
Items
```

- **Attunement is its own command, between Magic and Items.** It's reached with one D-pad press from either. Its submenu shows each element's Resonance and whether switching now would trigger a Release.
- **Navigation:** **Magic always opens the element select** (never straight into the last element's spells). After a spell or an attunement is chosen, the menu returns to the top with the cursor on **Attack**. In every menu (command and camp), **B goes back exactly one level**, never to the top.
- **Shortcuts:** LB + A / B / X / Y, with **KH3's three shortcut palettes from the start**, swapped exactly as in KH3. Any spell and any Attunement can be put on a shortcut.
- **Quick Cast** (ability): while LB is held, the shortcut panel shows a glow on the spell that would trigger a Reaction on the current target. (2026-10-28, the user: "remove how much MP something costs from Donald's shortcut commands": the panel never shows MP costs.)
- **The refused sound on a shortcut only for MP** (2026-10-28, the user: "remove the "error" sound from shortcut commands for every character, spamming magic makes it constantly play", "only play the error sound when you don't have enough MP for it"): a refused shortcut plays the refused SE only when he lacks the MP (the spell greyed for MP, or MP Charge); every other refusal (busy, mid-cast, needs Resonance, ...) is silent. The same rule holds for every character's shortcuts. (The panel shows no MP costs, Quick Cast above; his Magic submenu still shows "N MP" on a greyed row.)
- **HUD:**
  - the element ring and Resonance pips around his MP bar;
  - status icons (Wet, Chill, Burn, Reveal, Frozen) over the lock-on target;
  - the Arcane Flow charges (4 small stars) next to the Y command;
  - the Zettaflare tracker;
  - an ultimate mark on the Resonance ring: lit while the ultimate is ready in this attunement period, dark once it has been cast.

---

## 8. Movement

- **Run speed:** same as Sora (680), with no sprint slide. **Sprint** after 1 s of running is 900 (Sora: 980).
- **Jump:** lower than Sora's.
- **Glide:** **Aero Float**, a slow floaty glide, as now, with a **small Aero wind effect at the top of his wand**. With Aero attuned it becomes **Aero Glide**: faster, with longer air time, and the wand-tip effect is **bigger**. While gliding (Float or Glide) he **sways** gently, holding his staff.
- **Swim and ladders:** shared rules, as for every character (Y at a ladder jumps him up to its top).

---

## 9. Abilities (as many as Sora's)

He earns abilities from **levelling up**, and his spells and their tiers from **bosses**, each boss giving the next step up in its element (below), with AP costs like Sora's. Every ability has an in-game name and description. The full counts and AP costs are set when it's built; this is the list.

**Combat**

| Ability | What it does |
|---|---|
| Combo Plus ×2 | One extra missile step each |
| Air Combo Plus | One extra air missile |
| Charged Missile+ | Charge time 0.8 → 0.5 s |
| Missile Volley (finisher) | The slam fires 5 missiles at 0.5× each |
| Arcane Bolt (finisher) | The slam becomes one big piercing missile |
| Air Finisher: Starfall | Ends the air combo with a rain of missiles below |
| Wand Bonk+ | Bigger knockback up close |
| Cast Guard | 30 % chance a hit doesn't interrupt his spell |
| Second Chance / Withstand Combo | KH3 standard |

**Magic**

| Ability | What it does |
|---|---|
| Fire / Blizzard / Thunder / Water / Aero Boost | +20 % damage for that element |
| Cure Boost | +20 % healing |
| MP Haste / Hastera / Hastega | Shorter MP Charge |
| MP Rage | Restores MP when hit |
| Spell Weaver | Missiles right after a spell do +20 % damage |
| Reaction Plus | Reactions do +30 % damage |
| Resonance Keeper | Resonance drains slower |
| Swift Attune / Instant Attune | Attuning takes half / no cast time |
| Dual Attunement | 4.6 |
| Quick Cast | 7 |
| Spell Echo | Right after a -za spell, the next base-tier spell of the same element is free |
| **Scan** | KH3's Scan: shows the HP gauge of the enemy he targets. Without it, enemy HP isn't shown, as in KH3. Elemental Insight builds on it by adding weaknesses |
| Elemental Insight | Lock-on shows the target's weak and resisted elements, and the Attunement submenu marks the best element against it |
| Arcane Flow+ | Arcane Flow charges last 30 s instead of 20 s |
| Arcane Boost | Comet, Gravity, Meteor and Ultima +20 % damage |

**Support (party)**

| Ability | What it does |
|---|---|
| Low Profile / Hide Behind / Distraction / Out of Sight | 5.4 |
| Taunt Pass | 5.4 |
| Aura Range+ | His attunement aura reaches farther |
| Lucky Lucky, Treasure Magnet, Item Boost | KH3 standard |

**How Donald gets magic: Sora's system, plus one Shadow of the Erdtree boss per element.** Tiers 1 to 3 (up to -ga) use the same rule as Sora (`documentation/SORA_CURRENT.md`, section 5): each of an element's base-game bosses gives **one tier more of that element**, in any order. Donald has **one tier more than Sora**, the -za tier, and **the -za tier is only ever unlocked in Elden Ring's DLC (Shadow of the Erdtree)**: every element has one **DLC boss** that gives it. The magic bosses are **shared**:

- **Donald:** each base-game boss of the element gives his next tier, **up to -ga**; once he has -ga, a further base-game boss of that element gives him **Max MP +3**. The element's **DLC boss** gives the **-za tier and the element's ultimate**, plus any lower tiers he is still missing, so nothing is skipped. No base-game boss ever gives a -za tier. He gets all of it even if he isn't in the party yet, and joins with every tier the party has earned.
- **Sora:** **any** of the same bosses gives his next tier too, whichever comes first. If Sora **already has the -ga tier** of that element, the boss gives him **Max MP +3** instead. Sora **only** gets something from a boss of his own magic: Donald-only rewards (Flare and the other line-2 spells, the ultimates, the Arcane Y upgrades, Zettaflare) give Sora nothing.
- **Start:** Fire and Water at tier 1 for both, as KH3's prologue gives Sora. Donald also has their Attunements and Gravity Lv 1, but **no line-2 spell: no Flare, no Flood** (2026-10-27, the user: "Donald should not start with access to Flood or Flare").

**What each tier gives Donald** (Fire as the example; every element alike):

| Tier | Donald gets | Sora gets |
|---|---|---|
| 1 | Fire, Fire Attunement (when it is his first fire spell) | Fire |
| 2 | Fira | Fira |
| 3 | Firaga | Firaga |
| 4 (DLC boss only) | **Firaza** and the element's ultimate (**Inferno**) | His next tier, or Max MP +3 if he already has Firaga |

A tier boss gives **no line-2 level** (since 2026-10-27; it used to give Flare Lv 1 / 2 / 3 with tiers 1 / 2 / 3).

Donald casts the -za tier and the ultimate only at full-power attunement (attuned, Resonance 3), and the ultimate once per attunement period (6.1). No Cufflink is needed; the boss gives it.

**Line 2 has bosses of its own** (2026-10-27, the user: "Tier 2 magic should be given by different bosses, seperate from tier 1 bosses, following the same rules as tier 1 spell aquirements"). Each element's line-2 spell (Flare, Freeze, Ball Lightning, Flood, Aero Barrier, Banish) has **three base-game bosses**, none of them a boss of line 1, the Arcane lines, Zettaflare or the DLC:

- each of them gives **one level more** of that line-2 spell, in any order, never skipping (Lv 1, then 2, then 3);
- once the spell is at Lv 3, a further line-2 boss of it gives **Max MP +3**;
- if it is his first spell of that element, it **unlocks that element's Attunement** too (any spell of an element unlocks its attunement, 4.1);
- it counts even if he has not joined yet (he joins with it); **Sora gets nothing** from a line-2 boss (his share of a boss is that boss's own Sora reward).

Light's line 2 is **Banish**. Its first boss is the **Tree Sentinel**, the golden holy knight on horseback near the start of Limgrave, so he has a holy attack (and Light Attunement) from early on. Light's tiers give Cure (tier 1), Cura (tier 2), Curaga (tier 3) and Curaza + Holy (tier 4, from Light's DLC boss only); they no longer raise Banish.

The bosses of each line are **paced like the element's line 1** (2026-10-28, the user: "will donald's tier 2 magic bosses be appropriate for progression?"): early / mid / late where line 1 starts early, the first level in mid-game where line 1 itself starts in mid-game. Only boss rows the mod already recognises (their defeat flag) were used.

| Element | Line 2 | Lv 1 (any order) | Lv 2 | Lv 3 | Line 1's bosses, for comparison |
|---|---|---|---|---|---|
| Fire | Flare | Flying Dragon Agheel (Limgrave) | Magma Wyrm Makar (Liurnia) | Mohg, Lord of Blood (Mohgwyn Palace) | start, Rykard (Mt. Gelmir), Fire Giant (Mountaintops) |
| Water | Flood | Leonine Misbegotten (Weeping Peninsula) | Regal Ancestor Spirit (Nokron) | Mohg, the Omen (Leyndell, Shunning-Grounds) | start, Ancestor Spirit (Siofra River), Valiant Gargoyles (Siofra Aqueduct) |
| Thunder | Ball Lightning | Dragonkin Soldier of Nokstella (Ainsel River) | Elemer of the Briar (Altus Plateau) | Godfrey / Hoarah Loux (Leyndell, Ashen Capital) | Margit (Limgrave), Draconic Tree Sentinel (Capital Outskirts), Lichdragon Fortissax (Deeproot Depths) |
| Blizzard | Freeze | Red Wolf of Radagon (Liurnia, Raya Lucaria) | Mimic Tear (Nokron) | Loretta, Knight of the Haligtree (Haligtree) | Royal Knight Loretta (Liurnia), Rennala (Liurnia), Commander Niall (Mountaintops) |
| Aero | Aero Barrier | Godskin Noble (Mt. Gelmir, Volcano Manor) | Malenia (Haligtree, Elphael) | Sir Gideon Ofnir (Leyndell, Ashen Capital) | Starscourge Radahn (Caelid), Godskin Duo (Farum Azula), Dragonlord Placidusax (Farum Azula) |
| Light | Banish | **Tree Sentinel** (Limgrave) | Morgott (Leyndell) | Radagon / Elden Beast (the Erdtree, the last boss) | Godrick (Stormveil), Godfrey's golden shade (Leyndell), Maliketh (Farum Azula) |

Since the steps are "one level more, any order", the columns are the usual route's order, not a rule. Banish Lv 3 comes from the last boss (after it the world can still be explored) and Aero's last two are late, as are Aero's own tiers 2 and 3.

**An existing save** keeps every spell it has (a Flare / Flood Lv 1 from the old start included: a load never takes a spell away) and gets each new line-2 boss it has already beaten **once**, one level more or Max MP +3 at the top, through the same catch-up as any boss step (its Sora's `boss:` records, Donald's `don:` records). A save older than the overhaul starts from the new start (no Flare / Flood) and catches up every beaten boss once.

**The magic bosses** (Sora's current bosses for tiers 1 to 3, plus one **Shadow of the Erdtree** boss per element for the -za tier; the DLC ones are suggestions):

| Element | Tiers 1 to 3 (base game, any order) | -za tier + ultimate (DLC boss) |
|---|---|---|
| Fire | start, Rykard, Fire Giant | *Midra, Lord of Frenzied Flame* |
| Water | start, Ancestor Spirit, Valiant Gargoyles | *Putrescent Knight* |
| Thunder | Margit, Draconic Tree Sentinel, Lichdragon Fortissax | *Bayle the Dread* |
| Light (Cure) | Godrick, Godfrey (golden shade), Maliketh (Banish: its own line-2 bosses, above) | *Scadutree Avatar* |
| Blizzard | Royal Knight Loretta, Rennala, Commander Niall | *Rellana, Twin Moon Knight* |
| Aero | Starscourge Radahn, Godskin Duo, Dragonlord Placidusax | *Divine Beast Dancing Lion* |

**Arcane (Y) magic:**
- **Same rules as the elements:** each boss of a line gives **the next level** of that line, in any order, never skipping a level. A boss beaten once the line is at its top level gives Donald **Max MP +3** instead. They count even before Donald joins. These are Donald-only; Sora gets nothing from them.

| Y line | Levels | Its bosses (any order; suggestions) |
|---|---|---|
| Comet | Comet → Comet II | *Glintstone Dragon Smarag, Fallingstar Beast* |
| Gravity | Gravity (start) → Gravira → Graviga | *Onyx Lord, Alabaster Lord* |
| Meteor | Lv 1 → 2 → 3 | *Full-Grown Fallingstar Beast, Astel, Stars of Darkness, Glintstone Dragon Adula* |
| Ultima | one level | *Astel, Naturalborn of the Void* |

- **Zettaflare** comes from a boss **mid-way into the Shadow of the Erdtree DLC**. Suggested: **Messmer the Impaler**.

---

## 10. Level progression (outline)

| Level | Unlocks |
|---|---|
| Start | Fire and Water (tier 1, as Sora), their Attunements (no Flare / Flood: line 2 comes from its bosses, 9), base missiles, Magic Block, **Scan**, and **Gravity** on Y |
| 10 | Low Profile |
| 25 | Hide Behind |
| 30–50 | Combo Plus, Swift Attune, Distraction, Elemental Insight |
| 50–70 | Taunt Pass, Out of Sight, Spell Echo |
| 70+ | Instant Attune; Dual Attunement (~80) |

---

## 11. AI Donald (when he isn't the leader)

- **Range:** keeps 8–20 m from enemies, and retreats if one gets close.
- **Attunement:** picks it by situation:
  - the target's weakness;
  - Water when Sora or Goofy uses Thunder;
  - Light when the party is low on HP.
- **Heals:** uses Cure-line spells with KH3's healing rules, and Reactions when easy.
- **Aero Barrier:** as in KH1, casts it on the leader or another friend who is being attacked or is low on HP, and on himself when an enemy closes in. He renews it before it runs out during a fight.
- **Ball Lightning:** casts it on the friend fighting up close (often Goofy or the leader), or on himself when he is cornered.
- **Team role:** the aggro rules (5.4) apply to AI Donald too, so Goofy or Sora draws attention.
- **Release and ultimates:** never uses Release on purpose (too player-skill-heavy). **AI Donald can never cast Zettaflare**; only the player can. He may use an ultimate at full power, once per attunement period like the player.

---

## 12. Feel targets

- **Distance:** at 6–25 m with a good attunement loop, Donald out-damages Sora over time, but takes longer to set up (about 8–12 s to reach Resonance 3).
- **Up close:** he should lose a fair fight against a melee enemy. The player's answer is to dodge out, Wand Bonk, or let Goofy take the hit.
- **No easy mode:** a player who never opens the menu should feel Donald is weak. A player who learns Reactions and Release should feel he's the strongest character in the game.

---

## 13. What this replaces (removed when built)

- His current combo (staff Beat/Swing melee hits) and the **Fire-bolt finisher**.
- **Unison Fire, Meteor Drop and Flare Force as combo finishers.** Their team-attack versions with friends stay.
- **Spell Volley** (his shotlock). Its multi-target idea lives on in Missile Volley. **Donald has no shotlock at all**, and none is to be added (user decision, 2026-10-22).
- **Superslide.** No sprint slide; Ice Glide replaces it when Blizzard is attuned.
- **Air Blink**, replaced by the attunement dodges (Blink is Thunder's).
- **Flutter.** Aerial recovery follows the shared HR rule.
- **His current Magic list** (Sora's list). Replaced by section 6.
- **BAL's Donald ratios.** Replaced by section 2.
- **The XD_PLAYABLE_KITS Donald abilities.** Replaced by section 9.

These stay: shared systems (hit reactions, items, menus, conversations, saves, party pickup, the death flow), his look, his voice, and his team attacks with friends.

---