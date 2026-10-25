// Pass 1: every notify of every Sora / Kingdom Key clip -> notifies_all.json (one entry per notify event)
const L = require('./lib');
const { path, fs } = L;
const dirs = [['pc', 'Character/pc/p_ex001/anm'], ['wep', 'Character/wep/w_so010/anm']];
const out = [], hist = {}, noSidecar = [], clipsScanned = { pc: 0, wep: 0 }, classesOfExports = {};
for (const [who, rel] of dirs) {
  const files = L.walk(path.join(L.PK, L.KH, rel), /\.uasset\.json$/);
  for (const f of files) {
    const txt = fs.readFileSync(f, 'utf8');
    if (!/AnimNotify|"AnimSequence"|AnimMontage/.test(txt)) continue;
    const d = JSON.parse(txt);
    const seq = d.exports.map((e, i) => [e, i + 1]).filter(([e]) => /^(AnimSequence|AnimMontage|AnimComposite)$/.test(e.class || ''));
    for (const e of d.exports) { const c = e.class || ''; if (!/Notify/.test(c)) classesOfExports[c] = (classesOfExports[c] || 0) + 1; }
    if (!seq.length) continue;
    clipsScanned[who]++;
    const game = L.pkgFileToGame(f);
    const side = path.join(L.AN, L.KH, game.replace('/Game/', '') + '.anim.json');
    let events = null, length = null, rate = null;
    if (fs.existsSync(side)) {
      const s = L.readJson(side);
      const sp = Object.fromEntries(s.data.props.map((x) => [x.tag.name, x.tag.type === 'BoolProperty' ? x.tag.boolVal === 1 : x.value]));
      length = sp.SequenceLength ?? null; rate = sp.RateScale ?? 1;
      events = (sp.Notifies ? sp.Notifies.items : []).map((it, order) => {
        const o = Object.fromEntries(it.props.map((x) => [x.tag.name, x.tag.type === 'BoolProperty' ? x.tag.boolVal === 1 : x.value]));
        const link = Object.fromEntries(it.props.filter((x) => x.tag.name === 'LinkValue').map((x) => [x.tag.name, x.value]));
        const objIndex = (o.Notify && o.Notify.object) || (o.NotifyStateClass && o.NotifyStateClass.object) || 0;
        const isState = !!(o.NotifyStateClass && o.NotifyStateClass.object);
        const time = +(o.LinkValue ?? o.DisplayTime ?? 0);
        return { order, name: o.NotifyName, isState, objIndex, time, trig: +(o.TriggerTimeOffset ?? 0), duration: isState ? +(o.Duration ?? 0) : null, endTrig: +(o.EndTriggerTimeOffset ?? 0), chance: o.NotifyTriggerChance ?? 1, track: o.TrackIndex ?? null };
      });
    } else {
      noSidecar.push(game);
      // fall back: notify exports without times
      events = d.exports.map((e, i) => [e, i + 1]).filter(([e]) => /Notify/.test(e.class || '')).map(([e, i], order) => ({ order, name: null, isState: /NotifyState/.test(e.class), objIndex: i, time: null, trig: null, duration: null, endTrig: null, chance: null, track: null, noSidecar: true }));
    }
    for (const ev of events) {
      const exp = ev.objIndex > 0 ? d.exports[ev.objIndex - 1] : null;
      const cls = exp ? L.exportClass(d, exp) : '(none:' + ev.name + ')';
      hist[cls] = (hist[cls] || 0) + 1;
      out.push({ who, clip: game, cooked: L.pkgFileToCooked(f), seqClass: seq[0][0].class, length, rate, order: ev.order, name: ev.name, kind: ev.isState ? 'state' : 'instant', time: ev.time, trig: ev.trig, duration: ev.duration, endTrig: ev.endTrig, chance: ev.chance, track: ev.track, class: cls, object: exp ? exp.objectName : null, props: exp ? L.exportProps(d, exp) : {} });
    }
  }
}
fs.writeFileSync(path.join(L.OUT, 'notifies_all.json'), JSON.stringify(out));
fs.writeFileSync(path.join(L.OUT, 'notify_hist.json'), JSON.stringify({ clipsScanned, noSidecar: noSidecar.length, noSidecarList: noSidecar.slice(0, 50), hist: Object.fromEntries(Object.entries(hist).sort((a, b) => b[1] - a[1])), otherExportClasses: classesOfExports }, null, 1));
console.log(clipsScanned, 'events', out.length, 'noSidecar', noSidecar.length);
console.log(Object.entries(hist).sort((a, b) => b[1] - a[1]).map(([k, v]) => v + ' ' + k).join('\n'));
console.log(classesOfExports);
