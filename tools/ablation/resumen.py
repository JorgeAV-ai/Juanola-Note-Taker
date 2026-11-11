#!/usr/bin/env python3
"""Resume el eval.log del ablation en una tabla Markdown por reunión y modo.

    python3 tools/ablation/resumen.py eval.log

Media de las semillas por variante. «Tareas» es la suma de tareas del guion
encontradas sobre las esperadas en todas las semillas.
"""

import re
import statistics as st
import sys

# La columna de cobertura puede salir pegada («12/12100%»), de ahí el `\d+?`.
ROW = re.compile(
    r"^(\S+)\s+(\d)\s+(\d+)/(\d+?)\s*(100|\d{1,2})%\s+(\d+)/(\d+)\s+(\d+)% \((\d+)\)"
    r"\s+(\d+)\s+(\d+)\s+(\d+)%\s+([\d.]+)"
)
PLACEMENT = re.compile(r"colocación \d+/\d+ (\d+)%")


def main(path: str) -> None:
    blocks: dict[str, dict[str, list[dict]]] = {}
    cur = var = None
    for line in open(path, encoding="utf-8"):
        if line.startswith("=== "):
            cur = line[4:].strip()
            if cur != "FIN":
                blocks[cur] = {}
            continue
        m = ROW.match(line)
        if m and cur in blocks:
            var = m.group(1)
            blocks[cur].setdefault(var, []).append(
                dict(cov=int(m.group(5)), t=int(m.group(6)), tt=int(m.group(7)),
                     fid=int(m.group(8)), cop=int(m.group(12)), s=float(m.group(13)), col=None)
            )
        p = PLACEMENT.search(line)
        if p and cur in blocks and var in blocks[cur]:
            blocks[cur][var][-1]["col"] = int(p.group(1))

    mean = lambda xs: f"{round(st.mean(xs))} %" if xs else "—"
    for name, variants in blocks.items():
        tpl = "plantilla" in name
        print(f"\n### {name}\n")
        print("| Variante | Semillas | Cobertura | Tareas | Cifras fieles | Copiadas |" + (" Colocación |" if tpl else "") + " s |")
        print("|---|---|---|---|---|---|" + ("---|" if tpl else "") + "---|")
        for v, rs in variants.items():
            row = (f"| {v} | {len(rs)} | {mean([r['cov'] for r in rs])} | {sum(r['t'] for r in rs)}/{sum(r['tt'] for r in rs)}"
                   f" | {mean([r['fid'] for r in rs])} | {mean([r['cop'] for r in rs])} |")
            if tpl:
                row += f" {mean([r['col'] for r in rs if r['col'] is not None])} |"
            print(row + f" {st.mean(r['s'] for r in rs):.0f} |")


if __name__ == "__main__":
    main(sys.argv[1])
