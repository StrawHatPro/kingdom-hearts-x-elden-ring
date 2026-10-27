// Effect -> SE attach data (UParticleSystem.ParticleSeAssets -> SQEX_EffectSeAttachDataAsset.SEDatas). Writes seattach.json (folders given, default pc,magic,system,common,weapon)
const L=require('./lib');const fs=require('fs'),path=require('path');
const folders=(process.argv[2]||'pc,magic,system,common,weapon').split(',');const base='Sound/SE/_Data/SeAttach';const out=[];
// (the folder order of Windows (NTFS: names compared upper-cased) on every system: Linux lists folders in byte order,
// and the order of seattach.json is the order of the triggers in the audio pack)
const up=(c)=>c>=97&&c<=122?c-32:c<128?c:((u)=>u.length===1?u.charCodeAt(0):c)(String.fromCharCode(c).toUpperCase());
const ntfs=(a,b)=>{for(let i=0;i<Math.min(a.length,b.length);i++){const x=up(a.charCodeAt(i)),y=up(b.charCodeAt(i));if(x!==y)return x-y;}return a.length-b.length||(a<b?-1:a>b?1:0);};
function walk(d,cb){for(const f of fs.readdirSync(d,{withFileTypes:true}).sort((a,b)=>ntfs(a.name,b.name))){const p=path.join(d,f.name);if(f.isDirectory())walk(p,cb);else if(f.name.endsWith('.uasset.json'))cb(p);}}
for(const fo of folders){walk(path.join(L.PK,base,fo),p=>{const gp=path.relative(L.PK,p).split(path.sep).join('/').replace(/\.uasset\.json$/,'');const pkg=L.loadPkg(gp);if(!pkg)return;
 for(const e of pkg.exports){if(e.class!=='SQEX_EffectSeAttachData')continue;const pr=L.exportProps(pkg,e)||{};const a=pr.Sound?String(pr.Sound).replace(/\.[^.]*$/,''):null;const s=a?L.soundInfo(a):{};
  out.push({folder:fo,seattach:'/Game/'+gp,effect_name:gp.split('/').pop(),entry:e.objectName,GroupID:pr.GroupID??0,bAttach:pr.bAttach??null,bPlayAtEnd:pr.bPlayAtEnd??null,bNoSeFadeAtTheEffectEnd:pr.bNoSeFadeAtTheEffectEnd??null,bEmitterAttach:pr.bEmitterAttach??null,AttachEmitterName:pr.AttachEmitterName??null,AttachHistoryRate:pr.AttachHistoryRate??null,FadeOutTime:pr.FadeOutTime??null,
   sound_asset:a,bank:s.reference_bank||(s.embedded_bank?'(embedded)':null),bank_file:s.bank_file||null,sound_index:s.sound_index??null,volume:s.volume??null,duration:s.duration??null,attenuation:s.attenuation||null,sound_class:s.sound_class||null,raw_keys:Object.keys(pr).join(',')});}});}
fs.writeFileSync(path.join(__dirname,'seattach.json'),JSON.stringify(out,null,0).replace(/\},\{/g,'},\n{'));
const c={};for(const o of out)c[o.folder]=(c[o.folder]||0)+1;console.log(out.length,'entries',JSON.stringify(c));
const keys={};for(const o of out)for(const k of o.raw_keys.split(','))keys[k]=(keys[k]||0)+1;console.log(JSON.stringify(keys));
