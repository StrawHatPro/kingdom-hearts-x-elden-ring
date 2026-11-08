// (P6a) THE MOOGLE WORKSHOP'S RULES SHEET (sheets/port/kh-workshop.json), read by crates/kh-host/src/workshop.rs.
// KH3's Workshop 1:1 (documentation/KH3_WEAPONS_PLAN.md 0, 1.5, 6a, 6c, 6d; documentation/KH3_WEAPON_DATA.md 3): the 88
// recipes, the 78 Collector's Goals, the Keyblade Forge (17 Keyblades x 10 levels, materials only), the 60 materials, the
// Moogle shop's stock rules and its 7 levels, and the 20 Photo Missions, which in Elden Ring are ER milestones (boss
// defeats) instead of photos (the user 2026-11-03: "Unlock them another way"), with COMPOSED mission texts; since the user's
// 2026-11-07 "Journey to the Erdtree": unique iconic bosses, all listed from the start, recipes by tier.
//
//   node tools/kh_workshop_sheet.js     write the sheet and register it (its own "(P6a)" entry, replaced in place) in
//                                       tools/lib/sheet_defs.js; then node tools/extract.js, node tools/preflight.js --summary
//
// Inputs (read only): the KH3 data sheets of tools/kh3_weapon_sheets.js (sheets/port/kh3-synthesis, kh3-collect-goals,
// kh3-photo-missions, kh3-materials, kh3-shop-stock, kh3-keyblades, kh3-keyblade-levels, kh3-workshop-rules), the mod's
// item table (sheets/port/sora-item-table.json: what the Moogle can sell) and the Elden Ring defeat flags of
// sheets/port/sora-er-boss-rewards.json (every milestone / shop-level flag is taken from there and must be "verified";
// documentation/ER_BOSS_FLAGS.md).
'use strict';
const fs = require('fs');
const path = require('path');
const ROOT = path.resolve(__dirname, '..');
const VERSION = '1.0.0';
const P = (...a) => path.join(ROOT, ...a);
const readJson = (f) => JSON.parse(fs.readFileSync(P(f), 'utf8'));
const must = (c, m) => { if (!c) throw new Error(m); };
// A sheet's rows as objects by its columns.
const rowsOf = (file) => { const s = readJson(file); return s.rows.map((r) => Object.fromEntries(s.columns.map((c, i) => [c, r[i]]))); };

const SYN = rowsOf('sheets/port/kh3-synthesis.json');
const GOALS = rowsOf('sheets/port/kh3-collect-goals.json');
const PHOTOS = rowsOf('sheets/port/kh3-photo-missions.json');
const MATS = rowsOf('sheets/port/kh3-materials.json');
const STOCK = rowsOf('sheets/port/kh3-shop-stock.json');
const KB = rowsOf('sheets/port/kh3-keyblades.json');
const LV = rowsOf('sheets/port/kh3-keyblade-levels.json');
const RULES = new Map(readJson('sheets/port/kh3-workshop-rules.json').rows.map((r) => [r[0], r]));
const ITEMS = new Set(readJson('sheets/port/sora-item-table.json').rows.map((r) => r[0]));
const BOSS = new Map(readJson('sheets/port/sora-er-boss-rewards.json').rows.filter((r) => String(r[0]).startsWith('boss.')).map((r) => [r[0].slice(5), r[1]]));

must(SYN.length === 88 && GOALS.length === 78 && PHOTOS.length === 20 && MATS.length === 60 && KB.length === 17 && LV.length === 170, 'the KH3 data sheets are not the expected size (run node tools/kh3_weapon_sheets.js)');
const rule = (id) => { const r = RULES.get(id); must(r, 'kh3-workshop-rules has no row ' + id); return r; };
// A boss row's verified defeat flag.
const flag = (row) => { const b = BOSS.get(row); must(b, 'sora-er-boss-rewards has no row boss.' + row); must(b.flag && b.flag_conf === 'verified', 'boss.' + row + ' has no verified defeat flag'); return b.flag; };
const bossName = (row) => BOSS.get(row).name;

const COLS = ['id', 'value', 'unit', 'source', 'evidence', 'ref_addr', 'ref_conf', 'note'];
const OUT = [];
// (KH3TEXT 2026-11-08) This sheet is compiled into the DLL: KH3's own text quoted in an evidence / note cell (the KH3
// rule sheets quote the game's messages, e.g. dialog_0010 '...') is cut to its id; the text is in the player's pack.
const noKh3Quotes = (t) => (typeof t === 'string' ? t.replace(/ '([A-Z{](?:[^']|'(?=[a-z]))*)'(?=$|[\s;,.)])/g, ' (its text: the player\'s pack)') : t);
const add = (id, value, unit, source, evidence, refAddr, conf, note) => OUT.push([id, value, unit, source, noKh3Quotes(evidence) || '-', refAddr || '-', conf, noKh3Quotes(note) || '-']);
const pairs = (keys, counts) => keys.map((k, i) => [k, counts[i]]);
const KH3 = 'sheets/port/kh3-';

// ------------------------------------------------------------------------------------------------ workshop.* rules
const R = (id) => rule(id)[1];
add('workshop.stock_cap', 999, 'count', KH3 + 'workshop-rules synth.materials.deposit / upgrade.rule.gate_materials', R('synth.materials.deposit'), '0x14172EFF0', 'inferred', 'The Workshop stock holds up to 999 of each material (short[256] at inv+0xD68).');
add('workshop.count_cap', 999, 'count', KH3 + 'workshop-rules synth.save.synth_count', R('synth.save.synth_count'), '0x1415B2C20', 'inferred', 'The per-recipe synthesis counter (0..999).');
add('workshop.materials', { first: 0x701, last: 0x73c }, 'item id', KH3 + 'workshop-rules synth.collect.material_ids', R('synth.collect.material_ids'), '-', 'inferred', 'Only KH3\'s 60 materials count for the Workshop (the COMPOSED gloveworts are not Workshop materials).');
add('workshop.keyblades', { first: 0x301, last: 0x314 }, 'item id', KH3 + 'workshop-rules synth.recipe.can_synthesize / upgrade.rule.listed_weapons', R('upgrade.rule.listed_weapons'), '-', 'inferred', 'A Keyblade reward is offered only while none is held or equipped; the Forge lists only owned Keyblades.');
add('workshop.visit', ['deposit', 'goals', 'photos'], 'order', KH3 + 'workshop-rules synth.collect.eval', R('synth.collect.eval'), '0x14172EFF0', 'inferred', 'Opening the Workshop: carried materials move into the stock (dialog_0010), then every open Collector\'s Goal is tested in table order against the counts from before the visit\'s rewards (no chaining), then the Photo Missions (here: the ER milestones). The mod also opens it when a smith opens the Keyblade Forge (the Forge counts materials in the stock, as KH3).');
// THE MOOGLE'S top. The user 2026-11-07: "remove the ability to upgrade keyblades from moogle, that is what the blacksmiths
// are for": KH3's Keyblade Forge (menu_0020) is not on the Moogle's list; every blacksmith opens it straight (kh-ui
// Entry::Forge), which needs no entry here.
add('workshop.menu', [
  { entry: 'synthesize', text: 'kh3_menu.synthesis.menu_0010', kh3_gate: 'always', gate: 'always' },
  { entry: 'photo', text: 'kh3_menu.synthesis.menu_0030', kh3_gate: 'gameflow_TT >= TT_TRM_EVENT03', gate: 'always' },
  { entry: 'goals', text: 'kh3_menu.synthesis.menu_0040', kh3_gate: 'always', gate: 'always' },
  { entry: 'materials', text: 'kh3_menu.synthesis.menu_0050', kh3_gate: 'always', gate: 'always' },
], 'entries', KH3 + 'workshop-rules shop.workshop.menu.* / shop.workshop.forge_unlock / synth.motif.unlock_rule', R('shop.workshop.forge_unlock') + '; ' + R('synth.motif.unlock_rule'), '-', 'confirmed',
'THE USER 2026-11-07: the Moogle does not upgrade Keyblades ("that is what the blacksmiths are for"): KH3\'s Keyblade Forge entry (menu_0020, KH3 gate GM_EVENT2) is left out of the Moogle\'s list; the blacksmiths open the Forge straight. KH3 hides Photo Missions until TT_TRM_EVENT03 (the second world); the mod shows it from the start as "Journey to the Erdtree" (kh-ui workshop/texts.rs MOD), all 20 milestones listed from the start (THE USER 2026-11-07). No Free Development, no Material Exchange (synth.free_dev).');
add('workshop.unknown_name', '? ? ?', 'text', KH3 + 'workshop-rules synth.menu.5', 'the Material List and recipe rows name a material never obtained "? ? ?"', '-', 'inferred', 'A material counts as known once ever obtained (workshop.rs `WorkshopState::obtained`).');
add('workshop.log', { recipes: SYN.length, photos: PHOTOS.length, goals: GOALS.length }, 'count', KH3 + 'workshop-rules synth.log', R('synth.log'), '-', 'inferred', 'SYNTH LOG: Items Created = recipes made at least once / 88; Journey Milestones Completed (KH3: Photo Missions Completed) / 20; Collector\'s Goals Completed / 78; COMPLETE when equal.');

// ------------------------------------------------------------------------------------------------ the milestones' plan
// "JOURNEY TO THE ERDTREE" (THE USER 2026-11-07, verbatim: "make them all viewable from the start. use much more iconic
// bosses like Godskin Duo, Placidusax, Melania, Godfrey, instead of specificlly Leonine, Margit, Fire Giant, and others you
// deem. rename the menu itself to "Journey to the Erdtree". Reorder the list so that it suits new bosses that replace
// others"; then: "keep fire giant as a boss, remove ancestor spirit and any other boss that has duplicates in the game").
// KH3's 20 Photo Missions are 20 Elden Ring milestones: each is DONE when its boss is defeated (its verified defeat flag,
// sheets/port/sora-er-boss-rewards.json), all are LISTED from the start. Only one-of-a-kind fights (no boss fought twice or
// sharing a character / boss type with another fight). `order` = the natural progression (early game -> late game -> the
// Realm of Shadow -> the Erdtree -> the finale). A boss kept from the first list (2026-11-03, rev 1) keeps its IM row, so a
// save's completion of it stays true; `rev` = the milestone revision that last changed the row's subject (the save's
// completions of a row newer than the save's revision are re-checked once, kh-host workshop.rs `migrate_milestones`).
const MILESTONE_REV = 2;
const GREAT_RUNE = [...BOSS.entries()].filter(([, b]) => [...(b.user || []), ...(b.proposal || [])].includes('slot:great_rune')).map(([id]) => id);
must(GREAT_RUNE.length === 7, 'expected the 7 Great Rune bosses (slot:great_rune rows), got ' + GREAT_RUNE.join(','));
// [IM row, boss row(s), rev, the Moogle's line, place, who (the help's name; default the boss row's), why]
// The Moogle's lines are the mod's own (KH3TEXT 2026-11-08: none is KH3's text or a close copy of a KH3 line; checked
// against the pack below), except Radahn's: THE USER 2026-11-08 "keep radhan's milestone text what it was" -> KH3's own
// mission_0080 READ FROM THE PLAYER'S PACK at run time ({ pack, period_to }: its final "." shown as "!"), never compiled in.
const JOURNEY = [
  ['IM_12', ['godrick'], 1, 'All those grafted arms, and not one of them waves hello, kupo.', 'Limgrave / Stormveil Castle', null,
    'The first shardbearer and the first great lord of the journey; the end of Limgrave.'],
  ['IM_2', ['rennala'], 1, 'A queen hugging an amber egg under a full moon? Spooky, kupo.', 'Liurnia of the Lakes / Raya Lucaria Academy', null,
    'A shardbearer, the queen of the Carian royals; the next region.'],
  ['IM_17', ['radahn'], 1, { pack: 'kh3_menu.shop.mission_0080', period_to: '!' }, 'Caelid / Redmane Castle', null,
    'A shardbearer, the Radahn Festival. (Promised Consort Radahn is the same character: left out, no duplicates.)'],
  ['IM_7', ['mimic_tear'], 2, 'Down in the eternal city, someone looks just like you, kupo!', 'Nokron, Eternal City', 'the Mimic Tear',
    'A one-of-a-kind fight (the player against himself) that opens once Radahn\'s star falls; the mod\'s own Mimic boss.'],
  ['IM_0', ['rykard'], 1, 'That volcano is cooking up something nasty, kupo... Don\'t sniff it!', 'Mt. Gelmir / Volcano Manor', null,
    'A shardbearer, mid game.'],
  ['IM_16', ['morgott'], 1, 'He keeps the Tarnished out of the capital, kupo.', 'Leyndell, Royal Capital / Elden Throne', null,
    'A shardbearer, the Omen King at the capital. (Margit is the same character: left out.)'],
  ['IM_8', ['fortissax'], 2, 'Deep among the roots, a dragon dreams of death, kupo...', 'Deeproot Depths', null,
    'A one-of-a-kind dragon fight in a dream, after the capital (Fia\'s quest).'],
  ['IM_4', ['mohg_lord'], 1, 'Those falls are red, kupo... Not refreshing at all!', 'Mohgwyn Palace', null,
    'A shardbearer, a late optional region. (Mohg, the Omen is his duplicate: left out.)'],
  ['IM_10', ['fire_giant'], 1, 'There\'s a giant tromping through the snow up north, kupo! Brrr!', 'Mountaintops of the Giants / Flame Peak', 'the Fire Giant',
    'THE USER 2026-11-07: "keep fire giant as a boss"; the gate to the Forge of the Giants.'],
  ['IM_9', ['godskin_duo'], 2, 'Two godskins at once? That\'s twice the creepy, kupo!', 'Crumbling Farum Azula', 'the Godskin Duo',
    'THE USER\'s pick; a unique two-boss fight (the solo Godskins are left out).'],
  ['IM_5', ['placidusax'], 2, 'A dragon lord naps in the eye of the storm... Wake him, kupo!', 'Crumbling Farum Azula', null,
    'THE USER\'s pick; the hidden lord of the dragons at the edge of time.'],
  ['IM_3', ['maliketh'], 1, 'Show me that beast and his blade of black shadow, kupo!', 'Crumbling Farum Azula', null,
    'The keeper of Destined Death at the end of Farum Azula.'],
  ['IM_1', ['malenia'], 2, 'She\'s never known defeat, kupo... Wanna be the first?', 'Miquella\'s Haligtree / Elphael', null,
    'THE USER\'s pick ("Melania"); a shardbearer, the hardest optional fight of the base game.'],
  ['IM_11', ['hoarah_loux'], 2, 'The first Elden Lord came home, kupo... And he brought his axe!', 'Leyndell / Elden Throne', 'Godfrey, First Elden Lord',
    'THE USER\'s pick: the real First Elden Lord fight (boss.hoarah_loux), not the golden shade (a duplicate).'],
  ['IM_14', ['rellana'], 2, 'Two moons in one knight? Even the night sky\'s jealous, kupo!', 'Realm of Shadow / Castle Ensis', null,
    'Shadow of the Erdtree: the first great knight of the Realm of Shadow.'],
  ['IM_13', ['messmer'], 2, 'That keep in the shadow is all fire and snakes, kupo. Yikes!', 'Realm of Shadow / Shadow Keep', null,
    'Shadow of the Erdtree: Messmer the Impaler, the Realm of Shadow\'s main demigod.'],
  ['IM_15', ['midra'], 2, 'Don\'t stare at the yellow flame in those woods, kupo... Seriously!', 'Realm of Shadow / Abyssal Woods', null,
    'Shadow of the Erdtree: the Lord of Frenzied Flame, a one-of-a-kind fight.'],
  ['IM_18', ['bayle'], 2, 'Up on the jagged peak, a dragon\'s still holding a grudge, kupo!', 'Realm of Shadow / Jagged Peak', null,
    'Shadow of the Erdtree: Bayle the Dread, the great dragon of the peak.'],
  ['IM_19', ['elden_beast'], 2, 'At the end of the road, something shines in the Erdtree, kupo...', 'The Erdtree', 'the one waiting inside the Erdtree',
    'The final fight of the journey to the Erdtree (Radagon and the Elden Beast, one fight).'],
  ['IM_6', [...GREAT_RUNE, 'elden_beast'], 2, 'Every great foe in the Lands Between, beaten! The moogles are cheering, kupo!', 'The Lands Between', null,
    'THE FINALE (KH3\'s "seven princess" mission, counted the same way {0} / {1}): every shardbearer (the seven Great Rune holders) and the final fight in the Erdtree. Rev 2: the Elden Beast added.'],
];
must(JOURNEY.length === 20 && new Set(JOURNEY.map((j) => j[0])).size === 20, 'one milestone per Photo Mission');
must(new Set(JOURNEY.filter((j) => j[0] !== 'IM_6').map((j) => j[1][0])).size === 19, 'a boss once');
// (KH3TEXT) No composed line is KH3's text or a close copy (this sheet is compiled into the DLL): checked against the pack's
// English tables when the pack is there (build/sora_pack, made by the setup from the player's KH3 install): no KH3 line of
// 16+ characters equal to a composed line or sharing its first 15 characters, no run of 6 words in common, and (for a
// KH3 line of 7+ words) at most 60 % of its words in the same order. A pack reference must name an id the pack has.
{
  const sdir = P('build', 'sora_pack', 'ui', 'strings', 'en');
  if (fs.existsSync(sdir)) {
    const kh3 = new Map();
    for (const f of fs.readdirSync(sdir).filter((x) => x.endsWith('.json'))) {
      for (const [ns, rows] of Object.entries(JSON.parse(fs.readFileSync(path.join(sdir, f), 'utf8')).namespaces || {})) for (const [k, v] of Object.entries(rows)) if (typeof v === 'string') kh3.set(ns + '.' + k, v);
    }
    const flat = (t) => t.replace(/<br>/g, ' ').replace(/<[^>]*>/g, '').replace(/\s+/g, ' ').trim().toLowerCase();
    const words = (t) => flat(t).replace(/[^a-z0-9' ]/g, ' ').split(' ').filter(Boolean);
    const lcs = (a, b) => { const d = Array(b.length + 1).fill(0); for (const x of a) { let prev = 0; for (let j = 1; j <= b.length; j++) { const t = d[j]; d[j] = x === b[j - 1] ? prev + 1 : Math.max(d[j], d[j - 1]); prev = t; } } return d[b.length]; };
    const run = (a, b) => { let best = 0; const d = Array(b.length + 1).fill(0); for (const x of a) { for (let j = b.length; j >= 1; j--) { d[j] = x === b[j - 1] ? d[j - 1] + 1 : 0; best = Math.max(best, d[j]); } } return best; };
    const lines = [...new Set([...kh3.values()].map(flat).filter((t) => t.length >= 16))];
    const texts = JOURNEY.flatMap((j) => (typeof j[3] === 'string' ? [[j[0] + ' line', j[3]]] : []));
    for (const [what, line] of texts) {
      const t = flat(line), w = words(line);
      for (const k of lines) {
        must(t !== k && t.slice(0, 15) !== k.slice(0, 15), what + ': "' + line + '" is KH3 text (' + k + ')');
        const kw = words(k);
        must(run(w, kw) < 6 && (kw.length < 7 || lcs(w, kw) <= 0.6 * kw.length), what + ': "' + line + '" is a close copy of KH3 text (' + k + ')');
      }
    }
    for (const j of JOURNEY.filter((j) => typeof j[3] !== 'string')) must(kh3.has(j[3].pack), j[0] + ': the pack has no ' + j[3].pack);
  }
}
// The recipes by tier: KH3's 20 mission recipes sorted by their materials (sum of count x rarity: RANK00 8, RANK01 4,
// RANK02 2, RANK03 1, sheets/port/kh3-materials), then the item's KH3 price (m_BuyPrice), then KH3's own mission order
// (ui_priority); the Petite Ribbon (KH3's finale, IM_6) stays the finale's. The n-th milestone gives the n-th recipe.
const RANK_W = [8, 4, 2, 1];
const matRank = new Map(MATS.map((m) => [m.id, Number(String(m.rank).slice(4))]));
const PRICE = new Map(readJson('sheets/port/sora-item-table.json').rows.map((r) => [r[0], r[6]]));
const photoRecipes = SYN.filter((s) => s.condition === 'MOTIF_LIST').map((s) => ({ id: s.id, reward: s.reward, cost: s.materials.reduce((a, k, i) => a + s.counts[i] * RANK_W[matRank.get(k)], 0), price: PRICE.get(s.reward), kh3: PHOTOS.find((p) => p.id === s.condition_row).ui_priority }));
must(photoRecipes.length === 20, 'twenty mission recipes');
const FINALE_RECIPE = PHOTOS.find((p) => p.id === 'IM_6').unlocks_recipes[0];
const TIERS = photoRecipes.filter((r) => r.id !== FINALE_RECIPE).sort((a, b) => a.cost - b.cost || a.price - b.price || a.kh3 - b.kh3).concat(photoRecipes.filter((r) => r.id === FINALE_RECIPE));
const PLAN = new Map(JOURNEY.map((j, k) => [j[0], { order: k + 1, boss: j[1], rev: j[2], name: j[3], place: j[4], who: j[5], why: j[6], recipe: TIERS[k].id }]));
must(PLAN.get('IM_6').recipe === FINALE_RECIPE && PLAN.get('IM_6').order === 20, 'the finale gives the Petite Ribbon');
const missionOfRecipe = new Map([...PLAN].map(([im, p]) => [p.recipe, im]));

// ------------------------------------------------------------------------------------------------ materials
const RANK = { RANK00: 0, RANK01: 1, RANK02: 2, RANK03: 3 };
for (const m of MATS) {
  must(m.rank in RANK, m.id + ': rank ' + m.rank);
  add('mat.' + m.id, { item_id: m.item_id, rank: RANK[m.rank], grade: m.grade, name: m.name, max: m.max_num }, 'material', KH3 + 'materials ' + m.id, m.source, '-', m.ref_conf, 'rank = m_ItemRank (0 = RANK00 the rarest .. 3 = RANK03 the shards): what GET_RANK goals test.');
}

// ------------------------------------------------------------------------------------------------ syn.IS_n
const goalIds = new Set(GOALS.map((g) => g.id)), photoIds = new Set(PHOTOS.map((p) => p.id));
for (const s of SYN) {
  const cond = s.condition === 'NONE' ? 'none' : s.condition === 'COLLECT_LIST' ? 'goal' : s.condition === 'MOTIF_LIST' ? 'photo' : null;
  must(cond, s.id + ': condition ' + s.condition);
  must(cond === 'none' || (cond === 'goal' ? goalIds : photoIds).has(s.condition_row), s.id + ': unknown condition row ' + s.condition_row);
  must(ITEMS.has(s.reward) || /^WEP_/.test(s.reward), s.id + ': reward ' + s.reward + ' is not an item of the mod');
  // a mission recipe follows the milestone plan (by tier), KH3's own mission row kept in the note
  const row = cond === 'none' ? null : cond === 'photo' ? missionOfRecipe.get(s.id) : s.condition_row;
  const moved = cond === 'photo' && row !== s.condition_row ? ' Journey to the Erdtree (2026-11-07): listed by milestone ' + row + ' (by tier); KH3: ' + s.condition_row + '.' : '';
  add('syn.' + s.id, { flag: s.flag_index, reward: s.reward, name: s.reward_name, kind: s.reward_kind, item_id: s.reward_item_id, max: s.reward_max, condition: cond, row, materials: pairs(s.materials, s.counts) },
    'recipe', KH3 + 'synthesis ' + s.id, s.source, '-', s.ref_conf, 'Repeatable; costs no munny; count <= max - held and <= the stock; a Keyblade only while none is held (synth.recipe.can_synthesize).' + moved);
}

// ------------------------------------------------------------------------------------------------ goal.IC_n
const matByKey = new Map(MATS.map((m) => [m.id, m]));
for (const g of GOALS) {
  let cond;
  switch (g.condition) {
    case 'MAT_KIND_NUM': cond = { kind: 'kind_num', n: g.n }; break;
    case 'MAT_TOTAL_NUM': cond = { kind: 'total_num', n: g.n }; break;
    case 'MAT_GET_RANK': must(g.rank in RANK, g.id + ': rank'); cond = { kind: 'get_rank', rank: RANK[g.rank] }; break;
    case 'MAT_KIND_ALL': must(matByKey.has(g.x) && matByKey.has(g.y), g.id + ': range'); cond = { kind: 'kind_all', from: matByKey.get(g.x).item_id, to: matByKey.get(g.y).item_id }; break;
    case 'MAT_SPKIND_NUM': must(matByKey.has(g.x), g.id + ': material'); cond = { kind: 'spkind_num', material: g.x, n: g.n }; break;
    default: must(false, g.id + ': condition ' + g.condition);
  }
  let reward;
  switch (g.reward_kind) {
    case 'GET_ITEM': must(ITEMS.has(g.reward_item), g.id + ': reward item'); reward = { kind: 'get_item', item: g.reward_item }; break;
    case 'ENABLE_SYNTHESIS': must(g.unlocks_recipes.length > 0, g.id + ': no recipes'); reward = { kind: 'enable_synthesis', recipes: g.unlocks_recipes }; break;
    case 'ENABLE_BUY': must(g.unlocks_buy.length > 0, g.id + ': nothing to buy'); reward = { kind: 'enable_buy', items: g.unlocks_buy }; break;
    default: must(false, g.id + ': reward ' + g.reward_kind);
  }
  add('goal.' + g.id, { flag: g.flag_index, condition: cond, reward, text: g.text_id }, 'goal', KH3 + 'collect-goals ' + g.id, g.source, '-', g.ref_conf, 'Tested on each Workshop visit in table order, granted once (synth.collect.*, synth.reward.*).');
}
// every COLLECT recipe is named by its goal and back
for (const s of SYN.filter((x) => x.condition === 'COLLECT_LIST')) must(GOALS.find((g) => g.id === s.condition_row).unlocks_recipes.includes(s.id), s.id + ' is not in the recipes of ' + s.condition_row);

// ------------------------------------------------------------------------------------------------ photo.IM_n (ER milestones)
// "Journey to the Erdtree" (the plan above): one row per KH3 Photo Mission row (its flag index, its KH3 texts' ids), its
// milestone, its tier recipe, listed from the start.
add('workshop.milestone_rev', MILESTONE_REV, 'revision', 'tools/kh_workshop_sheet.js JOURNEY', 'rev 1 (2026-11-03): the first 20 milestones; rev 2 (THE USER 2026-11-07): Journey to the Erdtree (unique iconic bosses, all listed from the start, recipes by tier).', '-', 'confirmed',
  'A save\'s Workshop record keeps the revision it was checked against (ext "milestone_rev", absent = 1). On the first Moogle visit with a newer sheet, a stored completion of a row whose `rev` is newer is kept only if its new milestone is met now (its boss defeat flags); every changed row shows NEW again (kh-host workshop.rs migrate_milestones).');
for (const p of PHOTOS) {
  const m = PLAN.get(p.id); must(m, 'no milestone for ' + p.id);
  must(p.unlocks_recipes.length === 1, p.id + ': one recipe per mission');
  const all = m.boss.length > 1;
  const done = m.boss.map(flag);
  const help = all ? 'Prove to the moogle that every shardbearer has fallen and the journey reached the Erdtree. Defeated: {0} / {1}'
    : 'Prove to the moogle that ' + (m.who || bossName(m.boss[0]).replace(/ \(.*\)$/, '')) + (m.who && / Duo$/.test(m.who) ? ' have' : ' has') + ' fallen. (' + m.place + ')';
  const own = typeof m.name === 'string';
  add('photo.' + p.id, { flag: p.flag_index, recipes: [m.recipe], order: m.order, rev: m.rev, done: all ? { all: done } : { flag: done[0] }, bosses: m.boss, name: own ? m.name : '', ...(own ? {} : { name_from: m.name }), help, kh3: { name: p.name_id, help: p.help_id, gate: p.flag_name + ' >= ' + p.flag_label, seven: p.seven_princess, recipe: p.unlocks_recipes[0], order: p.ui_priority } },
    'milestone', KH3 + 'photo-missions ' + p.id + ' (flag index, texts); flags: sheets/port/sora-er-boss-rewards.json boss.' + m.boss.join(' / boss.') + ' (verified, documentation/ER_BOSS_FLAGS.md)',
    m.why, '-', 'confirmed', 'THE USER 2026-11-07 (Journey to the Erdtree): listed from the start; done at the boss\'s defeat (retroactively too), tested on each Moogle visit (dialog_0050), as KH3 tests photos. Recipe by tier (#' + m.order + '). COMPOSED texts (KH3\'s Moogle voice).');
}

// ------------------------------------------------------------------------------------------------ moogle.*
// 6c (the user 2026-11-03): 1 start; 2 Godrick; 3 the second Great Rune boss; 4 the third; 5 Morgott; 6 Maliketh; 7 Godfrey /
// Hoarah Loux. The level is the highest step reached (KH3 only ever raises it: BP_TresRiseShopLevel; level scripts set it on
// load). "The second / third Great Rune boss" = how many of the 7 Great Rune holders (the boss rows with slot:great_rune, the
// same count the boss rewards use for Sora's Great Rune slots) the player has defeated, by their defeat flags.
add('moogle.great_rune_bosses', { bosses: GREAT_RUNE, flags: GREAT_RUNE.map(flag) }, 'flags', 'sheets/port/sora-er-boss-rewards.json rows with slot:great_rune (verified flags)',
  'The 7 holders of a Great Rune: ' + GREAT_RUNE.map(bossName).join('; ') + '. Counted by defeat flag, not by the Great Rune items: the defeat flags are verified from the event scripts (ER_BOSS_FLAGS.md) and are what the boss rewards already count; every Great Rune comes from its holder\'s defeat, so both counts agree, and Morgott counting or not changes nothing (his own step 5 is higher than 4).', '-', 'inferred', '-');
const LEVELS = [
  [1, { start: true }, 'the start (KH3: level 1 is the floor, no setter)'],
  [2, { flag: flag('godrick'), boss: 'godrick' }, 'Godrick the Grafted (KH3: yt_50 YT_EVENT2)'],
  [3, { great_runes: 2 }, 'the second Great Rune boss the player defeats (KH3: sf_50, cutscene sf231)'],
  [4, { great_runes: 3 }, 'the third Great Rune boss (KH3: rg_50 RG_EVENT2)'],
  [5, { flag: flag('morgott'), boss: 'morgott' }, 'Morgott, the Omen King (KH3: kg_51 KG_INTERVAL_02)'],
  [6, { flag: flag('maliketh'), boss: 'maliketh' }, 'Maliketh, the Black Blade (KH3: gm_50 GM_EVENT7)'],
  [7, { flag: flag('hoarah_loux'), boss: 'hoarah_loux' }, 'Godfrey, First Elden Lord / Hoarah Loux (KH3: ew_27 EW_MISSION_END; no item needs level 7)'],
];
for (const [n, when, what] of LEVELS) add('moogle.level.' + n, { level: n, ...when }, 'shop level', 'documentation/KH3_WEAPONS_PLAN.md 6c (the user 2026-11-03); flags: sheets/port/sora-er-boss-rewards.json (verified)', what + '. ' + R('shop.level.set_rule'), '-', 'confirmed', 'The level = the highest step whose condition holds, never lowered.');
add('moogle.mog_card', { key: 'KEY_ITEM05', item_id: 0x905, flag: null }, 'key item', KH3 + 'workshop-rules shop.mogcard.rule', R('shop.mogcard.rule'), '-', 'inferred', 'OPEN: how KH3 hands out the M.O.G. Card was not traced (WD), and the mod has no source for it yet: the rule is in (workshop.rs `moogle_stock(.., has_mog_card)`), the host passes false until a source is chosen.');
// The stock: KH3's one global list (every Moogle sells the same), only items the mod has.
const sells = (r) => ITEMS.has(r.id) || /^WEP_(DONALD|GOOFY)_/.test(r.id);
const moogle = STOCK.filter((r) => r.shop_level > 0 || r.mog_card || r.unlock_flag >= 0);
const kept = moogle.filter(sells), dropped = moogle.filter((r) => !sells(r));
for (const r of kept) must(r.unlock_flag < 0 || GOALS.some((g) => g.flag_index === r.unlock_flag && g.reward_kind === 'ENABLE_BUY'), r.id + ': unlock flag without an ENABLE_BUY goal');
add('moogle.stock', kept.map((r) => ({ key: r.id, item_id: r.item_id, kind: r.kind, buy: r.buy, level: r.shop_level, mog_card: r.mog_card, unlock_flag: r.unlock_flag >= 0 ? r.unlock_flag : null, sold_out: r.sold_out, max: r.max_num })), 'items',
  KH3 + 'shop-stock (m_ShopLevel, m_bNeedMogCard, m_UnlockFlagIndex, m_bCanbeSoldOut, m_MaxNum)', R('shop.level.rule'), '-', 'confirmed', kept.length + ' rows, in the table\'s order. Prices are KH3\'s m_BuyPrice (the plan\'s Q2: the screens apply the mod\'s price factor).');
const why = { food: 'a dish of Le Grand Bistrot: the mod has no cooking', foodstuff: 'a cooking ingredient: no cooking', mognet: 'the Kupo Coin: not an item of the mod (no auto-revive item)' };
add('moogle.stock_excluded', dropped.map((r) => ({ key: r.id, kind: r.kind, level: r.shop_level, mog_card: r.mog_card, why: why[r.kind] || 'not an item of the mod' })), 'items', KH3 + 'shop-stock; sheets/port/sora-item-table.json', 'KH3 Moogle stock rows whose item the mod does not have (sora-item-table + the friends\' weapons).', '-', 'inferred', dropped.length + ' rows left out; the Gummi shop rows (m_GumiShopLevel only) are not the Moogle\'s.');

// ------------------------------------------------------------------------------------------------ forge.<KEY>
add('forge.rule', { munny: 0, sequential: true, stock_first: true }, 'rule', KH3 + 'workshop-rules upgrade.*', R('upgrade.rule.sequential') + '; ' + R('upgrade.rule.gate_materials') + '; ' + R('upgrade.rule.consume'), '0x141128CE0 / 0x14112A3C0', 'inferred', 'No munny at any level (upgrade.munny.*); only current + 1 is offered; materials counted in the Workshop stock, consumed from it first then from the bag, and added to the used count.');
for (const k of KB.slice().sort((a, b) => a.item_id - b.item_id)) {
  const lv = LV.filter((l) => l.keyblade === k.id).sort((a, b) => a.level - b.level);
  must(lv.length === k.max_level && lv.every((l, i) => l.level === i + 1), k.id + ': levels');
  const init = lv.filter((l) => l.init_achieved).reduce((m, l) => Math.max(m, l.level), 0);
  must(init === k.init_level, k.id + ': init level ' + init + ' vs ' + k.init_level);
  must(lv.every((l) => l.munny === 0), k.id + ': a level costs munny');
  add('forge.' + k.id, { item_id: k.item_id, name: k.name, strength: k.strength, magic: k.magic, abilities: k.abilities, max: k.max_level, init: k.init_level,
    levels: lv.map((l) => ({ level: l.level, strength: l.strength_plus, magic: l.magic_plus, ability: l.ability === '-' ? null : l.ability, materials: pairs(l.materials, l.counts), init: l.init_achieved })) },
  'keyblade', KH3 + 'keyblades ' + k.id + '; ' + KH3 + 'keyblade-levels ' + k.id + '.lv1..lv' + k.max_level, k.source, '-', 'confirmed', 'Stats = base + every done level (upgrade.rule.stats); init levels count as done on obtaining (P1 weapons.rs keeps the level).');
}

// ------------------------------------------------------------------------------------------------ write + register
const file = P('sheets', 'port', 'kh-workshop.json');
fs.writeFileSync(file, '{"sheet":"kh-workshop","group":"port","columns":' + JSON.stringify(COLS) + ',"rows":[\n' + OUT.map((r) => JSON.stringify(r)).join(',\n') + '\n]}\n');
console.log('sheets/port/kh-workshop.json: ' + OUT.length + ' rows (' + kept.length + ' stock rows, ' + dropped.length + ' left out)');

const NL = '\n', END = '] },' + NL;
const q = (s) => "'" + String(s).replace(/\\/g, '\\\\').replace(/'/g, "\\'") + "'";
const ENTRY = "  // (P6a) the Moogle Workshop's rules: recipes, Collector's Goals, Keyblade Forge, Moogle shop levels and stock, the Photo Missions as Elden Ring milestones. Written by tools/kh_workshop_sheet.js; read by crates/kh-host/src/workshop.rs (documentation/KH3_WEAPONS_PLAN.md P6a)" + NL +
  "  'kh-workshop': { group: 'port', file: 'kh-workshop.json', by: 'tools/kh_workshop_sheet.js', from: " + q('the KH3 data sheets sheets/port/kh3-* (documentation/KH3_WEAPON_DATA.md), the mod\'s item table and the verified Elden Ring defeat flags of sheets/port/sora-er-boss-rewards.json; the milestones (Journey to the Erdtree, the user 2026-11-07) with COMPOSED texts') + ', external: true, cols: [' + NL +
  "    ['id', K, 'str', '-', 'workshop.* rules; mat.<MAT_ITEMnn>; syn.<IS_n>; goal.<IC_n>; photo.<IM_n> (an Elden Ring milestone); moogle.level.<1..7>, moogle.great_rune_bosses, moogle.mog_card, moogle.stock, moogle.stock_excluded; forge.rule, forge.<WEP_KEYBLADE_SO_nn>'], ['value', V, 'json', '-', 'the row\\'s object (see the note of each row kind in tools/kh_workshop_sheet.js)'], ['unit', V, 'str', '-', 'what the row is'], ['source', V, 'str', '-', 'the KH3 sheet row / flag source', true], ['evidence', V, 'str', '-', 'KH3\\'s rule or why the choice was made', true], ['ref_addr', V, 'str', '-', 'function address(es) of KH3.exe'], ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = KH3 data / the user\\'s decision; inferred = decompiled C or a proposal'], ['note', V, 'str', '-', '-', true]" + END;
{
  const f = P('tools', 'lib', 'sheet_defs.js'); const before = fs.readFileSync(f, 'utf8'); let s = before;
  const at = s.indexOf("  'kh-workshop': {");
  if (at < 0) {
    const anchor = s.indexOf("  'sora-er-boss-rewards': {"); must(anchor >= 0, 'sheet_defs.js: anchor not found');
    const p = s.lastIndexOf(NL, s.lastIndexOf('  //', anchor)) + 1;
    s = s.slice(0, p) + ENTRY + s.slice(p);
  } else {
    const c = s.lastIndexOf(NL, at - 1) + 1, cs = s.lastIndexOf(NL, c - 2) + 1;
    must(s.slice(cs, c).includes('(P6a)'), 'the entry of kh-workshop is not this tool\'s (no "(P6a)" comment above it)');
    const e = s.indexOf(END, at); must(e > at, 'end of the entry of kh-workshop');
    s = s.slice(0, cs) + ENTRY + s.slice(e + END.length);
  }
  if (s !== before) { fs.writeFileSync(f, s); console.log('registered kh-workshop in tools/lib/sheet_defs.js'); }
}
