# Goofy — Playable Kit Overhaul (design)

Status: **design only, nothing implemented.** When built, this document **replaces Goofy's whole current kit**. That includes:
- the GK tank rework (combo strings, Combo Plus, finishers, Bombardier on Y, Brace, Perfect Guard, Counter Bash, the lunge rules);
- BAL's Goofy ratios;
- the XD_PLAYABLE_KITS Goofy sections.

Section 14 lists what stays (the GUARD bar, shared systems, team attacks).

Pad layout (KH3 on an Xbox pad):

| Button | Action |
|---|---|
| A | Attack / confirm |
| B | Jump |
| X | Guard / deflect |
| Y | Situation command |
| LB | Shortcuts |
| RB | Lock-on |
| D-pad | Command menu |

Keyboard is extra, never required.

---

## 1. Who Goofy is

From the user: Goofy plays like **Sekiro**. He is all about **parrying**, with strong counter gameplay and **different kinds of parries depending on the move**, unlocked through progression. His basic attacks are fairly weak and unimpressive, but he is **good at defense and at drawing aggro away from his friends**. He should be as mechanically deep as Donald's rework.

Pillars:

1. **Read, deflect, punish.** Goofy's damage doesn't come from his combo; it comes from answering the enemy's attacks correctly and cashing in with counters and Breakers.
2. **Every attack type has an answer.** Thrusts, sweeps, grabs, projectiles, magic, heavy blows and combo strings each have their own parry technique. Choosing the right one, at the right time, is the skill.
3. **Posture war.** Each successful parry fills the enemy's **Stagger** gauge. Each sloppy block drains Goofy's **GUARD**. Whoever breaks first loses.
4. **He's the shield of the party.** He pulls enemies onto himself and can step in front of a friend to parry for them.
5. **Deep in the hands, not the menus.** (user: as deep as Donald, but not menu-heavy; focused on the skills Sekiro requires) His command menu is just Attack / Items. Everything else is buttons, timing and reading the enemy.

---

## 2. Stats (overrides BAL for Goofy)

| Stat | vs Sora | Notes |
|---|---|---|
| Strength | **0.65×** | Weak basic attacks, by design. |
| Counter power | **1.6×** | Counters and Breakers use their own multipliers (sections 5, 6). |
| Max HP | **1.35×** | |
| Defense | **1.30×** | |
| Max MP | 0.80× | MP is only used for Shield Arts (section 8). |
| GUARD | His posture | Silver bar above MP, grows with level (kept from GG). |
| Run speed | 560 (sprint 800) | Heavier than Sora (680 / 980). |

Every value is a sheet row.

---

## 3. Basic attack: weak on purpose

- **Ground combo (A, A, A):** shield shove, shield swipe, two-handed shield push. Low damage (0.6 / 0.6 / 0.9), short reach, no lunge.
  - **Combo Plus is not in his kit.** His string never grows; his depth comes from the parry system.
  - Basic hits fill the enemy's **Stagger** gauge only slightly (section 5).
- **Air:** one downward shield bonk. He is a ground fighter.
- **What the string is for:**
  - poking to keep pressure;
  - **Taunt Hits**: every basic hit adds threat (aggro), so his weak string also keeps enemies looking at him (section 9);
  - **opening into a parry:** any step can be cancelled into Guard (X) at any time, with no recovery. Sekiro's attack-then-deflect rhythm is the point.

---

## 4. Guard, Deflect and GUARD (his posture)

### 4.1 Guard and Deflect

| Input | Name | Window | GUARD cost | Result |
|---|---|---|---|---|
| Hold X | **Guard** | Always | **High** (scaled by the hit's power) | Blocks the hit, no counter window |
| Tap X just before the hit | **Deflect** | 0.20 s at rank 1 (wider with ranks and Resolve, section 7) | **Tiny**, and refills a bit on success | Clang plus sparks; the enemy's Stagger gauge rises a lot; opens the **counter window** (section 6) |
| Mashing X | — | Each tap within 0.25 s of the previous one **shrinks** the window by 30 % | — | Spamming is weaker than timing (as Sekiro) |

### 4.2 GUARD = Goofy's posture

- **How it fills and drains:** it drains on guarded hits and failed parries, and refills slowly when he isn't guarding. Perfect deflects refill it slightly.
- **At 0, Guard Break:** Goofy staggers for 1.5 s and takes full damage. This replaces the GG "broken" state.
- **Level growth stays as GG built it.** The bar's look stays too: the silver bar, red loss and broken style.

### 4.3 Reading attacks: one danger cue, the rest is your eyes

Like Sekiro, there is **one** warning only: a single red KH3-style **"!"** over the enemy for a **perilous** attack (thrust, sweep or grab). **It doesn't say which.** The player must read the animation and pick the right answer:

| The enemy does… | The answer (an input, not a menu) |
|---|---|
| A normal attack | **Deflect**: tap X on the hit |
| A thrust / lunge (perilous) | **Pinning Guard**: tap X **while pushing the stick toward the enemy** as the thrust lands (Sekiro's Mikiri) |
| A sweep / low swing (perilous) | **Hop Counter**: jump (B) over it, then A in the air |
| A grab (perilous) | **Slip Counter**: dodge (stick + B) into the grab as it closes |
| An arrow / projectile | **Shield Reflect**: deflect it, with the stick aimed at the shooter to send it back at them |
| Magic / an incantation | **Spell Ward**: hold Guard through the cast, then tap X on impact |
| An unblockable heavy / charge | **Bull Brace**: hold X and the stick **away** from the enemy, set your feet before impact |
| A long combo string | **Rhythm Deflect**: deflect every hit in time; the last hit of the string opens the big counter |

- **No cues for normal attacks, projectiles or magic.** Recognising Elden Ring's wind-ups is the skill, just as in Sekiro.
- **Without the right technique learned,** the attack can only be guarded (expensive) or deflected plainly (half Stagger, no special counter).
- **Where the attack types come from:** Elden Ring's own attack data (damage type thrust / slash / strike, the guard-break and throw flags, bullets, magic or incantation type). Each enemy attack gets its type when it's built, which decides which answer works.
- **Danger Sense** (ability) shows the red "!" 0.15 s earlier. It still never says which kind.

---

## 5. Stagger: the enemy's posture

- A **Stagger gauge** appears under the locked-on enemy's KH3 HP gauge, in the same KH3 style.
- **Filled by:**
  - deflects (a lot);
  - the right parry technique against its attack type (a huge amount);
  - counters;
  - basic hits (a little).
- **Drains** slowly when he stops pressuring. It drains slower while the enemy's HP is low (Sekiro's rule).
- **When full, the enemy is BROKEN for 2.5 s.** Y offers **Goofy Breaker** (section 6.3).
- **Elden Ring link:** where Elden Ring has its own stance break for an enemy, a full Stagger also triggers it, so Elden Ring's own critical reaction plays under our Breaker.
- **Bosses:** a boss's gauge is larger, and it breaks into a shorter 1.5 s window, after which the gauge resets to half.

---

## 6. Counters and Breakers: where Goofy's damage lives

### 6.1 The counter window

After a successful parry, **A within 0.6 s** fires the counter for that parry type. The counter depends on the parry:

| Parry technique | Counter (A) | What it does |
|---|---|---|
| Deflect | **Counter Bash** | A quick shield strike: 2.0× damage, big Stagger |
| Pinning Guard (vs thrust) | **Pin Down** | Steps on the weapon, then an uppercut: 3.0×, huge Stagger, the enemy's next attack is delayed |
| Hop Counter (vs sweep) | **Goofy Stomp** | Lands on the enemy (Sekiro's jump kick): 2.5×, launches small enemies (juggles) |
| Slip Counter (vs grab) | **Pratfall Toss** | Goofy trips; the grabber flies past him into the ground: 2.5×, knockdown |
| Shield Reflect (vs projectile) | **Return Fire** | Bats the projectile back: its own damage ×1.5 plus Stagger |
| Spell Ward (vs magic) | **Ward Burst** | Absorbs the spell into GUARD (refill) and pulses back: 1.5× in a circle |
| Bull Brace (vs heavy) | **Shield Shove** | Huge push; the enemy is knocked off balance: 2.0× and a Stagger spike |
| Rhythm Deflect (vs a string) | **Crescendo** | After the last hit of the string: 1.0× per hit deflected in a row, up to 5× |

### 6.2 Counter chains

- **Counter, then parry, then counter:** a parry right after a counter (within 1 s) gives **+25 % counter damage**, stacking up to 3 times. Mastery means staying in that loop.
- **Shield Arts** (section 8) can follow a counter immediately for free (no MP) once per counter. That's the link from defense into his special moves.

### 6.3 Goofy Breaker (Y on a BROKEN enemy)

- **The finisher:** his deathblow equivalent. A big comic shield sequence (wind-up, spin, slam) for **5.0×** damage.
- **Elden Ring link:** it plays over Elden Ring's own critical reaction where there is one.
- **Defeating with it:** on non-boss enemies with less than 40 % HP left it **defeats them outright** (Sekiro's deathblow).
- **Breaker variants** come from abilities (section 11):

| Variant | What it adds |
|---|---|
| **Party Breaker** | Nearby friends join in, a mini team attack |
| **Launch Breaker** | Launches the enemy for air juggles by Sora and Donald |
| **Rally Breaker** | Heals the party 10 % |

---

## 7. Momentum: depth from execution, not menus

Goofy is **not menu-heavy**. His command menu is just **Attack / Items**, plus his two equipped Shield Arts on button presses (section 8). His depth is the Sekiro kind: timing, reading, risk, and managing two posture bars at once.

**Resolve**, a 3-pip meter beside GUARD, fills only from **clean play**:
- a full pip for the right answer to a perilous attack;
- a third of a pip for a perfect deflect;
- nothing for guarding.

It empties if he gets hit.

| Resolve | Effect, automatic (no button) |
|---|---|
| 1 pip | Deflect window +0.03 s, and counters +10 % |
| 2 pips | Deflects also refill GUARD a little more, and his threat doubles (enemies prefer him) |
| 3 pips | **In the Zone**: every deflect counts as the right technique, the counter window is longer, and Breakers do +50 %. Lasts until he's hit. |

So the reward for playing perfectly is playing even better, and one mistake drops him back down. That is the skill loop, not menu switching.

- **Risk:** **aggressive deflects** make it harder. If he deflects while **moving toward** the enemy (stick forward), Stagger gain is ×1.3 but the window is slightly tighter.
- **Two bars at once** (Sekiro's vitality vs posture): it's often right to take a small hit (HP) rather than guard (GUARD), to avoid a Guard Break before the enemy breaks.

---

## 8. Shield Arts: two equipped, on buttons (like Sekiro's combat art)

Goofy has **no magic and no Magic menu**. He **equips two Shield Arts** in the camp menu (Customize), outside combat. In combat they're fired directly:
- **LB + A:** the first Art;
- **LB + B:** the second.

Taunt has its own button, **LB + X**. Arts cost **MP**, which is cheap and recharged by counters. No menu is opened in combat.

| Art | Effect | Notes |
|---|---|---|
| **Goofy Charge** | Shield-first rush that pushes enemies along | Closes distance; ends in a guard stance |
| **Goofy Tornado** | Spinning shield whirl | Hits around him; deflects projectiles while spinning |
| **Goofy Bash** | Overhead slam | Big Stagger |
| **Goofy Turbo** | Rides the shield forward like a sled | Mobility; can be cancelled into Guard |
| **Taunt** | Goofy hollers and bangs the shield | All enemies within 10 m target him for 5 s (section 9) |
| **Interpose** | Dashes in front of the nearest threatened friend and auto-deflects the next hit aimed at them | The bodyguard tool |
| **Shield Toss** | Throws the shield, which returns | Ranged poke; while it's out he can only guard bare-handed (half GUARD) |

Arts gain **ranks** from progression (section 11). Items stay on KH3's shortcut palettes (LB + Y), the same as every character.

---

## 9. Aggro: drawing attention away from his friends

- **Threat:** every guard, deflect, basic hit and Taunt raises Goofy's **threat** on that enemy. Enemies pick the party member with the most threat, as long as he's within reach.
- **Base:** his threat counts ×1.5 compared with a friend's (×3 at Resolve 2 or more).
- **Taunt** (LB + X) forces targets.
- **Interpose** (Art) and the **Ally Guard** ability let him parry for a friend.
- **Synergy:**
  - with Donald's aggro abilities (Taunt Pass etc.: when Goofy guards, enemies near Donald switch to Goofy);
  - with Sora's combo pressure.
- **Solo:** the aggro tools matter less, but Taunt still pulls packs into one spot for his Tornado.

---

## 10. Movement and dodge

- **Dodge (X + stick, or B on the ground in a direction):** a clumsy hop with a **1.0 s cooldown** and short invincibility. Goofy is meant to parry, not dodge.
- **No air dodge.** **Jump** is low, but it's the input for **Hop Counter** over sweeps.
- **Swim and ladders:** shared rules (Y at a ladder jumps him up to its top).

---

## 11. Progression: how he gets his parries and Arts

Same model as Sora and Donald:
- **Levels** give core abilities.
- **Bosses** give the **next rank** of a technique tied to that boss's fighting style. A boss gives "Pinning Guard", meaning the next rank of it, never a fixed rank, so the order bosses are beaten in never matters.

### 11.1 Parry techniques and ranks

Each technique has 3 ranks:
- **Rank 1:** learned; the special counter is available.
- **Rank 2:** a wider window and more Stagger.
- **Rank 3:** a free follow-up such as Pin Down launching, or Return Fire splitting into three.

| Technique | Learned at | Ranks 2–3 from bosses whose style is… (suggested) |
|---|---|---|
| Deflect | Start | Duelists: Margit, Morgott, Malenia |
| Shield Reflect | Level 5 | Archers and casters: Starscourge Radahn, Red Wolf of Radagon |
| Hop Counter | Level 12 | Sweeping giants: Godrick, Fire Giant |
| Pinning Guard | Level 20 | Spear and thrust users: Crucible Knights, Draconic Tree Sentinel |
| Slip Counter | Level 28 | Grabbers: Godskin Duo, Mohg |
| Bull Brace | Level 36 | Heavy chargers: Tree Sentinel, Erdtree Avatars |
| Spell Ward | Level 44 | Mages: Rennala, Royal Knight Loretta |
| Rhythm Deflect | Level 55 | Long-string bosses: Malenia, Maliketh |

### 11.2 Shield Arts ranks

- **From the start:** Goofy Charge, Taunt.
- **By level:** Tornado 8, Bash 15, Turbo 25, Interpose 35, Shield Toss 50.
- **Ranks 2–3:** from any boss once the Art is known. That improves damage, cost, and an extra effect per Art.

### 11.3 Abilities (as many as Sora's; with AP costs, names and descriptions)

**Parry and counter**

| Ability | What it does |
|---|---|
| Danger Sense | Danger cues 0.15 s earlier |
| Steady Guard | Guard costs −20 % GUARD |
| Iron Posture | GUARD refills faster |
| Counter Chain+ | The counter chain stacks to 5 |
| Second Deflect | One mistimed tap per string is forgiven |
| Revenge Bash | After a Guard Break, his next deflect counts as perfect |
| Ally Guard | Deflects for a friend within 2 m automatically, once per 10 s |

**Breaker**

| Ability | What it does |
|---|---|
| Party Breaker | Nearby friends join the Breaker |
| Launch Breaker | The Breaker launches the enemy for juggles |
| Rally Breaker | The Breaker heals the party 10 % |
| Deep Breaker | +30 % Breaker damage |

**Resolve**

| Ability | What it does |
|---|---|
| Resolve Keeper | One hit only removes one Resolve pip, instead of all |
| Zone Extender | In the Zone lasts through one hit |
| Forward Deflect+ | Aggressive deflects lose less window |

**Aggro and support**

| Ability | What it does |
|---|---|
| Loud Mouth | +25 % threat |
| Bodyguard | Interpose costs no MP when a friend is under 30 % HP |
| Second Wind | Restores GUARD when HP drops under 25 % |
| Lucky Lucky, Treasure Magnet, Item Boost, Second Chance, Withstand Combo | KH3 standard |

**Stats:** Max GUARD increases through levels (GG).

---

## 12. AI Goofy (when he isn't the leader)

- **Taunting:** stays near the leader, and uses Taunt when a friend is targeted by two or more enemies.
- **Bodyguard:** uses Interpose when the leader is low, and parries with good (not perfect) timing. The rate grows with level.
- **Cashing in:** uses counters and Breakers when they're offered, and keeps Resolve up by favouring safe deflects.
- **Aggro:** his aggro rules apply as an ally too, so playing as Sora or Donald with AI Goofy feels protected.

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
- the shield-bash lunge rules;
- BAL's Goofy ratios;
- the XD_PLAYABLE_KITS Goofy kit.

**Kept:**
- **The GUARD bar** (GG), reused as his posture: same look, level growth and save field.
- The shared systems: hit reactions, items, menus, conversations, saves, pickup, death flow, rendering.
- His team attacks with friends: Trinity Guard and the others on Y when friends are near.
- The HR hit reactions; Guard Break uses his own stagger clip.
- Goofy's look and voice.

---

## 15. Open questions for the user

1. **Stagger and Elden Ring's stance break:** link them as described (5), or keep our gauge separate?
2. **Defeating with a Breaker:** should a Breaker defeat non-boss enemies under 40 % HP outright, or always just deal damage?
3. **Resolve / In the Zone** as the skill-reward loop instead of stances: good?
4. **The single red "!"** for perilous attacks only (Sekiro's way): keep it, or remove even that?
5. **Shield Toss** leaving him bare-handed: fun risk, or cut it?
