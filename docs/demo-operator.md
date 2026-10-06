# Demo — operator one-pager

**Launch:** `cargo run --release -p game` from the repository root (or the built
`target/release/game`). It opens full-screen, borderless, on the attract loop.
Everything runs offline.

**Operator corner:** hold the top-right corner of the screen for 3 s. The panel
lets you pin the current town (stop the rotation), skip to the next town, or restart the session. Quit with the OS shortcut (Cmd+Q / Alt+F4).
No keyboard shortcut does anything, by design.

**Reset:** the kiosk returns to the attract screen by itself after 60 s without
input (outside play), or after 3 min idle in play before any order. To test the
reset quickly start with `KIOSK_IDLE_S=10`.

**Frozen screen:** quit with the OS shortcut and relaunch. (A supervisor script
that relaunches on exit is not yet written.)

**Power:** disable sleep and the screensaver on the host.

**The three towns** (placeholder names): *Rocca Ventosa* — a hill village with two
exits and wind pushing the fire uphill; *Due Casali* — two hamlets with a fire
between them and a wind shift forecast; *Porto Pineta* — a coastal town whose one
road closes before the fire arrives.

**Talking points:** the fire goes where the wind blows; people need warning early,
but not too early; the forecast is a forecast and can be wrong; evacuation saves
people, only suppression saves houses.

**Diagnostics (not for visitors):** `KIOSK_FPS=1` logs the frame rate;
`KIOSK_SHOT=<dir>` photographs a session; `KIOSK_WINDOWED=1` runs in a window.
