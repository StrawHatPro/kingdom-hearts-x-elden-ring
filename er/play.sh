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

# me3's launcher (me3-launcher.exe, under Proton) talks to Steam as ELDEN RING: while it runs, Steam shows the game as
# running. When the game dies while me3 starts it, the launcher can wait for it forever (seen on Windows with me3 0.13.0),
# so, as play.bat does through setup\me3_watch.ps1: launchers left over from earlier runs are ended first (only when no
# Elden Ring is running; with one running nothing is started), and a watcher ends this run's launcher once the game has
# been gone 15 s, or when it has run 90 s without the game. The game itself is never touched.
# Only ever ends me3-launcher.exe processes whose program file is directly in THIS me3's folders: the folder of the me3
# picked above, and the folder me3's Linux installer puts its Windows parts in (${XDG_DATA_HOME:-~/.local/share}/me3/
# windows-bin); the program path is read from the process's own command line (Wine shows it as Z:\home\...\me3-launcher.exe)
# and compared as a full folder path. Never anything by name alone, never the game. Every process it ends is printed.
# NOT PERSISTENT: the watcher lives only until the game has gone (at most a day); no autostart, no cron / systemd job,
# no files written, no network, no root.
# (Wine names a process after its .exe, cut to 15 characters: "eldenring.exe", "me3-launcher.ex".)
lower() { tr 'A-Z\\' 'a-z/'; }
ME3_DIRS=$(for d in "$(dirname "$(readlink -f "$ME3" 2>/dev/null || echo "$ME3")")" "${XDG_DATA_HOME:-$HOME/.local/share}/me3/windows-bin"; do
  printf '%s\n' "$d" | lower; done)
game_running() { pgrep -x eldenring.exe >/dev/null 2>&1; }
launcher_pids() {
  for p in $({ pgrep -x me3-launcher.ex; pgrep -f '[m]e3-launcher\.exe'; } 2>/dev/null | sort -u); do
    a0=$(tr '\0' '\n' < "/proc/$p/cmdline" 2>/dev/null | head -n 1 | lower)
    a0=${a0#z:}; dir=${a0%/*}
    [ -n "$a0" ] && [ "${a0##*/}" = me3-launcher.exe ] && printf '%s\n' "$ME3_DIRS" | grep -qxF "$dir" && echo "$p"
  done
}
end_launchers() { # $1: only those at least this many seconds old, $2: what to call them
  for p in $(launcher_pids); do
    age=$(ps -o etimes= -p "$p" 2>/dev/null | tr -d ' ')
    from=$(ps -o lstart= -p "$p" 2>/dev/null)
    if [ -n "$age" ] && [ "$age" -ge "$1" ]; then kill -9 "$p" 2>/dev/null && echo "  ended $2 from $from (pid $p)"; fi
  done
}
if game_running; then
  echo "Elden Ring is already running. Close it first, then start play.sh again."
  exit 2
fi
if [ -n "$(launcher_pids)" ]; then
  echo "me3 launchers left over from earlier runs (they make Steam show Elden Ring as running):"
  end_launchers 30 "a leftover me3 launcher"
fi
(
  trap '' HUP
  seen=; gone=0; t=0
  while [ "$t" -lt 86400 ]; do
    sleep 1; t=$((t + 1))
    if game_running; then seen=1; gone=0; continue; fi
    if [ -z "$(launcher_pids)" ]; then
      if [ -n "$seen" ] || [ "$t" -ge 120 ]; then break; fi
      continue
    fi
    if [ -n "$seen" ]; then
      gone=$((gone + 1))
      if [ "$gone" -ge 15 ]; then
        echo; echo "Elden Ring closed 15 s ago but me3's launcher is still running; ending it so Steam no longer shows the game as running:"
        end_launchers 0 "me3's stuck launcher"; break
      fi
    elif [ "$t" -ge 90 ]; then
      echo; echo "me3's launcher has run 90 s without Elden Ring (the game closed or crashed while starting); ending it:"
      end_launchers 0 "me3's stuck launcher"; break
    fi
  done
) &
echo "Starting Elden Ring with me3 $VER: \"$ME3\""
if "$ME3" launch -g eldenring -p "$HERE/profiles/kh-sora.me3"; then exit 0; fi
echo
echo "me3 could not start the game (its own message is above)."
echo "To pick a different me3, delete er/me3_path.txt and run play.sh again."
exit 1
