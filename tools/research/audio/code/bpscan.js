// usage: node bpscan.js <file list txt | Content-relative package>...  -> sound-function calls in blueprint bytecode (function, offset, callee, args)
// Writes bp_sound_calls.json when given the list file.
const L=require('./lib');const fs=require('fs'),path=require('path');
const RE=/PlaySound|SpawnSound|PlayDialogue|SpawnDialogue|TresPlaySound2D_BP|TresSpawnSound2D_BP|CreateSound2D|SetSound|FadeOut|FadeIn|^Play$|^Stop$|PlayActorVoice|TresPlayFieldVoice|StartGameOverLoopSE|BP_TresStartGameOver/;
function arg(pkg,n){if(!n||typeof n!=='object')return n;switch(n.op){case 'ObjectConst':return L.ref(pkg,n.value!==undefined?n.value:n.object);case 'FloatConst':case 'IntConst':case 'ByteConst':return n.value;case 'True':return true;case 'False':return false;case 'NoObject':return null;case 'Self':return 'self';
  case 'LocalVariable':case 'InstanceVariable':case 'LocalOutVariable':case 'DefaultVariable':return n.op[0]+':'+(typeof n.property==='number'?(L.ref(pkg,n.property)||n.property):JSON.stringify(n.property));case 'NameConst':return 'Name('+(n.value&&n.value.s!==undefined?n.value.s:JSON.stringify(n.value))+')';case 'VectorConst':return 'Vec'+JSON.stringify(n.value);case 'RotationConst':return 'Rot'+JSON.stringify(n.value);
  case 'Context':case 'ContextFailSilent':return 'ctx('+arg(pkg,n.object)+').'+arg(pkg,n.expr||n.context||n.expression);default:{if(n.function!==undefined)return (n.op+':'+fname(pkg,n.function));return n.op;}}}
function fname(pkg,f){if(typeof f==='number')return String(L.ref(pkg,f)||f).replace(/ \(.*$/,'');if(f&&f.s!==undefined)return f.s;return JSON.stringify(f);}
function scan(gp){const pkg=L.loadPkg(gp);const out=[];if(!pkg)return out;
 for(const e of pkg.exports){const sc=e.data&&e.data.native&&e.data.native.script;if(!sc)continue;(function w(n){if(!n||typeof n!=='object')return;if(Array.isArray(n)){n.forEach(w);return;}
   if(n.op&&n.function!==undefined){const fn=fname(pkg,n.function);const short=fn.split('.').pop().split(':').pop();if(RE.test(short)&&!/^(K2_|SetSoundMix)/.test(short)){out.push({blueprint:'/Game/'+gp,function:e.objectName,at:n.at,op:n.op,callee:fn,args:(n.params||[]).filter(p=>p.op!=='EndFunctionParms').map(p=>arg(pkg,p))});}}
   for(const k in n)w(n[k]);})(sc);}
 return out;}
module.exports={scan};
if(require.main===module){let list=[];for(const a of process.argv.slice(2)){if(a.endsWith('.txt'))list.push(...fs.readFileSync(a,'utf8').split(/\r?\n/).filter(Boolean).map(l=>l.replace(/^.*Content\//,'').replace(/\.(uasset|umap)\.json$/,'')));else list.push(a);}
 const all=[];for(const gp of list){const r=scan(gp);all.push(...r);for(const x of r)console.log(gp.split('/').slice(-1)[0]+' :: '+x.function+' @'+x.at+' '+x.callee.split('/').pop()+'('+x.args.map(a=>typeof a==='string'?a.replace(/^\/Game\/Sound\//,'S:'):JSON.stringify(a)).join(', ')+')');}
 if(process.argv[2].endsWith('.txt'))fs.writeFileSync(path.join(__dirname,'bp_sound_calls.json'),JSON.stringify(all,null,1));console.log(all.length,'calls');}
