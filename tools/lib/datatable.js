// UDataTable export <-> plain object, lossless. Layout of the export data in the .uexp:
//   tagged properties of the UDataTable object (RowStruct, ...) terminated by None
//   int32  0                      (UObject "has GUID" flag)
//   int32  row count
//   rows:  FName row name, tagged properties of the row struct terminated by None
const ua = require('./uasset');
const up = require('./uprops');

// Returns { pkg, exportIndex, table } or null when the package has no DataTable export.
function parse(uassetBuf, uexpBuf) {
  const pkg = ua.parseHeader(uassetBuf);
  const ei = pkg.exports.findIndex((e) => ua.objectName(pkg, e.classIndex) === 'DataTable');
  if (ei < 0) return null;
  const e = pkg.exports[ei];
  const start = e.serialOffset - pkg.summary.totalHeaderSize;
  const r = new ua.Reader(uexpBuf, start);
  const table = { objectProps: up.readProps(r, pkg), guidFlag: r.i32(), rows: [] };
  const n = r.i32();
  for (let i = 0; i < n; i++) {
    const at = r.p;
    const name = up.nameStr(pkg, { index: r.i32(), number: r.i32() });
    const props = up.readProps(r, pkg);
    table.rows.push({ name, offset: at, size: r.p - at, props });
  }
  if (r.p !== start + e.serialSize) throw new Error(`table ends at ${r.p}, export ends at ${start + e.serialSize}`);
  table.before = uexpBuf.subarray(0, start).toString('hex');       // other exports' data, verbatim
  table.after = uexpBuf.subarray(r.p).toString('hex');             // following exports and the package tag
  const rs = table.objectProps.find((p) => p.tag.name === 'RowStruct');
  table.rowStruct = rs && rs.value && rs.value.object !== undefined ? ua.objectName(pkg, rs.value.object) : null;
  return { pkg, exportIndex: ei, table };
}

function serialize(pkg, table) {
  const w = new ua.Writer();
  w.hex(table.before);
  up.writeProps(w, pkg, table.objectProps);
  w.i32(table.guidFlag).i32(table.rows.length);
  for (const row of table.rows) {
    const f = up.nameIndex(pkg, row.name);
    w.i32(f.index).i32(f.number);
    up.writeProps(w, pkg, row.props);
  }
  w.hex(table.after);
  return w.concat();
}

module.exports = { parse, serialize };
