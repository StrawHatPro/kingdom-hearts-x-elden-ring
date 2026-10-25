// usage: node graphdump.js [--quiet]   -> writes build/sora_tmp/sora_animgraph.json and prints the blueprint functions as pseudocode
const fs=require('fs'),path=require('path');const R=path.resolve(__dirname,'../..');
const F=path.join(R,'assets_converted/packages/KINGDOM HEARTS III/Content/Character/pc/p_ex001/anm/p_ex001_Anim.uasset.json');
const d=JSON.parse(fs.readFileSync(F,'utf8'));
function full(i){ if(!i) return null; if(i<0){let im=d.imports[-i-1],s=im.objectName;while(im.outerIndex){im=d.imports[-im.outerIndex-1];s=im.objectName+'.'+s;}return s;} let e=d.exports[i-1],s=e.objectName; while(e.outerIndex>0){e=d.exports[e.outerIndex-1];s=e.objectName+'.'+s;} return s;}
function val(p){ // tagged property value -> plain
  if(p===null||typeof p!=='object') return p;
  if(Array.isArray(p)){ if(p.length&&p.every(e=>e&&typeof e==='object'&&('name' in e)&&('type' in e))){const o={};for(const e of p){o[e.name+(e.arrayIndex?'['+e.arrayIndex+']':'')]=prop(e);}return o;} return p.map(val);}
  if(('items' in p)&&('structName' in p)) return p.items.map(val);
  const o={};for(const k in p)o[k]=val(p[k]);return o;}
function prop(e){ if(e.type==='ObjectProperty') return full(e.value); return val(e.value); }
const cls=d.exports.find(e=>e.objectName==='p_ex001_Anim_C'); const cdo=d.exports.find(e=>/^Default__/.test(e.objectName));
const nodes=[];const other={};
for(const p of cdo.data.props){ if(p.type==='StructProperty'&&/^AnimNode_|^TresAnimNode_/.test(p.structName||'')){nodes.push(p);} else other[p.name]=prop(p); }
const N=nodes.length;
function links(o,pre,out){ if(o&&typeof o==='object'){ if(!Array.isArray(o)&&('LinkID' in o)&&Object.keys(o).length<=2){out.push({pin:pre,link_id:o.LinkID});return out;} for(const k in o) links(o[k],pre?(Array.isArray(o)?pre+'['+k+']':pre+'.'+k):k,out);} return out;}
const short=n=>n.replace(/_[0-9A-F]{32}$/,'');
const out={source:'Character/pc/p_ex001/anm/p_ex001_Anim (Default__p_ex001_Anim_C)',
 schema:'nodes[]: link_index = position in the class property order = the value other nodes use as LinkID (index into AnimNodeProperties at runtime); alloc_index = N-1-link_index = compiler allocation index (RootAnimNodeIndex / OrderedSavedPoseIndices are in this numbering); struct = node struct; links[] = {pin, link_id, to}; properties = every serialized property of the node struct in the class default object (complete; properties absent here keep the struct constructor default); bound = EvaluateGraphExposedInputs: function (blueprint function run at the node Update) and copy records {from (anim instance variable), to (node property), size, post_op, copy_type}',
 class:{}, class_default_non_node_properties:other, node_count:N, nodes:[]};
for(const p of cls.data.props) out.class[p.name]=prop(p);
out.class.superStruct=full(cls.data.native.superStruct);
nodes.forEach((p,i)=>{const v=val(p.value);const egi=v.EvaluateGraphExposedInputs;delete v.EvaluateGraphExposedInputs;
 const ls=links(v,'',[]).map(l=>({pin:l.pin,link_id:l.link_id,to:nodes[l.link_id]?short(nodes[l.link_id].name)+'#'+l.link_id:null}));
 const bound=egi?{'function':egi.BoundFunction,copy:(egi.CopyRecords||[]).map(r=>({from:r.SourcePropertyName+(r.SourceSubPropertyName!=='None'?'.'+r.SourceSubPropertyName:''),source_array_index:r.SourceArrayIndex,to:r.DestProperty,dest_array_index:r.DestArrayIndex,size:r.Size,instance_is_target:r.bInstanceIsTarget,post_op:r.PostCopyOperation,copy_type:r.CopyType}))}:null;
 out.nodes.push({link_index:i,alloc_index:N-1-i,name:p.name,struct:p.structName,links:ls,properties:v,bound:bound});});
fs.writeFileSync(path.join(__dirname,'sora_animgraph.json'),JSON.stringify(out,null,1));
if(process.argv[2]==='--quiet') process.exit(0);
// ---- kismet pretty printer
function nm(p){const s=full(p)||'null';return s.split('.').pop();}
function rest(e){return JSON.stringify(Object.fromEntries(Object.entries(e).filter(function(kv){return kv[0]!=='op'&&kv[0]!=='at';})));}
function ex(e){ if(!e) return '?'; switch(e.op){
 case 'LocalVariable':case 'LocalOutVariable':return 'L.'+nm(e.property);case 'InstanceVariable':return 'self.'+nm(e.property);case 'DefaultVariable':return 'default.'+nm(e.property);
 case 'IntConst':case 'FloatConst':case 'ByteConst':case 'IntConstByte':return String(e.value);case 'IntZero':return '0';case 'IntOne':return '1';case 'True':return 'true';case 'False':return 'false';case 'NoObject':return 'null';case 'Self':return 'self';case 'Nothing':return 'nothing';
 case 'NameConst':return 'name('+e.value+')';case 'ObjectConst':return 'obj('+full(e.object)+')';case 'StringConst':case 'UnicodeStringConst':return JSON.stringify(e.value);
 case 'VectorConst':case 'RotationConst':return e.op+rest(e);
 case 'VirtualFunction':return e['function']+'('+args(e.params)+')';case 'FinalFunction':case 'CallMath':return full(e['function']).split('.').slice(-2).join('.')+'('+args(e.params)+')';
 case 'Context':case 'Context_FailSilent':case 'ClassContext':return ex(e.object)+'->'+ex(e.context);
 case 'StructMemberContext':return ex(e.struct)+'.'+nm(e.property);case 'DynamicCast':return 'cast<'+nm(e['class'])+'>('+ex(e.value)+')';case 'MetaCast':return 'metacast<'+nm(e['class'])+'>('+ex(e.value)+')';
 case 'PrimitiveCast':return 'pcast'+e.castType+'('+ex(e.value)+')';case 'InterfaceContext':return ex(e.value);case 'ObjToInterfaceCast':return 'iface<'+nm(e['class'])+'>('+ex(e.value)+')';
 case 'ArrayGetByRef':return ex(e.array)+'['+ex(e.index)+']';
 case 'StructConst':return 'struct '+nm(e.struct)+'{'+args(e.members)+'}';case 'SkipOffsetConst':return '@'+e.value;
 default:return e.op+rest(e).slice(0,400);} }
function args(a){return (a||[]).filter(function(x){return !/^End/.test(x.op);}).map(ex).join(', ');}
function st(e){ switch(e.op){
 case 'Let':case 'LetBool':case 'LetObj':case 'LetWeakObjPtr':case 'LetDelegate':case 'LetMulticastDelegate':return ex(e.variable)+' = '+ex(e.value);
 case 'LetValueOnPersistentFrame':return 'frame.'+nm(e.property)+' = '+ex(e.value);
 case 'Jump':return 'goto '+e.target;case 'JumpIfNot':return 'if !('+ex(e.condition)+') goto '+e.target;case 'PushExecutionFlow':return 'push '+e.target;case 'PopExecutionFlow':return 'pop (goto popped)';case 'PopExecutionFlowIfNot':return 'if !('+ex(e.condition)+') pop';
 case 'ComputedJump':return 'goto computed '+ex(e.target);case 'Return':return 'return '+ex(e.value);case 'EndOfScript':return 'end';default:return ex(e);} }
for(const e of d.exports){ if(e.data&&e.data.native&&Array.isArray(e.data.native.script)&&e.data.native.script.length&&e.objectName!=='p_ex001_Anim_C'){ console.log('\n== function '+e.objectName+' flags=0x'+(e.data.native.functionFlags>>>0).toString(16)); for(const s of e.data.native.script) console.log(String(s.at).padStart(5),' ',st(s)); } }
console.log('\nother exports:'); d.exports.forEach(function(e,i){ if(i>=34&&i<49) console.log(i+1,e.objectName,full(e.classIndex),'outer',e.outerIndex,JSON.stringify(e.data).slice(0,260)); });
