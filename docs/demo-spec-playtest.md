# Demo — playtest spec (`play` binary, ASCII screen, blind playtester)

Read `docs/demo-spec.md` first. This spec is for the builder and the lead; the
**playtester never reads it** (nor any other file in this repository).

Legend: ✅ done · 🔶 partly · 🔲 to do.

## 1. The `play` binary

A headless front end on `demo::Referee` that plays exactly the kiosk's game,
one command per kiosk interaction, with the kiosk's own Italian text
(`crates/text`). Crate `crates/play`, binary `play`; no Bevy, no window.

### 1.1 Commands (one per kiosk gesture)

| Command | Kiosk gesture | Prints |
|---|---|---|
| `./play nuova [--seme N]` | click on the attract screen | the opening screen (turn 1). Without `--seme` the seed is drawn and **not shown** |
| `./play mostra` | looking at the screen | the current screen |
| `./play scegli <risorsa>` | click a resource card | the screen with the valid targets for it lit, each with its preview (*arriva in 4′ · difende 60 case*) |
| `./play ordina <risorsa> <bersaglio>` | click a target | the screen with the pending order |
| `./play annulla <risorsa>` | click the card again | the screen |
| `./play avanti` | *Avanti* | the turn playing (two intermediate map frames), then the report, then the next turn's screen; after turn 5 the finale (one frame per stamp) and the verdict |
| `./play riprova` | *Riprova* | turn 1 of the same fire |
| `./play aiuto` | — | the commands, in Italian, in five lines |

`<risorsa>` is the short id printed in the tray (`P`, `I`, `E1`, `E2`, `E3`,
`S`, `K`); `<bersaglio>` is the target number printed on the map. Unknown or
invalid input prints one Italian line saying what is valid — never a stack
trace, never English.

### 1.2 State

The session is a file `partita.json` in the working directory: seed, draw and
the order log by turn. Every command **replays** it deterministically through
`demo::Referee` (same seed + draw + orders ⇒ same game, gameplay §9), so there
is no hidden server. Replay of a full session must take < 2 s in release.

`./play stato --json` dumps the full state for the builder's tests; it is not
in `aiuto` and the playtest harness does not allow it (§3).

### 1.3 The screen

Plain text, ≤ 100 columns, no colour codes. Layout, top to bottom:

```
ROCCA VENTOSA · Turno 2 di 5 · T+08
Vento: da Sud, 35 km/h — spinge il fuoco verso Nord
Meteo: il vento potrebbe girare e soffiare da Est (probabile, 60 %), tra T+12 e T+26

     0 km           1              2              3              4
   N +----------------------------------------------------------------+
     |""""""""""""""....=....CCCC.........................""""""""""""|
     |"""""""""""".......=...CCCC....................."""""""""""""""|
     |......MMM...........=.....................BBBBBBBBB.............|
     |......MMM....A.......=====================BBBB[1]BBB.............|
     |.....................=......7.............BBBBBBBBB.....E1......|
     |""""""""""""""""""xxxxxx[5]**********[4]"""""""""""""""""""""""""|
     ...
     +----------------------------------------------------------------+
Legenda: " bosco/macchia  . campi  = strada  ~ acqua  * fuoco  x bruciato
         B Il Borgo  C Le Coste  M Il Mulino  A area di attesa
         [n] bersaglio  P pattuglia  E autobotte  S squadra  K canadair

QUARTIERI
  [1] Il Borgo   148 famiglie · fuoco a 700 m · MINACCIATO · avvisato: 90 in viaggio, 40 al sicuro
  [2] Le Coste    64 famiglie · fuoco a 1,4 km · attenzione
  [3] Il Mulino   38 famiglie · fuoco a 1,9 km · tranquillo

RISORSE
  P   Pattuglia     libera
  I   IT-alert      1 uso
  E1  Autobotte 1   al lavoro a Il Borgo · acqua 40 %
  E2  Autobotte 2   libera
  S   Squadra AIB   libera
  K   Canadair      non chiamato

BERSAGLI  [4] Testa  [5] Fianco ovest  [6] Fianco est  [7] Focolaio

RAPPORTO (turno 1)
  · Il Borgo: 90 famiglie in viaggio verso l'area di attesa.
  · Autobotte 1 arrivata a Il Borgo.

> ./play scegli <risorsa> · ./play ordina <risorsa> <bersaglio> · ./play avanti
```

Map rules:
- The 4 km window at **64 × 32** characters (62.5 m × 125 m per character);
  north up; a km ruler on top.
- One glyph per cell. Precedence: unit > target number > fire `*` > burnt `x` >
  houses (district letter) > area di attesa `A` > road `=` > water `~` >
  vegetation `"` > open `.`.
- Wind and forecast are **text** (the kiosk draws them as an arrow and a cone);
  the direction words are the same in both.
- Target numbers sit on the map where the kiosk's target markers sit. A
  district's target is its fire-facing post.
- After `scegli`, the lit targets are listed under the map with their preview;
  unlit ones are shown as `[n]` but not listed.

`avanti` prints two map frames during the turn (T+k+4, T+k+8), then the
report, then the next screen — the textual equivalent of watching the turn
play.

### 1.4 Verdict screen

Headline; families safe and homes hit, each against *senza ordini*; one row per
district with its stamp; ≤ 3 rule notes; `./play riprova`. Exactly the kiosk's
words (presentation §5).

## 2. `scripts/playtest.sh`

```sh
scripts/playtest.sh [--sessions N] [--model M] [--label tag]
```

1. Builds `play` in release.
2. Creates a sandbox **outside the repository** (`$TMPDIR/rv-playtest-<ts>/`)
   containing only the `play` binary (data embedded or under the sandbox — no
   path into the repo) and `brief.md` (§3).
3. Runs a fresh, non-interactive Claude Code session in the sandbox:
   `claude -p` with the prompt in §3, tools restricted to `Bash(./play:*)`,
   reading `brief.md`, and writing `report.md` in the sandbox. No access to the
   repository: no `--add-dir`, cwd is the sandbox, so no `CLAUDE.md`, memory or
   spec is loaded.
4. Copies `report.md` to `docs/playtests/<date>-<label>.md`, prepending the
   commit hash and the `play` seeds used (from the sandbox's `partita.json`
   history, not from the report).

Check before the first round: `claude --help` for the exact flags that restrict
tools in this version; the harness fails loudly if it cannot restrict them.

## 3. The playtester

**Knows:** the brief and the game's own screens. **Does not know:** that this
is an agent-built project, the specs, the lessons, the model, the findings,
previous reports.

`brief.md` (Italian, as a stand volunteer would say it):

> Sei a uno stand della Protezione Civile. Su questo schermo c'è un gioco: un
> incendio vicino a un paese, e tu sei il comandante delle operazioni. Prova a
> giocare. Si gioca con `./play nuova` e poi seguendo quello che dice lo
> schermo (`./play aiuto` se ti perdi).

Prompt (English is fine for the instructions to the agent; the game is
Italian):

1. **Play in character first.** You are a 16-year-old student who just walked
   up to the stand; curious, impatient, no firefighting knowledge. Play
   `--sessions` sessions (default 3; use `./play riprova` at least once, and
   `./play nuova` for a different fire). While playing, keep a short
   think-aloud log per turn: what you think is happening, what you are trying,
   what confuses you.
2. **Then step out and review as a game designer.** Write `report.md`:
   - **Sessions:** per session, the orders given per turn and the verdict, one
     line each.
   - **Usability** (1–5): could you tell what to do, what each resource does,
     what happened and why? Quote the exact screen text that confused you.
   - **Fun** (1–5) and **interest** (1–5): which moments were engaging, which
     dull; would a student play a second time, and why.
   - **What the game taught you:** in your own words, unprompted, 3–6 sentences.
   - **Decisions:** which choices felt real, which felt obvious or fake (a
     resource you never wanted, an order that always/never works).
   - **Suggested changes:** up to 7, each with the problem it fixes and how
     much it would change the game (small / medium / large), most valuable first.
   - **Bugs:** anything that looked wrong, with the command that showed it.
3. Do not try to read files other than `brief.md` and your own `report.md`, or
   to find out how the game is made. Judge only what a player sees.

## 4. Triage (lead + Mirko)

For each report: compare *what the game taught you* against demo-spec §1
(count lessons stated, and anything wrong it learnt); file each suggested change
as **accept → spec edit**, **reject (reason)** or **measure first (sweep)**;
append the decision to the report's file under `## Triage`. The builder works
only from the edited specs.
