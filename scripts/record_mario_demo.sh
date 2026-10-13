#!/bin/sh
# Reproduce the scripted fire-front demo at a deterministic 30 frames/second.
set -eu
cd "$(dirname "$0")/.."
output="${1:-out/mario_fire_demo}"
mkdir -p "$output/frames"
if test -e "$output/frames/frame_00000.png"; then
    echo "Choose a fresh output folder to avoid mixing recordings." >&2
    exit 1
fi
cargo build -p game --bin game
MARIO_DEMO=1 KIOSK_WINDOWED=1 KIOSK_SCALE=1 KIOSK_SIZE=1280x720 \
    KIOSK_SNAP="$output/frames" \
    KIOSK_SNAP_SPEC='name=demo,case=Rocco3,t=2,phase=pianifica,plan=none,at=fire,mario=1' \
    target/debug/game
ffmpeg -y -framerate 30 -i "$output/frames/frame_%05d.png" \
    -c:v libx264 -preset medium -crf 19 -pix_fmt yuv420p \
    -movflags +faststart "$output/mario_fire_demo.mp4"
