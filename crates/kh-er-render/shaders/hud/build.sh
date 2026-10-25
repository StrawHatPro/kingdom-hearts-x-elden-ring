#!/bin/sh
# Rebuilds the embedded shaders of the HUD overlay (run after editing hud.hlsl).
DXC="/c/Program Files (x86)/Windows Kits/10/bin/10.0.26100.0/x64/dxc.exe"
cd "$(dirname "$0")"
"$DXC" -T vs_6_0 -E vs -Fo hud.vs.dxil hud.hlsl && "$DXC" -T ps_6_0 -E ps -Fo hud.ps.dxil hud.hlsl && ls -la *.dxil
