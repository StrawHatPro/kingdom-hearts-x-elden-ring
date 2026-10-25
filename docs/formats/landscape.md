# Landscape

Code: `tools/lib/pkg_classes/landscape.js`. Evidence: the classes below in
`audit/package_manifest.json`.

## LandscapeComponent

| Field | Type | Notes |
|---|---|---|
| GrassData.HeightData | bulk array: int32 element size (2), int32 count, uint16 | count 0 in every export |
| GrassData.WeightData | int32 count + { package index grass type, int32 count + bytes } | count 0 in every export, so the entry layout (stock) is not exercised |
| bCooked | uint32 bool | 1 |
| bCookedMobileData | uint32 | 0 in every export (no mobile platform data) |

The common case is 20 bytes: `02 00 00 00 00 00 00 00 | 00 00 00 00 | 01 00 00 00 | 00 00 00 00`.

## LandscapeHeightfieldCollisionComponent

uint32 bCooked, then `CookedCollisionData` as a byte bulk array: int32 element size (1),
int32 count, bytes. The bytes are a PhysX 3.4 cooked heightfield - third-party data
identified by the signature `NXS\x01` `HFHF` - kept verbatim.

## LandscapeMeshCollisionComponent

The same, with a PhysX cooked triangle mesh (`NXS\x01` `MESH`), followed by one more uint32
bool (the subclass's own bCooked; 29 exports of this class exist).
