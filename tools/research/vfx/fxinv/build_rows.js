// Builds rows_effects.json, rows_effects_outofscope.json, effect_list.json from the mined data.
// Inputs: notifies_all.json (mine_notifies.js), animsets.json (animsets.js), decoded packages.
const L = require('./lib'); const { fs, path } = L;
const COLS = ['id', 'category', 'trigger_kind', 'trigger', 'clip', 'logical', 'time', 'duration', 'effect', 'attach', 'params', 'source', 'evidence', 'ref_addr', 'ref_conf', 'note'];
const MAIN = '/Game/Character/pc/p_ex001/anm/p_ex001_AnimSet';
const WEPSET = '/Game/Character/wep/w_so010/anm/w_so010_AnimSet';
const P = '/Game/Blueprints/Player/p_ex001/';
const notifies = L.readJson(path.join(L.OUT, 'notifies_all.json'));
const AS = L.readJson(path.join(L.OUT, 'animsets.json'));
const rows = [], oos = [], ids = new Set();
const pkgCache = new Map();
function pkg(g) { g = g.split('.')[0]; if (!pkgCache.has(g)) { const f = L.gameToPkgFile(g); pkgCache.set(g, fs.existsSync(f) ? L.loadPkg(f) : null); } return pkgCache.get(g); }
const cooked = (g) => 'KINGDOM HEARTS III/Content/' + g.split('.')[0].replace('/Game/', '') + '.uasset';
const short = (s) => s.replace(/^E?SQEX_ATTACH_OBJECT_/, '').replace(/^ESQEX_[A-Z_]*?_(TYPE|THEMES)_/, '');
function add(target, r) {
  let id = r.id, n = 1; while (ids.has(id)) id = r.id + '~' + (++n); ids.add(id); r.id = id;
  for (const c of COLS) if (!(c in r)) r[c] = c === 'clip' || c === 'logical' || c === 'ref_addr' ? '-' : null;
  if (r.ref_conf === 'guess') r.effect = null;
  target.push(COLS.map((c) => r[c]));
}
const fxPath = (s) => (s ? s : null);
const isFx = (s) => typeof s === 'string' && /^\/Game\/Effects\//.test(s);
function kkScope(effect, group) {
  if (!effect) return 'undetermined';
  const m = effect.match(/\/Effects\/(?:[a-z0-9_]+\/)*wep\/(w_[a-z]{2}\d{3})\//i);
  if (m && m[1].toLowerCase() !== 'w_so010') return 'other_keyblade';
  const bn = effect.split('.').pop().toLowerCase().match(/^[vw][ws]_(so|ex)(\d{3})_/);
  if (bn && !(bn[1] === 'so' && bn[2] === '010')) return m ? 'undetermined' : 'other_keyblade'; // other-weapon name inside the w_so010 folder: not decided here
  if (/\/Effects\/wep\/(anti)\//.test(effect)) return 'other_form';
  if (/\/Effects\/wep\/limit\//.test(effect)) return 'second_form';
  return 'kk';
}

// ---------- categories by logical animation name ----------
function catOf(names) {
  const t = (re) => names.some((n) => re.test(n));
  if (t(/^Selfie|^OpenTreasure|^PuddingEat|^Idle|^Battle(Start|End)|^RunBattle/)) return ['other', 'misc'];
  if (t(/^BadStat/)) return ['abnormal', 'bad status animation'];
  if (t(/^Item(Air)?Use/)) return ['item', 'item use animation'];
  if (t(/^StyleChange(Loop|End|Easy)?$/)) return ['formchange', 'formchange transition (generic StyleChange clips of the main set)'];
  if (t(/^StyleChange\d+$/)) return ['form_pointer', 'formchange transition of another keyblade/form'];
  if (t(/^M_14(1\d|2\d|3\d)$/)) return ['form_pointer', 'attack of a formchange moveset in the main set (Effects/wep/limit = Second Form; some rows other keyblades)'];
  if (t(/^ShootFlow|^M_14(5\d|6\d|7\d|8\d)$/)) return ['shotlock', 'shotlock (ShootFlow) animation'];
  if (t(/^Magic|^M_13(59|6\d)$/) && t(/^Magic/)) return ['magic', 'magic cast animation'];
  if (t(/^(Guard|GuardHit|AirGuard|AirGuardHit|Reflec|AirReflec|AttackReflect|AttackAirReflect)$/)) return ['guard', 'guard / reflect animation'];
  if (t(/^SGuard|^MCurling|^GShoot|^SCSwing|^JSpin|^DoU|^GooF|^CMeteo|^FForce|^Gsoldier|^M_15\d\d$|^BallPool|^BallRide|^M_1500|^M_1502/)) return ['link_attraction', 'team attack / attraction / link pointer'];
  if (t(/^(ChaseAttack|Revenge|BlowCounter|HookUpper|BackRaid|BackRush|WarpSlash|DiveFallAttack)|^13[0-2]\d$|^M_13(59|6\d)$|^M_114\d$|^M_118[89]$|^118\d$/)) return ['melee', 'attack animation'];
  if (t(/^0[2]\d\d$|^M_02\d\d$|^Catch|^AirRecovery/)) return ['damage', 'damage reaction / recovery animation'];
  if (t(/^(Walk|Run|Jump|Land|Glide|DiveFall|Dodge|Slide|AirDodge|AirSlide|OverSlide|UnderSlide|Climbing|WallRun|WallJump|WallKick|Pole|Rail|Hop|Balance|Grip|Swim|Diving|WaterFlow|Cover|Athletic|SuperSlide|SonicSlide|SlopeSlide|FallSlide|RollLand|LightLand|Turn)/)) return ['locomotion', 'movement animation'];
  return ['other', 'unclassified logical animation'];
}
function clipInfo(clip) {
  const ci = AS.clipIndex[clip] || [];
  const main = ci.filter((c) => c.set === MAIN || c.set === WEPSET);
  return { main: [...new Set(main.map((c) => c.name))], all: [...new Set(ci.map((c) => c.set.split('/').pop() + ':' + c.name))] };
}

// ---------- SQEX attach data expansion ----------
const commonEffects = {}; // owner kind -> [{CommonKey, GroupID, CommonEffect, source}]
const designated = {};    // owner kind -> [{Key, asset, source}]
function loadAttachComp(kind, g) {
  const d = pkg(g); if (!d) return;
  for (const e of d.exports) if (e.class === 'TresEffectAttachComponent') {
    const p = L.exportProps(d, e);
    for (const c of (p.CommonEffectList || { items: [] }).items) (commonEffects[kind] = commonEffects[kind] || []).push({ ...c, source: g });
    for (const c of (p.DesignatedAttachDataList || { items: [] }).items) (designated[kind] = designated[kind] || []).push({ Key: c.Key, asset: c.DesignatedAttachData, source: g });
    for (const c of p.CommonAttachDataList || []) (designated[kind] = designated[kind] || []).push({ Key: 'None', asset: c, source: g });
  }
}
loadAttachComp('weapon', P + 'w_so010_Wep'); loadAttachComp('weapon', P + 'w_so_KeybladeBase');
loadAttachComp('owner', P + 'p_ex001_Pawn'); loadAttachComp('owner', P + 'p_ex001_PawnBase');
function metaItems(m) { // SQEX_StructParticleAttachData -> list of {attach, params, effect entries}
  const attach = { to_attach: m.ToAttach, socket: m.SocketName, end_socket: m.EndSocketName === 'None' ? null : m.EndSocketName, offset: m.LocationOffset, rotation: m.RotationOffset, scale: m.ScaleOffset, absolute: { location: m.bAbsoluteLocation, rotation: m.bAbsoluteRotation, scale: m.bAbsoluteScale }, detach_at_end: m.DetachAtEnd };
  const par = { data_description: (m.Description || {}).s || '', themes: short(m.AttachThemes), play_number: m.PlayNumber, when_change_animation: short(m.WhenChangeAnimationType), end_type: short(m.AttachEffectEndType), fade_in: m.FadeIn, fade_out: m.FadeOut, start_delay: m.StartDelayTime, end_delay: m.EndDelayTime, allow_multiple: m.bAllowMultipleInstances, use_camera_position: m.bUseCameraPositon, fov: m.FieldOfView, color: m.ParticleColor, alpha: m.ParticleAlpha, cast_shadow: m.bOnCastShadow, takeover_visible: m.bTakeoverAttachObjectVisible, use_common_effect: m.bUseCommonEffect };
  return (m.EffectDataList || { items: [] }).items.map((e, k) => ({ attach, k, par: { ...par, group_id: e.GroupID, check_end_group: e.bCheckEndGroupID, end_group_id: e.EndGroupID, custom_attach_name: e.CustomAttachName === 'None' ? null : e.CustomAttachName, object_override: e.UseAttachObjectData ? { type: short(e.AttachObjectData.AttachType), socket: e.AttachObjectData.ObjectSearchSocketName } : null, area_effect: e.bUseAreaEffect ? e.AreaEffectType : null, no_cleanup_target: e.bNoCleanupTarget }, effect: e.AttachEffect || null }));
}
function eadItems(g) { const d = pkg(g); if (!d) return null; const out = []; for (const e of d.exports) if (e.class === 'SQEX_ParticleAttachDataAsset') { const p = L.exportProps(d, e); (p.EffectDataList || { items: [] }).items.forEach((m, j) => metaItems(m).forEach((x) => out.push({ ...x, j }))); } return out; }
function resolveCommon(kind, x) { // bUseCommonEffect: CommonKey == CustomAttachName && GroupID == data GroupID (fxsys: 0x141766eb0 [C])
  const c = (commonEffects[kind] || []).find((c) => c.CommonKey === x.par.custom_attach_name && c.GroupID === x.par.group_id);
  return c || null;
}

// ---------- 1. animation notifies ----------
const NOTIFY_ADDR = { TresAnimNotify_AttachEffect: '0x14176c560', TresAnimNotifyState_AttachEffect: '0x14176c870', TresAnimNotify_Shot: '0x140561b70', TresAnimNotify_SetWeaponVisible: '0x140561af0', TresAnimNotify_ThrowWeapon: '0x140561c20', TresAnimNotifyState_Attack: '0x140561f40' };
const VISUAL = /AttachEffect|SetWeaponVisible|ThrowWeapon|SetMeshVisible|SetActorVisible|PlayEffect|CameraShake|^TresAnimNotify_Shot$|^TresAnimNotifyState_Attack$/;
// projectile name -> class, hit-effect groups for Attack overrides
const projByName = {};
for (const [setName, g] of [['p_ex001_ProjectileSet', P + 'p_ex001_ProjectileSet'], ['w_so010_ProjSet', P + 'w_so010_ProjSet']]) { const d = pkg(g); for (const u of L.exportProps(d, d.exports[0]).Projectiles.items) projByName[u.ProjectileName] = { cls: u.ProjectileData, set: setName }; }
const hitGroups = {}; // group name -> [{owner, item, source}]
function loadHit(owner, g) { const d = pkg(g); for (const e of d.exports) if (e.class === 'TresAtkCollComponent') { const p = L.exportProps(d, e); ((p.m_HitEffects || {}).items || []).forEach((it, i) => (hitGroups[it.m_GrpName] = hitGroups[it.m_GrpName] || []).push({ owner, item: it, i, source: g, comp: e.objectName })); } }
loadHit('weapon', P + 'w_so010_Wep'); loadHit('pawn', P + 'p_ex001_PawnBase');
const stats = { notif_in: 0, notif_out: 0 };
for (const n of notifies) {
  if (!VISUAL.test(n.class)) continue;
  const ci = clipInfo(n.clip);
  const inScope = ci.main.length > 0 && (n.who === 'wep' || /\/anm\/(rt|localization\/rt)\/body/.test(n.clip));
  const isWepClip = n.who === 'wep';
  const target = inScope || isWepClip ? rows : oos;
  const clipId = n.clip.split('/').pop();
  const cls = n.class, clsShort = cls.replace(/^TresAnimNotify(State)?_/, (m, s) => (s ? 'S.' : '')).replace(/^AnimNotify_/, '');
  const objIdx = (n.object || '').match(/_(\d+)$/); const base = `${isWepClip ? 'wepanim' : 'anim'}.${clipId}.${clsShort}.${objIdx ? objIdx[1] : n.order}`;
  let [category, sub] = inScope ? catOf(ci.main) : ['out_of_scope', 'clip not in p_ex001_AnimSet (other form / world / link / event set)'];
  if (isWepClip) [category, sub] = ['keyblade_appear', 'weapon animation clip'];
  const tk = n.kind === 'state' ? 'anim_notify_state' : 'anim_notify';
  const common = { category, trigger_kind: tk, trigger: cls, clip: clipId, logical: inScope || isWepClip ? ci.main : ci.all, time: n.time, duration: n.kind === 'state' ? n.duration : null, source: n.cooked, ref_addr: NOTIFY_ADDR[cls] || '-' };
  const cp = { subcategory: sub, notify_description: (n.props.Description || {}).s || '', notify_name: n.name, trigger_time_offset: n.trig, end_trigger_time_offset: n.kind === 'state' ? n.endTrig : undefined, clip_length: n.length, trigger_chance: n.chance, track: n.track, anim_sets: inScope ? undefined : ci.all };
  if (inScope) stats.notif_in++; else stats.notif_out++;
  if (/AttachEffect/.test(cls)) {
    let k = 0;
    ((n.props.MetaAttachDataList || {}).items || []).forEach((l, i) => {
      ((l.MetaAttachData || {}).items || []).forEach((m, j) => metaItems(m).forEach((x) => {
        const scope = kkScope(x.effect, x.par.group_id);
        add(target, { ...common, id: `${base}.${k++}`, effect: x.effect, attach: { mode: short(l.AttachType), list_socket: l.SocketName === 'None' ? null : l.SocketName, ...x.attach }, params: { ...cp, ...x.par, kk_scope: scope }, evidence: `[A] ${n.cooked}: export ${n.object} (${cls}) MetaAttachDataList[${i}].MetaAttachData[${j}].EffectDataList[${x.k}].AttachEffect; time = FAnimNotifyEvent LinkValue of the sidecar`, ref_conf: x.effect ? 'confirmed' : 'confirmed', note: x.effect ? (scope === 'other_keyblade' ? 'effect of another keyblade inside a shared clip (GroupID ' + x.par.group_id + '); whether it plays with the Kingdom Key depends on the group filter in code [?]' : '') : 'AttachEffect is null in the asset (no effect spawned by this entry)' });
      }));
      if (!((l.MetaAttachData || {}).items || []).length) add(target, { ...common, id: `${base}.${k++}`, effect: null, attach: { mode: short(l.AttachType), list_socket: l.SocketName }, params: { ...cp, kk_scope: 'undetermined' }, evidence: `[A] ${n.cooked}: export ${n.object} MetaAttachDataList[${i}] has no MetaAttachData`, ref_conf: 'confirmed', note: 'empty entry' });
    });
    ((n.props.AttachDataList || {}).items || []).forEach((l, i) => {
      const mode = short(l.AttachType); const kind = /WEAPON/.test(mode) ? 'weapon' : 'owner';
      let assets = (l.AttachDatas || []).filter(Boolean); let how = 'AttachDatas';
      if (l.bUseCommonAttachData && !assets.length) { assets = (designated[kind] || []).filter((d) => d.Key === 'None').map((d) => d.asset); how = 'common'; }
      if (!assets.length) { add(target, { ...common, id: `${base}.${k++}`, effect: null, attach: { mode, list_socket: l.SocketName }, params: { ...cp, use_common_attach_data: l.bUseCommonAttachData, list_group_id: l.GroupID, kk_scope: 'undetermined' }, evidence: `[A] ${n.cooked}: export ${n.object} AttachDataList[${i}] {bUseCommonAttachData ${l.bUseCommonAttachData}, AttachDatas ${JSON.stringify(l.AttachDatas)}}`, ref_conf: 'confirmed', note: kind === 'owner' && l.bUseCommonAttachData ? 'owner has no common attach data (p_ex001 TresEffectAttach0 lists none): nothing resolved' : 'no data asset referenced: nothing spawned by this entry' }); return; }
      for (const a of assets) for (const x of eadItems(a) || []) {
        let effect = x.effect, conf = how === 'common' ? 'inferred' : 'confirmed', note = '', authored;
        if (x.par.use_common_effect) { const c = resolveCommon(kind, x); authored = x.effect; effect = c ? c.CommonEffect : null; conf = c ? 'inferred' : 'inferred'; note = c ? `common effect key ${x.par.custom_attach_name} (GroupID ${x.par.group_id}) resolved on ${c.source.split('/').pop()} TresEffectAttach0.CommonEffectList` : `common effect key ${x.par.custom_attach_name} not present on the ${kind}`; }
        if (how === 'common') note = (`bUseCommonAttachData: the ${kind}'s TresEffectAttach0.DesignatedAttachDataList[Key None] = ${a.split('.').pop()} (${(designated[kind].find((d) => d.asset === a) || {}).source.split('/').pop()}); ` + note).trim();
        add(target, { ...common, id: `${base}.${k++}`, effect, attach: { mode, list_socket: l.SocketName === 'None' ? null : l.SocketName, ...x.attach }, params: { ...cp, ...x.par, use_common_attach_data: l.bUseCommonAttachData, list_group_id: l.GroupID, data_asset: a.split('.')[0], authored_effect: authored, common_key: x.par.use_common_effect ? x.par.custom_attach_name : undefined, kk_scope: effect ? kkScope(effect, x.par.group_id) : 'undetermined' }, evidence: `[A] ${n.cooked}: export ${n.object} (${cls}) AttachDataList[${i}] -> ${cooked(a)} EffectDataList[${x.j}].EffectDataList[${x.k}]` + (x.par.use_common_effect ? ` (bUseCommonEffect, CustomAttachName ${x.par.custom_attach_name}) -> KINGDOM HEARTS III/Content/Blueprints/Player/p_ex001/w_so010_Wep.uasset TresEffectAttach0.CommonEffectList` : '') + '; lookup rule per fxsys strand 0x141766eb0 / 0x141771470 [C]', ref_conf: conf, note });
      }
    });
    if (k === 0) add(target, { ...common, id: `${base}.0`, effect: null, params: { ...cp, kk_scope: 'undetermined' }, evidence: `[A] ${n.cooked}: export ${n.object} has neither MetaAttachDataList nor AttachDataList entries`, ref_conf: 'confirmed', note: 'notify without data' });
  } else if (cls === 'TresAnimNotify_Shot') {
    if (!inScope) continue;
    const pn = n.props.m_ProjectileName || null; const pr = pn ? projByName[pn] : null;
    add(target, { ...common, id: base, category, effect: null, attach: { mode: 'projectile spawn', location_index: n.props.LocationIndex ?? null }, params: { ...cp, projectile_name: pn, projectile_class: pr ? pr.cls : null, projectile_set: pr ? pr.set : null, attach_fire_effect: n.props.m_bAttachFireEffect ?? false, kk_scope: 'kk' }, evidence: `[A] ${n.cooked}: export ${n.object} (TresAnimNotify_Shot) ${JSON.stringify(n.props)}`, ref_conf: 'confirmed', note: pn ? (pr ? `spawns projectile ${pn}; its effects are the rows proj.${pn}.*` : `projectile name ${pn} is not in p_ex001_ProjectileSet / w_so010_ProjSet (other set / form)`) : 'no m_ProjectileName: the projectile comes from the current command (magic rows magic.*)' });
  } else if (cls === 'TresAnimNotifyState_Attack') {
    if (!inScope || !n.props.m_bOverrideHitEffect) continue;
    const gname = n.props.m_HitEffectGrpName || 'None'; const gs = hitGroups[gname] || [];
    if (!gs.length) add(target, { ...common, id: base, effect: null, params: { ...cp, attack_id: n.props.m_AttackDataIDName, hit_effect_group: gname, kk_scope: 'undetermined' }, evidence: `[A] ${n.cooked}: export ${n.object} m_bOverrideHitEffect, m_HitEffectGrpName ${gname}`, ref_conf: 'confirmed', note: `hit-effect group ${gname} is defined neither on w_so010_Wep nor on p_ex001_PawnBase TresAtkColl0.m_HitEffects (other weapon / form)` });
    for (const g of gs) for (const [k, v] of Object.entries(g.item)) if (isFx(v)) add(target, { ...common, id: `${base}.${g.owner}.${k.replace(/[\[\]]/g, '')}`, effect: v, attach: { mode: 'hit position' }, params: { ...cp, attack_id: n.props.m_AttackDataIDName, collision_group: n.props.CollisionGrpName, hit_effect_group: gname, slot: k, kk_scope: kkScope(v) }, evidence: `[A] ${n.cooked}: export ${n.object} m_bOverrideHitEffect + m_HitEffectGrpName ${gname} -> ${cooked(g.source)} ${g.comp}.m_HitEffects[${g.i}].${k}`, ref_conf: 'inferred', note: 'group-name match between the notify and the attack-collision component; the selection code was not read' });
  } else {
    add(target, { ...common, id: base, category: /Weapon/.test(cls) && inScope ? 'keyblade_appear' : category, effect: null, attach: cls === 'AnimNotify_TresPlayEffect_C' ? { mode: n.props.Attached ? 'attached' : 'location', socket: n.props.SocketName, offset: n.props.LocalOffset, rotation: n.props.RotationOffset, absolute: { scale: n.props.AbsoluteScale } } : null, params: { ...cp, ...n.props, kk_scope: 'kk' }, evidence: `[A] ${n.cooked}: export ${n.object} (${cls}) ${JSON.stringify(n.props)}`, ref_conf: 'confirmed', note: cls === 'TresAnimNotify_SetWeaponVisible' ? 'shows/hides the keyblade; defaults of unserialized properties (m_bNewVisible, m_bAppearEffect) are the class defaults [?]; the appear effect itself is row weapon.appear' : cls === 'TresAnimNotify_ThrowWeapon' ? 'detaches and throws the keyblade (weapon animation m_WeaponAnimName)' : cls === 'AnimNotify_TresPlayEffect_C' ? 'blueprint notify; its particle template is a property default of the notify blueprint (not serialized here)' : '' });
  }
}

// ---------- generic walker over a package: every effect reference with its containing struct ----------
function scal(o) { const r = {}; if (o && typeof o === 'object' && !Array.isArray(o)) for (const [k, v] of Object.entries(o)) if (v === null || typeof v !== 'object') { if (typeof v !== 'string' || !/^\/Game\/Sound/.test(v)) r[k] = v; } else if (v && typeof v === 'object' && !Array.isArray(v) && ('x' in v || 'pitch' in v)) r[k] = v; return r; }
function walkRefs(v, pre, parent, out) {
  if (typeof v === 'string') { if (/^\/Game\//.test(v)) out.push({ prop: pre, target: v, parent }); return; }
  if (Array.isArray(v)) { v.forEach((x, i) => walkRefs(x, `${pre}[${i}]`, parent, out)); return; }
  if (v && typeof v === 'object') for (const k of Object.keys(v)) { if (k === 'structName') continue; walkRefs(v[k], k === 'items' ? pre : (pre ? pre + '.' + k : k), k === 'items' ? parent : v, out); }
}
function pkgRefs(g) { // [{export, cls, isCDO, prop, target, tclass, parent}]
  const d = pkg(g); if (!d) return null; const icm = {}; d.imports.forEach((im, k) => { icm[L.ref(d, -(k + 1))] = im.className; });
  const out = []; let sup = null;
  for (const e of d.exports) {
    const cls = L.exportClass(d, e);
    if (e.class === 'BlueprintGeneratedClass' && e.superIndex) { const s = L.ref(d, e.superIndex); if (/^\/Game\//.test(s)) sup = s.split('.')[0]; }
    if (/^(Function|.*Property$|SCS_Node|SimpleConstructionScript|BlueprintGeneratedClass)/.test(cls)) continue;
    const props = L.exportProps(d, e); const refs = []; walkRefs(props, '', props, refs);
    for (const r of refs) out.push({ export: e.objectName, cls, isCDO: /^Default__/.test(e.objectName), prop: r.prop, target: r.target, tclass: icm[r.target] || null, parent: scal(r.parent), compProps: scal(props) });
  }
  return { refs: out, sup };
}
function chainRefs(g) { // effective refs of a blueprint class including super blueprints (derived overrides base by export|prop; arrays replaced as a whole)
  const chain = []; let cur = g.split('.')[0]; const seen = new Set();
  while (cur && !seen.has(cur)) { seen.add(cur); const r = pkgRefs(cur); if (!r) break; chain.unshift({ g: cur, ...r }); cur = r.sup; }
  const eff = new Map();
  for (const c of chain) {
    const roots = new Set(c.refs.map((r) => (r.isCDO ? 'CDO' : r.export) + '|' + r.prop.split(/[.\[]/)[0]));
    for (const k of [...eff.keys()]) { const [ex, pr] = k.split('|'); if (roots.has(ex + '|' + pr.split(/[.\[]/)[0])) eff.delete(k); }
    for (const r of c.refs) eff.set((r.isCDO ? 'CDO' : r.export) + '|' + r.prop, { ...r, definedIn: c.g });
  }
  return { list: [...eff.values()], chain: chain.map((c) => c.g) };
}
function roleOf(r) {
  const p = r.prop;
  if (/^MyFireEffects/.test(p)) return 'fire';
  if (/m_PawnHitEffectPerAttr/.test(p)) return 'hit_attr' + ((p.match(/PerAttr\[(\d+)\]/) || [0, 0])[1]);
  if (/m_PawnHitEffect/.test(p)) return 'hit';
  if (/m_DirectHitEffect/.test(p)) return 'direct_hit';
  if (/m_MapHitEffect/.test(p)) return 'map_hit';
  if (r.cls === 'ParticleSystemComponent' && p === 'Template') return 'body';
  return p.replace(/^m_p?/, '').replace(/[.\[\]]+/g, '_').replace(/_$/, '');
}
const ATTR = ['PHYSICAL', 'FIRE', 'BLIZZARD', 'THUNDER', 'WATER', 'AERO', 'DARK', 'NOTYPE'];
function emitBlueprint(target, g, idBase, category, extra, depth = 0, seenCls = new Set(), kind = 'projectile') {
  g = g.split('.')[0]; if (seenCls.has(g)) return; seenCls.add(g);
  const { list, chain } = chainRefs(g);
  for (const r of list) {
    if (r.tclass === 'ParticleSystem' || /\/Effects\/.*\/(par|ead)\//.test(r.target)) {
      const role = roleOf(r); const isEad = r.tclass === 'SQEX_ParticleAttachDataAsset';
      const hitm = r.prop.match(/m_HitEffects\[(\d+)\]/);
      if (isEad) { emitEad(target, r.target, `${idBase}.${role}`, category, { ...extra, via: `${cooked(r.definedIn)} ${r.export}.${r.prop}` }, kind); continue; }
      add(target, { id: `${idBase}.${role}`, category, trigger_kind: kind, trigger: `${r.isCDO ? 'CDO' : r.export + ' (' + r.cls + ')'}.${r.prop}`, effect: r.target, attach: r.cls === 'ParticleSystemComponent' ? { mode: 'particle component of the spawned actor', component: r.export, ...Object.fromEntries(Object.entries(r.compProps).filter(([k]) => k !== 'Template')) } : /^MyFireEffects/.test(r.prop) ? { mode: 'fire effect (spawned at launch)', ...Object.fromEntries(Object.entries(r.parent).filter(([k]) => k !== 'MyParticleSystem')) } : /Hit/.test(role) ? { mode: role === 'map_hit' ? 'map hit position' : 'hit position' } : null, params: { ...extra, role, blueprint: g, defined_in: r.definedIn, class_chain: chain, hit_group: hitm ? r.parent.m_GrpName : undefined, hit_group_flags: hitm ? { m_bChangeMapHit: r.parent.m_bChangeMapHit, m_bEnableDecal: r.parent.m_bEnableDecal } : undefined, attribute: /hit_attr/.test(role) ? ATTR[+role.slice(8)] : undefined, depth, kk_scope: kkScope(r.target) }, source: cooked(r.definedIn), evidence: `[A] ${cooked(r.definedIn)}: export ${r.export} (${r.cls}) ${r.prop}` + (r.definedIn !== g ? ` (inherited by ${g.split('/').pop()})` : ''), ref_addr: '-', ref_conf: 'confirmed', note: depth ? 'effect of a projectile spawned by the parent projectile (one level followed)' : '' });
    } else if (r.tclass === 'BlueprintGeneratedClass' && /\/Blueprints\/(Projectile|Player)\//.test(r.target) && depth < 1 && r.prop !== '(super)') {
      emitBlueprint(target, r.target, `${idBase}.${r.prop.replace(/^m_/, '').replace(/[.\[\]]+/g, '_')}`, category, { ...extra, spawned_via: `${r.export}.${r.prop}` }, depth + 1, seenCls, kind);
    }
  }
}
function emitEad(target, g, idBase, categoryFn, extra, kind = 'component_table', filter = null) {
  const items = eadItems(g); if (!items) return;
  for (const x of items) {
    if (filter && !filter(x)) continue;
    const cat = typeof categoryFn === 'function' ? categoryFn(x) : categoryFn;
    add(target, { id: `${idBase}.g${x.par.group_id}.${(x.effect || 'null').split('.').pop()}`, category: Array.isArray(cat) ? cat[0] : cat, trigger_kind: kind, trigger: `${g.split('/').pop()} EffectDataList[${x.j}].EffectDataList[${x.k}] (GroupID ${x.par.group_id})`, effect: x.effect, attach: { mode: x.par.object_override ? x.par.object_override.type : 'component owner', ...x.attach }, params: { ...extra, subcategory: Array.isArray(cat) ? cat[1] : undefined, ...x.par, data_asset: g.split('.')[0], kk_scope: kkScope(x.effect) }, source: cooked(g), evidence: `[A] ${cooked(g)}: SQEX_ParticleAttachDataAsset EffectDataList[${x.j}] "${x.par.data_description}" .EffectDataList[${x.k}] {GroupID ${x.par.group_id}, AttachEffect}` + (extra && extra.via ? `; referenced by ${extra.via}` : ''), ref_addr: extra && extra.ref_addr ? extra.ref_addr : '-', ref_conf: 'confirmed', note: (extra && extra.note) || 'played by code that passes this GroupID to the effect-attach component (entry point not determined here [?])' });
  }
}

// ---------- 2. weapon tables ----------
{
  const g = P + 'w_so010_Wep', d = pkg(g), src = cooked(g);
  const cdo = L.exportProps(d, d.exports.find((e) => /^Default__/.test(e.objectName)));
  const wp = { m_AppearVisibleDelayTime: cdo.m_AppearVisibleDelayTime, m_bIsEnableAppearDitherAnim: cdo.m_bIsEnableAppearDitherAnim, m_bIsDelayAppearDitherAnim: cdo.m_bIsDelayAppearDitherAnim, m_AppearDitherAnimTime: cdo.m_AppearDitherAnimTime };
  add(rows, { id: 'weapon.appear', category: 'keyblade_appear', trigger_kind: 'component_table', trigger: 'w_so010_Wep_C.m_AppearEffect', effect: cdo.m_AppearEffect, attach: { mode: 'weapon', socket: cdo.m_AppearEffectAttach || null }, params: { ...wp, kk_scope: 'kk' }, source: src, evidence: `[A] ${src}: Default__w_so010_Wep_C.m_AppearEffect (+ timing properties); TresWeaponBase.m_AppearEffect offset 1448`, ref_addr: '-', ref_conf: 'confirmed', note: 'm_AppearEffectAttach is not serialized (class default [?]); the spawning function was not located' });
  add(rows, { id: 'weapon.disappear', category: 'keyblade_appear', trigger_kind: 'component_table', trigger: 'w_so010_Wep_C.m_DisappearEffect', effect: cdo.m_DisappearEffect, attach: { mode: 'weapon', socket: cdo.m_DisappearEffectAttach || null }, params: { m_DisappearVisibleDelayTime: cdo.m_DisappearVisibleDelayTime, m_DisappearScaleAnimTime: cdo.m_DisappearScaleAnimTime, m_bIsEnableDisappearDitherAnim: cdo.m_bIsEnableDisappearDitherAnim, m_DisappearDitherAnimTime: cdo.m_DisappearDitherAnimTime, m_DisappearWithScaleTime: cdo.m_DisappearWithScaleTime, kk_scope: 'kk' }, source: src, evidence: `[A] ${src}: Default__w_so010_Wep_C.m_DisappearEffect; TresWeaponBase.m_DisappearEffect offset 1496`, ref_addr: '-', ref_conf: 'confirmed', note: 'same particle system as the appear effect' });
  for (const [gname, gs] of Object.entries(hitGroups)) for (const h of gs) for (const [k, v] of Object.entries(h.item)) if (isFx(v)) {
    const am = k.match(/PerAttr(?:\[(\d+)\])?/); const role = am ? 'attr' + (am[1] || 0) : k.replace(/^m_/, '').replace('Effect', '').toLowerCase();
    const isW = h.owner === 'weapon';
    add(rows, { id: `${isW ? 'weapon' : 'pawn'}.hit.${gname === 'None' ? 'default' : gname}.${role}`, category: isW && (gname === 'None' || gname === 'RaidRide') ? 'melee' : (isW ? 'form_pointer' : 'link_attraction'), trigger_kind: 'component_table', trigger: `${h.comp}.m_HitEffects[${h.i}].${k}`, effect: v, attach: { mode: /Map/.test(k) ? 'map hit position' : 'hit position' }, params: { subcategory: /Map/.test(k) ? 'map hit' : /Direct/.test(k) ? 'direct hit' : 'pawn hit', hit_group: gname, attribute: am ? ATTR[+(am[1] || 0)] : undefined, m_bChangeMapHit: h.item.m_bChangeMapHit, m_bEnableDecal: h.item.m_bEnableDecal, kk_scope: kkScope(v) }, source: cooked(h.source), evidence: `[A] ${cooked(h.source)}: ${h.comp} (TresAtkCollComponent) m_HitEffects[${h.i}] {m_GrpName ${gname}}.${k}; attribute index = ETresDamageAttribute`, ref_addr: '-', ref_conf: 'confirmed', note: isW ? (gname === 'None' ? 'default group of the keyblade attack collision' : `named hit group ${gname} (selected by attack notifies with m_HitEffectGrpName)`) : 'attack collision of the pawn itself (named groups used by team attacks / other forms)' });
  }
  for (const c of commonEffects.weapon || []) add(rows, { id: `weapon.common.${c.CommonKey}`, category: c.CommonKey === 'shf_tame' ? 'shotlock' : 'melee', trigger_kind: 'component_table', trigger: `TresEffectAttach0.CommonEffectList {CommonKey ${c.CommonKey}, GroupID ${c.GroupID}}`, effect: c.CommonEffect || null, attach: null, params: { subcategory: 'common effect key table', common_key: c.CommonKey, group_id: c.GroupID, kk_scope: 'kk' }, source: cooked(c.source), evidence: `[A] ${cooked(c.source)}: TresEffectAttach0 (TresEffectAttachComponent) CommonEffectList`, ref_addr: '0x141766eb0', ref_conf: 'confirmed', note: c.CommonEffect ? 'key table row: substituted for the AttachEffect of attach data with bUseCommonEffect and this CustomAttachName (rows anim.*.S.AttachEffect with common_key)' : 'key present with no effect: attach data using this key spawns nothing on the Kingdom Key' });
  for (const dsg of designated.weapon || []) emitEad(rows, dsg.asset, 'weapon.designated.' + dsg.asset.split('.').pop(), 'melee', { subcategory: 'designated/common attach data of the weapon', designated_key: dsg.Key, via: `${cooked(dsg.source)} TresEffectAttach0.DesignatedAttachDataList`, note: 'attach data used by anim notifies with bUseCommonAttachData on the weapon; AttachEffect here is the authored placeholder, replaced through the common key table (weapon.common.key_trail0)' });
  const kb = pkg(P + 'w_so_KeybladeBase'); const kcdo = L.exportProps(kb, kb.exports.find((e) => /^Default__/.test(e.objectName)));
  // magic cast effect: group id = TresPlayerMagicSet m_EffectGroup
  const ms = pkg(P + 'p_ex001_MagicSet'); const units = L.exportProps(ms, ms.exports[0]).m_Units.items;
  const byGroup = {}; for (const u of units) (byGroup[u.m_EffectGroup] = byGroup[u.m_EffectGroup] || []).push(u.m_Command.replace('TRES_CMD_KIND_', ''));
  emitEad(rows, kcdo.m_CmnMagicCastEffect, 'weapon.magic_cast', (x) => [(byGroup[x.par.group_id] || []).some((c) => /CUR/.test(c)) ? 'cure' : 'magic', 'cast effect on the keyblade (' + ((byGroup[x.par.group_id] || []).join('/') || 'group not in p_ex001_MagicSet') + ')'], { via: `${cooked(P + 'w_so_KeybladeBase')} Default__w_so_KeybladeBase_C.m_CmnMagicCastEffect (TresWeaponBase offset 1552)`, magic_commands_by_group: undefined, note: 'GroupID equals TresPlayerMagicSet m_Units[].m_EffectGroup of the spell being cast (data equality; the call site was not read)' });
  for (const r of rows) if (/^weapon\.magic_cast\./.test(r[0])) { const p = r[COLS.indexOf('params')]; p.magic_commands = byGroup[p.group_id] || []; if (!p.magic_commands.length) r[COLS.indexOf('category')] = 'magic'; }
  emitEad(rows, kcdo.m_CmnBadStatesEffect, 'weapon.badstate', 'abnormal', { subcategory: 'bad status effect on the weapon', via: `${cooked(P + 'w_so_KeybladeBase')} Default__w_so_KeybladeBase_C.m_CmnBadStatesEffect (TresWeaponBase offset 1560)` });
  // form change charge
  const eq = pkg(P + 'w_so010_Equip'); const eqp = L.exportProps(eq, eq.exports[0]);
  emitEad(rows, eqp.m_CmnFormChangeChargeEffect, 'weapon.formchange_charge', 'formchange', { subcategory: 'formchange charge effect on the keyblade', via: `${cooked(P + 'w_so010_Equip')} m_CmnFormChangeChargeEffect (same asset on ${cooked(P + 'p_ex001_WepSet')})` });
}

// ---------- 3. pawn tables ----------
{
  const g = P + 'p_ex001_PawnBase', d = pkg(g), src = cooked(g);
  const cdo = L.exportProps(d, d.exports.find((e) => /^Default__/.test(e.objectName)));
  const map = { m_pHopEffect: ['locomotion', 'hop (m_pHopEffect)', 5352], m_pAthleticHitEffect: ['locomotion', 'athletic flow hit', 5392], m_pFocusAspirEffect: ['other', 'focus aspir (focus gauge absorb)', 5408], m_pSwanDiveEffect: ['locomotion', 'swan dive into water', 5416], m_pGuardReflectEffect: ['guard', 'guard reflect', 5424], m_pJustGuardEffect: ['guard', 'just guard', 5432], m_pCriticalCounterHitEffect: ['guard', 'critical counter hit', 5440], m_pCoverShotDestroyEffect: ['link_attraction', 'cover shot destroy', 5448] };
  for (const [k, [cat, sub, off]] of Object.entries(map)) add(rows, { id: 'pawn.' + k.replace(/^m_p/, ''), category: cat, trigger_kind: 'code', trigger: 'TresPlayerPawnBase.' + k, effect: cdo[k] || null, attach: null, params: { subcategory: sub, property_offset: off, kk_scope: 'kk' }, source: src, evidence: `[A] ${src}: Default__p_ex001_PawnBase_C.${k}; reflection: TresPlayerPawnBase.${k} offset ${off}`, ref_addr: '-', ref_conf: 'confirmed', note: 'effect asset confirmed; the function that spawns it (reads pawn+' + off + ') was not located' });
  for (const k of ['m_pFireEffect', 'm_pPoleEffect', 'm_pWallRunEffect', 'm_pSprintEffect', 'm_pDeathSentenceCountEffect']) add(rows, { id: 'pawn.' + k.replace(/^m_p/, ''), category: k === 'm_pDeathSentenceCountEffect' ? 'abnormal' : 'locomotion', trigger_kind: 'code', trigger: 'TresPlayerPawnBase.' + k, effect: null, attach: null, params: { subcategory: 'property not serialized on p_ex001_PawnBase / p_ex001_Pawn', kk_scope: 'undetermined' }, source: src, evidence: `[A] ${src}: property ${k} absent from Default__p_ex001_PawnBase_C and Default__p_ex001_Pawn_C => class default of TresPlayerPawnSora; no hard-coded /Game/Effects path exists in the exe strings`, ref_addr: '0x141423210', ref_conf: 'inferred', note: 'null unless the constructor assigns it (constructor 0x141423210 verdict differs; not read)' });
  add(rows, { id: 'pawn.LensEffectClass', category: 'shotlock', trigger_kind: 'code', trigger: 'TresPlayerPawnBase.m_LensEffectClass', effect: null, attach: { mode: 'camera lens effect actor' }, params: { subcategory: 'shotlock camera lens effect blueprint', blueprint: cdo.m_LensEffectClass, property_offset: 5400, kk_scope: 'kk' }, source: src, evidence: `[A] ${src}: Default__p_ex001_PawnBase_C.m_LensEffectClass = ${cdo.m_LensEffectClass}`, ref_addr: '-', ref_conf: 'confirmed', note: 'blueprint class; its particle template is row pawn.LensEffectClass.body' });
  emitBlueprint(rows, cdo.m_LensEffectClass, 'pawn.LensEffectClass', 'shotlock', { subcategory: 'shotlock lens effect blueprint' }, 0, new Set(), 'component_table');
  emitBlueprint(rows, cdo.m_AthleticRoot, 'pawn.AthleticRoot', 'locomotion', { subcategory: 'athletic flow root marker actor (m_AthleticRoot)' }, 0, new Set(), 'component_table');
  const auraCat = (x) => { const gI = x.par.group_id; return gI >= 3200 && gI <= 3202 ? ['locomotion', 'free/athletic flow and rail slide aura'] : gI === 3405 ? ['guard', 'critical / just guard success aura'] : gI === 15 || gI === 16 ? ['form_pointer', 'Ultima warp'] : gI === 17 || gI === 18 ? ['form_pointer', 'anti form warp'] : ['melee', 'warp vanish/appear (generic 11/12, counter slash 13/14)']; };
  emitEad(rows, cdo.m_CmnAuraEffect, 'pawn.aura', auraCat, { via: `${src} Default__p_ex001_PawnBase_C.m_CmnAuraEffect (TresCharPawnBase offset 2272)` });
  const absCat = (x) => { const gI = x.par.group_id; if (gI === 3010) return ['death', 'KO faint stars']; if (gI >= 3120 && gI <= 3155) return ['item', 'item effect: ' + x.par.data_description]; if (gI >= 1420 && gI <= 1425) return ['cure', 'cure target / area effect']; if (gI === 3090) return ['cure', 'recovery effect']; if (gI === 3110) return ['other', 'focus aspir']; if ([3170, 3190, 3181].includes(gI)) return ['link_attraction', x.par.data_description]; return ['abnormal', x.par.data_description]; };
  emitEad(rows, cdo.m_CmnBadStatesEffect, 'pawn.badstate', absCat, { via: `${src} Default__p_ex001_PawnBase_C.m_CmnBadStatesEffect (TresCharPawnBase offset 2288)`, ref_addr: '0x1405b7c50', note: 'played through the pawn bad-state effect helpers 0x1405b7c50 / 0x1405b77b0 (reference m_CmnBadStatesEffect, verdict same-partial, [C] by member name only); GroupID chosen by code' });
  emitEad(rows, cdo.m_AntiFormEffect, 'pawn.antiform', 'form_pointer', { subcategory: 'anti form effects (pointer)', via: `${src} Default__p_ex001_PawnBase_C.m_AntiFormEffect` });
  for (const k of ['m_CmnMagicCastEffect', 'm_CmnAppearEffect']) add(rows, { id: 'pawn.' + k, category: k === 'm_CmnMagicCastEffect' ? 'magic' : 'other', trigger_kind: 'code', trigger: 'TresCharPawnBase.' + k, effect: null, params: { subcategory: 'property not serialized on Sora', kk_scope: 'undetermined' }, source: src, evidence: `[A] ${src}: ${k} absent from the pawn class default objects => inherited native default (no hard-coded effect path in the exe)`, ref_addr: '-', ref_conf: 'inferred', note: 'the keyblade supplies the cast effect instead (weapon.magic_cast.*)' });
}

// ---------- 4. load assets ----------
{
  const g = '/Game/Load/Player/TresPlayerPawnLoad_ex001', d = pkg(g), p = L.exportProps(d, d.exports[0]), src = cooked(g);
  for (const [k, sub] of [['m_PlayerGameOverHeartEffect', 'game over: heart'], ['m_PlayerGameOverLightEffect', 'game over: light'], ['m_PlayerMissionFailedLightEffect', 'mission failed: light']]) add(rows, { id: 'load.player.' + k.replace('m_Player', ''), category: 'death', trigger_kind: 'load_asset', trigger: 'TresPlayerPawnLoadAsset.' + k, effect: p[k] || null, attach: null, params: { subcategory: sub, camera_anims: { die: p.m_PlayerDieCameraAnim, game_over: p.m_PlayerGameOverCameraAnim, mission_failed: p.m_PlayerMissionFailedCameraAnim }, kk_scope: 'kk' }, source: src, evidence: `[A] ${src}: TresPlayerPawnLoad_ex001.${k} (asset referenced by p_ex001_PawnBase m_LoadAsset)`, ref_addr: '-', ref_conf: 'confirmed', note: 'player-specific game-over effect; location: see TresGameModeLoad m_CmnGameOver*Location' });
  const g2 = '/Game/Load/Common/TresGameModeLoad', d2 = pkg(g2), q = L.exportProps(d2, d2.exports[0]), src2 = cooked(g2);
  const gm = { m_CmnGuardReflectEffect: ['guard', 'common guard reflect'], m_CmnStaggerReflectEffect: ['guard', 'common stagger (attack bounced)'], m_CmnInvincibleHitEffect: ['damage', 'hit on an invincible target'], m_CmnArmorHitEffect: ['damage', 'hit on armor'], m_CmnIgnoreGuardHitEffect: ['damage', 'guard-ignoring hit'], m_CmnGameOverHeartEffect: ['death', 'common game over heart'], m_CmnGameOverLightEffect: ['death', 'common game over light'], m_CmnMissionFailedLightEffect: ['death', 'common mission failed light'] };
  for (const [k, [cat, sub]] of Object.entries(gm)) add(rows, { id: 'load.gamemode.' + k.replace('m_Cmn', ''), category: cat, trigger_kind: 'load_asset', trigger: 'TresGameModeLoadAsset.' + k, effect: q[k] || null, attach: /GameOver|MissionFailed/.test(k) ? { mode: 'location relative to the player', offset: q[k.replace('Effect', 'Location')] } : { mode: 'hit position' }, params: { subcategory: sub, kk_scope: 'kk' }, source: src2, evidence: `[A] ${src2}: TresGameModeLoad.${k} (asset named by DefaultGame.ini m_GameModeLoadAssetName)`, ref_addr: /Reflect|Hit/.test(k) ? '0x1406445e0' : '-', ref_conf: 'confirmed', note: /Reflect|Hit/.test(k) ? 'read by ATresAtkCollManager 0x1406445e0 (verdict same-partial, [C] member names)' : '' });
  add(rows, { id: 'load.gamemode.CriticalHitEffect', category: 'melee', trigger_kind: 'load_asset', trigger: 'TresGameModeLoadAsset.m_CmnCriticalHitEffect', effect: null, params: { subcategory: 'common critical hit effect: not set', kk_scope: 'undetermined' }, source: src2, evidence: `[A] ${src2}: m_CmnCriticalHitEffect (offset 264) is not serialized => null`, ref_addr: '-', ref_conf: 'confirmed', note: 'none found' });
  emitEad(rows, q.m_CmnPlayerDeadEffectEad, 'load.gamemode.dead', 'death', { subcategory: 'player death blur / heart', via: `${src2} m_CmnPlayerDeadEffectEad` });
  // prizes
  for (const [k, v] of Object.entries(q)) if (/Prize/.test(k) && typeof v === 'string') emitBlueprint(rows, v, 'prize.' + k.replace(/^m_/, ''), 'prize', { subcategory: 'prize actor ' + v.split('/').pop().split('.')[0], via: `${src2} ${k}` }, 0, new Set(), 'component_table');
  // physical material effects
  // (release) the surface names are the game's own: read from the player's KH3 config (assets/KINGDOM HEARTS III/Config/
  // DefaultEngine.ini, +PhysicalSurfaces), not stored here; the English after them is ours
  const surfaceNames = (en) => { const ini = require('fs').readFileSync(require('path').resolve(__dirname, '../../../../assets/KINGDOM HEARTS III/Config/DefaultEngine.ini'), 'utf8'); const jp = {}; for (const m of ini.matchAll(/^\+PhysicalSurfaces=\(Type=SurfaceType(\d+),Name="([^"]*)"\)/gm)) jp[m[1]] = m[2]; const out = {}; for (const [k, e] of Object.entries(en)) { if (k !== '0' && jp[k] === undefined) throw new Error('DefaultEngine.ini: no name for SurfaceType' + k); out[k] = k === '0' ? e : [jp[k], e].filter(Boolean).join(' '); } return out; };
  const SURF = surfaceNames({ 0: 'Default', 1: 'mud', 2: 'soil', 4: 'sand', 6: 'grass', 8: 'fallen leaves', 9: 'snow', 10: 'rock/concrete', 11: 'marble/ice', 12: 'carpet', 13: 'wood', 14: 'cloth', 15: 'glass', 16: 'plastic', 17: 'roof tile', 18: 'net', 19: 'metal', 20: 'iron plate', 21: 'treasure', 22: 'paper box', 25: 'gravel', 30: 'frost', 31: 'snow', 32: 'puddle', 33: 'water knee', 34: 'water waist', 35: '', 36: '', 37: '' });
  const physDir = path.join(L.PK, L.KH, 'Load/PhysEffect');
  for (const f of fs.readdirSync(physDir)) {
    const pg = '/Game/Load/PhysEffect/' + f.replace('.uasset.json', ''); const pd = pkg(pg), pp = L.exportProps(pd, pd.exports[0]); const world = f.replace('TresPhysEffects', '').replace('.uasset.json', '');
    const isCommon = q.m_CmnPhysMatEffect && q.m_CmnPhysMatEffect.split('.')[0] === pg;
    for (const [k, v] of Object.entries(pp)) { const m = k.match(/^m_Effects(?:\[(\d+)\])?$/); if (!m) continue; const si = +(m[1] || 0);
      for (const slot of ['m_NmlEffect', 'm_HighEffect', 'm_StillEffect', 'm_EnterEffect', 'm_LeaveEffect']) if (v[slot]) add(rows, { id: `physmat.${world}.s${si}.${slot.replace('m_', '').replace('Effect', '').toLowerCase()}`, category: 'locomotion', trigger_kind: 'data_table', trigger: `TresPhysMatEffectAsset.m_Effects[${si}].${slot}`, effect: v[slot], attach: { mode: 'foot / contact position' }, params: { subcategory: isCommon ? 'surface effect, common table (m_CmnPhysMatEffect)' : 'surface effect, world table ' + world, world, common_table: !!isCommon, surface_index: si, surface_name: SURF[si] || null, m_bEnableInnerWater: v.m_bEnableInnerWater, m_NmlSpeedParam: v.m_NmlSpeedParam, m_HighSpeedParam: v.m_HighSpeedParam, kk_scope: 'kk' }, source: cooked(pg), evidence: `[A] ${cooked(pg)}: ${f.replace('.uasset.json', '')} (TresPhysMatEffectAsset) m_Effects[${si}].${slot}; surface names from Config/DefaultEngine.ini PhysicalSurfaces` + (isCommon ? `; referenced by ${src2} m_CmnPhysMatEffect` : '; referencer of the world tables not determined [?]'), ref_addr: '0x140ed0020', ref_conf: 'confirmed', note: 'slot semantics (normal / high speed / standing still / enter / leave) from the property names; 0x140ed0020 is the asset function touching these members (verdict same), caller chain not read' });
    }
  }
  // footstep set
  const pw = pkg(P + 'p_ex001_Pawn'); const mesh = pw.exports.find((e) => e.objectName === 'TresCharMesh0'); const mp = L.exportProps(pw, mesh);
  const fsets = new Set(); for (const u of (mp.m_CheckFootSteps || { items: [] }).items) {
    add(rows, { id: 'footstep.check.' + u.m_BoneName, category: 'locomotion', trigger_kind: 'component_table', trigger: `TresCharMesh0.m_CheckFootSteps {m_BoneName ${u.m_BoneName}}`, effect: null, attach: { mode: 'foot bone contact test', bone: u.m_BoneName, offset: u.m_OffsetLocation, rotation: u.m_OffsetRotation, scale: u.m_EffectScale }, params: { subcategory: 'footstep contact probe (drives footprints / surface effects)', shape: u.m_ShapeType, size: u.m_Size, check_dist: u.m_CheckDist, touched_check_dist: u.m_TouchedCheckDist, foot_step_set: u.m_FootStepSet, foot_step_size: u.m_FootStepSize, stamp_dir: u.m_FootStampDir, stamp_check_dist: u.m_StampCheckDist, kk_scope: 'kk' }, source: cooked(P + 'p_ex001_Pawn'), evidence: `[A] ${cooked(P + 'p_ex001_Pawn')}: TresCharMesh0 (TresSkeletalMeshComponent) m_CheckFootSteps[] (TresSkeletalFootStepUnit)`, ref_addr: '-', ref_conf: 'confirmed', note: 'no particle system in this struct: it references the TresFootStepSet (decal materials); dust comes from the physical-material table rows physmat.*' });
    if (u.m_FootStepSet) fsets.add(u.m_FootStepSet); }
  for (const s of fsets) { const sd = pkg(s), sp = L.exportProps(sd, sd.exports[0]); for (const [k, v] of Object.entries(sp)) add(rows, { id: 'footstep.set.' + k.replace('m_', ''), category: 'locomotion', trigger_kind: 'data_table', trigger: 'TresFootStepSet.' + k, effect: null, attach: { mode: 'decal at the foot contact' }, params: { subcategory: 'footprint decal / trail material (not a particle system)', material: v, kk_scope: 'kk' }, source: cooked(s), evidence: `[A] ${cooked(s)}: ${s.split('.').pop()} (TresFootStepSet).${k} = ${v}`, ref_addr: '-', ref_conf: 'confirmed', note: 'material, listed for completeness (wet / mud / snow footprints)' }); }
}

// ---------- 5. magic ----------
{
  const ms = pkg(P + 'p_ex001_MagicSet'); const units = L.exportProps(ms, ms.exports[0]).m_Units.items; const src = cooked(P + 'p_ex001_MagicSet');
  units.forEach((u, i) => { const cmd = u.m_Command.replace('TRES_CMD_KIND_', ''); const cat = /^(S_)?CUR/.test(cmd) ? 'cure' : 'magic';
    for (const slot of ['m_AssetData00', 'm_AssetData01']) if (u[slot]) emitBlueprint(rows, u[slot], `magic.${cmd}${slot === 'm_AssetData01' ? '.asset01' : ''}`, cat, { subcategory: 'spell ' + cmd, command: u.m_Command, effect_group: u.m_EffectGroup, cast_type: u.m_bIsCastType, magic_set_index: i, via: `${src} m_Units[${i}].${slot}` });
  });
  for (const [setG, label] of [[P + 'p_ex001_ProjectileSet', 'p_ex001_ProjectileSet'], [P + 'w_so010_ProjSet', 'w_so010_ProjSet']]) { const d = pkg(setG); L.exportProps(d, d.exports[0]).Projectiles.items.forEach((u, i) => { const n = u.ProjectileName;
    const cat = /^KC_/.test(n) ? 'shotlock' : /^AN_/.test(n) ? 'form_pointer' : /^UNISON|^FUSION|COVERSHOT|SLIDESHOT/.test(n) ? 'link_attraction' : /^SEA_/.test(n) ? 'locomotion' : /ATHLETIC/.test(n) ? 'locomotion' : 'melee';
    emitBlueprint(rows, u.ProjectileData, 'proj.' + n, cat, { subcategory: 'projectile ' + n + ' (' + label + ')', projectile_name: n, projectile_set: label, via: `${cooked(setG)} Projectiles[${i}]` }); }); }
}

// ---------- 6. explicit "none found" rows ----------
for (const [id, cat, what, ev] of [
  ['none.levelup', 'levelup', 'level up effect', 'no ParticleSystem / ead reference named or grouped for level up in p_ex001 blueprints, Load/Player, Load/Common, ead_com_abn0, ead_pc_aura_com0; no Effects asset with lvu/levelup in its name'],
  ['none.lowhp', 'lowhp', 'low HP effect', 'no property or data-asset entry for a low-HP particle effect on the pawn, its load asset or the common ead tables'],
  ['none.footstep_dust_default', 'locomotion', 'footstep dust on ordinary ground in the common table', 'TresPhysEffectsEX (the m_CmnPhysMatEffect) only fills surface 33 (water knee); dust for dry surfaces exists only in world tables (HE, PO, RA, BX, CA, DW)'],
]) add(rows, { id, category: cat, trigger_kind: 'unknown', trigger: what, effect: null, params: { subcategory: 'none found', kk_scope: 'undetermined' }, source: '-', evidence: '[A] ' + ev, ref_addr: '-', ref_conf: 'confirmed', note: 'none found in the mined asset data; code or UI (widget) driven effects are outside this mining' });

// ---------- output ----------
fs.writeFileSync(path.join(L.OUT, 'rows_effects.json'), '[\n' + rows.map((r) => JSON.stringify(r)).join(',\n') + '\n]\n');
fs.writeFileSync(path.join(L.OUT, 'rows_effects_outofscope.json'), '[\n' + oos.map((r) => JSON.stringify(r)).join(',\n') + '\n]\n');
fs.writeFileSync(path.join(L.OUT, 'rows_columns.json'), JSON.stringify(COLS));
// effect list
const EI = COLS.indexOf('effect'), PI = COLS.indexOf('params'), CI = COLS.indexOf('category');
const POINTER = new Set(['form_pointer', 'link_attraction', 'out_of_scope']);
const eff = new Map();
function note(r, scope) { for (const e of [r[EI], r[PI] && r[PI].authored_effect]) { if (!e) continue; const key = e.split('.')[0].toLowerCase(); if (!eff.has(key)) eff.set(key, { effect: e, used_by: [], cats: new Set(), core: false }); const x = eff.get(key); x.used_by.push(r[0]); x.cats.add(r[CI]); if (r[PI].common_table === false) { x.world = true; continue; } if (e === r[EI] && !POINTER.has(r[CI]) && (r[PI].kk_scope === 'kk' || (r[CI] === 'formchange' && r[PI].kk_scope === 'second_form'))) x.core = true; } }
for (const r of rows) note(r);
const allFiles = new Map(); // case-insensitive lookup of package files
function findPkg(g) { const f = L.gameToPkgFile(g); if (fs.existsSync(f)) { const dir = path.dirname(f); if (!allFiles.has(dir)) allFiles.set(dir, fs.readdirSync(dir)); const real = allFiles.get(dir).find((n) => n.toLowerCase() === path.basename(f).toLowerCase()); return path.join(dir, real || path.basename(f)); } return null; }
const list = [...eff.values()].map((x) => { const f = findPkg(x.effect); let cls = null; if (f) { try { const d = L.loadPkg(f); const main = d.exports.find((e) => !e.outerIndex) || d.exports[0]; cls = L.exportClass(d, main); } catch (e) { cls = 'unreadable'; } }
  return { effect: x.effect.split('.')[0] + '.' + x.effect.split('.')[0].split('/').pop(), referenced_as: x.effect, package: f ? path.relative(L.ROOT, f).replace(/\\/g, '/') : null, exists: !!f, class: cls, priority: x.core ? 'core' : x.world ? 'world' : 'pointer', categories: [...x.cats].sort(), used_by: x.used_by }; }).sort((a, b) => (a.priority === b.priority ? a.effect.localeCompare(b.effect, 'en') : a.priority === 'core' ? -1 : 1));
fs.writeFileSync(path.join(L.OUT, 'effect_list.json'), '[\n' + list.map((r) => JSON.stringify(r)).join(',\n') + '\n]\n');
// summary
const cnt = (i) => { const o = {}; for (const r of rows) o[r[i]] = (o[r[i]] || 0) + 1; return o; };
const summary = { rows: rows.length, out_of_scope_rows: oos.length, by_category: cnt(CI), by_trigger_kind: cnt(COLS.indexOf('trigger_kind')), by_conf: cnt(COLS.indexOf('ref_conf')), null_effect_rows: rows.filter((r) => !r[EI]).length, distinct_effects: list.length, core_effects: list.filter((l) => l.priority === 'core').length, world_effects: list.filter((l) => l.priority === 'world').length, missing_packages: list.filter((l) => !l.exists).map((l) => l.effect), classes: list.reduce((o, l) => { o[l.class] = (o[l.class] || 0) + 1; return o; }, {}), notifies: stats };
fs.writeFileSync(path.join(L.OUT, 'summary.json'), JSON.stringify(summary, null, 1));
console.log(JSON.stringify(summary, null, 1));
