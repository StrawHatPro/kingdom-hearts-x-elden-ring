// usage: node scan_notifies.js [animset rel ...] > writes notify_rows.json (+ sound_refs.json intermediate)
// One row per sound/voice-related notify occurrence in every animation of the AnimSet(s).
const L=require('./lib.js');
const sets=process.argv.slice(2).filter(a=>!a.startsWith('--')); if(!sets.length) sets.push('Character/pc/p_ex001/anm/p_ex001_AnimSet');
const outName=(process.argv.find(a=>a.startsWith('--out='))||'--out=notify_rows.raw.json').slice(6);
const SOUND_CLASS=/^(SQEXSEAD|AnimNotify_PlaySound)|Voice|PlaySE|SwingSE|HitSE|FootStep/;
const ALSO=/^(TresAnimNotifyState_Attack|TresAnimNotifyState_BpEvent|TresAnimNotify_BpEvent|AnimNotify_.*_C)$/; // carry sound-relevant props or BP hooks
const rows=[],counts={},bp=[];
const seqs=new Map();
function addSeq(rel,info){ if(!seqs.has(rel))seqs.set(rel,[]); seqs.get(rel).push(info); }
for(const setRel of sets){ const d=L.loadPkg(setRel); const e=d.exports.find(e=>e.class==='TresAnimSet'); const anims=L.simp(d,e.data.props).Anims; const setName=setRel.split('/').pop();
  for(const r of anims){ if(!r.AnimData)continue; const rel=L.gamePathToRel(r.AnimData); const p=L.loadPkg(rel); if(!p){console.error('missing',rel);continue;}
    const main=p.exports.find(x=>x.objectName===rel.split('/').pop())||p.exports[0];
    if(main.class==='AnimSequence') addSeq(rel,{set:setName,name:r.AnimName,via:null});
    else { const pr=L.simp(p,main.data.props||[]); const samples=(pr.SampleData||[]).map(s=>s.Animation).filter(Boolean);
      const seen=new Set(); for(const im of p.imports) if(im.className==='AnimSequence'&&!/^Default__/.test(im.objectName)){ const pk=p.imports[-im.outerIndex-1]; const srel=L.gamePathToRel(pk.objectName+'.x'); if(seen.has(srel))continue; seen.add(srel); addSeq(srel,{set:setName,name:r.AnimName,via:main.class+' '+r.AnimData}); }
    } } }
for(const [rel,infos] of seqs){ const sf=L.AN+rel+'.anim.json'; const p=L.loadPkg(rel); if(!L.fs.existsSync(sf)||!p){console.error('missing sidecar/pkg',rel);continue;}
  const j=JSON.parse(L.fs.readFileSync(sf,'utf8')); const P={}; for(const q of j.data.props)P[q.tag.name]=L.ssimp(q.value);
  const len=P.SequenceLength, nf=P.NumFrames, rate=P.RateScale===undefined?1:P.RateScale;
  for(const n of (P.Notifies||[])){ const isState=!!(n.NotifyStateClass&&n.NotifyStateClass.object); const idx=isState?n.NotifyStateClass.object:(n.Notify&&n.Notify.object);
    if(!idx){ counts['(named notify) '+n.NotifyName]=(counts['(named notify) '+n.NotifyName]||0)+infos.length; continue; }
    const ex=L.exportProps(p,idx); if(!SOUND_CLASS.test(ex.class)&&!ALSO.test(ex.class)) continue;
    const props=ex.props;
    if(ex.class==='TresAnimNotifyState_Attack'){ if(!('m_HitSEAsset' in props)&&!('m_WeaponHitSEAliasID' in props)) { counts['TresAnimNotifyState_Attack (no SE props set)']=(counts['TresAnimNotifyState_Attack (no SE props set)']||0)+1; continue; } }
    const t=n.LinkValue; const fps=(nf-1)/len;
    const row={ anim_asset:'/Game/'+rel, sequence_length:len, num_frames:nf, rate_scale:rate,
      logical:infos.map(i=>i.set==='p_ex001_AnimSet'&&!i.via?i.name:(i.set==='p_ex001_AnimSet'?'':i.set+':')+i.name+(i.via?' (sample of '+i.via+')':'')),
      notify_class:ex.class, notify_object:ex.name, notify_name:n.NotifyName, is_state:isState,
      time_s:t, frame:Math.round(t*fps*1000)/1000, duration_s:isState?n.Duration:null, end_s:isState?(n.EndLink?n.EndLink.LinkValue:t+n.Duration):null, duration_frames:isState?Math.round(n.Duration*fps*1000)/1000:null,
      trigger_time_offset:n.TriggerTimeOffset, end_trigger_time_offset:isState?n.EndTriggerTimeOffset:null, trigger_chance:n.NotifyTriggerChance, trigger_weight_threshold:n.TriggerWeightThreshold, filter_type:n.NotifyFilterType, filter_lod:n.NotifyFilterLOD, montage_tick_type:n.MontageTickType, track_index:n.TrackIndex,
      props };
    rows.push(row); counts[ex.class]=(counts[ex.class]||0)+1; } }
rows.sort((a,b)=>a.anim_asset<b.anim_asset?-1:a.anim_asset>b.anim_asset?1:a.time_s-b.time_s);
L.fs.writeFileSync(__dirname+'/'+outName,JSON.stringify(rows,null,1));
console.log('sequences',seqs.size,'rows',rows.length); console.log(Object.entries(counts).sort((a,b)=>b[1]-a[1]).map(x=>x[1]+'\t'+x[0]).join('\n'));
