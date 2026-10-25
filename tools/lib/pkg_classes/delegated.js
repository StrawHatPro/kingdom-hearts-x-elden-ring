// Classes whose export data is decoded (with its own round-trip audit) by another converter of
// this project. package.js records such an export as { delegated, size, sha256 } and takes the
// bytes from the caller when encoding.
const { T, register, delegate } = require('../package');

delegate(['Texture2D', 'LightMapTexture2D', 'ShadowMapTexture2D', 'TextureLightProfile', 'TextureCube'], 'tools/lib/texture.js (audit/texture_manifest.json)');
delegate(['StaticMesh', 'SkeletalMesh'], 'tools/lib/mesh.js (audit/mesh_manifest.json)');
delegate(['SQEXSEADSound', 'SQEXSEADMusic', 'SQEXSEADSoundBank'], 'tools/lib/sead.js (audit/sead_manifest.json)');
delegate(['AnimSequence'], 'tools/lib/anim.js (audit/anim_manifest.json)');

// UDataTable: int32 row count + { FName row name, tagged properties of the row struct }
register('DataTable', T.struct([['rows', T.arr(T.struct([['name', T.name], ['props', T.props]]), 16)]]), 'datatable');
