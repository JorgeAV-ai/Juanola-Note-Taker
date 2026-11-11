# Plan: notas de reuniones con IA local, en Rust y con modelos open source

Fecha: 6 de noviembre de 2026; reescrito el 11 de noviembre con el alcance y el vocabulario propios de JuanolaNT.

Los modelos concretos quedan fuera de este plan: cada sección de IA define un "slot" (qué hace, qué formato de pesos necesita el runtime y qué latencia exige) y tú eliges el modelo que lo ocupa.

> **Estado (11 nov).** Este es el plan de partida. Lo que ya se decidió y construyó no se repite aquí: el stack y los runtimes en uso están en [HANDOFF.md](HANDOFF.md) («Código, crates y runtimes»), los modelos elegidos en [modelos-mvp.md](modelos-mvp.md) y el avance de cada fase en §12.

---

## 0. Resumen ejecutivo

**Qué es JuanolaNT.** Un cuaderno de reuniones sin bot: captura el micrófono y el audio del sistema del propio ordenador, transcribe en vivo y, al terminar, une las notas del usuario con la transcripción en un **resumen** en el que cada línea apunta al momento de la reunión del que sale. Todo corre en el equipo: transcripción, resumen, búsqueda y, más adelante, chat. Sin cuenta, sin servidor y sin telemetría.

**Decisión de arquitectura.** Tauri v2 + núcleo Rust (workspace de crates) + frontend web con editor TipTap/ProseMirror. El editor de texto rico es el componente más caro de construir, y en web está resuelto; todo lo demás (audio, ASR, LLM, datos, calendario, detección de reuniones) va en Rust. Las alternativas evaluadas están en la sección 3.

**Qué implica hacerlo en local.** ASR en streaming, diarización, LLM y embeddings corren en el equipo. Eso obliga a gestionar modelos (descarga, verificación, elección según el hardware) y a cuidar el rendimiento en GPU y CPU, y permite cosas que una app en la nube no ofrece: funcionar sin conexión, editar la transcripción, importar audio y exportar todo en formatos abiertos.

**Fases.** Seis fases, de un MVP que graba y resume a un producto completo. Detalle en la sección 12.

---

## 1. Alcance del producto

### 1.1 Plataformas

| Plataforma | Plan |
|---|---|
| macOS 14+ (Apple Silicon) | Plataforma principal: captura de micrófono y audio del sistema, STT en MLX. |
| Windows 10/11 | Después del MVP: WASAPI loopback y modelos ONNX/GGUF. |
| Linux | Extra: PipeWire/Pulse. |
| Móvil | Fuera de alcance por ahora. |

### 1.2 Inicio

- **Agenda**: las próximas reuniones del calendario, paginadas. Un clic crea la nota de esa reunión. Se ocultan los eventos rechazados y las entradas de ausencia, concentración o ubicación de trabajo.
- **Notas por día**: filas con título, hora y asistentes (los que sobran se colapsan en «+N»).
- **Barra lateral**: buscar, Inicio, Mis notas, Chat, carpetas, Papelera y Ajustes. Solo lo que tiene sentido en una app local sin cuentas.
- **Preguntar**: barra abajo para preguntar sobre las reuniones; abre el chat.
- Búsqueda con `⌘K`.

### 1.3 La nota durante la reunión

- Lienzo limpio, título editable en línea.
- Editor con Markdown ligero: encabezados, viñetas, casillas, negrita, cursiva, imágenes.
- Control de grabación abajo: nivel de audio, Pausar/Reanudar y Parar; abre el panel de la transcripción en vivo (Yo / Ellos por canal), con copiar todo, búsqueda e idioma de la reunión.
- Aviso claro si falla el permiso de micrófono o de audio del sistema.

### 1.4 La nota después

- Al parar se genera el resumen. Se puede volver a las notas propias o al resumen.
- En el resumen se distingue lo que escribió el usuario de lo que añade el modelo; lo que el usuario edita pasa a ser suyo.
- Cada línea del modelo lleva una cita que abre el fragmento exacto de la transcripción.
- Plantillas: Auto por defecto y plantillas propias (ver [plantillas.md](plantillas.md)); regenerar pide confirmación si hay ediciones.
- Copiar el resumen en Markdown y, más adelante, exportar.

### 1.5 Chat

- Preguntas sobre una reunión, una carpeta o todo el historial, con citas a la fuente. Todo con el modelo local.
- Historial de chats. Sugerencias rápidas (tareas, correo de seguimiento).

### 1.6 Plantillas y prompts guardados

- Plantilla en tres partes: propósito, estilo y secciones.
- Prompts guardados que se lanzan sobre una o varias reuniones (fase 3).

### 1.7 Organización

- Carpetas de un nivel; una nota puede estar en varias. Papelera con restaurar y borrar para siempre.
- Personas a partir de los asistentes del calendario (fase 4).

### 1.8 Ajustes

- Preferencias (idioma de la interfaz, idioma de las reuniones, arranque), audio y permisos, calendario, notificaciones, **Modelos** (descarga, estado y espacio) y datos (carpeta local, exportar, retención).

### 1.9 Notificaciones y detección de reuniones

- Ventanas propias de la app en la esquina superior derecha.
- **Calendario**: aviso un minuto antes, solo para eventos con dos o más asistentes; abre la videollamada y la nota y empieza a transcribir.
- **Llamadas sin evento**: se detecta que otra app usa el micrófono (Slack, FaceTime, WhatsApp, Zoom, Teams, Meet…) y se ofrece tomar notas. Si ocurre poco después de un evento, se asocia a él.
- Nunca graba sin una nota abierta.

### 1.10 Hablantes

Por defecto «Yo» y «Ellos» por canal. Más adelante: nombres por Accesibilidad en las apps de escritorio de videollamada y diarización acústica como respaldo.

### 1.11 Dictado

Atajo global para dictar en cualquier app con el STT local; limpieza opcional con el LLM pequeño (fase 5).

### 1.12 Consentimiento

Recordatorio para pedir permiso antes de transcribir a otros.

### 1.13 MCP, API e integraciones (fase 6)

- Servidor MCP local para consultar reuniones, carpetas y transcripciones desde otros clientes.
- API REST local y exportaciones; integraciones opcionales con claves del usuario.

---

## 2. Diseño local

| Capacidad | En JuanolaNT |
|---|---|
| ASR en streaming | Modelo local en streaming sobre dos canales (micrófono y sistema) con VAD |
| Re-transcripción de calidad | Pase batch opcional al terminar con un modelo mayor |
| Hablantes | «Yo / Ellos» por canal; nombres por Accesibilidad y diarización acústica como mejora |
| Resumen, chat, prompts | LLM local (GGUF) con salida estructurada; endpoint OpenAI-compatible opcional |
| Búsqueda | FTS local (tantivy) y embeddings locales para el chat |
| Compartir y sincronizar | Local-first; servidor opcional self-hosted en fase 6 |
| Audio | No se guarda por defecto; guardado opcional para re-transcribir |

El "motor" incluye un gestor de modelos (descarga, verificación, elección del backend de GPU), que en la interfaz vive en Ajustes › Modelos.

---

## 3. Decisión de stack

### 3.1 Opción elegida: Tauri v2 + Rust core + frontend web (React + TipTap)

**Por qué.**
- El editor de notas (Markdown ligero, imágenes, menú `/`, marca de texto del modelo, enlaces de cada línea a la transcripción) es el componente más caro de construir. TipTap/ProseMirror lo resuelve con extensiones; en Rust nativo no existe nada equivalente maduro (ver 3.3).
- Hay proyectos open source con el mismo stack (anarlog, Meetily) con código de referencia para captura de audio, plugins de Tauri, descarga de modelos y empaquetado.
- Tauri v2 tiene plugins oficiales para lo periférico: global-shortcut, notification, tray, autostart, updater, single-instance, window-state, deep-link, store, clipboard, dialog.
- Todo el valor diferencial (audio, ASR, diarización, LLM, datos, calendario, detección, MCP) está en Rust puro, reutilizable desde cualquier UI futura.

**Coste.** El frontend es TypeScript.

### 3.2 Alternativa: Dioxus 0.7/0.8 (UI en Rust sobre webview) — descartada

Permite escribir la UI en Rust con sintaxis tipo React. Problema: el editor de texto rico; habría que embeber ProseMirror vía interop JS o escribir un editor propio. Dioxus Native sigue en alpha.

### 3.3 Alternativa: 100 % nativo (GPUI + gpui-component, o Iced) — descartada

GPUI rinde muy bien, pero no trae un editor WYSIWYG por bloques con imágenes, menús `/` y decoraciones por tramo. Habría que construirlo desde cero: el editor por sí solo duplicaría el esfuerzo total.

### 3.4 Stack concreto

> Lista de partida, con alternativas por capa. Lo que se usa de verdad está en [HANDOFF.md](HANDOFF.md), «Código, crates y runtimes» (por ejemplo, el STT en Mac va con `mlxrs` y Qwen3-ASR, no con sherpa-onnx ni whisper).

**Rust (núcleo, workspace de crates)**
- Runtime: `tokio`. Errores: `thiserror`/`anyhow`. Logs: `tracing`.
- Audio: `cpal` (micrófono en los tres SO; WASAPI loopback en Windows; PipeWire/Pulse `.monitor` en Linux), `screencapturekit` (audio del sistema en macOS 13+), Core Audio process taps vía `objc2`/`coreaudio-sys` (macOS 14.2+), `rubato` (resampling a 16 kHz mono), `hound`/`symphonia`, cancelación de eco entre canales.
- ASR y voz: `sherpa-onnx`, `whisper-rs` o `transcribe-rs`; diarización con `speakrs`.
- LLM: `llama-cpp-2` (GGUF, Metal/CUDA/Vulkan; GBNF y JSON schema) por defecto; `mistral.rs` como alternativa; cliente OpenAI-compatible (`reqwest`) para Ollama/LM Studio.
- Embeddings: `fastembed` o llama.cpp con un modelo GGUF; índice en `sqlite-vec` o `usearch`.
- Datos: `sqlx` (SQLite, migraciones), `tantivy` (FTS), `keyring`, `serde`.
- Calendario: EventKit en macOS; Google Calendar y Microsoft Graph con OAuth PKCE.
- Detección de reuniones: uso del micrófono por proceso (Core Audio en macOS; `IAudioSessionManager2` en Windows; PipeWire en Linux) y apps de videollamada por bundle id.
- Accesibilidad (nombres de hablante, contexto del dictado): `objc2-application-services` en macOS; UI Automation en Windows.
- MCP: `rmcp`. API: `axum`.
- Tauri: `tauri` 2 + `tauri-specta` + plugins oficiales.

**Frontend**
- React + TypeScript + Vite + Tailwind. Editor: TipTap (StarterKit, TaskList, Image, Placeholder, menú `/`, marca para el texto del modelo, nodo de cita a la transcripción). Estado: Zustand.

---

## 4. Arquitectura

```
juanola/
├─ apps/
│  └─ desktop/            # Tauri v2 (src-tauri + src)
├─ crates/
│  ├─ core-types/         # Modelos de dominio (Note, Transcript, Segment, Meeting, Person, Folder, Template)
│  ├─ db/                 # SQLite (sqlx), migraciones, repos
│  ├─ search/             # tantivy FTS + índice vectorial
│  ├─ audio-capture/      # Captura mic + sistema por SO, resample, niveles
│  ├─ audio-store/        # Grabación a disco (opcional)
│  ├─ stt/                # Transcripción en streaming y batch
│  ├─ diarize/            # Diarización y fusión con nombres por Accesibilidad
│  ├─ llm/                # Trait `ChatModel` + backends llama.cpp, openai-compatible
│  ├─ embed/              # Trait `Embedder`
│  ├─ enhance/            # Pipelines: resumen, plantillas, chat, seguimiento
│  ├─ prompts/            # Plantillas de prompts versionadas
│  ├─ calendar/           # Proveedores de calendario, agenda, recurrencia
│  ├─ detect/             # Detección de reuniones (mic en uso, apps)
│  ├─ speaker-tags/       # Accesibilidad: nombres de hablante con timestamps
│  ├─ dictation/          # Dictado: hotkey, captura, STT, limpieza, pegado
│  ├─ models/             # Catálogo, descarga (HF), verificación SHA, elección de backend
│  ├─ mcp-server/         # rmcp: consultar reuniones desde otros clientes
│  ├─ api-server/         # axum: REST local
│  └─ integrations/       # Integraciones opcionales
└─ models.toml            # Catálogo de modelos por slot
```

**Principios.**
- Local-first: SQLite y ficheros en `~/Library/Application Support/<app>` (o equivalente). Sin cuenta.
- Traits por capacidad (`Transcriber`, `Diarizer`, `ChatModel`, `Embedder`) para que cambiar de modelo o runtime sea configuración, no código.
- Un supervisor por sesión de grabación coordina captura → VAD → STT → persistencia → eventos al frontend.
- Todo lo que cruza a la UI va tipado.

---

## 5. Captura de audio

Dos fuentes separadas en la transcripción («Yo» y «Ellos»):

| SO | Micrófono | Audio del sistema | Permisos | Notas |
|---|---|---|---|---|
| macOS 14.2+ | `cpal` (CoreAudio) | **Core Audio process tap** sobre la salida por defecto (`AudioHardwareCreateProcessTap`) | Micrófono + permiso de audio del sistema | Recomendado. |
| macOS 13 – 14.1 | `cpal` | `screencapturekit` (SCStream solo audio) | Micrófono + grabación de pantalla y audio | Respaldo. |
| Windows 10/11 | `cpal` (WASAPI) | WASAPI loopback de la salida por defecto | Automáticos | Opcional: process loopback para capturar solo la app de la llamada. |
| Linux | `cpal` (PipeWire/Pulse) | fuente `.monitor` del sink por defecto | Ninguno | Extra. |

Pipeline común: dos streams → resample a 16 kHz mono → cancelación de eco si no hay auriculares → niveles a 20 Hz → VAD por canal → ventanas al STT. Reconexión automática al cambiar de dispositivo (Bluetooth, docks).

---

## 6. ASR, VAD y diarización (slots de modelo)

> Modelos elegidos para cada slot: [modelos-mvp.md](modelos-mvp.md). Resultados medidos: [experimentos.md](experimentos.md), «STT».

| Slot | Función | Requisitos | Formato / runtime |
|---|---|---|---|
| **VAD** | Cortar habla por canal; parada automática por silencio | <10 ms por frame en CPU | Silero (MLX u ONNX) |
| **STT streaming** | Transcripción en vivo, 2 canales en paralelo, latencia de pocos segundos | Multilingüe, parciales y finales | MLX en Mac; ONNX o whisper.cpp en el resto |
| **STT batch** (opcional) | Re-transcripción de calidad al terminar | Sin límite estricto de latencia | GGML u ONNX |
| **Puntuación** | Si el modelo streaming no la da | CPU | ONNX |
| **Diarización** | Separar hablantes dentro de «Ellos» y en reuniones presenciales | Offline al terminar | `speakrs` o sherpa-onnx |
| **Embeddings de hablante** | Reconocer hablantes recurrentes | CPU | ONNX |
| **Vocabulario** | Términos y nombres propios del usuario | Contexto en el prompt del STT | — |

Hablantes: primero nombres por Accesibilidad; si no hay, diarización acústica; si no, «Yo / Ellos».

Gestión de modelos: catálogo `models.toml` por slot con repo de Hugging Face, SHA-256, tamaño y backends; descarga con progreso en Ajustes › Modelos; elección automática según plataforma y memoria.

---

## 7. LLM: resumen, chat, prompts y seguimiento (slots de modelo)

> El resumen tal como funciona hoy (Markdown con citas, pasadas de detalle, post-proceso en código) está en [HANDOFF.md](HANDOFF.md), «El enhance hoy»; el punto 1 de abajo es el diseño de partida.

| Slot | Función | Requisitos |
|---|---|---|
| **LLM principal** | Resumen, plantillas, chat, prompts guardados, correos de seguimiento | Contexto ≥ 32k (una hora de reunión son ~10-15k tokens), salida estructurada, streaming |
| **LLM pequeño** | Limpieza del dictado, títulos, tareas en vivo | Latencia < 1 s para 100 tokens |
| **Embeddings de texto** | Búsqueda semántica y chat | CPU, multilingüe |
| **Reranker** (opcional) | Mejores citas en el chat | CPU |

Runtime por defecto `llama-cpp-2` (GGUF). Opcional: endpoint OpenAI-compatible propio en Ajustes.

**Pipelines (crate `enhance`).**
1. **Resumen**: entrada = notas del usuario (ProseMirror → Markdown con ids de bloque), transcripción segmentada con tiempos y canal, datos del evento, plantilla, perfil e idioma. Salida validada: secciones → viñetas, cada una con su fuente (nota del usuario o tramo de la transcripción). El frontend pinta la distinción y las citas a partir de esa fuente.
2. **Chat**: alcance → recuperación (FTS + vectorial) → prompt con citas numeradas → streaming.
3. **Prompts guardados**: prompt del usuario + alcance, como el chat.
4. **Plantillas**: se inyectan en el prompt del resumen como propósito / estilo / secciones.
5. **Correo de seguimiento**: borrador editable; envío por `mailto:` o un conector opcional.
6. **Chat en vivo**: sobre la transcripción parcial, sin persistir.

---

## 8. Datos

**SQLite** (sqlx, WAL): `workspaces`, `spaces`, `folders`, `folder_notes`, `notes`, `segments` (note_id, canal, hablante, t0, t1, texto, confianza), `speakers`, `people`, `calendar_events`, `templates`, `chats`, `settings`, `models`. (`workspaces` y `spaces` quedan como estructura interna; la interfaz no los muestra.)

**Búsqueda**: tantivy sobre título, notas, resumen y segmentos; vectores por chunk de ~300 tokens.

**Ficheros**: `audio/<note_id>/` solo si el usuario activa guardar audio; `models/<slot>/<name>/`.

**Retención**: papelera y borrado automático configurable de transcripciones.

---

## 9. Calendario, detección y notificaciones

- **Calendario**: EventKit en macOS; Google y Microsoft con OAuth PKCE y sincronización incremental. Filtro de rechazados y ausencias; recurrentes vinculados por id.
- **Notificaciones**: ventanas propias, sin decoración, siempre encima, esquina superior derecha. Un minuto antes y con 2+ asistentes; para llamadas sin evento, título según la app; asociación a un evento cercano.
- **Parada automática**: evento terminado + app de la llamada cerrada + silencio prolongado + suspensión.

---

## 10. Interfaz

Identidad propia de JuanolaNT: el icono del rombo, el fondo del DMG y la paleta «regaliz» (papel crema, negro regaliz y acento eucalipto). Vocabulario propio: Resumen, Agenda, Preguntar, Mis notas, Carpetas.

Componentes:

- Estructura: barra lateral plegable, área principal, barra de preguntar abajo, modo oscuro y atajos (`⌘K`, `⌘N`, `⌘,`, Esc).
- Inicio: agenda paginada y notas por día.
- Nota: editor TipTap con la marca del texto del modelo, citas con el fragmento de la transcripción, alternar notas propias / resumen, plantillas y regenerar.
- Control de grabación + panel de transcripción (Yo / Ellos, copiar todo, búsqueda, idioma).
- Chat: historial, citas y sugerencias.
- Carpetas, Papelera y Ajustes (con **Modelos**).
- Primer arranque: permisos (micrófono, audio del sistema, calendario) y descarga de modelos.

---

## 11. MCP, API, integraciones, compartir

- **MCP local** (`rmcp`, en `localhost` con token local): buscar y listar reuniones, carpetas y transcripciones.
- **API REST local** (`axum`): notas, búsqueda, transcripción, carpetas.
- **Integraciones** opcionales con claves del usuario en `keyring`.
- **Compartir**: fase 6, servidor opcional self-hosted con enlaces de solo lectura y sincronización cifrada. Hasta entonces: exportar Markdown/HTML/PDF y copiar el resumen.

---

## 12. Roadmap por fases

Estado a 11 nov: ✅ hecho · 🟡 en parte · ⬜ sin empezar. Todo lo hecho es en Mac; Windows y Linux están sin empezar.

**Fase 0 — Cimientos** 🟡 workspace, esquema SQLite, gestor de modelos con descarga desde HF, CI y releases hechos; firma, notarización y `tauri-specta`, pendientes.
- Workspace Rust + Tauri v2; CI para macOS (y más adelante Windows y Linux); firma y notarización.
- Esquema SQLite inicial, gestor de modelos con `models.toml`.
- Elección y medición de los modelos por slot.

**Fase 1 — MVP que graba y resume** ✅ en Mac (captura dual, STT en dos canales, editor, resumen con citas, Inicio, papelera, ajustes y modelos), salvo la parada automática por silencio y el respaldo con ScreenCaptureKit.
- Captura dual en macOS y Windows; niveles.
- STT streaming en dos canales con VAD; transcripción Yo / Ellos; Pausar / Reanudar.
- Editor TipTap con Markdown ligero e imágenes.
- Resumen con LLM local y citas a la transcripción; plantilla Auto.
- Inicio con notas por día; nota nueva; papelera.
- Ajustes: Modelos, idioma, preferencias básicas.

**Fase 2 — Calendario y detección** 🟡 detección de llamadas, calendario de macOS (EventKit), agenda, avisos y «Tomar notas» hechos; Google y Microsoft directos, indicador flotante y tray, pendientes.
- Google y Microsoft Calendar, avisos propios, asociación evento↔nota, recurrentes.
- Detección del micrófono en uso y de apps de videollamada.
- Indicador flotante y tray.

**Fase 3 — Plantillas, chat y prompts guardados** 🟡 núcleo de plantillas y cinco integradas hechos, sin conectar a la app ([plantillas.md](plantillas.md)); búsqueda ⌘K por texto hecha; chat, prompts guardados y embeddings sin empezar.
- Elegir plantilla y regenerar; gestionar plantillas; `/` en la nota.
- FTS + embeddings; chat por nota, carpeta o historial, con citas e historial.
- Prompts guardados.

**Fase 4 — Organización** 🟡 carpetas y papelera hechas; lo demás sin empezar.
- Subcarpetas, auto-añadir recurrentes a una carpeta, multiselección.
- Personas desde los asistentes.
- Exportar Markdown/HTML/CSV; retención y borrado automático.

**Fase 5 — Hablantes, dictado y seguimiento** ⬜
- Nombres por Accesibilidad en las apps de videollamada; diarización acústica como respaldo.
- Dictado con atajo global, modo manos libres y limpieza con el LLM.
- Correos de seguimiento.
- Recordatorio de consentimiento.

**Fase 6 — Ecosistema** ⬜
- MCP y API locales; integraciones.
- Servidor opcional self-hosted para compartir y sincronizar.
- Linux; importar audio/SRT/VTT; editar la transcripción.
- Móvil (evaluar más adelante).

---

## 13. Riesgos y mitigaciones

| Riesgo | Mitigación |
|---|---|
| Latencia del STT local en CPU sin GPU | Modelos pequeños cuantizados en streaming; batch de calidad al terminar; elección por hardware |
| Captura de audio del sistema en macOS < 14.2 y permisos TCC | Doble ruta (tap / SCK) y un primer arranque que explica cada permiso |
| cpal y loopback WASAPI inestables | `wasapi` directo en Windows |
| Resumen de 1 h de reunión con contexto limitado | Ventanas con solapamiento; segmentos con ids para mantener las citas |
| Nombres por Accesibilidad que cambian con cada versión de Zoom/Teams | Selectores por versión y diarización como respaldo |
| Tamaño del bundle (modelos + runtimes GPU) | Los modelos se descargan tras instalar; runtimes por feature flags |
| Licencias de modelos y de código de referencia | Verificar la licencia de cada modelo; el código de referencia open source (MIT/Apache-2) se lee y se adapta con atribución |

---

## 14. Fuentes

Proyectos open source de referencia: [anarlog / Hyprnote](https://github.com/fastrepl/hyprnote) y [docs](https://docs.anarlog.so), [Meetily](https://github.com/Zackriya-Solutions/meetily), [Vexa desktop](https://github.com/Vexa-ai/vexa-desktop), [hex](https://github.com/anomalyco/hex).

Rust, audio: [screencapturekit](https://crates.io/crates/screencapturekit), [scrybe-meeting-capture-mac (tap vs SCK)](https://docs.rs/scrybe-meeting-capture-mac/latest/scrybe_capture_mac/), [recall.ai sobre system audio](https://www.recall.ai/blog/how-to-get-access-to-system-audio), [cpal](https://crates.io/crates/cpal), [cpal loopback issue](https://github.com/RustAudio/cpal/issues/476), [wasapi](https://docs.rs/wasapi), [dual-track WASAPI](https://dev.to/madnessbrains_i/building-a-dual-track-audio-pipeline-on-windows-mic-system-audio-without-joining-the-call-1d7a), [side-huddle (detección)](https://docs.rs/crate/side-huddle/0.2.1/source/src/lib.rs).

Rust, voz e IA: [sherpa-onnx docs](https://k2-fsa.github.io/sherpa/onnx/index.html), [sherpa-onnx crate](https://crates.io/crates/sherpa-onnx), [transcribe-rs](https://github.com/cjpais/transcribe-rs), [whisper-rs](https://openapps.pro/packages/whisper-rs), [yamabiko-whisper](https://docs.rs/yamabiko-whisper), [wavo](https://github.com/Dtatarchuk/wavo), [speakrs](https://docs.rs/speakrs), [pyannote](https://github.com/pyannote/pyannote-audio), [mistral.rs review](https://hysenlabs.com/projects/ericlbuehler-mistral-rs), [Rust LLM ecosystem](https://hackmd.io/@Hamze/Hy5LiRV1gg), [fastembed-rs](https://hnshowcase.com/post/48538943/fastembed-rs-rust-library-for-generating-vector-embeddings-r), [rmcp](https://docs.rs/crate/rmcp/3.1.0/source/README.md).

Rust, UI y plataforma: [Rust GUI 2026 (gpui-starter)](https://gpui-starter.freeoxide.com/blog/rust-gui-frameworks-comparison-2026/), [survey 2026](https://blog.wybxc.cc/blog/rust-gui-survey-2026/), [Dioxus 0.7](https://dioxuslabs.com/blog/release-070), [twrite-gpui](https://docs.rs/crate/twrite-gpui/0.9.2), [Tauri sidecar](https://stuffbucket.github.io/skills/catalog/tauri-sidecar/), [Tauri local LLM](https://codenote.pages.dev/en/posts/tauri-cross-platform-app-with-local-llm), [eventkit-rs](https://docs.rs/eventkit-rs), [oauth2 msgraph example](https://rust-digger.code-maven.com/src/oauth2-5.0.0-rc.1/examples/msgraph.rs), [oauth-toolkit](https://docs.rs/oauth-toolkit).
