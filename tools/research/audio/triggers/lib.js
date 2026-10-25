// shared helpers for the triggers strand (read-only over assets_converted)
const fs=require('fs'),path=require('path');
const R=path.resolve(__dirname,'../../../..');
const PK=R+'/assets_converted/packages/KINGDOM HEARTS III/Content/';
const AN=R+'/assets_converted/animations/KINGDOM HEARTS III/Content/';
const AU=R+'/assets_converted/audio/KINGDOM HEARTS III/Content/';
const cache=new Map();
function gamePathToRel(p){ // /Game/a/b.b -> a/b
  let s=p.replace(/^\/Game\//,''); const i=s.lastIndexOf('.'); if(i>s.lastIndexOf('/')) s=s.slice(0,i); return s; }
function loadPkg(rel){ if(cache.has(rel))return cache.get(rel); const f=PK+rel+'.uasset.json'; let d=null; if(fs.existsSync(f)) d=JSON.parse(fs.readFileSync(f,'utf8')); if(cache.size>40)cache.clear(); cache.set(rel,d); return d; }
function full(d,i){ if(!i) return null; if(i<0){let im=d.imports[-i-1],s=im.objectName;while(im.outerIndex){im=d.imports[-im.outerIndex-1];s=im.objectName+'.'+s;}return s;} return 'E'+i+':'+d.exports[i-1].objectName; }
function impClass(d,i){ return i<0?d.imports[-i-1].className:d.exports[i-1].class; }
// simplify tagged properties of the package JSON form ({name,type,value})
function simp(d,p){ if(p===null||typeof p!=='object') return p; if(Array.isArray(p)){ if(p.length&&p.every(e=>e&&typeof e==='object'&&'name' in e&&'type' in e)){const o={};for(const e of p){o[e.name+(e.arrayIndex?'['+e.arrayIndex+']':'')]=sv(d,e);}return o;} return p.map(x=>simp(d,x)); }
  if('name' in p&&'type' in p) return sv(d,p);
  if('items' in p&&Object.keys(p).length<=3) return p.items.map(x=>simp(d,x));
  const o={}; for(const k in p)o[k]=simp(d,p[k]); return o; }
function sv(d,e){ if(e.type==='ObjectProperty') return full(d,e.value); if(e.type==='ArrayProperty'&&e.innerType==='ObjectProperty'&&Array.isArray(e.value)) return e.value.map(x=>full(d,x)); return simp(d,e.value); }
// sidecar form ({tag:{name,type},value})
function ssimp(v){ if(v===null||typeof v!=='object')return v; if(Array.isArray(v))return v.map(ssimp); if(v.props){const r={};for(const p of v.props)r[p.tag.name]=ssimp(p.value);return r;} if(v.items)return v.items.map(ssimp); return v; }
function exportProps(d,idx){ const e=d.exports[idx-1]; return {name:e.objectName,class:e.class,props:simp(d,(e.data&&e.data.props)||[])}; }
function sead(gamePath){ const rel=gamePathToRel(gamePath); const f=AU+rel+'.sead.json'; if(!fs.existsSync(f)) return {rel,exists:false}; return {rel,exists:true,json:JSON.parse(fs.readFileSync(f,'utf8')),hasSab:fs.existsSync(AU+rel+'.sab'),hasMab:fs.existsSync(AU+rel+'.mab')}; }
module.exports={R,PK,AN,AU,gamePathToRel,loadPkg,full,impClass,simp,ssimp,exportProps,sead,fs,path};
// ---- sound resolution -------------------------------------------------------------------
// resolveSound('/Game/Sound/.../x.x') -> {asset, class, exists, own_bank_file, reference_bank, bank_file, sound_index, props...}
const sndCache=new Map();
function resolveSound(gamePath){ if(!gamePath) return null; if(sndCache.has(gamePath)) return sndCache.get(gamePath);
  const rel=gamePathToRel(gamePath); const out={asset:'/Game/'+rel,exists:false}; sndCache.set(gamePath,out);
  const sf=AU+rel+'.sead.json'; if(!fs.existsSync(sf)){ out.note=fs.existsSync(PK+rel+'.uasset.json')?'package exists but no SEAD sidecar':'no such package'; if(fs.existsSync(PK+rel+'.uasset.json')){const d=JSON.parse(fs.readFileSync(PK+rel+'.uasset.json','utf8')); out.class=(d.exports[0]||{}).class;} return out; }
  const j=JSON.parse(fs.readFileSync(sf,'utf8')); out.exists=true; out.class=j.class;
  let d=null; const pf=PK+rel+'.uasset.json'; if(fs.existsSync(pf)) d=JSON.parse(fs.readFileSync(pf,'utf8'));
  const props={}; for(const p of j.props){ let v=p.value; if(p.tag.type==='ObjectProperty'){ v=d?full(d,v.object):v; } else v=ssimp(v); props[p.tag.name]=v; }
  out.props=props;
  out.own_bank=j.bank?{file:'assets_converted/audio/KINGDOM HEARTS III/Content/'+rel+(fs.existsSync(AU+rel+'.mab')?'.mab':'.sab'),size:j.bank.size,record_version:j.bank.version}:null;
  out.reference_bank=props.ReferenceBank||null;
  if(out.reference_bank){ const brel=gamePathToRel(out.reference_bank); const bf=AU+brel+'.sab'; out.bank_file='assets_converted/audio/KINGDOM HEARTS III/Content/'+brel+'.sab'; out.bank_file_exists=fs.existsSync(bf); out.sound_index=('SoundIndex' in props)?props.SoundIndex:0; out.sound_index_serialized=('SoundIndex' in props); }
  else if(out.own_bank){ out.bank_file=out.own_bank.file; out.bank_file_exists=true; out.sound_index=('SoundIndex' in props)?props.SoundIndex:0; out.sound_index_serialized=('SoundIndex' in props); }
  out.volume=('Volume' in props)?props.Volume:null; out.duration=props.Duration; out.attenuation=props.AttenuationSettings||null; out.sound_class=props.SoundClassObject||null;
  return out; }
function enVariant(gamePath){ return gamePath.replace('/Localization/ja/','/Localization/en/').replace('/localization/rt/body/ja/','/localization/rt/body/en/'); }
module.exports.resolveSound=resolveSound; module.exports.enVariant=enVariant;
