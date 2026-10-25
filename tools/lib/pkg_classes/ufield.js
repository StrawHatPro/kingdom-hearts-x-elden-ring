// UField family: UProperty subclasses, UFunction, UClass (blueprint generated classes),
// UScriptStruct, UEnum. Docs: docs/formats/blueprint.md.
const { T, register } = require('../package');
const kismet = require('../kismet');

// UField::Serialize: Next. UProperty::Serialize: ArrayDim, PropertyFlags, RepNotifyFunc.
const propBase = [['next', T.obj], ['arrayDim', T.i32], ['propertyFlags', T.u64], ['repNotifyFunc', T.name]];
const prop = (extra = []) => T.struct([...propBase, ...extra]);
register(['IntProperty', 'FloatProperty', 'StrProperty', 'NameProperty', 'TextProperty', 'Int8Property', 'Int16Property', 'Int64Property', 'UInt16Property', 'UInt32Property', 'UInt64Property', 'DoubleProperty'], prop(), 'blueprint');
register(['ObjectProperty', 'WeakObjectProperty', 'LazyObjectProperty', 'AssetObjectProperty'], prop([['propertyClass', T.obj]]), 'blueprint');
register(['ClassProperty', 'AssetClassProperty'], prop([['propertyClass', T.obj], ['metaClass', T.obj]]), 'blueprint');
register('StructProperty', prop([['struct', T.obj]]), 'blueprint');
register('ArrayProperty', prop([['inner', T.obj]]), 'blueprint');
register('SetProperty', prop([['elementProp', T.obj]]), 'blueprint');
register('MapProperty', prop([['keyProp', T.obj], ['valueProp', T.obj]]), 'blueprint');
register('ByteProperty', prop([['enum', T.obj]]), 'blueprint');
register('EnumProperty', prop([['enum', T.obj], ['underlyingProp', T.obj]]), 'blueprint');
register('BoolProperty', prop([['boolSize', T.u8], ['nativeBool', T.u8]]), 'blueprint');
register(['DelegateProperty', 'MulticastDelegateProperty'], prop([['signatureFunction', T.obj]]), 'blueprint');
register('InterfaceProperty', prop([['interfaceClass', T.obj]]), 'blueprint');

// UStruct::Serialize: SuperStruct, Children, script.
const ustruct = [['next', T.obj], ['superStruct', T.obj], ['children', T.obj], ['script', kismet.script]];
const FUNC_Net = 0x40;
register(['Function', 'DelegateFunction'], T.struct([...ustruct,
  ['functionFlags', T.u32],
  ['repOffset', T.when((o) => (o.functionFlags & FUNC_Net) !== 0, T.i16)],
  ['eventGraphFunction', T.obj], ['eventGraphCallOffset', T.i32],
]), 'blueprint');

const uclass = T.struct([...ustruct,
  ['funcMap', T.map(T.name, T.obj, 12)],
  ['classFlags', T.u32], ['classWithin', T.obj], ['classConfigName', T.name], ['classGeneratedBy', T.obj],
  ['interfaces', T.arr(T.struct([['class', T.obj], ['pointerOffset', T.i32], ['implementedByK2', T.b32]]), 12)],
  ['deprecatedForceScriptOrder', T.b32], ['dummy', T.name], ['cooked', T.b32], ['classDefaultObject', T.obj],
]);
register(['BlueprintGeneratedClass', 'WidgetBlueprintGeneratedClass', 'AnimBlueprintGeneratedClass'], uclass, 'blueprint');

register(['ScriptStruct', 'UserDefinedStruct'], T.struct([...ustruct, ['structFlags', T.u32]]), 'blueprint');
register(['Enum', 'UserDefinedEnum'], T.struct([['next', T.obj], ['names', T.arr(T.struct([['name', T.name], ['value', T.i64]]), 16)], ['cppForm', T.u8]]), 'blueprint');
