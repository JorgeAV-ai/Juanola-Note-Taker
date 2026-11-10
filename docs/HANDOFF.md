# Contexto para continuar el proyecto Juanola

Eres el modelo que continúa este proyecto. Este documento dice **dónde está el proyecto hoy y qué queda**. Antes de tocar nada:

- Lee [README.md](README.md) (qué hay en cada documento) y, según lo que vayas a hacer, [plan.md](plan.md) y [modelos-mvp.md](modelos-mvp.md).
- Antes de probar un cambio de prompt, modelo o pipeline, busca en [experimentos.md](experimentos.md): recoge lo ya probado y descartado. Al terminar un experimento, añade su fila aunque salga mal.
- El detalle de cómo se construyó cada parte (fallos, causas, arreglos) está en [historial.md](historial.md).
- Responde siempre en español.

## Qué es

Réplica de Granola (notepad de reuniones con IA, sin bot: captura micrófono + audio del sistema, transcribe en vivo y, al terminar, fusiona las notas del usuario con la transcripción en «notas mejoradas») hecha en Rust con modelos open source locales. UI en Tauri v2 + React + TipTap, con la estructura y las funciones de Granola (que es Electron). Núcleo en un workspace de crates Rust.

**Nombre: Juanola** (desde el 6 nov 2026). Crates `juanola-*`, binario `juanola-stt-worker`, identificador `dev.juanola.desktop`, variables de entorno `JUANOLA_*`. «Granola» en docs y comentarios es el producto que se replica; también se mantienen la tool MCP `query_granola_meetings` (replica su API) y el test `titles_match_granola`. La carpeta del repo sigue llamándose `granola-rs`.

Proyecto en `/Users/jorgeav/toys/granola-rs`. Rama de trabajo `feat/plantillas-fase0` (sobre `main`), con commits sin subir.

## Normas de trabajo

- Español en todo lo dirigido al usuario; identificadores y código en inglés.
- No hacer commit ni push sin que el usuario lo pida.
- Descargar modelos o archivos grandes requiere permiso del usuario: di nombre, origen y tamaño antes.
- Los modelos los elige el usuario; proponer con datos verificados (API de Hugging Face para repos y tamaños), no inventar ids de repos.
- Cuando algo no se pueda verificar, decirlo.
- **Foco de calidad (desde el 9 nov): castellano e inglés.** Los experimentos del enhance se miden y ajustan solo en esos dos idiomas; el resto tiene que seguir funcionando.
- Medir con 3 semillas como mínimo: entre semillas hay ±10 puntos de ruido.

## Decisiones cerradas por el usuario (no reabrir)

- **Stack:** Tauri v2 + núcleo Rust + React/TipTap. Dioxus y GPUI descartados.
- **LLM:** LFM3-1.7B de Liquid AI, GGUF **Q8_0** (`LiquidAI/LFM3-1.7B-GGUF`, 1,85 GB, desde el 9 nov; antes Q4_K_M). Corre con `llama-cpp-2` + Metal (llama.cpp b11960, en `third_party/`), porque ningún crate Rust de MLX implementa LFM3 (la entrada MLX está `ready = false`). Razonamiento desactivado: el token `<think>` lleva logit bias −∞. Muestreo de Liquid: temperatura 0,1, top-k 50, repetición 1,1. Un modelo mayor no mejora las tareas (Qwen3.5 4B y 9B, medidos el 10 nov).
- **STT:** Qwen3-ASR sobre `mlxrs` en Mac, **1.7B de 4 bits** (`mlx-community/Qwen3-ASR-1.7B-4bit`, 1,6 GB) desde el 9 nov; el 0.6B de 4 bits (0,71 GB) en Mac con menos de 16 GB. El prompt lleva contexto (título, asistentes, descripción, notas y vocabulario del usuario): WER del 6,3 al 3,7 %. En Windows, ONNX int8 de sherpa-onnx (0,94 GB), sin implementar.
- **Embeddings:** EmbeddingGemma 2 solo texto, ONNX q8 (`onnx-community/embeddinggemma-2-ONNX`, 314 MB), truncado a 256d. Sin implementar.
- **VAD** Silero (sobre MLX en Mac); **diarización** pyannote community-1 vía `speakrs` (sin implementar).
- Formatos nativos por plataforma: MLX/CoreML en Mac Apple Silicon, GGUF/ONNX en Windows. La licencia de los modelos no importa (no se vende); prima calidad/peso.

## Código, crates y runtimes

- Workspace Cargo con 19 crates en `crates/` (core-types, db, search, audio-capture, audio-store, stt, diarize, llm, embed, prompts, enhance, calendar, detect, speaker-tags, dictation, models, mcp-server, api-server, integrations) y la app Tauri en `apps/desktop` (`src-tauri` + `src`). Implementados: audio-capture, stt, llm, enhance, prompts, db, models, detect y calendar; el resto son esqueletos (traits y tipos). Esquema SQLite en `crates/db/migrations/0001_init.sql`.
- Catálogo de modelos en `models.toml` (por slot, con `runtime`, `format`, `platforms`, `backends`, `ready`); `Catalog::pick` elige el formato más nativo de la plataforma y salta las entradas `ready = false`.
- Audio: `cpal` 0.18 (mic en todas; process tap de Core Audio en macOS 14.6+ y loopback WASAPI en Windows), `rubato` para pasar a 16 kHz mono. Fallback `screencapturekit` para macOS 13-14.5, pendiente.
- STT: `mlxrs` (encoder y decoder de Qwen3-ASR; el transcriptor es propio, `qwen3_mlx.rs`) en un proceso hijo, porque sus `Array` no son `Send` y un fallo de Metal aborta el proceso. `sherpa-onnx` para Windows/Linux.
- LLM: `llama-cpp-2` sobre llama.cpp b11960 (`third_party/llama-cpp-sys-2`), Metal/Vulkan/CUDA; cliente OpenAI-compatible (Ollama/LM Studio) como opción.
- Enhance: ITN castellano propio e inglés con `text-processing-rs`; validación con `jsonschema`.
- Previstos: `ort` o `fastembed` (embeddings), `tantivy` (FTS), `sqlite-vec`, `keyring`, `tauri-specta` (tipos TS, sin introducir).
- Frontend: React + TypeScript + Vite, Tailwind v4, shadcn/ui sobre Radix, TipTap, zustand. pnpm 11 necesita `allowBuilds: esbuild: true` en `apps/desktop/pnpm-workspace.yaml` (ya está).
- Referencias open source del mismo stack: anarlog (ex-Hyprnote, `github.com/fastrepl/hyprnote`, MIT) y Meetily (`github.com/Zackriya-Solutions/meetily`, MIT).

## Estado por área (10 nov)

| Área | Estado | Dónde |
|---|---|---|
| Audio dual | Hecho en Mac: mic y sistema (Core Audio process tap vía cpal 0.18) a 16 kHz, WAV alineados, watchdog que reconstruye streams mudos. Validado en llamadas reales de Meet | `crates/audio-capture` |
| STT en vivo | Hecho en Mac: proceso hijo `juanola-stt-worker`, VAD + segmentador propio, parciales cada 1 s, retraso mediano ~0,2 s con el 0.6B y ~1-3 s con el 1.7B. Idioma «Detectar» con `LanguageLock`. Confianza por token en cada segmento | `crates/stt` |
| Enhance | Hecho: pipeline híbrido descrito abajo, en Auto y con plantilla | `crates/enhance`, `crates/llm`, `crates/prompts` |
| Plantillas | Núcleo, las cinco integradas (1:1, stand-up, semanal, entrevista con cliente y llamada de ventas) y evaluación hechos (fase 1). **La app aún no las usa**: `enhance_note` pasa `template: None` | [plantillas.md](plantillas.md) |
| App | Bucle completo: grabar → transcripción en vivo → notas mejoradas editables (texto de la IA en gris, lupa a la cita). Home, carpetas, papelera, búsqueda ⌘K, ajustes, gestor de modelos con descarga desde HF, diseño «regaliz» claro y oscuro | `apps/desktop` |
| Detección y calendario (fase 2) | Hecho en Mac: aviso al detectar una llamada (Core Audio), EventKit para el calendario, «Coming up», «Take Notes» que graba y para sola al acabar la llamada | `crates/detect`, `crates/calendar`, `src-tauri/src/detect.rs` |
| Windows, embeddings, chat, recipes, diarización | Sin empezar | [plan.md](plan.md) §12 |

### El enhance hoy

1. La transcripción se numera por líneas («[12] (03:40) Ellos: …»). Antes, el ITN pasa los números escritos con letras a dígitos: el propio (`itn.rs`) en castellano y `text-processing-rs` (NeMo) en inglés.
2. **Resumen** en Markdown (`## tema`, viñetas con citas `[n, m]`), con las notas del usuario por bloque en el prompt. Prompt en `crates/prompts/templates/enhance.user.md` (Auto) o `enhance.template.md` (plantilla). El tope de tokens crece con la reunión.
3. **Pasadas de detalle** cada 4 min de transcripción, en el mismo lote que el resumen (`complete_many`): como mucho 12 datos concretos por fragmento, que se añaden a su apartado si faltan.
4. El backend de llama.cpp **cierra cada cita a las 6 referencias** (`GenOptions::max_refs`), para que el modelo no gaste el tope enumerando líneas.
5. **Post-proceso en código:** encaje en la plantilla (`fit_to_template`), duplicados entre apartados, **encargos con dueño copiados a «Próximos pasos»** (`promote_commitments`), notas del usuario colocadas literales en su apartado, citas a tiempos reales y validación contra `enhanced_notes_schema()`.

Cifras de referencia (3 semillas): reuniones de 1 hora, cobertura de hechos 61-91 % y 81 de 168 tareas acertadas; reuniones de 5-10 min, cobertura 72-100 %. Una hora de reunión tarda unos 90-150 s en el M4 Pro. Detalle y comparativas en [experimentos.md](experimentos.md).

## Cómo arrancar y probar

- **App en desarrollo:** `pnpm tauri dev` en `apps/desktop` (Vite en el puerto 1420). El usuario puede tenerlo abierto en su terminal: no abras otro, chocaría en el puerto. Con `pnpm tauri dev` el audio del sistema da silencio (el binario suelto no tiene `NSAudioCaptureUsageDescription`).
- **.app con permisos:** `apps/desktop/scripts/bundle-dev.sh [--open]` genera `target/debug/bundle/macos/Juanola.app` con el worker como sidecar. Cada build cambia la firma ad hoc y macOS vuelve a pedir los permisos.
- **Autotest** en la .app: `open -n --env RUST_LOG=info --env JUANOLA_AUTOTEST=<modo> --stdout recordings/app.log --stderr recordings/app.log target/debug/bundle/macos/Juanola.app`. Modos: `1` (recorrido completo con audio), `enhance`, `ui`, `detect`, `detect-live`, `detect-calendar`, `context`. Escribe `OK`/`FAIL` y «FIN: todo OK».
- **Frontend en el navegador** con backend simulado: `.claude/launch.json` → `frontend-mock` (puerto 1430). Parámetros: `?models=missing`, `?calendar=none`, `#/notification?notice=calendar|adhoc`.
- **Evaluar el enhance:** `cargo run --release -p juanola-enhance --example eval_enhance -- <segmentos.json> <hechos.facts.json> [notas.txt] --runs 3 --variant <nombre>=itn[+…] [--template clave]`. Guiones en `crates/enhance/fixtures/`; para pasarlos a segmentos, `tools/script_to_segments.py`. Las variantes para apagar piezas están en la cabecera de `eval_enhance.rs`. `JUANOLA_GGUF` cambia de modelo y `JUANOLA_LLM_TRACE=1` imprime las salidas.
- **STT sin la app:** `crates/stt/examples/transcribe_session.rs`, `simulate_stream.rs` (dos canales a ritmo real) y `compare_transcripts.rs` (WER/CER con normalizador de cifras). `tools/script_to_audio.py` convierte un guion en `mic.wav` + `system.wav` con `say`.
- Base de datos de la app: `~/Library/Application Support/dev.juanola.desktop/juanola.sqlite`; modelos en `…/dev.juanola.desktop/models/`.
- Entorno: rustc ≥ 1.95 (por `mlxrs`); sin Xcode, `third_party/mlxrs-sys` usa el `mlx.metallib` de la wheel `mlx-metal` (ver `third_party/README.md`).

## Pendientes

Por orden de valor, según lo hablado con el usuario:

1. **Conectar las plantillas a la app** (fase 2 de [plantillas.md](plantillas.md) §7): submenú, aviso de caída a Auto y autotest.
2. **Ver el resumen antes:** mostrarlo a los ~20 s y añadir los detalles después; más adelante, procesar durante la reunión. Ojo: con el enhance en la GPU, el STT en vivo se queda más de un minuto atrás (medido); tendría que ceder la GPU o esperar a los silencios.
3. **Validar con grabaciones reales** en castellano e inglés: todo lo medido es con guiones y voces de `say` ([eval-dataset.md](eval-dataset.md), capa D).
4. **Audio:** cancelación de eco sin auriculares (candidato: LocalVQE, ver experimentos) y fallback ScreenCaptureKit para macOS 13-14.5.
5. **STT:** el 0.6B para los parciales y el 1.7B para los finales (sin probar en directo); pasar el contexto en caliente (lo escrito durante la reunión no entra).
6. **Enhance, calidad:** en la weekly con plantilla salen demasiadas tareas (30-38 de 14); desajuste plantilla/reunión sin detectar; viñetas copiadas en el stand-up.
7. **Fase 2:** Google y Microsoft directos con OAuth, reposición de ventana, indicador flotante, tray, auto-stop por fin del evento o 15 min sin voz. Sin verificar con Zoom, Teams ni Slack reales.
8. **Firma estable** de la .app para que macOS no pida los permisos en cada build.
9. Fuera del foco: en alemán, el modelo a veces razona en inglés dentro de las notas hasta agotar el tope.
