// Dumps a TresVoiceGroupSet (default: Sora) with every voice resolved. Writes voice_groups_<name>.json
const L=require('./lib');const fs=require('fs'),path=require('path');
const gp=process.argv[2]||'Blueprints/Player/p_ex001/p_ex001_VoiceGroupSet';
const pkg=L.loadPkg(gp);const e=pkg.exports[0];const raw=e.data.props.find(p=>p.name==='m_VoiceGroups');
const groups=[];const p=L.exportProps(pkg,e);
for(const g of p.m_VoiceGroups){const o={group:g.m_GroupName,play_on_group_not_found:g.m_bPlayOnGroupNotFound===undefined?null:g.m_bPlayOnGroupNotFound,default_voice:g.m_pDefaultVoice||null,table:[]};
 for(const u of (g.m_Table||[])){const a=(u.m_pVoice||'').replace(/\..*$/,'');const s=a?L.soundInfo(a):null;o.table.push({group_no:u.m_GroupNo===undefined?0:u.m_GroupNo,asset:a||null,bank:s&&s.reference_bank,sound_index:s&&s.sound_index,volume:s&&s.volume,duration:s&&s.duration,sound_class:s&&s.sound_class,attenuation:s&&s.attenuation,exists:s&&s.exists});}
 groups.push(o);}
const name=gp.split('/').pop();fs.writeFileSync(path.join(__dirname,'voice_groups_'+name+'.json'),JSON.stringify(groups,null,1));
for(const g of groups){console.log('## '+g.group+' pogn='+g.play_on_group_not_found+' default='+(g.default_voice||'-').replace(/^.*\//,''));console.log('   '+g.table.map(u=>u.group_no+':'+(u.asset||'-').replace(/^.*\/(battle|Localization)\//,'')+'#'+u.sound_index).join('  '));}
