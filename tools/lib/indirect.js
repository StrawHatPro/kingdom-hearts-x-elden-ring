// Prototypes for indirect calls.
// The decompiler prints a call through a pointer as (**(code **)(expr))(args), and `code` is an
// unprototyped function type, so C promotes float arguments to double and passes them the
// variadic way: not what the original does. The exporter marks each indirect call with its
// call-site address (/*@ci:<VA>*/) and records the argument classes it saw there; this gives the
// call a real prototype built from those classes whenever a float or double is involved, as an
// argument or as the returned value (a site is [VA, argument classes, return class if used]).
const TYPE = { 0: 'undefined8', 1: 'undefined1', 2: 'undefined2', 4: 'undefined4', 8: 'undefined8', 20: 'float', 24: 'double' };
const MARK = /\/\*@ci:([0-9A-F]+)\*\//g;

// index of the parenthesis matching the one at `open`, or -1
function match(s, open) {
  let depth = 0;
  for (let i = open; i < s.length; i++) {
    const c = s[i];
    if (c === '(') depth++;
    else if (c === ')') { if (--depth === 0) return i; }
    else if (c === ';' || c === '{' || c === '}') return -1;
  }
  return -1;
}
// number of top-level arguments in the text between a call's parentheses
function countArgs(s) {
  if (!s.trim()) return 0;
  let depth = 0, n = 1;
  for (const c of s) { if (c === '(' || c === '[') depth++; else if (c === ')' || c === ']') depth--; else if (c === ',' && depth === 0) n++; }
  return n;
}

// Returns the C text with markers removed and prototypes applied where they are needed and the
// call shape is recognised; anything else is left exactly as the decompiler wrote it.
function apply(c, sites) {
  if (c.indexOf('/*@ci:') < 0) return c;
  const cls = new Map(sites.map(([a, k]) => [a, k]));
  const rets = new Map(sites.map(([a, , r]) => [a, r]));
  const edits = [];
  for (const m of c.matchAll(MARK)) {
    const k = cls.get(m[1]), r = rets.get(m[1]);
    const fret = r === 20 || r === 24;
    if (!k || !(fret || k.some((x) => x === 20 || x === 24))) continue;
    if (k.some((x) => !TYPE[x] || x === 0)) continue;   // a class with no scalar type: leave the call alone
    // the callee group either encloses the marker "(" MARK "**(code **)(...))" or follows it
    let open = m.index > 0 && c[m.index - 1] === '(' ? m.index - 1 : -1;
    if (open < 0) {
      const eol = c.indexOf(';', m.index);
      const at = c.indexOf('(**(code **)', m.index);
      if (at < 0 || (eol >= 0 && at > eol)) continue;
      open = at;
    }
    const close = match(c, open);
    if (close < 0) continue;
    let args = close + 1;                       // a long call is wrapped before its argument list
    while (c[args] === ' ' || c[args] === '\n' || c[args] === '\r') args++;
    if (c[args] !== '(') continue;
    const argsClose = match(c, args);
    if (argsClose < 0 || countArgs(c.slice(args + 1, argsClose)) !== k.length) continue;
    const proto = `${fret ? TYPE[r] : 'undefined8'} (*)(${k.length ? k.map((x) => TYPE[x] || 'undefined8').join(',') : 'void'})`;
    edits.push([open, close + 1, `((${proto})${c.slice(open, close + 1)})`]);
  }
  edits.sort((a, b) => b[0] - a[0]);
  for (const [s, e, text] of edits) c = c.slice(0, s) + text + c.slice(e);
  return c.replace(MARK, '');
}

module.exports = { apply };
