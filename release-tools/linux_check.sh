#!/bin/sh
# The Linux setup's end-to-end check on a real Linux (WSL, a Linux PC, a Steam Deck): unpacks a linux-x64 release tarball
# into a new empty folder, runs ./setup-linux there against the games, then compares the pack it made with a reference
# pack built on Windows (release-tools/pack_compare.js, run by the setup's own Node). Launches no game. Not part of a
# release download.
#
#   sh release-tools/linux_check.sh <release .tar.gz> <work folder> <reference build folder> [setup options]
#
# e.g. under WSL, with the games and the Windows clean room reachable under /mnt:
#   sh release-tools/linux_check.sh /mnt/c/.../kingdom-hearts-x-elden-ring-0.2-linux-x64.tar.gz ~/khcheck \
#     "/mnt/c/.../cleanroom/build" --kh3 "/mnt/d/SteamLibrary/steamapps/common/KINGDOM HEARTS III" \
#     --er "/mnt/c/Program Files (x86)/Steam/steamapps/common/ELDEN RING"
# (the work folder needs about 80 GB; on WSL put it in the Linux file system, ~/..., not under /mnt, which is slow)
set -u
TGZ=$1; WORK=$2; REF=$3; shift 3
[ -f "$TGZ" ] || { echo "no $TGZ"; exit 2; }
[ -d "$REF/sora_pack" ] || { echo "no $REF/sora_pack (the reference build folder)"; exit 2; }
if [ -e "$WORK" ] && [ -n "$(ls -A "$WORK" 2>/dev/null)" ]; then echo "$WORK is not empty: give a new folder"; exit 2; fi
mkdir -p "$WORK" && tar -xzf "$TGZ" -C "$WORK" || exit 1
MOD="$WORK/kingdom-hearts-x-elden-ring"
echo "== modes (want 755): $(stat -c '%a %n' "$MOD/setup-linux" "$MOD/er/play.sh" "$MOD/setup/setup.sh" "$MOD/tools/bin/kh-shaderc" "$MOD/tools/bin/kh-oodle" "$MOD/tools/bin/kh-er-selfcheck" | tr '\n' ' ')"
echo "== no flat names: $(find "$MOD" -name '*\\*' | wc -l) file(s) with a backslash in the name"
cd "$MOD" || exit 1
./setup-linux --yes "$@" 2>&1 | tee "$WORK/setup.log"
CODE=$?
echo "== setup exit $CODE"
./setup-linux --run release-tools/pack_compare.js --pack build/sora_pack --ref "$REF/sora_pack" --gen build/gen/rust --ref-gen "$REF/gen/rust" --out "$WORK/pack_compare.json"
echo "== the DLL's self-check (native): $(./tools/bin/kh-er-selfcheck er/mods | tail -n 1)"
echo "== play.sh syntax: $(sh -n er/play.sh && echo ok)"
