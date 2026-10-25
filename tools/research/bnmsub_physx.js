const fs=require('fs'),path=require('path');const R=path.resolve(__dirname,'../..');
const f=R+'/assets_converted/packages/KINGDOM HEARTS III/Content/Character/pc/p_ex001/mdl/p_ex001_0_Physics.uasset.json';
const d=JSON.parse(fs.readFileSync(f,'utf8'));
const man=JSON.parse(fs.readFileSync(R+'/build/sora_pack/manifest.json','utf8')); const bones=man.skeleton.bones;
const bidx=n=>bones.findIndex(b=>(b.name||b)===n);
function full(i){ if(!i) return null; if(i<0){let im=d.imports[-i-1],s=im.objectName;while(im.outerIndex){im=d.imports[-im.outerIndex-1];s=im.objectName+'.'+s;}return s;} return d.exports[i-1].objectName;}
function pv(p){ if(p===null||typeof p!=='object')return p; if(Array.isArray(p)){ if(p.length&&p.every(e=>e&&typeof e==='object'&&'name' in e&&'type' in e)){const o={};for(const e of p)o[e.name+(e.arrayIndex?'['+e.arrayIndex+']':'')]=e.type==='ObjectProperty'?full(e.value):pv(e.value);return o;} return p.map(pv);} 
 if(p.items)return p.items.map(pv); if('value' in p&&'type' in p)return pv(p.value); const o={};for(const k in p)o[k]=pv(p[k]);return o;}
const pa=d.exports[0]; const pp=pv(pa.data.props);
const out={_schema:{
 source:'Character/pc/p_ex001/mdl/p_ex001_0_Physics (UPhysicsAsset, UE 4.17 SQEX fork). Every tagged property stored in the package is reproduced under `props`/`raw`; a property that is absent from the asset has the class default (listed under `stock_defaults_not_in_asset`, stock UE 4.17 values, NOT read from the exe).',
 units:'cm, degrees; KSphylElem Length = cylinder part length (total height = Length + 2*Radius), capsule axis = local Z of the element (element transform = Rotation(pitch,yaw,roll) then Center, in the bone local frame)',
 physicsAsset:'props of the PhysicsAsset export: BoundsBodies (indices into bodies[] in SkeletalBodySetups order), plus native collisionDisableTable (pairs of body indices whose mutual collision is disabled)',
 bodies:'in PhysicsAsset.SkeletalBodySetups order (index = body index used by BoundsBodies / collisionDisableTable). fields: export, bone, boneIndex (index into build/sora_pack/manifest.json skeleton.bones, -1 if missing), physicsType, collisionTraceFlag, aggGeom{spheres[],boxes[],sphyls[],convex[]}, raw (all tagged props), native',
 constraints:'in PhysicsAsset.ConstraintSetup order. fields: export, jointName, bone1 (child), bone2 (parent), frame1{pos,priAxis,secAxis} in bone1 space, frame2{...} in bone2 space (absent pos = (0,0,0), priAxis = (1,0,0), secAxis = (0,1,0)), profile (ProfileInstance props if stored), raw (all tagged props)',
 skeletalMeshReference:'how p_ex001_0 (SkeletalMesh) references this asset'},
 skeletalMeshReference:{mesh:'Character/pc/p_ex001/mdl/p_ex001_0',PhysicsAsset:'/Game/Character/pc/p_ex001/mdl/p_ex001_0_Physics.p_ex001_0_Physics',ShadowPhysicsAsset:'/Game/Character/pc/p_ex001/mdl/p_ex001_0_Physics.p_ex001_0_Physics',evidence:'assets_converted/meshes/.../p_ex001_0.mesh.json data.props PhysicsAsset / ShadowPhysicsAsset = import -37'},
 stock_defaults_not_in_asset:{body:{PhysicsType:'PhysType_Default',CollisionTraceFlag:'CTF_UseDefault',bConsiderForBounds:true,'DefaultInstance.MassScale':1,'DefaultInstance.LinearDamping':0.01,'DefaultInstance.AngularDamping':0,'DefaultInstance.bEnableGravity':true,'DefaultInstance.CollisionProfileName/ObjectType/CollisionEnabled':'BodyInstance defaults (QueryAndPhysics, profile resolved by owning component at runtime)',PhysMaterial:null},
  constraint:{LinearLimit:{XMotion:'LCM_Locked',YMotion:'LCM_Locked',ZMotion:'LCM_Locked',Limit:0},ConeLimit:{Swing1Motion:'ACM_Free',Swing2Motion:'ACM_Free',Swing1LimitDegrees:45,Swing2LimitDegrees:45},TwistLimit:{TwistMotion:'ACM_Free',TwistLimitDegrees:45},softConstraints:'bSoftConstraint true, Stiffness 50, Damping 5 for cone and twist; linear soft off',drives:'all linear/angular drives disabled',bDisableCollision:false,note:'stock UE 4.17 FConstraintProfileProperties defaults, from the public source, not verified in the exe'}},
 physicsAsset:{export:pa.objectName,props:pp,native:pa.data.native}, bodies:[], constraints:[]};
const raw=pa.data.props; const get=n=>raw.find(p=>p.name===n).value;
for(const ei of get('SkeletalBodySetups')){ const e=d.exports[ei-1]; const p=pv(e.data.props); const g=p.AggGeom||{};
 out.bodies.push({export:e.objectName,exportIndex:ei,bone:p.BoneName,boneIndex:bidx(p.BoneName),physicsType:p.PhysicsType||'(default PhysType_Default)',collisionTraceFlag:p.CollisionTraceFlag||'(default)',
  aggGeom:{spheres:g.SphereElems||[],boxes:g.BoxElems||[],sphyls:g.SphylElems||[],convex:g.ConvexElems||[]},raw:p,native:e.data.native}); }
for(const ei of get('ConstraintSetup')){ const e=d.exports[ei-1]; const p=pv(e.data.props); const di=p.DefaultInstance||{};
 out.constraints.push({export:e.objectName,exportIndex:ei,jointName:di.JointName,bone1:di.ConstraintBone1,bone1Index:bidx(di.ConstraintBone1),bone2:di.ConstraintBone2,bone2Index:bidx(di.ConstraintBone2),
  frame1:{pos:di.Pos1||{x:0,y:0,z:0},priAxis:di.PriAxis1||{x:1,y:0,z:0},secAxis:di.SecAxis1||{x:0,y:1,z:0}},frame2:{pos:di.Pos2||{x:0,y:0,z:0},priAxis:di.PriAxis2||{x:1,y:0,z:0},secAxis:di.SecAxis2||{x:0,y:1,z:0}},
  profile:di.ProfileInstance||'(not stored: all defaults)',raw:p,native:e.data.native}); }
fs.writeFileSync(R+'/build/sora_tmp/sora_physics_asset.json',JSON.stringify(out,null,1));
console.log('PA props',JSON.stringify(pp).slice(0,300)); console.log('disable pairs',pa.data.native.collisionDisableTable.length, Object.keys(pa.data.native));
for(const b of out.bodies) console.log(b.bone,b.boneIndex,Object.keys(b.raw).join(','),'|',JSON.stringify(b.aggGeom).replace(/(\d+\.\d{2})\d+/g,'$1').slice(0,330));
for(const c of out.constraints) console.log(c.export,c.bone1,'->',c.bone2,Object.keys(c.raw.DefaultInstance).join(','),JSON.stringify(c.raw).replace(/(\d+\.\d{3})\d+/g,'$1').length, JSON.stringify(c.profile).slice(0,200));
console.log(d.imports.map(i=>i.objectName).join(','));
