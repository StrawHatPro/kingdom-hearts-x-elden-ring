const fs=require('fs'),path=require('path');const R=path.resolve(__dirname,'../..');
const A=R+'/assets_converted/animations/KINGDOM HEARTS III/Content/Character/pc/p_ex001/anm/rt/body/';
const P=R+'/assets_converted/packages/KINGDOM HEARTS III/Content/Character/pc/p_ex001/anm/rt/body/';
const clips='0001 0002 0003 0003L 0003R 0004 0005 0006 0007 0007L 0007R 0007BL 0007BR 0011 0012 0031 0032 0033 0034 0120'.split(' ');
function simp(v){ if(v===null||typeof v!=='object')return v; if(Array.isArray(v))return v.map(simp); if(v.props){const r={};for(const p of v.props)r[p.tag.name]=simp(p.value);return r;} if(v.items)return v.items.map(simp); return v; }
function pv(p){ // package-json property list -> plain
 if(p===null||typeof p!=='object')return p; if(Array.isArray(p)){ if(p.every(e=>e&&typeof e==='object'&&'name' in e&&'type' in e)){const o={};for(const e of p)o[e.name]=pv(e.value);return o;} return p.map(pv);} 
 if(p.items)return p.items.map(pv); if('value' in p&&'type' in p)return pv(p.value); const o={};for(const k in p)o[k]=pv(p[k]);return o;}
const out={_schema:{source:'AnimSequence.Notifies (FAnimNotifyEvent) of Character/pc/p_ex001/anm/rt/body/<clip>; notify object properties from the package export; only non-default properties are stored in the asset (absent = class default, see bnmsub_notifies_physics.md)',
 length:'SequenceLength seconds', numFrames:'NumFrames', rateScale:'RateScale (absent=1)', notifies:'only events whose notify object class matches /Bonamik|KBD|KineDriver/',
 fields:{class:'notify object class',name:'export object name',notifyName:'FAnimNotifyEvent.NotifyName',start:'trigger time s (FAnimLinkableElement.LinkValue, Absolute link, segment begin 0)',duration:'FAnimNotifyEvent.Duration s (0 for non-state)',end:'EndLink.LinkValue s',track:'TrackIndex',triggerTimeOffset:'',endTriggerTimeOffset:'',isState:'NotifyStateClass set',props:'decoded tagged properties of the notify object'},
 other_notify_classes:'classes of the remaining (non-Bonamik) notify events of the clip, for reference'},clips:{}};
for(const c of clips){ const j=JSON.parse(fs.readFileSync(A+c+'.anim.json','utf8')); const d=JSON.parse(fs.readFileSync(P+c+'.uasset.json','utf8'));
 const g={}; for(const p of j.data.props) g[p.tag.name]=simp(p.value);
 const rec={length:g.SequenceLength,numFrames:g.NumFrames,rateScale:g.RateScale===undefined?1:g.RateScale,notifies:[],other_notify_classes:[]};
 for(const n of (g.Notifies||[])){ const oi=(n.NotifyStateClass&&n.NotifyStateClass.object)||(n.Notify&&n.Notify.object)||0; const e=oi>0?d.exports[oi-1]:null; const cls=e?e.class:null;
  if(!cls||!/Bonamik|KBD|KineDriver/.test(cls)){ rec.other_notify_classes.push(cls||('name:'+n.NotifyName)); continue; }
  rec.notifies.push({class:cls,name:e.objectName,notifyName:n.NotifyName,isState:!!(n.NotifyStateClass&&n.NotifyStateClass.object),start:n.LinkValue,duration:n.Duration||0,end:n.EndLink?n.EndLink.LinkValue:null,track:n.TrackIndex===undefined?0:n.TrackIndex,triggerTimeOffset:n.TriggerTimeOffset,endTriggerTimeOffset:n.EndTriggerTimeOffset,segmentBeginTime:n.SegmentBeginTime,linkMethod:n.LinkMethod,triggerWeightThreshold:n.TriggerWeightThreshold,montageTickType:n.MontageTickType,props:pv(e.data.props)}); }
 out.clips[c]=rec; }
fs.writeFileSync(R+'/build/sora_tmp/bnmsub_notifies.json',JSON.stringify(out,null,1));
for(const c in out.clips){const r=out.clips[c]; console.log(c,'len',r.length,'frames',r.numFrames,'rate',r.rateScale, r.notifies.length?'':'NONE'); for(const n of r.notifies) console.log('   ',n.name,'t',n.start,'dur',n.duration,'end',n.end,'trk',n.track,'off',n.triggerTimeOffset,n.endTriggerTimeOffset,n.linkMethod,JSON.stringify(n.props));}
