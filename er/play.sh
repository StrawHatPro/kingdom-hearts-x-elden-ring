#!/bin/sh
# Kingdom Hearts x Elden Ring: starts Elden Ring through me3 with the mod's profile, on Linux (the counterpart of
# er\play.bat). me3's official Linux build (tested: 0.13.0; its installer: curl --proto '=https' --tlsv1.2 -sSfL
# https://github.com/garyttierney/me3/releases/download/v0.13.0/installer.sh | sh, which puts me3 in ~/.local/bin and its
# Windows parts in ~/.local/share/me3/windows-bin) starts the game through Steam: it uses the Proton (compatibility tool)
# Steam is set to use for ELDEN RING, else Steam's global default, else the one Steam Deck verifies, in the game's own
# Proton prefix (steamapps/compatdata/1245620), inside the Steam Linux Runtime. Run the game once from Steam first.
# OFFLINE ONLY (no Easy Anti-Cheat, no online play). Steam must be running.
# me3 is looked for in: the place remembered in er/me3_path.txt, PATH, the usual install folders (~/.local/bin, where
# me3's installer puts it); if it is still not found, a window (zenity / kdialog) or the terminal asks for it once and
# remembers it.
# (play.bat's administrator checks have no Linux counterpart: Linux has no "always run as administrator" flag, and the
# game is started by your own Steam. Steam refuses to run as root, so this refuses to run as root too.)
cd "$(dirname "$0")" || exit 1
HERE=$(pwd)
if [ ! -f "$HERE/mods/kh_er_loader.dll" ]; then
  echo "The mod is not set up yet: run ./setup-linux (or setup/setup.sh) first."
  exit 1
fi
if [ "$(id -u)" = 0 ]; then
  echo "Do not run play.sh as root (or with sudo): Steam does not run as root, and the game must be started by your own Steam."
  exit 1
fi

SAVED="$HERE/me3_path.txt"
ME3=
[ -f "$SAVED" ] && ME3=$(head -n 1 "$SAVED" | tr -d '\r')
if [ -z "$ME3" ] || [ ! -f "$ME3" ]; then
  ME3=$(command -v me3 2>/dev/null)
  if [ -z "$ME3" ]; then
    for c in "$HOME/.local/bin/me3" "${XDG_DATA_HOME:-$HOME/.local/share}/me3/bin/me3" "$HOME/me3/bin/me3"; do
      if [ -z "$ME3" ] && [ -f "$c" ]; then ME3=$c; fi
    done
  fi
  if [ -z "$ME3" ]; then
    echo "me3 was not found. Pick the me3 program (in the \"bin\" folder of wherever you unpacked or installed me3)."
    if { [ -n "$DISPLAY" ] || [ -n "$WAYLAND_DISPLAY" ]; } && command -v zenity >/dev/null 2>&1; then
      ME3=$(zenity --file-selection --title="Find me3 (Mod Engine 3)" --filename="$HOME/" 2>/dev/null)
    elif { [ -n "$DISPLAY" ] || [ -n "$WAYLAND_DISPLAY" ]; } && command -v kdialog >/dev/null 2>&1; then
      ME3=$(kdialog --getopenfilename "$HOME" --title "Find me3 (Mod Engine 3)" 2>/dev/null)
    elif [ -t 0 ]; then
      printf 'The full path of me3 (empty = stop): '
      read -r ME3
    fi
  fi
  if [ -z "$ME3" ] || [ ! -f "$ME3" ]; then
    echo "No me3 was picked. Get me3 from https://github.com/garyttierney/me3/releases and run play.sh again."
    exit 1
  fi
  printf '%s\n' "$ME3" > "$SAVED" 2>/dev/null
fi
[ -x "$ME3" ] || chmod +x "$ME3" 2>/dev/null

# me3 older than 0.13.0 may ignore the profile's own save file and start the game on your NORMAL save (ER0000.sl2)
VER=$("$ME3" --version 2>/dev/null | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -n 1)
MAJ=${VER%%.*}
REST=${VER#*.}
MIN=${REST%%.*}
OK=
if [ -n "$VER" ]; then
  if [ "$MAJ" -gt 0 ] 2>/dev/null; then OK=1; elif [ "$MIN" -ge 13 ] 2>/dev/null; then OK=1; fi
fi
if [ -z "$OK" ]; then
  echo
  echo "STOPPED: your me3 is too old (version: ${VER:-unknown}). The mod needs me3 0.13.0 or newer."
  echo "An older me3 can ignore the mod's own save file and open your NORMAL Elden Ring saves."
  echo "Update me3 from https://github.com/garyttierney/me3/releases and run play.sh again."
  echo "(me3 used: \"$ME3\". To pick a different one, delete er/me3_path.txt.)"
  exit 1
fi

echo "Starting Elden Ring with me3 $VER: \"$ME3\""
if "$ME3" launch -g eldenring -p "$HERE/profiles/kh-sora.me3"; then exit 0; fi
echo
echo "me3 could not start the game (its own message is above)."
echo "To pick a different me3, delete er/me3_path.txt and run play.sh again."
exit 1
