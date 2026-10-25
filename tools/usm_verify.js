// Round-trip proof for every CRI Sofdec2 movie (.usm) under the game's Content/CriMovie.
//   node tools/usm_verify.js                 all files, 4 parallel workers; writes audit/usm_manifest.json
//   node tools/usm_verify.js --match <text>  only files whose path contains <text> (prints, writes nothing)
//   node tools/usm_verify.js --one <path relative to gameRoot>
//
// Per file, in one sequential pass and without writing any stream to disk (tools/lib/usm.js scan):
//   1. every chunk is parsed into a descriptor (header fields, @UTF tables as rows, markers);
//   2. stream payloads are split into elementary streams per (signature, chno); video and alpha
//      payloads are unmasked first, so the elementary stream is the plain codec bitstream;
//   3. the container is rebuilt from the descriptors + elementary streams (headers and @UTF
//      tables re-serialised, padding regenerated, video masked again) and its SHA-256 is compared
//      with the SHA-256 of the original bytes, which is computed separately from the raw reads.
// It also cross-checks the metadata against the chunks (sizes, seek table, frame counts, time
// stamps) so that docs/formats/usm.md can state which fields are confirmed.
const fs = require('fs');
const path = require('path');
const { spawn } = require('child_process');
const { ROOT, CONFIG, logRun, writeJson } = require('./lib/log');
const usm = require('./lib/usm');

const VERSION = '1.0.0';
const OUT = path.join(ROOT, 'audit', 'usm_manifest.json');
const MOVIE_DIR = 'KINGDOM HEARTS III/Content/CriMovie';
const PAR = 4;

function listUsm() {
  const dir = path.join(CONFIG.gameRoot, MOVIE_DIR);
  return fs.readdirSync(dir, { recursive: true }).filter((f) => /\.usm$/i.test(f)).map((f) => (MOVIE_DIR + '/' + f).replace(/\\/g, '/')).sort();
}

function getMask(relFiles) {
  if (process.env.USM_VIDEO_MASK) return { ok: true, m2: Buffer.from(process.env.USM_VIDEO_MASK, 'hex') };
  return usm.findVideoMask(relFiles.map((f) => path.join(CONFIG.gameRoot, f)));
}

function verifyOne(rel, m2) {
  const file = path.join(CONFIG.gameRoot, rel);
  // per-stream bookkeeping for the metadata cross-checks
  const per = new Map(); // key -> { offsets: Map(offset -> index), n, timeFloorBad, timeRoundBad, padNotMinimal }
  const pad = { stream_padding_not_minimal: 0, header_chunks_padding: {} };
  const metaChunkTotals = new Map(); // key -> total bytes of type 3 chunks
  let firstNonStreamAfterStream = 0, seenStream = false;
  const order = [];
  const nonStream = {}; // "<sig>/<type>" -> { "<total chunk bytes>": count } for header, metadata and marker chunks
  const nonStreamFields = {}; // frameRate / frameTime header fields of those chunks
  const r = usm.scan(file, {
    onDescriptor(d, offset) {
      const key = usm.streamKey(d.sig, d.chno);
      if (order.length < 64) order.push(`${d.sig}/${usm.TYPE_NAMES[d.type]}`);
      if (d.type === 0) {
        seenStream = true;
        let p = per.get(key);
        if (!p) { p = { offsets: new Map(), n: 0, times: [] }; per.set(key, p); }
        p.offsets.set(offset, p.n);
        p.times.push(d.frameTime);
        p.n++;
        if (d.paddingSize !== ((-(0x20 + d.payloadSize)) & 0x1f)) pad.stream_padding_not_minimal++;
      } else {
        if (seenStream && d.type !== 2) firstNonStreamAfterStream++;
        const plen = d.utf ? usm.buildUtf(d.utf).length : d.marker !== undefined ? d.marker.length + 1 : d.raw_payload_hex.length / 2;
        const ck = d.sig + '/' + usm.TYPE_NAMES[d.type];
        const g = nonStream[ck] || (nonStream[ck] = {});
        g[0x20 + plen + d.paddingSize] = (g[0x20 + plen + d.paddingSize] || 0) + 1;
        const fk = 'frameRate=' + d.frameRate + ' frameTime=' + (d.frameTime === 0 ? '0' : 'nonzero');
        nonStreamFields[fk] = (nonStreamFields[fk] || 0) + 1;
        if (d.type === 3) metaChunkTotals.set(key, (metaChunkTotals.get(key) || 0) + 8 + 0x18 + d.paddingSize + (d.utf ? usm.buildUtf(d.utf).length : d.raw_payload_hex.length / 2));
      }
    },
  }, { videoMask: m2 });

  const crid = r.crid ? usm.utfRows(r.crid) : null;
  const checks = {};
  const note = (name, ok) => { const c = checks[name] || (checks[name] = { ok: 0, fail: 0 }); ok ? c.ok++ : c.fail++; };
  if (crid) note('crid_row0_filesize_is_file_size', crid[0] && crid[0].filesize === r.size);

  const streams = [];
  for (const [key, s] of r.streams) {
    if (s.sig === 'CRID') continue;
    const p = per.get(key) || { offsets: new Map(), n: 0, times: [] };
    const hdr = s.header ? usm.utfRows(s.header)[0] : null;
    const stmid = Buffer.from(s.sig, 'latin1').readUInt32BE(0);
    const dir = crid ? crid.find((x) => x.stmid === stmid && x.chno === s.chno) : null;
    const probe = s.probe ? s.probe.result() : null;
    const row = {
      id: key, signature: s.sig, chno: s.chno,
      source_filename: dir ? dir.filename : null,
      header_table: s.header ? s.header.name : null,
      header: hdr,
      metadata_tables: s.metadata.map((m) => ({ name: m.name, rows: m.rows.length, columns: m.columns.map((c) => `${c.name}:${c.type}:${c.storage}`) })),
      section_markers: s.markers,
      stream_chunks: s.chunks, elementary_bytes: s.bytes, sha256_elementary: s.hash.digest('hex'),
      payload_size_min: s.chunks ? s.minPayload : null, payload_size_max: s.chunks ? s.maxPayload : null,
      chunk_frame_rate_field: s.frameRates, first_frame_time: s.firstTime, last_frame_time: s.lastTime,
      codec: probe ? probe.codec : null,
      unmasked_by_tool: s.unmasked,
      chunks_below_mask_minimum: s.unmasked ? s.chunksBelowMaskMin : null,
      largest_chunk_left_as_stored: s.unmasked && s.chunksBelowMaskMin ? s.largestPlainChunk : null,
      smallest_chunk_unmasked: s.unmasked && s.smallestMaskedChunk !== Infinity ? s.smallestMaskedChunk : null,
      bitstream_probe: probe,
      stored_bytes_probe: s.rawProbe ? s.rawProbe.result() : null,
    };
    // Verdict on masking, from the two probes.
    if (usm.isVideoSig(s.sig)) {
      const raw = row.stored_bytes_probe;
      row.masked = probe && raw ? (probe.plaintext_evidence === true && raw.plaintext_evidence === false ? true : probe.plaintext_evidence === true ? 'indeterminate (stored bytes also look plain)' : 'unknown') : 'unknown';
    } else {
      row.masked = probe && probe.plaintext_evidence === true ? false : 'unknown';
    }

    if (dir) note('crid_filesize_is_elementary_bytes', dir.filesize === s.bytes);
    note('crid_has_row_for_stream', !!dir);
    if (hdr) {
      if (hdr.metadata_size !== undefined) note('metadata_size_is_metadata_chunk_bytes', hdr.metadata_size === (metaChunkTotals.get(key) || 0));
      if (hdr.metadata_count !== undefined) note('metadata_count_is_metadata_chunks', hdr.metadata_count === s.metadata.length);
      if (usm.isVideoSig(s.sig)) {
        note('total_frames_is_stream_chunks', hdr.total_frames === s.chunks);
        if (probe && probe.sequence_header) note('header_size_matches_mpeg_sequence_header', hdr.width === probe.sequence_header.width && hdr.height === probe.sequence_header.height);
        let floorBad = 0;
        const num = 2997n * BigInt(hdr.framerate_d), den = BigInt(hdr.framerate_n);
        for (let i = 0; i < p.times.length; i++) if (BigInt(p.times[i]) !== (BigInt(i) * num) / den) floorBad++;
        note('video_time_is_floor_i_x_2997_x_d_over_n', floorBad === 0);
        row.video_time_mismatches = floorBad;
      } else if (s.sig === '@SFA' && probe && probe.codec === 'hca' && probe.header) {
        const h = probe.header;
        note('audio_header_matches_hca', hdr.sampling_rate === h.sample_rate && hdr.num_channels === h.channels);
        note('hca_first_chunk_is_header_then_one_block_per_chunk', s.chunks === h.block_count + 1 && s.maxPayload === h.block_size);
        let roundBad = 0;
        for (let i = 1; i < p.times.length; i++) {
          const x = BigInt(i - 1) * 1024n * 2997n * 2n, d2 = BigInt(h.sample_rate) * 2n;
          if (BigInt(p.times[i]) !== (x + BigInt(h.sample_rate)) / d2) roundBad++;
        }
        note('audio_time_is_round_block_x_1024_x_2997_over_rate', roundBad === 0 && p.times[0] === 0);
        row.audio_time_mismatches = roundBad;
        const am = s.metadata.find((m) => m.name === 'AUDIO_HEADER');
        if (am) note('audio_header_metadata_is_copy_of_hca_header', !!am.rows[0] && !!am.rows[0].hca_header && am.rows[0].hca_header.data_hex.length / 2 === h.header_size);
      }
    }
    for (const m of s.metadata) {
      if (m.name !== 'VIDEO_SEEKINFO') continue;
      let bad = 0;
      for (const sr of m.rows) if (p.offsets.get(Number(sr.ofs_byte)) !== sr.ofs_frmid) bad++;
      note('seekinfo_ofs_byte_is_offset_of_chunk_ofs_frmid', bad === 0);
      if (probe && probe.codec === 'h264') note('seekinfo_rows_equal_sps_count', m.rows.length === (probe.nal_units_by_type[7] || 0));
      if (probe && probe.codec && probe.codec.startsWith('mpeg')) note('seekinfo_rows_equal_sequence_headers', m.rows.length === (probe.start_codes.sequence_header || 0));
    }
    note('three_section_markers', s.markers.join('|') === '#HEADER END     ===============|#METADATA END   ===============|#CONTENTS END   ===============');
    streams.push(row);
  }
  note('no_header_or_metadata_chunk_after_first_stream_chunk', firstNonStreamAfterStream === 0);
  note('stream_chunk_padding_is_minimal_to_0x20', pad.stream_padding_not_minimal === 0);
  note('all_chunks_0x20_aligned', r.align.chunk_total_mod_0x20_nonzero === 0 && r.align.chunk_offset_mod_0x20_nonzero === 0);

  const exceptionCount = Object.values(r.exceptions).reduce((a, b) => a + b, 0);
  const matches = r.sha256_original === r.sha256_rebuilt && r.rebuilt_size === r.size && r.trailing_bytes === 0;
  return {
    path: rel, size: r.size, sha256: r.sha256_original,
    chunks: r.chunks, chunk_counts: r.chunkCounts,
    leading_chunk_order: order.slice(0, order.findIndex((x) => x.endsWith('/stream')) + 1 || order.length),
    first_stream_chunk_offset: r.firstStreamOffset,
    padding_size_min: r.paddingSizes.min, padding_size_max: r.paddingSizes.max,
    non_stream_chunk_total_sizes: nonStream, non_stream_chunk_fields: nonStreamFields,
    crid_table: r.crid ? r.crid.name : null, crid,
    streams,
    representation_exceptions: r.exceptions, // chunks that needed raw bytes kept instead of a structural description
    structural: exceptionCount === 0,
    checks,
    trailing_bytes: r.trailing_bytes,
    converter: 'tools/usm_extract.js', converter_version: usm.VERSION,
    reversible: matches,
    roundtrip_sha256: r.sha256_rebuilt, roundtrip_matches: matches,
  };
}

function summarise(rows, errors, mask) {
  const tot = {
    files: rows.length + errors.length, parsed: rows.length, roundtrip_ok: 0, roundtrip_fail: 0, errors: errors.length, bytes: 0, chunks: 0,
    chunk_counts: {}, fully_structural_files: 0, representation_exceptions: {}, streams: 0, stream_layouts: {}, codecs: {},
    video_parameters: {}, audio_parameters: {}, masked: {}, hca_cipher_types: {}, elementary_bytes: {}, checks: {},
    video_chunks_below_mask_minimum: 0, largest_video_chunk_left_as_stored: 0, smallest_video_chunk_unmasked: Infinity, padding_size_max: 0, tables: {},
    file_size: { min: Infinity, max: 0 }, chunks_per_file: { min: Infinity, max: 0 }, first_stream_chunk_offset: { min: Infinity, max: 0 },
    leading_chunk_orders: {}, non_stream_chunk_total_sizes: {}, non_stream_chunk_fields: {}, crid_rows_per_file: {}, crid_fields: {}, header_fields: {}, metadata_table_columns: {}, payload_size: {},
    mask_evidence: {
      h264: { streams: 0, bytes: 0, stored_violations: 0, stored_expected_violations_if_random: 0, stored_chunks_without_slice_nal: 0, unmasked_violations: 0, unmasked_chunks_without_slice_nal: 0, unmasked_slice_nals_per_chunk: {}, unmasked_nal_types: {}, sps: {} },
      mpeg: { streams: 0, stored_pictures_found: 0, stored_pictures_with_all_slice_rows: 0, unmasked_pictures: 0, unmasked_pictures_with_all_slice_rows: 0, unmasked_slice_order_errors: 0, stream_chunks: 0, sequence_headers: {} },
      hca: { streams: 0, blocks: 0, blocks_with_sync: 0, blocks_with_valid_crc: 0, headers_with_valid_crc: 0, headers_with_masked_tags: 0, formats: {} },
    },
  };
  const inc = (o, k, n = 1) => { o[k] = (o[k] || 0) + n; };
  const mm = (o, v) => { if (v < o.min) o.min = v; if (v > o.max) o.max = v; };
  // value distribution of a metadata field over all files; collapsed to min/max later if it has many values
  const field = (group, name, v) => { const g = group[name] || (group[name] = {}); inc(g, String(v)); };
  const sumViol = (v) => v.forbidden_zero_bit + v.bad_escape + v.seq_000002 + v.stray_zero_run;
  for (const r of rows) {
    r.roundtrip_matches ? tot.roundtrip_ok++ : tot.roundtrip_fail++;
    tot.bytes += r.size; tot.chunks += r.chunks;
    if (r.structural) tot.fully_structural_files++;
    for (const [k, v] of Object.entries(r.chunk_counts)) inc(tot.chunk_counts, k, v);
    for (const [k, v] of Object.entries(r.representation_exceptions)) inc(tot.representation_exceptions, k, v);
    for (const [k, v] of Object.entries(r.checks)) { const c = tot.checks[k] || (tot.checks[k] = { ok: 0, fail: 0 }); c.ok += v.ok; c.fail += v.fail; }
    inc(tot.stream_layouts, r.streams.map((s) => `${s.signature}:${s.codec}`).join(' + '));
    mm(tot.first_stream_chunk_offset, r.first_stream_chunk_offset); mm(tot.file_size, r.size); mm(tot.chunks_per_file, r.chunks);
    inc(tot.leading_chunk_orders, r.leading_chunk_order.join(' '));
    inc(tot.crid_rows_per_file, r.crid.length);
    for (const [k, g] of Object.entries(r.non_stream_chunk_total_sizes)) for (const [sz, n] of Object.entries(g)) field(tot.non_stream_chunk_total_sizes, k, sz), tot.non_stream_chunk_total_sizes[k][sz] += n - 1;
    for (const [k, n] of Object.entries(r.non_stream_chunk_fields)) inc(tot.non_stream_chunk_fields, k, n);
    r.crid.forEach((c, i) => {
      const who = i === 0 ? 'row0' : Buffer.from([c.stmid >>> 24, (c.stmid >>> 16) & 255, (c.stmid >>> 8) & 255, c.stmid & 255]).toString('latin1');
      for (const [k, v] of Object.entries(c)) field(tot.crid_fields, who + '.' + k, k === 'filename' ? '*' + path.extname(v) : v);
    });
    if (r.crid_table) inc(tot.tables, r.crid_table);
    tot.padding_size_max = Math.max(tot.padding_size_max, r.padding_size_max);
    for (const s of r.streams) {
      tot.streams++;
      inc(tot.codecs, `${s.signature}:${s.codec}`);
      inc(tot.masked, `${s.signature}:${s.masked}`);
      inc(tot.elementary_bytes, s.signature, s.elementary_bytes);
      if (s.header_table) inc(tot.tables, s.header_table);
      for (const m of s.metadata_tables) inc(tot.tables, m.name);
      if (s.chunks_below_mask_minimum) tot.video_chunks_below_mask_minimum += s.chunks_below_mask_minimum;
      if (s.largest_chunk_left_as_stored !== null) tot.largest_video_chunk_left_as_stored = Math.max(tot.largest_video_chunk_left_as_stored, s.largest_chunk_left_as_stored);
      if (s.smallest_chunk_unmasked !== null) tot.smallest_video_chunk_unmasked = Math.min(tot.smallest_video_chunk_unmasked, s.smallest_chunk_unmasked);
      for (const [k, v] of Object.entries(s.header || {})) field(tot.header_fields, s.signature + '.' + k, v);
      field(tot.header_fields, s.signature + '.(chunk header frameRate field of stream chunks)', Object.keys(s.chunk_frame_rate_field).join(','));
      for (const m of s.metadata_tables) inc(tot.metadata_table_columns, s.signature + ' ' + m.name + ': ' + m.columns.join(', '));
      const ps = tot.payload_size[s.signature] || (tot.payload_size[s.signature] = { min: Infinity, max: 0 });
      mm(ps, s.payload_size_min); mm(ps, s.payload_size_max);
      const p = s.bitstream_probe, raw = s.stored_bytes_probe, ev = tot.mask_evidence;
      if (p && p.codec === 'h264') {
        const e = ev.h264; e.streams++; e.bytes += s.elementary_bytes;
        e.unmasked_violations += sumViol(p.annexb_violations); e.unmasked_chunks_without_slice_nal += p.chunks_without_slice_nal;
        inc(e.unmasked_slice_nals_per_chunk, p.slice_nals_per_chunk_min + '..' + p.slice_nals_per_chunk_max);
        inc(e.unmasked_nal_types, Object.keys(p.nal_units_by_type).join(','));
        inc(e.sps, 'profile_idc=' + p.sps.profile_idc + ' level_idc=' + p.sps.level_idc);
        if (raw) { e.stored_violations += sumViol(raw.annexb_violations); e.stored_expected_violations_if_random += raw.expected_violations_if_random; e.stored_chunks_without_slice_nal += raw.chunks_without_slice_nal; }
      } else if (p && (p.codec === 'mpeg1' || p.codec === 'mpeg2')) {
        const e = ev.mpeg; e.streams++; e.stream_chunks += s.stream_chunks;
        e.unmasked_pictures += p.pictures; e.unmasked_pictures_with_all_slice_rows += p.pictures_with_all_slice_rows; e.unmasked_slice_order_errors += p.slice_order_errors;
        inc(e.sequence_headers, p.codec + ' ' + JSON.stringify(p.sequence_header));
        if (raw) { e.stored_pictures_found += raw.pictures; e.stored_pictures_with_all_slice_rows += raw.pictures_with_all_slice_rows; }
      } else if (p && p.codec === 'hca' && p.header) {
        const e = ev.hca, hh = p.header; e.streams++; e.blocks += p.blocks; e.blocks_with_sync += p.blocks_with_sync; e.blocks_with_valid_crc += p.blocks_with_valid_crc;
        if (hh.header_crc_ok) e.headers_with_valid_crc++;
        if (hh.tags_masked) e.headers_with_masked_tags++;
        inc(e.formats, 'version=0x' + hh.version.toString(16) + ' header_size=' + hh.header_size + ' channels=' + hh.channels + ' sample_rate=' + hh.sample_rate + ' block_size=' + hh.block_size + ' cipher_type=' + hh.cipher_type + ' chunks=' + hh.chunks.join('/'));
      }
      const h = s.header;
      if (h && usm.isVideoSig(s.signature)) inc(tot.video_parameters, `${s.signature} ${h.width}x${h.height} disp ${h.disp_width}x${h.disp_height} mpeg_codec=${h.mpeg_codec} mpeg_dcprec=${h.mpeg_dcprec} alpha_type=${h.alpha_type} fps=${h.framerate_n}/${h.framerate_d} ${s.codec}`);
      if (h && s.signature === '@SFA') {
        inc(tot.audio_parameters, `audio_codec=${h.audio_codec} ${h.sampling_rate}Hz ch=${h.num_channels} ambisonics=${h.ambisonics} ${s.codec}`);
        if (s.bitstream_probe && s.bitstream_probe.header) inc(tot.hca_cipher_types, String(s.bitstream_probe.header.cipher_type));
      }
    }
  }
  tot.mask_evidence.h264.stored_expected_violations_if_random = +tot.mask_evidence.h264.stored_expected_violations_if_random.toFixed(1);
  for (const group of [tot.crid_fields, tot.header_fields, tot.non_stream_chunk_total_sizes]) {
    for (const [k, g] of Object.entries(group)) {
      const keys = Object.keys(g);
      if (keys.length > 12 && keys.every((x) => /^-?[0-9]+$/.test(x))) { const n = keys.map(Number); group[k] = { distinct: keys.length, min: Math.min(...n), max: Math.max(...n) }; }
    }
  }
  tot.video_mask = { derived: mask.ok, files_used: mask.files_used, samples: mask.samples, min_peak_ratio: mask.min_peak_ratio, relations_ok: mask.relations_ok, mask_sha256: mask.mask_sha256 };
  return tot;
}

if (process.argv[2] === '--one') {
  const rel = process.argv[3];
  const mask = getMask(listUsm());
  const row = verifyOne(rel, mask.ok ? mask.m2 : null);
  process.stdout.write('RESULT ' + JSON.stringify(row) + '\n');
} else {
  const all = listUsm();
  const only = process.argv[2] === '--match' ? all.filter((p) => p.includes(process.argv[3])) : all;
  const mask = usm.findVideoMask(all.map((f) => path.join(CONFIG.gameRoot, f)));
  console.log(`video mask: ${mask.ok ? 'derived' : 'NOT derived'} from ${mask.files_used} files (${mask.samples} bytes), peak ratio ${mask.min_peak_ratio}, relations ${mask.relations_ok}`);
  const sizeOf = (p) => fs.statSync(path.join(CONFIG.gameRoot, p)).size;
  const queue = only.slice().sort((a, b) => sizeOf(b) - sizeOf(a));
  const rows = [], errors = [];
  let running = 0, done = 0;
  const env = { ...process.env };
  if (mask.ok) env.USM_VIDEO_MASK = mask.m2.toString('hex'); // handed to the workers in memory only; never written to disk
  const finish = () => {
    rows.sort((a, b) => a.path.localeCompare(b.path));
    const totals = summarise(rows, errors, mask);
    if (only.length === all.length) {
      writeJson(OUT, {
        generated_by: 'tools/usm_verify.js', version: VERSION, lib_version: usm.VERSION,
        note: 'roundtrip_matches: the container rebuilt from the parsed representation (chunk descriptors + elementary streams) has the same SHA-256 as the original file. The video mask table is derived from the movies at run time and is not stored; only its SHA-256 is recorded.',
        totals, errors, files: rows,
      });
      logRun({ tool: 'usm_verify', version: VERSION, outputs: [OUT], note: `${totals.roundtrip_ok}/${totals.files} round-trip, ${totals.roundtrip_fail} mismatches, ${errors.length} errors` });
    }
    console.log('\n' + JSON.stringify(totals, null, 1));
    if (errors.length) console.log('ERRORS', JSON.stringify(errors, null, 1));
    if (totals.roundtrip_fail || errors.length) process.exitCode = 1;
  };
  const next = () => {
    while (running < PAR && queue.length) {
      const rel = queue.shift();
      running++;
      const c = spawn(process.execPath, [__filename, '--one', rel], { stdio: ['ignore', 'pipe', 'pipe'], env });
      let out = '', err = '';
      c.stdout.on('data', (d) => (out += d));
      c.stderr.on('data', (d) => (err += d));
      c.on('close', (code) => {
        running--; done++;
        const m = /^RESULT (.*)$/m.exec(out);
        if (code === 0 && m) rows.push(JSON.parse(m[1]));
        else errors.push({ path: rel, error: err.slice(-400) });
        process.stdout.write(`\r${done}/${only.length}  `);
        if (queue.length) next();
        else if (!running) finish();
      });
    }
  };
  if (!queue.length) finish(); else next();
}
