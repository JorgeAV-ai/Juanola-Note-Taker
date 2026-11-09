#!/usr/bin/env python3
"""Sintetiza un guion `Hablante|texto` con `say` (macOS) para evaluar el STT.

«Yo» va a `mic.wav` y el resto a `system.wav`, mono 16 kHz y alineados,
como los graba la app. Cada hablante tiene su voz. `ref.json` guarda el texto
exacto de cada intervención con su canal y sus tiempos reales: es la
referencia para el WER.

    python3 tools/script_to_audio.py guion.txt carpeta_salida [--lang es]

El audio de TTS es más limpio que una reunión real: el WER saldrá optimista.
"""

import json
import os
import subprocess
import sys
import tempfile
import wave

RATE = 16_000
GAP = 0.35  # silencio entre intervenciones, en segundos

VOICES = {
    "es": ["Mónica", "Eddy (Spanish (Spain))", "Flo (Spanish (Spain))", "Grandpa (Spanish (Spain))", "Reed (Spanish (Spain))"],
}


def synth(text: str, voice: str, tmp: str) -> bytes:
    path = os.path.join(tmp, "x.wav")
    subprocess.run(["say", "-v", voice, "-o", path, "--data-format=LEI16@16000", text], check=True)
    with wave.open(path) as w:
        assert w.getframerate() == RATE and w.getnchannels() == 1 and w.getsampwidth() == 2
        return w.readframes(w.getnframes())


def write(path: str, pcm: bytearray) -> None:
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(bytes(pcm))


def main(script: str, out_dir: str, lang: str) -> None:
    os.makedirs(out_dir, exist_ok=True)
    voices = VOICES[lang]
    assigned: dict[str, str] = {}
    tracks = {"mic": bytearray(), "system": bytearray()}
    ref = []
    t = 0  # en muestras
    with open(script, encoding="utf-8") as f, tempfile.TemporaryDirectory() as tmp:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#") or "|" not in line:
                continue
            speaker, text = (s.strip() for s in line.split("|", 1))
            channel = "mic" if speaker.lower() == "yo" else "system"
            if speaker not in assigned:
                assigned[speaker] = voices[len(assigned) % len(voices)]
            pcm = synth(text, assigned[speaker], tmp)
            n = len(pcm) // 2
            for ch, buf in tracks.items():
                buf.extend(b"\0\0" * (t - len(buf) // 2))
                if ch == channel:
                    buf.extend(pcm)
            ref.append({"channel": channel, "speaker": speaker, "start": round(t / RATE, 2),
                        "end": round((t + n) / RATE, 2), "text": text})
            t += n + int(GAP * RATE)
            print(f"\r{len(ref)} intervenciones, {t / RATE / 60:.1f} min", end="", file=sys.stderr)
    for ch, buf in tracks.items():
        buf.extend(b"\0\0" * (t - len(buf) // 2))
        write(os.path.join(out_dir, f"{ch}.wav"), buf)
    with open(os.path.join(out_dir, "ref.json"), "w", encoding="utf-8") as f:
        json.dump(ref, f, ensure_ascii=False, indent=1)
    print(f"\n{out_dir}: {t / RATE / 60:.1f} min, voces {assigned}", file=sys.stderr)


if __name__ == "__main__":
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    lang = sys.argv[sys.argv.index("--lang") + 1] if "--lang" in sys.argv else "es"
    if "--lang" in sys.argv:
        args.remove(lang)
    if len(args) != 2:
        sys.exit(__doc__)
    main(args[0], args[1], lang)
