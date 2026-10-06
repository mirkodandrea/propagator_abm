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

**How a session goes (~3½ min):** the briefing stops the clock — the visitor
reads the wind arrow and the forecast and gives first orders from the chips over
each district (*Avvisa* warns that district, *Difendi* sends a fire engine to
protect its homes), then presses *Via!*. The fire runs ~2½ min; advisors comment;
*Veloce* fast-forwards. The end panel tells each district's story, compares with
"no orders" on the same fire, and gives up to three medals.

**The three towns** (placeholder names), three districts each: *Rocca Ventosa* —
the wind drives the fire up at Il Borgo; if it turns, Le Coste is next; Il
Mulino is never in danger. *Due Casali* — a car fire between two hamlets; the
west one first, the east one if the wind swings; Fondovalle is safe.
*Porto Pineta* — the pines behind town; La Pineta first, then the Centro; the
seafront is the assembly area.

**Talking points:** the fire goes where the wind blows; warn early (preparing to
leave takes time); warn who is at risk, not everyone — a false alarm costs
credibility; the forecast is odds, not a verdict, and acting on it early is the
point; evacuation saves people, fire engines save houses, and only where the fire
is going; the *area di attesa* (green sign, blue tents) is where people go.

**Diagnostics (not for visitors):** `KIOSK_FPS=1` logs the frame rate;
`KIOSK_SHOT=<dir>` photographs a session; `KIOSK_WINDOWED=1` runs in a window.
