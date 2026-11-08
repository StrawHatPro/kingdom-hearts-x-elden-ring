// (AUDIT-B F-HAMMER) THE FORM WEAPONS' / STYLE PAWNS' OWN CLIP NOTIFIES as `wnotify` rows, in F-SHIP's columns (the rows
// tools/sora_form_ship.js writes for Wheel of Fate; kh-core combat/forms/ship.rs `wnotify_pass` plays them, kh-host
// vfx/kbweapon.rs resolves their `kb:` anchors). KH3 gives each weapon actor of a form (its units, its transform units)
// and each style pawn an anim instance of its own, which plays the row of the logical name Sora plays; that clip's
// notifies fire on THEIR mesh: AttachEffect rows on the weapon's own bones / sockets (OWNER = the weapon mesh), AttachSound
// / PlaySound rows at the weapon. One row per effect entry / sound:
//   clip   = the sequence SORA plays with it (the weapon clip's logical name `M_<seq>` -> <seq>, else the weapon's own
//            sequence: StyleChange3 3401, StyleChange20 K401 ...),
//   flags  = [anchor `kb:<model>:<mount>:<bone>[:abs][:free][:fo<s>]`, end type, state / instant, g<GroupID>, ...extra],
//   values = [time, end (-1 instant), location x, y, z, rotation pitch, yaw, roll, fade out s].
// The same reading as F-SHIP's block (which predates this file); used by tools/sora_form_hammer.js.
'use strict';
const fs = require('fs');
const path = require('path');

const r4 = (x) => Math.round(x * 10000) / 10000;
const pv = (list) => { const o = {}; for (const p of list || []) o[p.name] = p.type === 'StructProperty' ? (Array.isArray(p.value) ? pv(p.value) : p.value) : p.type === 'ArrayProperty' ? (p.value && p.value.items ? p.value.items.map((it) => (Array.isArray(it) ? pv(it) : it)) : p.value) : p.value; return o; };
const tail = (s) => String(s || '').split('/').pop().split('.').pop();

/// (AUDIT-B W-VFX) true when the pack's effect `name` (keyblades/vfx/effects or vfx/effects) has a TypeDataSQEX_Thunder emitter.
const thunderCache = new Map();
function thunderEffect(pack, name) {
  const key = String(name || '').toLowerCase();
  if (!thunderCache.has(key)) {
    let found = false;
    for (const dir of [path.join(pack, 'keyblades', 'vfx', 'effects'), path.join(pack, 'vfx', 'effects')]) {
      const file = fs.existsSync(dir) ? fs.readdirSync(dir).find((f) => f.toLowerCase() === key + '.json') : null;
      if (!file) continue;
      const fx = JSON.parse(fs.readFileSync(path.join(dir, file), 'utf8').replace(/^﻿/, ''));
      found = (fx.emitters || []).some((e) => e.typedata && e.typedata.kind === 'sqex_thunder');
      break;
    }
    thunderCache.set(key, found);
  }
  return thunderCache.get(key);
}

/// The sequence Sora plays while the weapon plays clip `c`.
function soraSequence(c) {
  const m = (c.logical_names || []).find((n) => /^M_/.test(n));
  return m ? m.slice(2) : c.sequence;
}

/// rows for `models` = [{ model, mount, form: (clip) -> form key or null (null: no row), extra: (clip) -> [flags] }]
/// read from `<pack>/keyblades/<model>/model.json`.
function wnotifyRows(pack, models) {
  const out = [];
  for (const { model, mount, form, extra } of models) {
    const file = path.join(pack, 'keyblades', model, 'model.json');
    const mj = JSON.parse(fs.readFileSync(file, 'utf8').replace(/^﻿/, ''));
    for (const c of mj.clips || []) {
      const key = form(c);
      if (!key) continue;
      const clip = soraSequence(c);
      const more = extra ? extra(c) : [];
      let k = 0;
      for (const n of c.notifies || []) {
        const np = n.properties || {};
        const t = r4(n.trigger_time !== undefined ? n.trigger_time : n.time);
        const state = /NotifyState_/.test(n.class);
        const end = state ? r4(n.end_trigger_time !== undefined ? n.end_trigger_time : n.end_time) : -1;
        const base = { what: 'wnotify', form: key, clip, length: r4(c.duration_s || 0), ref_addr: 'build/sora_pack/keyblades/' + model + '/model.json', ref_conf: 'confirmed',
          note: 'the ' + model + ' clip ' + c.sequence + ' (' + (c.logical_names || []).join('/') + ') with Sora\'s ' + clip + ': ' + n.class + '; values = [time, end (-1 instant), location x, y, z, rotation pitch, yaw, roll, fade out s]' };
        const id = () => 'wnotify.' + model + '.' + c.sequence + '.' + k++;
        if (/AttachEffect/.test(n.class)) {
          for (const item of (np.MetaAttachDataList && np.MetaAttachDataList.items) || []) {
            const top = pv(item);
            for (const meta of top.MetaAttachData || []) {
              const m = Array.isArray(meta) ? pv(meta) : meta;
              for (const e of m.EffectDataList || []) {
                const d = Array.isArray(e) ? pv(e) : e;
                if (!d.AttachEffect || d.AttachEffect === 'null') continue;
                if (!String(top.AttachType).endsWith('OWNER')) throw new Error(model + ' ' + c.sequence + ': attach type ' + top.AttachType + ' (only OWNER is ported)');
                const sock = m.SocketName && m.SocketName !== 'None' ? m.SocketName : top.SocketName && top.SocketName !== 'None' ? top.SocketName : '';
                const endType = String(m.AttachEffectEndType || '').replace('ESQEX_ATTACH_EFFECT_END_TYPE_', '');
                // (AUDIT-B W-VFX) the row's EndSocketName for a TypeDataSQEX_Thunder effect (`:to<socket>`): KH3's attach spawn
                // 0x141772CD0 hands SocketName / EndSocketName to SetThunderAttachBones, the beam's two ends (kh-vfx BeamDef)
                const endSock = m.EndSocketName && m.EndSocketName !== 'None' && thunderEffect(pack, tail(d.AttachEffect)) ? ':to' + m.EndSocketName : '';
                const anchor = 'kb:' + model + ':' + mount + ':' + sock + (m.bAbsoluteRotation ? ':abs' : '') + (m.ToAttach === false ? ':free' : '') + (endType === 'FADEOUT' ? ':fo' + r4(m.FadeOut || 0) : '') + endSock;
                const lo = m.LocationOffset || {}, ro = m.RotationOffset || {};
                out.push({ ...base, id: id(), code: 0, logical: 'effect', flags: [anchor, endType, state ? 'state' : 'instant', 'g' + d.GroupID, ...more], effects: [tail(d.AttachEffect)],
                  // (AUDIT-B W-FIN) + [FadeIn, StartDelayTime] when the row has either (0x141772CD0's post-spawn; kh-core ship.rs
                  // `wnotify_pass` sends them through `Effects::timing`); rows without keep their 9 values
                  values: [t, end, r4(lo.x || 0), r4(lo.y || 0), r4(lo.z || 0), r4(ro.pitch || 0), r4(ro.yaw || 0), r4(ro.roll || 0), endType === 'FADEOUT' ? r4(m.FadeOut || 0) : 0, ...((m.FadeIn || 0) || (m.StartDelayTime || 0) ? [r4(m.FadeIn || 0), r4(m.StartDelayTime || 0)] : [])] });
              }
            }
          }
        } else if (/AttachSound|PlaySound/.test(n.class)) {
          const anchor = 'kb:' + model + ':' + mount + ':' + (np.AttachPointName && np.AttachPointName !== 'None' ? np.AttachPointName : '');
          out.push({ ...base, id: id(), code: 1, logical: 'sound', flags: [anchor, '-', state ? 'state' : 'instant', 'g-', ...more], sounds: [tail(np.Sound)], values: [t, end, 0, 0, 0, 0, 0, 0, r4(np.FadeOutDuration || 0)] });
        } else if (/WeaponSwingSE|WeaponHitSE/.test(n.class)) {
          const anchor = 'kb:' + model + ':' + mount + ':' + (np.m_SocketName && np.m_SocketName !== 'None' ? np.m_SocketName : '');
          out.push({ ...base, id: id(), code: 2, logical: /Hit/.test(n.class) ? 'hit_se' : 'swing_se', flags: [anchor, np.m_bFollow ? 'follow' : 'fixed', state ? 'state' : 'instant', 'g-', ...more], sounds: [String(np.m_SoundAliasID || '').split('::').pop()], values: [t, end, 0, 0, 0, 0, 0, 0, 0] });
        }
      }
    }
  }
  return out;
}

/// (AUDIT-B W-VFX2) a TypeDataSQEX_Thunder row's host-resolved anchor for the core-side `wfx` routes (tools/sora_form_claws.js,
/// tools/sora_form_nano.js): `kb:<model>:<mount>:<socket>[:abs][:free][:fo<s>][:to<EndSocketName>]`, the same name this file's
/// rows carry, so the host resolves the beam's ends (SetThunderAttachBones: SocketName / EndSocketName, or the bone list) on
/// THAT mesh every step (kh-host `EffectHost::beam_ends`). '' when `effect` is not a thunder effect of the pack. The anchor
/// carries no LocationOffset / RotationOffset (the kb: route places the node on the socket itself): a row with either is
/// refused, so no data is silently moved.
function thunderAnchor(pack, effect, model, mount, socket, d, endType) {
  if (!thunderEffect(pack, tail(effect))) return '';
  const lo = d.LocationOffset || {}, ro = d.RotationOffset || {};
  if ([lo.x, lo.y, lo.z, ro.pitch, ro.yaw, ro.roll].some((x) => Math.abs(+x || 0) > 1e-6)) throw new Error(model + ' ' + effect + ': a thunder row with a Location / RotationOffset (the kb: anchor has none)');
  const sock = socket && socket !== 'None' ? socket : '';
  const end = d.EndSocketName && d.EndSocketName !== 'None' ? ':to' + d.EndSocketName : '';
  return 'kb:' + model + ':' + mount + ':' + sock + (d.bAbsoluteRotation ? ':abs' : '') + (d.ToAttach === false ? ':free' : '') + (endType === 'FADEOUT' ? ':fo' + r4(d.FadeOut || 0) : '') + end;
}

module.exports = { wnotifyRows, soraSequence, thunderEffect, thunderAnchor }; // (AUDIT-B W-VFX2) + thunderEffect, thunderAnchor
