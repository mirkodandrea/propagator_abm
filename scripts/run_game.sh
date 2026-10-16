#!/usr/bin/env bash
# F5 exits with 75: rebuild and reopen at the saved desktop position.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
export KIOSK_WINDOWED=1
export KIOSK_RELOAD=1
export KIOSK_WINDOW_STATE="$root/target/kiosk-window.json"
mkdir -p target
echo "F5: ricompila e riavvia nella stessa posizione. Chiudi la finestra per uscire."
while true; do
  # A failed build leaves the launcher open for a retry after fixing the code.
  if ! cargo build --release -p game --bin game --target-dir "$root/target"; then
    echo "Compilazione fallita. Invio per riprovare, Ctrl+C per uscire."
    read -r || exit 1
    continue
  fi
  result=0
  "$root/target/release/game" "$@" || result=$?
  if [[ "$result" != 75 ]]; then
    exit "$result"
  fi
done
