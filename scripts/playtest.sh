#!/usr/bin/env bash
# Blind playtest of the kiosk game (docs/demo-spec-playtest.md §2).
#
#   scripts/playtest.sh [--sessions N] [--model M] [--label tag] [--dry-run]
#
# 1. Builds `play` in release.
# 2. Creates a sandbox OUTSIDE the repository ($TMPDIR/rv-playtest-<ts>/) with
#    only the `play` binary (Rocca Ventosa's data is compiled into it) and
#    brief.md.
# 3. Runs a fresh, non-interactive Claude Code session in the sandbox, its
#    tools restricted to `./play` (plus reading brief.md and writing
#    report.md); no --add-dir, cwd is the sandbox, CLAUDE.md / memory / skills
#    / MCP / user settings are all switched off, so no spec is ever loaded.
# 4. Copies report.md to docs/playtests/<date>-<label>.md, prepending the
#    commit and the seeds played (from the sandbox's partita.json, not from the
#    report).
#
# --dry-run builds the sandbox and prints the exact `claude` command without
# running it. The harness refuses to run if this `claude` cannot restrict its
# tools the way it needs.
set -euo pipefail

SESSIONS=3
MODEL=""
LABEL="playtest"
DRY=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --sessions) SESSIONS="$2"; shift 2 ;;
    --model) MODEL="$2"; shift 2 ;;
    --label) LABEL="$2"; shift 2 ;;
    --dry-run) DRY=1; shift ;;
    -h|--help) sed -n '2,22p' "$0"; exit 0 ;;
    *) echo "playtest.sh: unknown argument: $1" >&2; exit 2 ;;
  esac
done
[[ "$SESSIONS" =~ ^[0-9]+$ ]] || { echo "playtest.sh: --sessions takes a number" >&2; exit 2; }
[[ "$LABEL" =~ ^[A-Za-z0-9._-]+$ ]] || { echo "playtest.sh: --label: letters, digits, . _ - only" >&2; exit 2; }

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

die() { echo "playtest.sh: $*" >&2; exit 1; }

# --- 0. can this claude restrict its tools? ---------------------------------------
command -v claude >/dev/null || die "the 'claude' CLI is not on PATH"
HELP="$(claude --help 2>&1)" || die "'claude --help' failed"
for flag in --print --tools --allowedTools --disallowedTools --permission-mode --permission-prompts \
            --restricted --safe-mode --strict-mcp-config --no-session-persistence --disable-slash-commands; do
  grep -q -- "$flag" <<<"$HELP" || die "this claude has no $flag: cannot sandbox the playtester. Refusing to run."
done
grep -q -- "dontAsk" <<<"$HELP" || die "this claude has no 'dontAsk' permission mode: cannot sandbox the playtester. Refusing to run."

# --- 1. build ------------------------------------------------------------------------
(cd "$ROOT" && cargo build --release -p play) || die "cargo build of play failed"
BIN="$ROOT/target/release/play"
[[ -x "$BIN" ]] || die "no play binary at $BIN"

# --- 2. sandbox ------------------------------------------------------------------------
TS="$(date +%Y%m%d-%H%M%S)"
BASE="${TMPDIR:-/tmp}"
SANDBOX="${BASE%/}/rv-playtest-$TS"
case "$SANDBOX" in "$ROOT"/*) die "the sandbox $SANDBOX is inside the repository" ;; esac
mkdir -p "$SANDBOX"
cp "$BIN" "$SANDBOX/play"
cat >"$SANDBOX/brief.md" <<'EOF'
Sei a uno stand della Protezione Civile. Su questo schermo c'è un gioco: un
incendio vicino a un paese, e tu sei il comandante delle operazioni. Prova a
giocare. Si gioca con `./play nuova` e poi seguendo quello che dice lo
schermo (`./play aiuto` se ti perdi).
EOF
# The binary must run from the sandbox with no path into the repository.
(cd "$SANDBOX" && env -u PLAY_DATA ./play aiuto >/dev/null) || die "play does not run from the sandbox"

PROMPT="$(cat <<EOF
Read brief.md in this directory: it is all you know about this game. The game is in Italian; these instructions are in English.

1. Play in character first. You are a 16-year-old student who just walked up to the stand: curious, impatient, no firefighting knowledge. Play $SESSIONS sessions using only ./play commands (use ./play riprova at least once, and ./play nuova for a different fire). While playing, keep a short think-aloud log per turn: what you think is happening, what you are trying, what confuses you.

2. Then step out and review as a game designer. Write report.md in this directory, in English, with these sections:
   - Sessions: per session, the orders given per turn and the verdict, one line each.
   - Usability (1-5): could you tell what to do, what each resource does, what happened and why? Quote the exact screen text that confused you.
   - Fun (1-5) and Interest (1-5): which moments were engaging, which dull; would a student play a second time, and why.
   - What the game taught you: in your own words, unprompted, 3-6 sentences.
   - Decisions: which choices felt real, which felt obvious or fake (a resource you never wanted, an order that always/never works).
   - Suggested changes: up to 7, each with the problem it fixes and how much it would change the game (small / medium / large), most valuable first.
   - Bugs: anything that looked wrong, with the command that showed it.
   Include your think-aloud log at the end, under "Log".

3. Do not try to read files other than brief.md and your own report.md, or to find out how the game is made. Judge only what a player sees.
EOF
)"

CMD=(claude -p "$PROMPT"
  --restricted
  --safe-mode
  --strict-mcp-config
  --disable-slash-commands
  --no-session-persistence
  --tools "Bash,Read,Write,Edit"
  --allowedTools "Bash(./play:*)" "Read(./brief.md)" "Read(./report.md)" "Write(./report.md)" "Edit(./report.md)"
  --disallowedTools "Bash(./play stato:*)" "Read(./partita.json)" "Read(./play)"
  --permission-mode dontAsk
  --permission-prompts none)
if [[ -n "$MODEL" ]]; then
  CMD+=(--model "$MODEL")
fi

if [[ $DRY -eq 1 ]]; then
  echo "sandbox: $SANDBOX"
  ls -la "$SANDBOX"
  echo
  echo "would run, in $SANDBOX:"
  printf 'cd %q && ' "$SANDBOX"
  printf '%q ' "${CMD[@]}"
  echo
  exit 0
fi

# --- 3. the playtest ------------------------------------------------------------------
echo "playtest.sh: playing in $SANDBOX (this takes a while)" >&2
(cd "$SANDBOX" && env -u PLAY_DATA "${CMD[@]}") >"$SANDBOX/transcript.txt" 2>&1 || die "the claude session failed: see $SANDBOX/transcript.txt"
[[ -s "$SANDBOX/report.md" ]] || die "no report.md was written: see $SANDBOX/transcript.txt"

# --- 4. file the report ------------------------------------------------------------------
mkdir -p "$ROOT/docs/playtests"
OUT="$ROOT/docs/playtests/$(date +%F)-$LABEL.md"
[[ -e "$OUT" ]] && OUT="$ROOT/docs/playtests/$(date +%F)-$LABEL-$TS.md"
COMMIT="$(cd "$ROOT" && git rev-parse --short HEAD)$(cd "$ROOT" && git diff --quiet HEAD -- . || echo '+dirty')"
SEEDS="$(python3 -c 'import json,sys; s=json.load(open(sys.argv[1])).get("semi",[]); print(", ".join(str(x) for x in dict.fromkeys(s)) or "none")' "$SANDBOX/partita.json" 2>/dev/null || echo "unknown (no partita.json)")"
{
  echo "<!-- playtest.sh: commit $COMMIT · play seeds: $SEEDS · sessions asked: $SESSIONS · model: ${MODEL:-default} · sandbox: $SANDBOX -->"
  echo
  echo "Commit \`$COMMIT\` · seeds played: $SEEDS"
  echo
  cat "$SANDBOX/report.md"
} >"$OUT"
echo "playtest.sh: report filed at $OUT" >&2
