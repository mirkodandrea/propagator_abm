#!/bin/bash
B="$(cd "$(dirname "$0")" && pwd)"
for p in q1_nuovo q2_kid q3_minmax q4_ux; do
  D="$B/$p"; mkdir -p "$D"
  cat "$B/$p.md" "$B/common.md" > "$D/prompt.md"
  echo "$(date +%H:%M) start $p" >> "$B/progress.log"
  (cd "$D" && codex exec -m gpt-6.1-sol --sandbox workspace-write --skip-git-repo-check "$(cat prompt.md)" < /dev/null > codex.log 2>&1)
  echo "$(date +%H:%M) done $p exit $? $( [ -f "$D/report.md" ] && echo report-ok || echo NO-REPORT )" >> "$B/progress.log"
done
echo "ALL DONE" >> "$B/progress.log"
