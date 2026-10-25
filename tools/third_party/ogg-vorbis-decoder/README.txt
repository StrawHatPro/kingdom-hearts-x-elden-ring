ogg-vorbis-decoder.min.js
  npm package @wasm-audio-decoders/ogg-vorbis 0.1.20 (dist/ogg-vorbis-decoder.min.js), unmodified.
  Copyright 2021-2025 Ethan Halsall, https://github.com/eshaz/wasm-audio-decoders (package license: MIT).
  The bundle embeds libvorbis/libogg (Xiph.Org, BSD-style) compiled to WebAssembly and codec-parser
  (LGPL-3.0, text in LICENSE.codec-parser). No native code; runs in plain Node.
  sha256 158dd1bd7c676b3859b3134878b8df5d37e9ebe77e3bcfd57186bdd5d5f6d904
  Fetched 2026-10-11 with: npm install --ignore-scripts @wasm-audio-decoders/ogg-vorbis
Used by tools/lib/vorbis.js to decode the 310 Ogg Vorbis materials (SEAD codec 3) to PCM.
The bundle asks for the module "@eshaz/web-worker" (only needed by its worker class); the loader
passes a stub.

In this repository the bundle is NOT included: setup/setup.js copies it from node_modules after `npm install`
(package @wasm-audio-decoders/ogg-vorbis 0.1.20, MIT; its licence texts come with the npm package).
