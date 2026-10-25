// Resolves ETresStaticSE -> TresStaticSESet.m_Assets -> sound asset -> bank/index. Writes static_se.json
const L=require('./lib');const fs=require('fs'),path=require('path');
const pkg=L.loadPkg('Load/Common/TresStaticSE');const p=L.exportProps(pkg,pkg.exports[0]);
const ev={};for(const l of fs.readFileSync(path.join(L.R,'sheets/reflect/enum-values.json'),'utf8').split('\n')){if(l[0]!=='[')continue;let r;try{r=JSON.parse(l.replace(/,\s*$/,'').replace(/\]\}\s*$/,''));}catch(e){continue;}if(r[1]==='ETresStaticSE')ev[r[3]]=r[2];}
const out=[];for(let i=0;i<43;i++){const k=i?`m_Assets[${i}]`:'m_Assets';const a=(p[k]||'').replace(/\..*$/,'');const s=L.soundInfo(a)||{};
 out.push({id:i,enum:ev[i],asset:a,bank:s.reference_bank,bank_file:s.bank_file,sound_index:s.sound_index,volume:s.volume??null,duration:s.duration,sound_class:s.sound_class||null,attenuation:s.attenuation||null,bIsUISound:s.bIsUISound===undefined?undefined:true,extra:Object.fromEntries(Object.entries(s).filter(([k])=>/^b|Fade|Port|Concurr/.test(k)&&k!=='bank_file'))});}
fs.writeFileSync(path.join(__dirname,'static_se.json'),JSON.stringify(out,null,1));
for(const o of out)console.log([o.id,o.enum,o.sound_index,o.volume,o.duration&&o.duration.toFixed(2),o.sound_class&&o.sound_class.split('/').pop(),Object.keys(o.extra).join('+')].join('\t'));
