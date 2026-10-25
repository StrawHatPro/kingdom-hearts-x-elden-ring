// Magic set -> projectile blueprints (closure through /Game/Blueprints/Projectile and parents) -> sound references. Writes magic_proj_sounds.json
const L=require('./lib');const S=require('./sndrefs');const fs=require('fs'),path=require('path');
const sets=(process.argv[2]||'Blueprints/Player/p_ex001/p_ex001_MagicSet').split(',');
const out={units:[],packages:{}};
function closure(gp,depth,seen){gp=gp.replace(/^\/Game\//,'');if(seen.has(gp)||depth>4)return;seen.add(gp);const pkg=L.loadPkg(gp);if(!pkg)return;
 const snd=S.refs(gp)||[];const parent=(pkg.imports.filter(i=>i.className==='BlueprintGeneratedClass').map(i=>L.ref(pkg,-(pkg.imports.indexOf(i)+1))).map(s=>s.replace(/\..*$/,'')));
 out.packages[gp]={sounds:snd.map(x=>{const s=L.soundInfo(x.sound)||{};return {export:x.export,cls:x.cls,path:x.path,sound:x.sound,bank:s.reference_bank||(s.embedded_bank?'(embedded)':null),bank_file:s.bank_file||null,sound_index:s.sound_index??null,volume:s.volume??null,duration:s.duration??null,attenuation:s.attenuation||null};}),refs:[]};
 const next=new Set(parent);for(const r of S.objrefs(gp,/^\/Game\/Blueprints\/(Projectile|Player|Effect)\//))next.add(r);
 for(const n of next){if(/_(VoiceGroupSet|DataTableSet)$/.test(n))continue;out.packages[gp].refs.push(n);closure(n,depth+1,seen);}}
const seen=new Set();
for(const set of sets){const pkg=L.loadPkg(set);const p=L.exportProps(pkg,pkg.exports[0]);for(const u of (p.m_Units||[])){const bp=[u.m_AssetData00,u.m_AssetData01].filter(Boolean).map(x=>String(x).replace(/\..*$/,''));out.units.push({set,command:u.m_Command,cast:!!u.m_bIsCastType,turn:!!u.m_bIsTurnType,direct_pos:!!u.m_bIsDirectPos,effect_group:u.m_EffectGroup,voice_group:u.m_VoiceGroup,voice_group_finish:u.m_VoiceGroupFinish,projectiles:bp});bp.forEach(b=>closure(b,0,seen));}
 for(const pr of (p.Projectiles||[])){const b=String(pr.ProjectileData||'').replace(/\..*$/,'');out.units.push({set,projectile_name:pr.ProjectileName,projectiles:[b]});if(b)closure(b,0,seen);}}
fs.writeFileSync(path.join(__dirname,'magic_proj_sounds_'+sets[0].split('/').pop()+'.json'),JSON.stringify(out,null,1));
let n=0;for(const k of Object.keys(out.packages)){const s=out.packages[k].sounds;if(!s.length)continue;n+=s.length;console.log('## '+k.replace('Blueprints/Projectile/','')+'  <- refs '+out.packages[k].refs.map(r=>r.split('/').pop()).join(','));for(const x of s)console.log('   '+x.export.replace(/_GEN_VARIABLE$/,'')+'.'+x.path+' = '+x.sound.replace('/Game/Sound/','')+' #'+x.sound_index);}
console.log(Object.keys(out.packages).length,'packages',n,'sound refs');
