#!/bin/bash
B="$(cd "$(dirname "$0")" && pwd)"
D="$B/p_prima_volta"; mkdir -p "$D"
cat "$B/persona.md" "$B/common.md" > "$D/prompt.md"
echo "$(date +%H:%M) start" >> "$B/progress.log"
(cd "$D" && codex exec -m gpt-6.1-sol --sandbox workspace-write --skip-git-repo-check "$(cat prompt.md)" < /dev/null > codex.log 2>&1)
echo "$(date +%H:%M) done exit $? $( [ -f "$D/report.md" ] && echo report-ok || echo NO-REPORT )" >> "$B/progress.log"
