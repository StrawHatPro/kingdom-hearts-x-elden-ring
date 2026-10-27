#!/bin/sh
# Kingdom Hearts x Elden Ring: downloads the pinned portable Node.js for Linux (release-tools/node_pin.json linux_x64: the
# official linux-x64 tarball of nodejs.org, checked by its SHA-256 from that release's SHASUMS256.txt) into tools/bin/node.
# The counterpart of setup\get_node.ps1; used by setup/setup.sh in a source checkout that has no Node 20 or newer (a
# release download has Node built into setup-linux and never runs this).
#   sh setup/get_node.sh
# Exit code 0 = tools/bin/node/bin/node is the pinned Node.
cd "$(dirname "$0")/.." || exit 1
PIN=release-tools/node_pin.json
# the linux_x64 block's values (one "key": "value" per line in the file)
val() { sed -n '/"linux_x64"/,/}/s/^ *"'"$1"'": *"\([^"]*\)".*/\1/p' "$PIN" | head -n 1; }
URL=$(val tgz_url); SUM=$(val tgz_sha256); FOLDER=$(val tgz_folder); NODE_SUM=$(val node_sha256)
if [ -z "$URL" ] || [ -z "$SUM" ] || [ -z "$FOLDER" ] || [ -z "$NODE_SUM" ]; then echo "   $PIN has no linux_x64 pin"; exit 1; fi
case "$(uname -m)" in x86_64|amd64) ;; *) echo "   this Node.js is for x86-64 (linux-x64); this machine is $(uname -m)"; exit 1 ;; esac
sha() { if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi; }
BIN=tools/bin
DEST=$BIN/node
EXE=$DEST/bin/node
if [ -f "$EXE" ] && [ "$(sha "$EXE")" = "$NODE_SUM" ]; then echo "   Node: $EXE (already there)"; exit 0; fi
mkdir -p "$BIN" || exit 1
TGZ=$BIN/$FOLDER.tar.gz
echo "   downloading Node.js (portable, official): $URL"
if command -v curl >/dev/null 2>&1; then curl -fsSL --proto '=https' -o "$TGZ" "$URL" || { echo "   the download failed"; rm -f "$TGZ"; exit 1; }
elif command -v wget >/dev/null 2>&1; then wget -q -O "$TGZ" "$URL" || { echo "   the download failed"; rm -f "$TGZ"; exit 1; }
else echo "   neither curl nor wget is installed"; exit 1; fi
GOT=$(sha "$TGZ")
if [ "$GOT" != "$SUM" ]; then rm -f "$TGZ"; echo "   the download is not the expected file (SHA-256 $GOT, expected $SUM)"; exit 1; fi
TMP=$BIN/node_unpack
rm -rf "$TMP" && mkdir -p "$TMP" || exit 1
tar -xzf "$TGZ" -C "$TMP" || { echo "   could not unpack $TGZ"; rm -rf "$TMP" "$TGZ"; exit 1; }
rm -rf "$DEST" && mv "$TMP/$FOLDER" "$DEST" && rm -rf "$TMP" "$TGZ"
if [ "$(sha "$EXE")" != "$NODE_SUM" ]; then echo "   bin/node is not the expected file"; exit 1; fi
echo "   Node: $EXE"
exit 0
