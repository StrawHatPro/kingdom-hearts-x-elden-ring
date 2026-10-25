// usage: node sndrefs.js <Content-relative package>...   (or require: refs(gp) -> [{export, cls, path, sound}])
// Lists every property path in a decoded package whose value references a sound asset (import under /Game/Sound or class SQEXSEAD*/Sound*).
const L=require('./lib');
function walk(v,p,out,exp){if(v==null)return;if(typeof v==='string'){if(/^\/Game\/Sound\/|\((SQEXSEADSound|SQEXSEADMusic|SoundCue|SoundWave|SQEXSEADSoundReferenceEnumSet|TresSoundAliasSet|TresVoiceGroupSet)\)$/.test(v))out.push({export:exp.objectName,cls:exp.class,path:p,sound:v.replace(/\.[^ .]+( \(.*\))?$/,m=>{const c=m.match(/\((.*)\)/);return c&&c[1]!=='SQEXSEADSound'?' ('+c[1]+')':'';})});return;}
 if(Array.isArray(v)){v.forEach((x,i)=>walk(x,p+'['+i+']',out,exp));return;}if(typeof v==='object'){for(const k in v)walk(v[k],p?p+'.'+k:k,out,exp);}}
function refs(gp){const pkg=L.loadPkg(gp);if(!pkg)return null;const out=[];for(const e of pkg.exports){const pr=L.exportProps(pkg,e);if(pr)walk(pr,'',out,e);}return out;}
function objrefs(gp,re){const pkg=L.loadPkg(gp);if(!pkg)return [];const out=new Set();for(const e of pkg.exports){const pr=L.exportProps(pkg,e);(function w(v){if(v==null)return;if(typeof v==='string'){if(re.test(v))out.add(v.replace(/\.[^ .]+( \(.*\))?$/,''));}else if(Array.isArray(v))v.forEach(w);else if(typeof v==='object')for(const k in v)w(v[k]);})(pr);}return [...out];}
module.exports={refs,objrefs};
if(require.main===module){for(const gp of process.argv.slice(2)){const r=refs(gp);console.log('## '+gp+(r?'':' (missing)'));for(const x of (r||[])){const s=L.soundInfo(x.sound);console.log('  '+x.export+' ('+x.cls+') '+x.path+' = '+x.sound+(s&&s.exists?'  [bank '+String(s.reference_bank||'embedded').split('/').pop()+' #'+s.sound_index+']':''));}}}
