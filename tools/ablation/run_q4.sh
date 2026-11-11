#!/bin/zsh
# Pipeline completo con LFM3 Q4_K_M en las seis reuniones, para comparar con
# la fila `full` (Q8_0) del ablation.
#
#   JUANOLA_GGUF=/ruta/LFM3-1.7B-Q4_K_M.gguf tools/ablation/run_q4.sh <dir_salida>
set -u
cd "${0:A:h}/../.."
OUT=${1:?uso: run_q4.sh <dir_salida>}
: ${JUANOLA_GGUF:?define JUANOLA_GGUF con la ruta del Q4_K_M}
mkdir -p "$OUT"
F=crates/enhance/fixtures
B=./target/release/examples/eval_enhance
typeset -A TEMPLATE TITLE
TEMPLATE=(stand_up standup uno_a_uno 1on1 llamada_ventas ventas entrevista_cliente entrevista weekly_planning semanal revision_trimestral semanal)
TITLE=(stand_up "Stand-up móvil" uno_a_uno "1:1 con Diego" llamada_ventas "Logística Norte" entrevista_cliente "Entrevista Ferretera del Norte" weekly_planning "Weekly planning" revision_trimestral "Revisión trimestral Q3")
{
  for base in stand_up uno_a_uno llamada_ventas entrevista_cliente weekly_planning revision_trimestral; do
    seg="$OUT/$base.segments.json"
    python3 -I tools/script_to_segments.py $F/$base.txt > "$seg"
    echo "=== $base · plantilla"; $B "$seg" $F/$base.facts.json $F/$base.notas.txt --runs 3 --title "${TITLE[$base]}" --variant q4=itn --template ${TEMPLATE[$base]} 2>/dev/null
    echo "=== $base · auto"; $B "$seg" $F/$base.facts.json $F/$base.notas.txt --runs 3 --title "${TITLE[$base]}" --variant q4=itn 2>/dev/null
  done
  echo "=== FIN"
} | tee -a "$OUT/eval.log"
