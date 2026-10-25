// plain-object view of tagged properties (static arrays -> arrays by arrayIndex)
const fs=require('fs');
function isProps(p){return Array.isArray(p)&&p.length>0&&p.every(e=>e&&typeof e==='object'&&'name' in e&&'type' in e);}
function conv(p){
  if(isProps(p)){
      const o={};
      for(const q of p){const val=cv(q);
        if(q.arrayIndex!==undefined){ if(!Array.isArray(o[q.name])) o[q.name]=(q.name in o)?[o[q.name]]:[]; o[q.name][q.arrayIndex]=val; }
        else o[q.name]=val;}
      return o;}
  if(Array.isArray(p)) return p.map(conv);
  if(p&&typeof p==='object'){
    if('s' in p&&'enc' in p) return p.s;
    if('items' in p&&Array.isArray(p.items)) return p.items.map(conv);
    if('name' in p&&'type' in p) return cv(p); const o={}; for(const k in p)o[k]=conv(p[k]); return o;}
  return p;}
function cv(q){ return conv(q.value);}
module.exports={conv,load:(f)=>JSON.parse(fs.readFileSync(f,'utf8'))};
if(require.main===module){const d=module.exports.load(process.argv[2]);const o=conv(d.exports[+(process.argv[3]||0)].data.props);fs.writeFileSync(process.argv[4],JSON.stringify(o,null,1));
 for(const k in o) console.log(k, Array.isArray(o[k])?('array '+o[k].length):JSON.stringify(o[k]).slice(0,300));}
