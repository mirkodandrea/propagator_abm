#!/bin/bash
# Semi extra 4-8 per confronto di strategie chiave (non richiesti, per ridurre il rumore)
cd /Users/mirko/dev/experiments/propagator_abm
R=target/release/rocca
ALL=(Castelvento "Pian dei Grilli" Fornaci "San Rocco" "Le Terrazze" "Le Ghiande")
civ() { local out="" t="$1" o="$2"; shift 2; for d in "$@"; do out+="${out:+,}$t:$d=$o"; done; echo "$out"; }
EVAC=$(civ 0 evacua "${ALL[@]}"); PRE=$(civ 0 preallerta "${ALL[@]}"); PE=$(civ 30 evacua "${ALL[@]}")
for s in 4 5 6 7 8; do
for cfg in "Rocco3|San Rocco|Castelvento,San Rocco" "Forna3_gira|Fornaci|Fornaci,San Rocco,Le Terrazze" "Coste2_gira|Le Ghiande,Le Terrazze|Pian dei Grilli,Le Ghiande"; do
  IFS='|' read lv best thrs <<< "$cfg"
  IFS=',' read -ra T <<< "$thrs"
  THR0=$(civ 0 evacua "${T[@]}")
  others=(); for d in "${ALL[@]}"; do k=1; for t in "${T[@]}"; do [ "$d" = "$t" ] && k=0; done; [ $k = 1 ] && others+=("$d"); done
  PA="$PRE,$THR0"; PB="$PRE,$PE"; MIX="$THR0,$(civ 0 preallerta "${others[@]}"),$(civ 30 evacua "${others[@]}")"
  for spec in "S1|$EVAC|" "S5|$EVAC|$best" "S3|| $best" "PA|$PA|$best" "PB|$PB|$best" "PD|$PE|$best" "S4|$MIX|$best"; do
    IFS='|' read lab ci pr <<< "$spec"
    pr=${pr# }
    args=(); [ -n "$pr" ] && args+=(--priorita "$pr"); [ -n "$ci" ] && args+=(--civili "$ci")
    echo "### $lv seme $s | $lab | priorita=[$pr] civili=[$ci]"
    $R "$lv" --seme "$s" "${args[@]}" | sed -n '2,$p'
  done
done
done
