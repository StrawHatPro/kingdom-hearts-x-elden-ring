#!/bin/sh
# Kingdom Hearts x Elden Ring: the first-run setup on Linux (the counterpart of setup\setup.bat). In the linux-x64 release
# download, run ./setup-linux in the mod's folder; this file runs it too. Options are passed on, e.g.
#   setup/setup.sh --kh3 "$HOME/.local/share/Steam/steamapps/common/KINGDOM HEARTS III"
# When a game is not found, a folder picker opens (zenity or kdialog, on a desktop), else you are asked for the folder.
# Node.js is never needed beforehand:
#   0. setup-linux (the release download: Node.js built in, as a single executable application) runs the setup;
#   1. else tools/bin/node/bin/node (the portable Node a source checkout downloaded before, see 3);
#   2. else a Node.js 20 or newer on PATH (a source checkout);
#   3. else the pinned portable Node.js (release-tools/node_pin.json linux_x64, from nodejs.org, checked by its SHA-256) is
#      downloaded into tools/bin/node (setup/get_node.sh); not with --no-download.
# The npm packages come bundled in a release download; a source checkout installs the locked versions once (npm ci).
cd "$(dirname "$0")/.." || exit 1
export KH_SETUP_PICKER=1
export KH_SETUP_NO_PAUSE=1
if [ -f ./setup-linux ]; then
  [ -x ./setup-linux ] || chmod +x ./setup-linux 2>/dev/null
  exec ./setup-linux "$@"
fi
NODL=
for a in "$@"; do [ "$a" = "--no-download" ] && NODL=1; done
NODE=
NPM=
if [ -x tools/bin/node/bin/node ]; then
  NODE="$PWD/tools/bin/node/bin/node"
  NPM="$NODE $PWD/tools/bin/node/lib/node_modules/npm/bin/npm-cli.js"
else
  MAJOR=$(node -p "process.versions.node.split('.')[0]" 2>/dev/null)
  if [ -n "$MAJOR" ] && [ "$MAJOR" -ge 20 ] 2>/dev/null; then NODE=node; NPM=npm; fi
fi
if [ -z "$NODE" ]; then
  if [ -n "$NODL" ]; then
    echo "No Node.js 20 or newer was found, and --no-download was given: run without --no-download, or install Node.js 20+."
    exit 1
  fi
  echo "Getting the portable Node.js the mod uses, into tools/bin/node ..."
  if ! sh setup/get_node.sh; then
    echo "Could not get Node.js: check the connection, or install Node.js 20+ and run this again."
    exit 1
  fi
  NODE="$PWD/tools/bin/node/bin/node"
  NPM="$NODE $PWD/tools/bin/node/lib/node_modules/npm/bin/npm-cli.js"
fi
if [ ! -f node_modules/gltf-validator/package.json ] || [ ! -f node_modules/iced-x86/package.json ] || [ ! -f node_modules/ajv/package.json ] || [ ! -f node_modules/@wasm-audio-decoders/ogg-vorbis/package.json ]; then
  if [ -n "$NODL" ]; then
    echo "The npm packages are missing and --no-download was given: use a release download, or run without --no-download."
    exit 1
  fi
  echo "Installing the locked npm packages (npm ci) ..."
  # (shell word splitting: "node npm-cli.js" for the portable Node; paths with spaces are passed through PATH instead)
  if [ "$NPM" = npm ]; then npm ci --omit=dev --no-fund --no-audit || { echo "npm ci failed."; exit 1; }
  else "$NODE" "$PWD/tools/bin/node/lib/node_modules/npm/bin/npm-cli.js" ci --omit=dev --no-fund --no-audit || { echo "npm ci failed."; exit 1; }
  fi
fi
exec "$NODE" setup/setup.js "$@"
