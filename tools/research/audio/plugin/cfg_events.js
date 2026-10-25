// node cfg_events.js -> sead_config_events.json : categories, layers, events (decoded commands), external params, macros (names) from ../bank/sead_config.bin
const fs=require('fs'),path=require('path');const d=fs.readFileSync(path.join(__dirname,'../bank/sead_config.bin'));
const n=d[8];const secs={};const order=[];for(let i=0;i<n;i++){const o=16+i*16;const tag=d.toString('latin1',o,o+4).trim();secs[tag]=d.readUInt32LE(o+8);order.push([tag,d.readUInt32LE(o+8)]);}
order.sort((a,b)=>a[1]-b[1]);const endOf=t=>{const i=order.findIndex(x=>x[0]===t);return i+1<order.length?order[i+1][1]:d.length;};
const ents=t=>{const b=secs[t];const cnt=d.readUInt16LE(b+4),hs=d.readUInt16LE(b+2);const r=[];for(let i=0;i<cnt;i++){const o=b+d.readUInt32LE(b+hs+i*4);const e=i+1<cnt?b+d.readUInt32LE(b+hs+(i+1)*4):endOf(t);r.push([o,e]);}return r;};
const cstr=o=>{let s='';while(d[o])s+=String.fromCharCode(d[o++]);return s;};
const out={};
out.categories=ents('ctgr').map(([o,e])=>({number:d[o+1],ver:d[o],s16_4:d.readInt16LE(o+4),nameLen:d.readUInt16LE(o+6),raw8:d.toString('hex',o+8,o+16),name:cstr(o+16)}));
out.layers=ents('ctgl').map(([o,e])=>({number:d[o+1],u32_4:d.readUInt32LE(o+4),name:cstr(o+16)}));
const catName=k=>{const c=out.categories.find(c=>c.number===k);return c?c.name:'#'+k;};
out.events=ents('evt').map(([o,e])=>{const nameLen=d.readUInt32LE(o+4),size=d.readUInt32LE(o+8);const name=cstr(o+16);let p=o+16+Math.ceil((nameLen+1)/16)*16;if(d.readUInt32LE(o+4)%16===0&&false)p=o+16+nameLen;const cmds=[];
 // commands start after name padded: find by scanning for first plausible cmd header (byte1==8)
 let q=o+16+nameLen;q=o+16+Math.ceil((nameLen+1)/8)*8;while(q<e&&d[q+1]!==8)q+=4;
 while(q+8<=e){const flag=d[q],hs=d[q+1],cmd=d[q+2],cls=d[q+3],ps=d.readUInt32LE(q+4);const pl=q+hs;let c={flag,cmd,cls,size:ps,raw:d.toString('hex',pl,pl+ps)};
  if(cls===2){if(cmd===0&&ps===12)c={op:'CategorySetVolume',volume:d.readFloatLE(pl),fade:d.readFloatLE(pl+4),category:catName(d.readUInt16LE(pl+8)),layer:d.readUInt16LE(pl+10),flag};
   else if(cmd===1&&ps===12)c={op:'CategorySetPitch',pitch:d.readFloatLE(pl),fade:d.readFloatLE(pl+4),category:catName(d.readUInt16LE(pl+8)),w:d.readUInt16LE(pl+10),flag};
   else if(ps===8)c={op:['?','?','CategoryPause','CategoryResume','CategoryStop'][cmd]||('Category#'+cmd),fade:d.readFloatLE(pl),category:catName(d.readUInt16LE(pl+4)),w:d.readUInt16LE(pl+6),flag};}
  else if(cls===3&&ps===8)c={op:'EffectPreset#'+cmd,preset:d.readUInt32LE(pl),fade:d.readFloatLE(pl+4),flag};
  else if(cls===0&&cmd===1){cmds.push({op:'End'});break;}
  cmds.push(c);q=pl+ps;}
 return {number:d[o+1],name,cmds};});
out.externalParams=ents('expr').map(([o,e])=>({number:d[o+1],raw:d.toString('hex',o,o+32),name:cstr(o+32)}));
out.macros=ents('mcr').map(([o,e])=>({number:d[o+1],len:e-o,head:d.toString('hex',o,o+16),name:cstr(o+16)}));
out.effectPresets=ents('eps').map(([o,e])=>({number:d[o+1],len:e-o,name:cstr(o+16)}));
out.buses=ents('bus').map(([o,e])=>({number:d[o+1],len:e-o,raw:d.toString('hex',o,Math.min(e,o+48)),name:cstr(o+16)}));
out.zeroone=ents('zeon').map(([o,e])=>({raw:d.toString('hex',o,e)}));
fs.writeFileSync(path.join(__dirname,'sead_config_events.json'),JSON.stringify(out,null,1));
console.log('categories:',out.categories.filter(c=>!/^dummy/.test(c.name)).map(c=>c.number+':'+c.name+'('+c.s16_4+')').join(' '));
console.log('layers:',out.layers.map(c=>c.number+':'+c.name+'('+c.u32_4+')').join(' '));
for(const ev of out.events){const g={};for(const c of ev.cmds){if(c.op==='End')continue;const k=c.op+(c.volume!==undefined?' v='+(+c.volume.toFixed(3))+' L'+c.layer:'')+(c.pitch!==undefined?' p='+(+c.pitch.toFixed(3)):'')+(c.preset!==undefined?' preset='+c.preset:'')+(c.fade!==undefined?' fade='+(+c.fade.toFixed(3)):'')+(c.raw?' raw '+c.cls+'/'+c.cmd+' '+c.raw:'');(g[k]=g[k]||[]).push(c.category||'');}
 console.log('EVENT',ev.number,ev.name);for(const k of Object.keys(g))console.log('   ',k,':',g[k].join(','));}
console.log('expr:',out.externalParams.map(c=>c.number+':'+c.name).join(' '));
console.log('eps:',out.effectPresets.map(c=>c.number+':'+c.name).join(' '));
console.log('bus:',out.buses.map(c=>c.number+':'+c.name).join(' '));
console.log('macros:',out.macros.length,out.macros.slice(0,12).map(c=>c.number+':'+c.name+'/'+c.len).join(' '));
