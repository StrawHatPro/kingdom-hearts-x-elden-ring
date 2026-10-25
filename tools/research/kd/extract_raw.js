// flatten p_ex001_0_kdi tagged properties to plain JSON (raw, asset order)
const fs=require('fs');
const f=process.argv[2]||"assets_converted/packages/KINGDOM HEARTS III/Content/Character/pc/p_ex001/mdl/p_ex001_0_kdi.uasset.json";
const d=JSON.parse(fs.readFileSync(f,'utf8'));
const fbuf=Buffer.alloc(4);
function num(x){ if(x&&typeof x==='object'&&'f32bits' in x){fbuf.writeUInt32LE(x.f32bits>>>0,0);return fbuf.readFloatLE(0);} return x;}
function val(p){
  if(p.type==='StructProperty'){const v=p.value; if(Array.isArray(v)) return props(v); const o={}; for(const k in v)o[k]=num(v[k]); return o;}
  if(p.type==='ArrayProperty'){ const v=p.value; if(Array.isArray(v)) return v.map(x=> (x&&typeof x==='object'&&!('f32bits' in x))? (Array.isArray(x)?props(x):Object.fromEntries(Object.entries(x).map(([k,y])=>[k,num(y)]))) : num(x));
     if(v&&v.items) return v.items.map(it=>Array.isArray(it)?props(it):Object.fromEntries(Object.entries(it).map(([k,y])=>[k,num(y)]))); return v; }
  if(p.type==='ByteProperty'&&typeof p.value==='string') return p.value.replace(/^ESQEX_KD_[A-Za-z]+Type_/,'');
  return num(p.value);
}
function props(a){const o={}; for(const p of a) o[p.name]=val(p); return o;}
const raw=props(d.exports[0].data.props);
module.exports=raw;
if(require.main===module){ fs.writeFileSync('build/sora_tmp/kd/kdi_raw.json',JSON.stringify(raw,null,1));
 for(const k in raw) console.log(k, Array.isArray(raw[k])?raw[k].length:raw[k]);
 const c={}; raw.Operators.forEach(o=>c[o.OpType]=(c[o.OpType]||0)+1); console.log(c);}
