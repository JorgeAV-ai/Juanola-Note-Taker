# Contexto para continuar el proyecto Juanola

Eres el modelo que continúa este proyecto. Lee este documento y después `docs/plan.md` y `docs/modelos-mvp.md` antes de tocar nada. Responde siempre en español.

## Qué es

Réplica de Granola (notepad de reuniones con IA, sin bot: captura micrófono + audio del sistema, transcribe en vivo, y al terminar fusiona las notas del usuario con la transcripción en "notas mejoradas") hecha en Rust con modelos open source locales. UI en Tauri v2 + React + TipTap, replicando exactamente la estructura y features de Granola (que es Electron). Núcleo en un workspace de crates Rust.

**Nombre: Juanola** (renombrado desde "Granola RS" el 6 nov 2026). Crates `juanola-*`, binario `juanola-stt-worker`, identificador `dev.juanola.desktop` y variables de entorno `JUANOLA_*`. "Granola" en docs y comentarios se refiere al producto real que se replica; también se mantienen la tool MCP `query_granola_meetings`, porque replica la API de Granola, y el test `titles_match_granola`. La carpeta del repo sigue llamándose `granola-rs`.

Proyecto en `/Users/jorgeav/toys/granola-rs`. Repo git con un commit inicial. Hay cambios staged sin commit (models.toml, crates/models, docs/modelos-mvp.md): no hagas commit sin que el usuario lo pida.

## Estado actual

- Workspace Cargo con 19 crates en `crates/` (core-types, db, search, audio-capture, audio-store, stt, diarize, llm, embed, prompts, enhance, calendar, detect, speaker-tags, dictation, models, mcp-server, api-server, integrations). Todos compilan sin avisos (`cargo check --workspace`). Son esqueletos: traits, tipos de dominio, esquema SQLite (18 tablas en `crates/db/migrations/0001_init.sql`), JSON Schema del enhance en `crates/enhance`, prompts en `crates/prompts/templates`. Los backends reales de audio/STT/LLM NO están implementados; están gated por features de Cargo vacías (`sherpa`, `whisper`, `llamacpp`, `mlx`...).
- App Tauri en `apps/desktop` (src-tauri + src). Shell de UI funcionando: sidebar (My notes, Team space, Customer calls, Recipes, Views, menú de workspace), Home (Coming up, Your notes, New note), vista de nota (título inline, pestañas My notes/Enhanced, botón ✨, editor TipTap, píldora de grabación con barras animadas y drawer de transcript con burbujas Them/Me), barra "Ask anything". Sin lógica detrás: 5 comandos Tauri stub en `src-tauri/src/commands.rs`. Tokens CSS en `src/styles/tokens.css` son aproximados (la auditoría visual con la app real de Granola está pendiente).
- `pnpm tauri dev` arranca la app (Vite en el puerto 1420). pnpm 11 necesita `allowBuilds: esbuild: true` en `apps/desktop/pnpm-workspace.yaml` (ya está). El usuario puede tener `pnpm tauri dev` corriendo en su terminal: no abras otro, chocaría en el puerto 1420.
- `crates/models` lee `models.toml` (catálogo por slot con `runtime`, `format`, `platforms`, `backends`, `ready`) y `Catalog::pick(slot, machine)` elige la entrada por defecto en el formato más nativo de la plataforma, saltando las `ready = false`. 5 tests pasan.
- Base de datos de la app en `~/Library/Application Support/dev.juanola.desktop/juanola.sqlite`.

## Decisiones cerradas por el usuario (no reabrir)

- Stack: Tauri v2 + núcleo Rust + React/TipTap. Alternativas (Dioxus, GPUI) descartadas.
- LLM: **LFM3-1.7B** de Liquid AI (`LiquidAI/LFM3-1.7B-GGUF` Q4_K_M 1,1 GB; `LiquidAI/LFM3-1.7B-MLX-4bit` 0,97 GB). Razonamiento viene activado por defecto: desactivar (`enable_thinking=false`) y filtrar `<think>`. Muestreo: temperature 0.1, top_k 50, repetition_penalty 1.1. Contexto 256K.
- STT: **Qwen3-ASR 0.6B** (confirmado frente a LFM3-Audio-2B, que solo hace streaming en inglés). MLX 4-bit `mlx-community/Qwen3-ASR-0.6B-4bit` (0,71 GB) en Mac; ONNX int8 de sherpa-onnx (`sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25.tar.bz2`, 0,94 GB) en Windows.
- Embeddings: **EmbeddingGemma 2** solo texto, ONNX q8 `onnx-community/embeddinggemma-2-ONNX` (314 MB), truncado a 256d, prefijos `task: search result | query: …` / `title: none | text: …`.
- VAD Silero (ONNX), diarización pyannote community-1 vía crate `speakrs` (CoreML en Mac, ONNX en Windows).
- Formatos nativos por plataforma: MLX/CoreML en Mac Apple Silicon, GGUF/ONNX en Windows. La licencia de los modelos no importa (no se vende); prima calidad/peso.
- Bloqueo conocido: ningún crate Rust de MLX implementa LFM3 (`mlxrs` 0.2 trae qwen3 y lfm2-vl). En Mac el LLM corre GGUF con `llama-cpp-2` + Metal hasta que exista soporte; la entrada MLX está `ready = false` en el catálogo.

## Crates y runtimes a usar

- Audio: `cpal` (mic en todas; WASAPI loopback en Windows), Core Audio process tap vía `objc2`/`coreaudio-sys` en macOS 14.2+ (sin permiso de grabación de pantalla), `screencapturekit` como fallback en macOS 13-14.1, `rubato` para resample a 16 kHz mono.
- STT: `mlxrs` (feature `audio`, modelo `qwen3-asr`) en Mac, aislado en proceso hijo porque sus `Array` no son `Send` y un fallo de Metal aborta el proceso; `sherpa-onnx` crate en Windows/Linux y como fallback en Mac.
- LLM: `llama-cpp-2` con build de llama.cpp que incluya LFM3 (b11960+), Metal/Vulkan/CUDA; cliente OpenAI-compatible para Ollama/LM Studio como opción.
- Embeddings: `ort` (ONNX Runtime) o `fastembed` con modelo custom.
- Datos: `sqlx` SQLite, `tantivy` FTS, `sqlite-vec` vectorial, `keyring`.
- Tauri: `tauri-specta` para tipos TS (pendiente de introducir), plugins oficiales ya registrados en `src-tauri/src/lib.rs`.
- Referencias open source del mismo stack: anarlog (ex-Hyprnote, `github.com/fastrepl/hyprnote`, MIT) y Meetily (`github.com/Zackriya-Solutions/meetily`, MIT).

## Siguiente trabajo acordado: prueba técnica de una semana

Antes de más UI, validar los tres puntos de riesgo, en este orden, en el Mac del usuario (Apple Silicon, macOS reciente):

1. **Audio dual** en `crates/audio-capture`: mic con `cpal` + sistema con Core Audio process tap, resample a 16 kHz, escritura a dos WAV. Hecho cuando en una llamada de Meet la voz del usuario esté en un WAV y la de los demás en el otro.
2. **STT streaming** en `crates/stt`: backend `mlx` con `mlxrs` + Silero VAD, en proceso hijo. Hecho cuando el español aparezca en consola con menos de 2 s de retraso. Comprobar en paralelo si el paquete sherpa-onnx de Qwen3-ASR hace streaming o solo offline (decide la ruta de Windows).
3. **Enhance** en `crates/llm` + `crates/enhance`: backend `llamacpp` con Metal, GGUF Q4_K_M de LFM3-1.7B, salida JSON validada contra `enhanced_notes_schema()`. Hecho cuando una transcripción de 20 min produzca secciones con bullets que citen tramos del transcript.
4. **Cerrar el bucle en la app**: Start de la píldora → transcript en el drawer → Enhanced. Hecho cuando una reunión real produzca notas sin tocar la consola.

Descargar modelos (~2 GB) a la carpeta de datos de la app requiere permiso del usuario; pídelo antes.

## Progreso de la prueba técnica (rama `spike/audio-dual`, sin commit)

- **Punto 1, audio dual: hecho.** Validado en una llamada real de Meet (6 nov 2026): voz del usuario solo en `mic.wav`, la de los demás solo en `system.wav`, fuga < −110 dB en ambos sentidos. `CpalCapture` en `crates/audio-capture/src/cpal_backend.rs` implementa `AudioCapture`. El audio del sistema también va por cpal 0.18: `build_input_stream` sobre la salida por defecto crea el Core Audio process tap (macOS 14.6+) o el loopback WASAPI. Downmix a mono en el callback, resample a 16 kHz con `rubato::Fft` (`src/resample.rs`). Los dos canales cuentan desde el mismo t0 y los huecos se rellenan con silencio, así que los WAV quedan alineados. Si un stream cambia de dispositivo o de frecuencia, se reconstruye solo. Un watchdog reconstruye cualquier stream que pase 2 s sin callbacks, con backoff de 2 a 32 s.
- Verificado en el Mac del usuario (macOS 26.3, auriculares Bluetooth JBL Tune 720BT): la voz de `say` queda en `system.wav` y no se cuela en `mic.wav`.
- Hallazgos: (a) con auriculares BT, abrir el mic los pasa a perfil manos libres (salida de 44,1 kHz estéreo a 16 kHz mono) y el primer tap a menudo se queda mudo sin aviso de Core Audio; el watchdog lo recupera en unos 2 s. (b) Terminal.app y Warp no declaran `NSAudioCaptureUsageDescription` y el tap devuelve silencio desde ellas. Por eso existe `crates/audio-capture/scripts/record-dual-app.sh`, que empaqueta el ejemplo `record_dual` en una `.app` con las claves de permiso. La app Tauri ya declara ambas claves en `src-tauri/Info.plist`.
- Pendiente del punto 1 (no bloquea): AEC (sin auriculares, el mic captura los altavoces) y fallback ScreenCaptureKit para macOS 13–14.5.
- `CpalCapture::session_start()` da el instante de `timestamp = 0`. Abrir un mic Bluetooth tarda unos 3 s y `start()` no vuelve hasta entonces, así que para medir latencias hay que usar ese instante y no el momento en que vuelve `start()`.

- **Punto 2, STT en streaming: hecho.** Validado en vivo el 6 nov 2026 durante una llamada real. Usuario en «Me» y vídeo/llamada en «Them», sin cruces entre canales. Retraso respecto al final del tramo de audio: mediana 0,20 s, p90 0,60 s y máximo 0,73 s, sobre 70 resultados. Con el mic Bluetooth en manos libres la calidad del canal «Me» baja algo (p. ej. «tercer alvez»). Observado: el canal del sistema no recibió ningún callback hasta los 44 s; confirmado con el usuario que no sonaba nada: el tap no entrega datos mientras la salida está en silencio. No se pierde audio; el relleno de silencio mantiene los relojes alineados. Las reconstrucciones del watchdog en esos tramos sobran, pero no hacen daño. Decisión del usuario (6 nov 2026): Qwen3-ASR sobre `mlxrs` en Mac, no sherpa-onnx.
  - `mlxrs` 0.2 no trae transcriptor de Qwen3-ASR, solo el encoder de audio y el decoder de texto (de su forced aligner). El transcriptor propio está en `crates/stt/src/backends/qwen3_mlx.rs` (feature `mlx`). Hace: log-mel de Whisper sin relleno, encoder, prompt de chat con las filas de audio en los `<|audio_pad|>`, decodificación voraz con caché KV y cabeza atada (`quantized_matmul` sobre `model.embed_tokens`). Validado contra mlx-audio 0.32 en Python: mismo texto y mismos 38 tokens en un WAV de `say`, y mismo texto en 15 s de la grabación real de Meet con detección automática de idioma. Tarda 0,28 s para 7,7 s de audio (la referencia, 0,55 s).
  - El repo MLX no trae `tokenizer.json`. Se generó con `transformers` en la carpeta del modelo. Pendiente: generarlo en Rust al instalar (BPE de Qwen2 desde `vocab.json` + `merges.txt` + tokens especiales de `tokenizer_config.json`).
  - VAD: Silero sobre MLX (`mlx-community/silero-vad`), en `src/backends/silero_mlx.rs`. Segmentación en `src/segmenter.rs`, Rust puro con tests: umbral 0,5 con histéresis a 0,35, frase cerrada tras 600 ms de silencio, parcial cada 1 s, corte a 20 s.
  - Proceso hijo `juanola-stt-worker` (`src/bin/`). Recibe por stdin tramas `[canal u8][n u32][f32…]` y escribe eventos JSON por línea en stdout (`src/worker.rs`, que incluye el cliente `SttWorker`). Si la decodificación se retrasa, los parciales intermedios se saltan.
  - Medido en vivo (captura real del audio del sistema → worker): retraso del texto respecto al final del tramo de audio con mediana 0,16 s, p90 0,62 s y máximo 0,79 s. Una palabra concreta puede tardar como mucho ~1 s más, por la cadencia de parciales. Simulación con WAV en tiempo real: `examples/simulate_stream.rs`. En vivo: `crates/stt/scripts/live-app.sh`, que empaqueta el ejemplo `live` como `.app` con permisos.
  - Windows: el crate `sherpa-onnx` 2.0.1 solo expone Qwen3-ASR offline (`OfflineQwen3ASRModelConfig`), así que la ruta de Windows será VAD + streaming simulado, igual que en Mac.
  - Entorno: `mlxrs` exige rustc ≥ 1.95 (se actualizó `stable` a 1.99.1). Sin Xcode no hay compilador `metal`, y por eso existe `third_party/mlxrs-sys` (ver `third_party/README.md`): usa el `mlx.metallib` de la wheel `mlx-metal` 0.32.2 (`scripts/fetch-mlx-metallib.sh`), lista los headers JIT con clang y enlaza `libclang_rt.osx.a`. `.cargo/config.toml` apunta `MLXRS_PREBUILT_METALLIB`.
  - Referencia Python de mlx-audio instalada en un venv temporal fuera del repo, que no persiste.

- **Punto 3, enhance: hecho (con matices de calidad).** LFM3-1.7B Q4_K_M sobre `llama-cpp-2` con Metal.
  - `crates/llm/src/backends/llamacpp.rs` (feature `llamacpp`) implementa `ChatModel`. Usa el muestreo de Liquid AI (temperatura 0,1, top-k 50, repetición 1,1) y una gramática GBNF opcional a partir de JSON Schema (`json_schema_to_grammar`).
  - Prompt: `crates/llm/src/chat_format.rs` reproduce a mano la plantilla Jinja del GGUF con `enable_thinking=false`, porque usa `{% generation %}`, que `minijinja` no entiende. Hay tests contra la salida de jinja2. `<think>` se filtra.
  - Velocidad en el M4 Pro: 120 tok/s; carga en 0,3 s, salvo ~15 s la primera vez por la compilación de shaders. llama.cpp b11960 vía `third_party/llama-cpp-sys-2` (ver `third_party/README.md`).
  - Pipeline en `crates/enhance/src/pipelines/enhance.rs`. Forzar JSON con gramática hundía la calidad: el modelo copiaba líneas del transcript. Por eso el modelo escribe Markdown (`## tema`, viñetas con `[n, m]` = líneas de la transcripción numerada) y el código lo convierte a `EnhancedNotes`: las referencias pasan a `Citation::Transcript{start,end}` con tiempos reales, las notas del usuario se colocan literales en la sección con más palabras en común y «Próximos pasos» pasa a `is_action_item`. Se valida contra `enhanced_notes_schema()` con `jsonschema`. Prompt en `crates/prompts/templates/enhance.user.md`, con las instrucciones después de la transcripción.
  - Prueba: reunión ficticia de 17,5 min (`crates/enhance/fixtures/reunion_producto.txt`, sintetizada con `say` a dos canales) → STT (`examples/transcribe_session`: 136 segmentos en 62 s, canal correcto 136/136, WER 16 % mic / 7 % sistema) → enhance (`examples/enhance_session`) en ~14 s: 17 secciones, 47 bullets, 42 con cita al transcript, 5 notas del usuario y 7 tareas, sin referencias inválidas.
  - **Calidad afinada (6 nov, segunda iteración), medida con `crates/enhance/examples/eval_enhance.rs`.** El evaluador compara la salida con 36 hechos y tareas del guion (`fixtures/reunion_producto.facts.json`). Solo cuenta los hechos que el transcript contiene, para no culpar al LLM de errores del STT. Mide también qué parte de las cifras aparece en el tramo citado, el número de secciones y temas duplicados y las viñetas copiadas (8 o más palabras seguidas iguales al transcript).
    - ITN (`crates/enhance/src/itn.rs`): los números escritos con letras pasan a dígitos antes del LLM («treinta y nueve» → 39, «112 mil» → 112000, «dos coma cuatro» → 2,4). Las cifras fieles suben del 86 % al 98-100 % y las viñetas copiadas bajan del 70 % al 12-24 %.
    - Pipeline híbrido (por defecto, `EnhanceOptions::detail_chunk_secs = Some(240)`). (1) Resumen en una pasada, que fija la estructura y las tareas. (2) Por cada fragmento de 4 min, extracción de como mucho 12 datos concretos (`prompts/templates/enhance.details.md`, 450 tokens). Cada dato va a la sección cuyas citas están más cerca y se descarta si ya está, si no tiene cifra ni nombre propio o si copia una línea con hablante. Resultado final (3 ejecuciones): cobertura 81-96 % de 26 hechos alcanzables (solo resumen: 69-77 %), cifras fieles 90-100 % y 9-10 secciones sin duplicados.
    - Coste: ~60-80 s para 17,5 min de reunión en el M4 Pro (una sola pasada: ~20 s). En contexto largo LFM3 va a ~830 tok/s de prefill y ~50 tok/s de generación; ni flash attention ni `n_ubatch` lo cambian. Ahora la caché KV se dimensiona al prompt (antes se reservaban 32K). Idea pendiente: mostrar el resumen a los ~20 s y añadir los detalles después.
    - Descartado (medido): resumir por fragmentos y reagrupar temas con el modelo (JSON con gramática) da 25-39 secciones, o reagrupa de más, y pierde tareas. Pedir «más viñetas» o «todas las cifras» en una sola pasada no mejora de forma estable. Una pasada aparte para las tareas en JSON (quién/qué/cuándo) dio 0-2 de 6 tareas, con frases copiadas.
    - **Punto débil: las tareas con responsable.** Salen 1-4 de 6 según la semilla (4-5 antes de corregir «112 mil» en el ITN: el modelo es muy sensible a cambios pequeños en la entrada). Faltan responsables («Preparar análisis por país» sin «Luis») o los mezcla. Es el límite de un modelo de 1,7B; lo siguiente a probar es Q8_0 o un modelo mayor solo para las tareas.
    - Sigue fallando: hechos de los primeros minutos (1840 clínicas, 9 operaciones perdidas), alguna tarea con el responsable mezclado y errores heredados del STT («dos mil» por «doce mil», «ODER», «Vaquen»). Sin probar: Q8_0 (1,8 GB) o QAD-Q4_0.
  - ASR: añadido un tope de tokens por segundo de audio y un detector de bucles en `qwen3_mlx.rs`, porque Qwen3-ASR entró en bucle en un segmento. Queda un bucle no exacto en 1 de 136 segmentos.
- **Punto 4, cerrar el bucle en la app: implementado; falta que el usuario lo pruebe en una reunión real.**
  - DB: repositorios `crates/db/src/repo/{notes,segments,workspaces}.rs` con consultas en tiempo de ejecución (sin `DATABASE_URL`) y un test de ida y vuelta. Las notas cuelgan del workspace personal (id nulo).
  - Backend (`apps/desktop/src-tauri/src/recording.rs`): un hilo por sesión, dueño de `CpalCapture` y de `SttWorker`. Emite `recording://state`, `recording://levels` (~20 Hz) y `transcript://segment` (parciales y finales) y guarda los finales en `segments`. Resume continúa los tiempos tras el último segmento. Comandos en `commands.rs`: notas (crear, leer, título, `raw_doc`), segmentos, `start/pause/resume/stop_recording` y `enhance_note`. Este último carga LFM3 una vez (`OnceCell`), ejecuta el pipeline híbrido, guarda `enhanced_doc` (JSON de `EnhancedNotes`) y emite `note://status`.
  - Frontend: `NoteView` persiste el título y las notas crudas (debounce). `RecordingPill` tiene barras con el nivel real, Start/Pause/Resume/Stop y el drawer de transcript en vivo, con los parciales en semitransparente. `EnhancedView` muestra el texto del usuario en negro y el de la IA en gris, con una lupa que abre el fragmento citado. Al parar, el enhance se lanza solo. El estado del transcript vive en `src/state/transcript.ts` (zustand).
  - Empaquetado: `apps/desktop/scripts/bundle-dev.sh [--open]` genera `target/debug/bundle/macos/Juanola.app` (perfil debug), mete dentro `juanola-stt-worker` (release, feature `mlx`) y firma ad hoc. Hace falta la `.app`: con `pnpm tauri dev` el tap de audio del sistema da silencio, porque el binario suelto no tiene `NSAudioCaptureUsageDescription`. En desarrollo, `Paths::worker()` busca el worker junto al ejecutable, en `JUANOLA_STT_WORKER` o en `target/release`.
  - `Cargo.toml`: `profile.dev.package.{llama-cpp-sys-2,mlxrs-sys}` con `opt-level = 3` y `debug = false`, para que CMake compile llama.cpp y MLX en Release también en desarrollo.
  - El icono (`icons/icon.png`) venía corrupto (CRC); se generó uno provisional (cuadrado verde con una «J») y el juego completo con `tauri icon`.
  - Pendiente: sidecar con `externalBin` de Tauri en lugar del script, convertir `EnhancedNotes` a documento TipTap editable (ahora es una vista de solo lectura), selector de idioma (fijo en «Spanish») y avisos de permisos denegados (hoy solo se ve silencio).

## Normas de trabajo

- Español en todo lo dirigido al usuario; identificadores y código en inglés.
- No hacer commit ni push sin que el usuario lo pida.
- Cuando algo no se pueda verificar, decirlo.
- Los modelos los elige el usuario; proponer con datos verificados (API de Hugging Face para repos y tamaños), no inventar ids de repos.
