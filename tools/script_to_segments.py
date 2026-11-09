#!/usr/bin/env python3
"""Convierte un guion `Hablante|texto` en `segments.json` para eval_enhance.

Sin TTS ni STT: lo que se evalúa es el enhance. «Yo» va al canal `mic` y el
resto a `system`. Cada intervención se parte en frases agrupadas hasta ~30
palabras (como los segmentos del STT), con tiempos estimados a ~2,5
palabras/s y 0,4 s de pausa entre intervenciones.

    python3 tools/script_to_segments.py guion.txt > segments.json
"""

import json
import re
import sys

WORDS_PER_SEC = 2.5
GAP = 0.4
MAX_WORDS = 30


def chunks(text: str):
    sentences = [s for s in re.split(r"(?<=[.!?…])\s+", text.strip()) if s]
    cur: list[str] = []
    for s in sentences:
        if cur and len(" ".join(cur + [s]).split()) > MAX_WORDS:
            yield " ".join(cur)
            cur = []
        cur.append(s)
    if cur:
        yield " ".join(cur)


def main(path: str) -> None:
    t = 0.0
    out = []
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#") or "|" not in line:
                continue
            speaker, text = line.split("|", 1)
            channel = "mic" if speaker.strip().lower() == "yo" else "system"
            for c in chunks(text):
                dur = max(1.0, len(c.split()) / WORDS_PER_SEC)
                out.append({"channel": channel, "start": round(t, 2), "end": round(t + dur, 2), "text": c})
                t += dur + 0.15
            t += GAP
    json.dump(out, sys.stdout, ensure_ascii=False, indent=1)
    sys.stdout.write("\n")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    main(sys.argv[1])
