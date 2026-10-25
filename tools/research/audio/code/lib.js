// Shared helpers for the audio/code strand. Read-only over assets_converted/ and sheets/.
const fs=require('fs'),path=require('path');
const R=path.resolve(__dirname,'../../../..');
const PK=path.join(R,'assets_converted/packages/KINGDOM HEARTS III/Content');
const AU=path.join(R,'assets_converted/audio/KINGDOM HEARTS III/Content');
const cache=new Map();
// game path "/Game/X/Y" or "X/Y" -> package json (or null)
function pkgFile(gp){gp=gp.replace(/^\/Game\//,'').replace(/\.[^./]+$/,m=>/^\.(uasset|umap)$/.test(m)?'':m);for(const e of ['.uasset.json','.umap.json']){const f=path.join(PK,gp+e);if(fs.existsSync(f))return f;}return null;}
function loadPkg(gp){if(cache.has(gp))return cache.get(gp);const f=pkgFile(gp);let j=null;if(f){try{j=JSON.parse(fs.readFileSync(f,'utf8'));}catch(e){j=null;}}if(cache.size>400)cache.clear();cache.set(gp,j);return j;}
// object index -> readable ref
function ref(pkg,i){if(i===0||i==null)return null;if(typeof i==='object'&&i!==null&&'object' in i)i=i.object;
 if(i>0){const e=pkg.exports[i-1];return e?('export:'+e.objectName+'('+e.class+')'):('export#'+i);}
 const im=pkg.imports[-i-1];if(!im)return 'import#'+i;let o=im,parts=[im.objectName];let guard=0;while(o.outerIndex<0&&guard++<8){o=pkg.imports[-o.outerIndex-1];parts.unshift(o.objectName);}
 return parts[0]+(parts.length>1?'.'+parts.slice(1).join('.'):'')+(im.className&&im.className!=='Package'?' ('+im.className+')':'');}
function refPath(pkg,i){const r=ref(pkg,i);if(!r)return null;return r.replace(/ \(.*$/,'').replace(/\..*$/,'');}
// simplify a decoded property tree, resolving object refs
function isPropList(v){return Array.isArray(v)&&v.length>0&&v.every(x=>x&&typeof x==='object'&&'name' in x&&'type' in x&&'value' in x);}
function simp(pkg,v,type){if(v===null||v===undefined)return v;if(isPropList(v))return props(pkg,v);if(Array.isArray(v))return v.map(x=>simp(pkg,x,type));
 if(typeof v==='object'&&'items' in v&&Array.isArray(v.items)&&('structName' in v||Object.keys(v).length<=2))return v.items.map(x=>simp(pkg,x,type));
 if(typeof v==='object'){if('props' in v&&Array.isArray(v.props))return props(pkg,v.props);if('name' in v&&'type' in v&&'value' in v)return {[v.name]:simp(pkg,v.value,v.type)};
  if('object' in v&&Object.keys(v).length===1)return ref(pkg,v.object);if('s' in v&&'enc' in v)return v.s;const o={};for(const k in v)o[k]=simp(pkg,v[k]);return o;}
 if(type==='ObjectProperty'&&typeof v==='number')return ref(pkg,v);return v;}
function props(pkg,ps){const o={};for(const p of ps){let k=p.name||(p.tag&&p.tag.name);const t=p.type||(p.tag&&p.tag.type);const ai=p.arrayIndex||(p.tag&&p.tag.arrayIndex)||0;if(ai)k+='['+ai+']';
  let val=p.value;if((t==='ObjectProperty')&&typeof val==='number')val=ref(pkg,val);else if(t==='ArrayProperty'&&Array.isArray(val)&&p.innerType==='ObjectProperty')val=val.map(x=>typeof x==='number'?ref(pkg,x):simp(pkg,x));else val=simp(pkg,val,p.innerType||t);o[k]=val;}return o;}
function exportProps(pkg,e){const d=e.data||{};return d.props?props(pkg,d.props):null;}
// SEAD sound asset info from the sidecar + package imports
function soundInfo(gp){if(!gp)return null;gp=gp.replace(/^\/Game\//,'').replace(/ \(.*$/,'').replace(/\..*$/,'');const sf=path.join(AU,gp+'.sead.json');const out={asset:'/Game/'+gp,exists:fs.existsSync(sf)};if(!out.exists)return out;
 const s=JSON.parse(fs.readFileSync(sf,'utf8'));const pkg=loadPkg(gp);out.class=s.class;out.embedded_bank=!!s.bank;if(s.bank){for(const e of ['.sab','.mab'])if(fs.existsSync(path.join(AU,gp+e)))out.bank_file='assets_converted/audio/KINGDOM HEARTS III/Content/'+gp+e;}
 for(const p of s.props){const n=p.tag.name;let v=p.value;if(v&&typeof v==='object'&&'object' in v)v=pkg?refPath(pkg,v.object):('#'+v.object);
  if(n==='ReferenceBank'){out.reference_bank=v;const b=String(v).replace(/^\/Game\//,'');for(const e of ['.sab','.mab'])if(fs.existsSync(path.join(AU,b+e)))out.bank_file='assets_converted/audio/KINGDOM HEARTS III/Content/'+b+e;}
  else if(n==='SoundIndex')out.sound_index=v;else if(n==='Volume')out.volume=v;else if(n==='Duration')out.duration=v;else if(n==='SoundClassObject')out.sound_class=v;else if(n==='AttenuationSettings')out.attenuation=v;else out[n]=v;}
 if(out.reference_bank&&out.sound_index===undefined)out.sound_index=0; // property omitted = class default 0 (11,435 of 11,504 carry it)
 return out;}
module.exports={R,PK,AU,loadPkg,pkgFile,ref,refPath,simp,props,exportProps,soundInfo};
