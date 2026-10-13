#!/bin/bash
# Playtest headless (Haiku 5.5): strategie x livelli x semi 1-3.
# Uso: bash docs/fase6/playtest_haiku_headless/run.sh   (dalla radice del repo)
set -u
R=target/release/rocca
D=docs/fase6/playtest_haiku_headless
OUT=$D/raw.txt
# Uso: run.sh          -> tutte le strategie (azzera raw.txt)
#      run.sh extra    -> solo il blocco aggiuntivo s12-s13 (accoda a raw.txt)
MODE=${1:-all}
[ "$MODE" = all ] && : > "$OUT"

PRI_NEAR="San Rocco,Castelvento,Le Terrazze,Fornaci,Le Ghiande,Pian dei Grilli"
PRI_FAR="Pian dei Grilli,Le Ghiande,Fornaci,Le Terrazze,Castelvento,San Rocco"

ALL=("Castelvento" "Pian dei Grilli" "Fornaci" "San Rocco" "Le Terrazze" "Le Ghiande")
civ() { # civ <minuto> <stato>  -> "min:P=stato,..." per tutti i paesi
  local m=$1 s=$2 out=""
  for p in "${ALL[@]}"; do out+="${m}:${p}=${s},"; done
  echo "${out%,}"
}
PRE_ALL_T0="$(civ 0 p),$(civ 0 e)"
EVAC_ALL_T0="$(civ 0 e)"
PRE_ALL_T0_ONLY="$(civ 0 p)"
PRE_ALL_EVAC_T30="$(civ 0 p),$(civ 30 e)"
PRE_EVAC_TWO="0:Castelvento=p,0:Castelvento=e,0:San Rocco=p,0:San Rocco=e"

run() { # run <livello> <seme> <nome> [argomenti...]
  local L=$1 S=$2 N=$3; shift 3
  local log="$D/logs/${L}_s${S}_${N}.txt"
  $R "$L" --seme "$S" "$@" --registro > "$log" 2>&1
  local p a s
  p=$(grep '^punteggio A' "$log")
  a=$(grep '^autorizzazioni della sindaca A' "$log")
  s=$(grep '^stelle A' "$log")
  echo "$L | seme $S | $N | $p | $a | $s" >> "$OUT"
}

if [ "$MODE" = all ]; then
for L in easy medium expert; do
  for S in 1 2 3; do
    run $L $S s00_nessun_ordine
    run $L $S s01_pre_tutti_T0_evac_tutti_T0  --priorita "$PRI_NEAR" --civili "$PRE_ALL_T0" 
    run $L $S s02_evac_tutti_T0_senza_pre   --civili "$EVAC_ALL_T0"
    run $L $S s03_pre_tutti_T0_nessuna_evac --civili "$PRE_ALL_T0_ONLY"
    run $L $S s04_pre_evac_Castelvento_SanRocco --civili "$PRE_EVAC_TWO"
    run $L $S s05_pre_T0_evac_tutti_T30     --civili "$PRE_ALL_EVAC_T30"
    run $L $S s06_pri_vicini_pre_evac_tutti --priorita "$PRI_NEAR" --civili "$PRE_ALL_T0"
    run $L $S s07_pri_lontani_pre_evac_tutti --priorita "$PRI_FAR" --civili "$PRE_ALL_T0"
    run $L $S s08_pri_vicini_nessun_ordine  --priorita "$PRI_NEAR"
    run $L $S s09_pri_lontani_nessun_ordine --priorita "$PRI_FAR"
    run $L $S s10_pri_lontani_evac_tutti_T0 --priorita "$PRI_FAR" --civili "$EVAC_ALL_T0"
    run $L $S s11_pri_vicini_evac_tutti_T0  --priorita "$PRI_NEAR" --civili "$EVAC_ALL_T0"
  done
done
fi
# Blocco aggiuntivo: pre-allerta con priorità vicine, senza evacuazione (confronto pulito con s11)
# e controllo del caso «nessuna priorità = nessuna squadra».
if [ "$MODE" = extra ]; then
for L in easy medium expert; do
  for S in 1 2 3; do
    run $L $S s12_pri_vicini_pre_tutti_T0_nessuna_evac --priorita "$PRI_NEAR" --civili "$PRE_ALL_T0_ONLY"
    run $L $S s13_pri_vicini_evac_tutti_T30 --priorita "$PRI_NEAR" --civili "$(civ 30 e)"
  done
done
fi
echo "fatto: $(wc -l < "$OUT") righe"
