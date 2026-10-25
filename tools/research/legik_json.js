// usage: node legik_json.js  -> writes build/sora_tmp/sora_legik.json (leg-IK node properties of p_ex001_Anim + footstep units of p_ex001_PawnBase TresCharMesh0)
const fs=require('fs'),path=require('path');const R=path.resolve(__dirname,'../..');
const C=R+'/assets_converted/packages/KINGDOM HEARTS III/Content/';
function load(rel){const d=JSON.parse(fs.readFileSync(C+rel,'utf8'));
 const full=i=>{ if(!i) return null; if(i<0){let im=d.imports[-i-1],s=im.objectName;while(im.outerIndex){im=d.imports[-im.outerIndex-1];s=im.objectName+'.'+s;}return s;} return 'E'+i+':'+d.exports[i-1].objectName;};
 const v=p=>{ if(p===null||typeof p!=='object') return p; if(Array.isArray(p)) return p.map(v); if('name' in p&&'type' in p){ let val=p.value; if(p.type==='ObjectProperty') val=full(val); else val=v(val); return {[p.name+(p.arrayIndex?'['+p.arrayIndex+']':'')]:val}; } const o={}; for(const k in p) o[k]=v(p[k]); return o;};
 const flat=x=>{ if(Array.isArray(x)){ if(x.length&&x.every(e=>e&&typeof e==='object'&&!Array.isArray(e)&&Object.keys(e).length===1)){const o={};let ok=true;for(const e of x){const k=Object.keys(e)[0];if(k in o){ok=false;break;}o[k]=flat(e[k]);} if(ok) return o;} return x.map(flat);} if(x&&typeof x==='object'){ if(x.structName&&Array.isArray(x.items)) return x.items.map(flat); const o={};for(const k in x)o[k]=flat(x[k]);return o;} return x;};
 return {d,props:e=>flat(v(e.data)).props}; }
const A=load('Character/pc/p_ex001/anm/p_ex001_Anim.uasset.json');
const cdo=A.d.exports.find(e=>/^Default__/.test(e.objectName)); const p=A.props(cdo);
const keys=Object.keys(p).filter(k=>/AnimGraphNode_/.test(k));
const node=(k,i)=>{const n=Object.assign({},p[k]); const b=n.EvaluateGraphExposedInputs; delete n.EvaluateGraphExposedInputs; const bind=((b&&b.CopyRecords)||[]).map(r=>({from:r.SourcePropertyName,to:String(r.DestProperty).split('.').pop()})); return {graph_node:k,node_index:i,bound_inputs:bind,properties:n};};
const coll=[],multi=[];let blend=null;
keys.forEach((k,i)=>{ if(/CollTwoBoneIK/.test(k)) coll.push(node(k,i)); else if(/TwoBoneIKMultiped/.test(k)) multi.push(node(k,i)); else if(/BlendListByBool/.test(k)) blend=node(k,i); });
const P=load('Blueprints/Player/p_ex001/p_ex001_PawnBase.uasset.json');
const mesh=P.d.exports.find(e=>e.objectName==='TresCharMesh0'); const mp=P.props(mesh);
const out={
 schema:{
  note:'Leg-IK data of Sora. Values are the tagged properties serialized in the assets (floats as stored, cm / degrees / seconds). A property missing from "properties" was not serialized: use native_defaults. node_index = index of the node in the AnimBlueprint (LinkID space). bound_inputs: anim-instance variable copied into the node property before every update.',
  sources:['KINGDOM HEARTS III/Content/Character/pc/p_ex001/anm/p_ex001_Anim (Default__p_ex001_Anim_C)','KINGDOM HEARTS III/Content/Blueprints/Player/p_ex001/p_ex001_PawnBase (TresCharMesh0)'],
  graph:'Root(0) <- C2L(17) <- ModifyBone atama(24) <- ModifyBone kubi(23) <- ExLookAt(12) <- L2C(18) <- BlendListByBool(13; bActiveValue <- IKinemaSW; BlendPose[0]=true pose=C2L(33)<-Multiped(31)<-L2C(32)<-UseCachedPose(16); BlendPose[1]=false pose=C2L(19)<-CollTwoBoneIK L_ashi1(6)<-CollTwoBoneIK R_ashi1(5)<-L2C(4)<-UseCachedPose(15)); both cached poses = SavedPose2910 (node 14)',
  native_defaults:{
   TresAnimNode_CollTwoBoneIK:{ctor:'0x14052ABE0',Alpha:1,AlphaScaleBias:{Scale:1,Bias:0},LODThreshold:-1,IKBone:'None',EffectorLocation:[0,0,0],JointTargetLocation:[0,0,0],EffectorSpaceBoneName:'None',bTakeRotationFromEffectorSpace:false,bMaintainEffectorRelRot:false,bAllowStretching:false,StretchLimits:'FVector2D::ZeroVector (0x149B5C790)',StartStretchRatio:1,MaxStretchScale:1.2,EffectorLocationSpace:'BCS_BoneSpace(3)',JointTargetLocationSpace:'BCS_BoneSpace(3)',JointTargetSpaceBoneName:'None',m_bMaintainEffectorHitNormal:false,m_MaintainEffectorLimitRot:25,m_MaintainEffectorStartInterpTime:0.1,m_MaintainEffectorEndInterpTime:0.2,CollisionShape:'SPHERE(0)',CollisionOffset:[0,0,0],CollisionRotation:[0,0,0],CollisionSize:[1,1,1],CheckMin:0,CheckMax:30,IkOffset:0,runtime:{'+0x118':'DeltaTime 0','+0x120':'FQuat identity (stored normal quat)','+0x130':'normal blend 0','+0x134':'previous ActualAlpha 0','+0x138 bit0':'alpha jumped flag 0'}},
   TresAnimNode_TwoBoneIKMultiped:{ctor:'0x14052B380',Alpha:1,LODThreshold:-1,bUseMayaSolver:false,HipsTransform:'None',HipsOffsetPercentage:0.7,HipsRotationLimit:0,HipsRotationResetLimit:0,bLimitHipsTransHeight:false,bExHitpsRotateDetect:false,ExHitpsRotateDetectSampleNum:16,ExHitpsRotateDetectRadius:200,ExHitpsRotateDetectCheckMax:230,ExHitpsRotateDetectCheckMin:-200,MyDeltaTime:0,RaycastCacheMaximumTranslation:0,RaycastCacheMaximumRotation:0,RaycastForceFrameRate:10,m_RaycastCacheRootDisplacement:false,m_KeepHighestRaycastHitInSmallRadius:false,m_KeepHighest_XY_MaxRadius:4,m_KeepHighest_Z_Displacement:2,FootSmoothingVelocity:15,TimeStartSmoothing:0,DrawDebug:false,HipsController:{value:0,velocity:0,Gain:140,damping:23.6643,maxVelocity:6000,note:'Gain is forced to 140 by Initialize 0x1405453D0; damping 0x41BD5087 and maxVelocity 6000 are not reflected'}},
   TresTwoBoneLegIKDefinition:'constructor not read: every field Sora uses is serialized except LimitTwoBoneIkInternalAngle-derived cos (runtime) [?]',
   TresAnimNotifyState_IK:{ctor:'0x14055A0C0',m_StartInterpolationTime:0,m_EndInterpolationTime:0,m_TargetAlpha:1},
   TresCharPawnBase:{MySkelCtrl_IkDisableFlag:false,MySkelCtrl_IkInitValue:0},
   TresCharAnimInstance:{IKinemaSW:true,m_LegIKAlpha:0,m_LegIKOffset:0}
  }
 },
 blend_list_by_bool:blend, coll_two_bone_ik:coll, two_bone_ik_multiped:multi,
 footstep:{m_bEnableCollisionFootStep:mp.m_bEnableCollisionFootStep,m_CheckFootSteps:mp.m_CheckFootSteps,note:'footstep effect checks only (UTresSkeletalMeshComponent 0x1405CAFF0); not read by the IK nodes'},
 mesh_component:{RelativeRotation:mp.RelativeRotation,RelativeLocation:'not serialized (native default 0,0,0)'},
 ik_notify_windows_file:'build/sora_tmp/sora_ik_notifies.tsv (name, package id, class, length, IK@start+duration[start=StartInterp,end=EndInterp,alpha=TargetAlpha])'
};
fs.writeFileSync(__dirname+'/sora_legik.json',JSON.stringify(out,null,1));
console.log('coll',coll.length,'multi',multi.length,'blend',!!blend,'footsteps',(mp.m_CheckFootSteps||[]).length);
