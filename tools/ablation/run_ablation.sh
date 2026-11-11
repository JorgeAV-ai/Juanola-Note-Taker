#!/bin/zsh
# Ablation del enhance: el pipeline completo y, en cada variante, una pieza quitada.
#
#   tools/ablation/run_ablation.sh <dir_salida> [reunión[:tpl|:auto] ...]
#
# Sin reuniones, corre las seis (es/en), plantilla y Auto. Para reanudar, pasa
# solo las que faltan, p. ej. `entrevista_cliente:auto weekly_planning revision_trimestral`.
# Escribe la salida en <dir_salida>/eval.log y el stderr de llama.cpp en *.err.
# Requiere `cargo build --release -p juanola-enhance --example eval_enhance`.
set -u
cd "${0:A:h}/../.."
OUT=${1:?uso: run_ablation.sh <dir_salida> [reunión[:tpl|:auto] ...]}; shift
mkdir -p "$OUT"
F=crates/enhance/fixtures
B=./target/release/examples/eval_enhance
TPL=(--variant full=itn --variant noitn=noitn --variant nodetail=itn+nodetail --variant nopromote=itn+nopromote --variant nocitelimit=itn+nocitelimit --variant nonotes=itn+nonotes --variant gbnf=itn+gbnf)
AUTO=(--variant full=itn --variant noitn=noitn --variant nodetail=itn+nodetail --variant nopromote=itn+nopromote --variant nocitelimit=itn+nocitelimit --variant nonotes=itn+nonotes --variant nofloor=itn+nofloor)
typeset -A TEMPLATE TITLE EXTRA
TEMPLATE=(stand_up standup uno_a_uno 1on1 llamada_ventas ventas entrevista_cliente entrevista weekly_planning semanal revision_trimestral semanal)
TITLE=(stand_up "Stand-up móvil" uno_a_uno "1:1 con Diego" llamada_ventas "Logística Norte" entrevista_cliente "Entrevista Ferretera del Norte" weekly_planning "Weekly planning" revision_trimestral "Revisión trimestral Q3")
EXTRA=(weekly_planning "--variant nosuffix=itn+nosuffix")
ALL=(stand_up uno_a_uno llamada_ventas entrevista_cliente weekly_planning revision_trimestral)

run() { # reunión modo(tpl|auto|ambos)
  local base=$1 mode=$2 seg="$OUT/$1.segments.json"
  local extra=(${=EXTRA[$base]:-})
  python3 -I tools/script_to_segments.py $F/$base.txt > "$seg"
  if [[ $mode != auto ]]; then
    echo "=== $base · plantilla"
    $B "$seg" $F/$base.facts.json $F/$base.notas.txt --runs 3 --title "${TITLE[$base]}" $TPL $extra --template ${TEMPLATE[$base]} 2>"$OUT/$base.tpl.err"
  fi
  if [[ $mode != tpl ]]; then
    echo "=== $base · auto"
    $B "$seg" $F/$base.facts.json $F/$base.notas.txt --runs 3 --title "${TITLE[$base]}" $AUTO $extra 2>"$OUT/$base.auto.err"
  fi
}

items=("$@"); (( $#items )) || items=($ALL)
{
  for item in $items; do
    base=${item%%:*}; mode=ambos; [[ $item == *:* ]] && mode=${item#*:}
    run $base $mode
  done
  echo "=== FIN"
} | tee -a "$OUT/eval.log"
