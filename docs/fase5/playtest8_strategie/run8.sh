#!/bin/bash
# Playtest 8: strategie per livello, semi 1-3. Solo target/release/rocca.
cd /Users/mirko/dev/experiments/propagator_abm
R=target/release/rocca
ALL=(Castelvento "Pian dei Grilli" Fornaci "San Rocco" "Le Terrazze" "Le Ghiande")
# ordine civile: $1 = minuto, $2 = ordine (preallerta|evacua), $3 = lista quartieri separata da spazi (usa '|' come separatore)
civ() { local out="" t="$1" o="$2"; shift 2; for d in "$@"; do out+="${out:+,}$t:$d=$o"; done; echo "$out"; }
run() { # livello seme etichetta priorita civili
  local lv=$1 s=$2 lab=$3 pr=$4 ci=$5 args=()
  [ -n "$pr" ] && args+=(--priorita "$pr")
  [ -n "$ci" ] && args+=(--civili "$ci")
  echo "### $lv seme $s | $lab | priorita=[$pr] civili=[$ci]"
  $R "$lv" --seme "$s" "${args[@]}" | sed -n '2,$p'
}
for s in 1 2 3; do
for lv in Rocco3 Forna3_gira Coste2_gira; do
  case $lv in
    Rocco3)      near="San Rocco"; best="San Rocco"; inv="Pian dei Grilli"; thr=(Castelvento "San Rocco") ;;
    Forna3_gira) near="Fornaci";   best="Fornaci";   inv="Castelvento";     thr=(Fornaci "San Rocco" "Le Terrazze") ;;
    Coste2_gira) near="Pian dei Grilli"; best="Le Ghiande,Le Terrazze"; inv="Castelvento"; thr=("Pian dei Grilli" "Le Ghiande") ;;
  esac
  EVAC=$(civ 0 evacua "${ALL[@]}")
  PRE=$(civ 0 preallerta "${ALL[@]}")
  others=(); for d in "${ALL[@]}"; do keep=1; for t in "${thr[@]}"; do [ "$d" = "$t" ] && keep=0; done; [ $keep = 1 ] && others+=("$d"); done
  THR0=$(civ 0 evacua "${thr[@]}")
  OTH0=$(civ 0 preallerta "${others[@]}")
  OTH30=$(civ 30 evacua "${others[@]}")
  PA="$PRE,$THR0"
  PB="$PRE,$(civ 30 evacua "${ALL[@]}")"
  MIX="$THR0,$OTH0,$OTH30"
  PE="$(civ 30 evacua "${ALL[@]}")"
  run $lv $s "S1 evacua tutti T+0, nessuna priorita" "" "$EVAC"
  run $lv $s "S2 evacua tutti T+0 + difesa piu vicino" "$near" "$EVAC"
  run $lv $s "S3 solo difesa piu vicino, nessun ordine" "$near" ""
  run $lv $s "S4 misto (evacua minacciati T+0, preallerta altri, evacua altri T+30) + piu vicino" "$near" "$MIX"
  run $lv $s "S5 evacua tutti T+0 + migliore S5 pl7" "$best" "$EVAC"
  run $lv $s "PA preallerta tutti T+0, evacua minacciati T+0 + migliore" "$best" "$PA"
  run $lv $s "PB preallerta tutti T+0, evacua tutti T+30 + migliore" "$best" "$PB"
  run $lv $s "PC evacua tutti T+0 + priorita invertite" "$inv" "$EVAC"
  run $lv $s "PD evacua tutti T+30 + migliore (ritardo)" "$best" "$PE"
  run $lv $s "PE evacua tutti T+0, nessuna priorita (= S1)" "" "$EVAC"
done
done
