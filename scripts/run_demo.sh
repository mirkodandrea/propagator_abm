#!/usr/bin/env bash
# Kiosk supervisor: run the demo full-screen and relaunch it if it dies.
# A deliberate quit from the operator panel exits 0 and stops the loop;
# anything else (a crash, a GPU reset) relaunches after a short pause.
#
#   scripts/run_demo.sh            # build once, then run forever
#   KIOSK_TOWN=porto scripts/run_demo.sh
set -u
cd "$(dirname "$0")/.."
cargo build --release -p game || exit 1
while true; do
    ./target/release/game
    code=$?
    if [ "$code" -eq 0 ]; then
        echo "kiosk: operator quit, supervisor stopping"
        exit 0
    fi
    echo "kiosk: exited with $code at $(date '+%H:%M:%S'), relaunching in 3 s" >&2
    sleep 3
done
