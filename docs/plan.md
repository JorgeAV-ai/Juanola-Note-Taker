# Plan: réplica de Granola en Rust con modelos open source

Fecha: 6 de noviembre de 2026. Investigación hecha sobre la documentación oficial de Granola (docs.granola.ai, 135 páginas indexadas), su web de producto y precios, reseñas de 2026, y el código de los tres clones open source escritos en Rust + Tauri (anarlog, antes Hyprnote; Meetily; Open Granola).

Los modelos concretos quedan fuera de este plan: cada sección de IA define un "slot" (qué hace, qué formato de pesos necesita el runtime y qué latencia exige) y tú eliges el modelo que lo ocupa.

> **Estado (10 nov).** Este es el plan de partida (6 nov), y su inventario de Granola (§1) sigue siendo la referencia de qué replicar. Lo que ya se decidió y construyó no se repite aquí: el stack y los runtimes en uso están en [HANDOFF.md](HANDOFF.md) («Código, crates y runtimes»), los modelos elegidos en [modelos-mvp.md](modelos-mvp.md) y el avance de cada fase en §12.

---

## 0. Resumen ejecutivo

**Qué es Granola hoy.** Un notepad de reuniones sin bot: captura micrófono + audio del sistema del propio ordenador, transcribe en tiempo real en la nube (AssemblyAI entre sus proveedores, según su propio post-mortem de seguridad), y al terminar fusiona tus notas crudas con la transcripción en unas "notas mejoradas" (texto tuyo en negro, texto de la IA en gris, cada bullet enlazado al momento del transcript). Encima de eso: Chat con una reunión, una carpeta o todo el historial; Recipes (prompts guardados invocados con `/`); Templates; Spaces y Folders; People & Companies; Briefs pre-reunión; follow-up emails; Voice Typing (dictado en cualquier app); speaker tags vía Accesibilidad; MCP y API; apps móviles y watch.

**Dato clave para la UI.** Granola es una app Electron (lo confirman sus propias ofertas de empleo "Product Engineer (Desktop/Electron)" y el teardown de Desktop Insights: Electron 41, build universal de ~540 MB). Su interfaz es HTML/CSS. Por tanto, la manera más fiable de clonar la UI "exactamente igual" es renderizarla también en un webview, con el núcleo (audio, ASR, LLM, datos, calendario, detección de reuniones) en Rust.

**Decisión de arquitectura recomendada.** Tauri v2 + núcleo Rust (workspace de crates) + frontend web con editor TipTap/ProseMirror. Es exactamente el stack de anarlog y Meetily, los dos clones con más tracción, y minimiza el riesgo en el punto más caro: el editor de texto rico. Hay dos alternativas evaluadas en la sección 3 (Dioxus para "UI también en Rust"; GPUI para 100 % nativo), con sus costes.

**Qué cambia respecto a Granola.** Todo lo que Granola hace en la nube pasa a local: ASR en streaming, diarización, LLM, embeddings. Eso obliga a decisiones que Granola no tiene (gestión de modelos, GPU por plataforma, rendimiento en CPU) y permite cosas que Granola no ofrece (guardar audio, editar transcripts, importar archivos, funcionar offline).

**Fases.** Seis fases, de un MVP grabable y "enhanceable" a la paridad completa. Detalle en la sección 12.

---

## 1. Inventario de Granola (lo que hay que replicar)

### 1.1 Plataformas y planes

| Plataforma | Estado en Granola |
|---|---|
| macOS 14+ | App principal (Electron). Captura mic + system audio. |
| Windows 10+ | App completa desde junio 2025 (Electron). Sin menú de audio propio. |
| iOS / Android / Apple Watch | Solo reuniones presenciales y llamadas telefónicas (iOS). Sin captura de Zoom/Meet. |
| Web (notes.granola.ai) | Solo ver y editar notas compartidas, sin transcribir. |

Planes: Basic (gratis, historial de 30 días, chat solo "Auto"), Business (14 $/usuario/mes), Enterprise (35 $). El plan no afecta a este proyecto salvo para decidir qué es "core" (todo lo de Basic + Business).

### 1.2 Pantalla de inicio (Home)

Cinco áreas, según la guía oficial "Granola 101":

1. **Coming up**: próximas reuniones del calendario, cinco a la vez, con flechas para paginar. Clic crea la nota de esa reunión. Oculta eventos rechazados y entradas tipo Out of Office / Focus Time / Working Location.
2. **Your notes** (lista cronológica "Recent meetings"): filas, no tarjetas. Cada fila: título, hora, asistentes (los extra colapsados en "+N"). Hover muestra checkbox para multiselección (Shift+clic para rangos, "Select all") y menú de tres puntos (Add to folder, Move to trash…).
3. **Sidebar izquierda**: dos Spaces fijos, **My notes** (privado) y **Team space** (nombre del workspace); espacios custom (admins); carpetas con icono y descripción, botón **+** junto a cada space, subcarpetas de un nivel por drag & drop; **Customer calls**; **Recipes**; icono **Views** abajo a la izquierda (People, Companies); icono de **Trash** al final solo cuando hay algo en la papelera; nombre del workspace abajo a la izquierda que abre el menú (switcher de workspaces, Add workspace, Settings, Manage templates, Manage spaces, cuenta).
4. **Barra "Ask anything"** (Granola Chat) en la parte inferior; con `/` abre el menú de recipes.
5. **Banners**: "Suggested follow-up emails" (3 días), "Connect Gmail", nota demo en el primer arranque.

Paleta de comandos: `⌘K` / `Ctrl+K` ("Go to people", "Go to companies", etc.).

### 1.3 Vista de nota (durante la reunión)

- Lienzo en blanco estilo Apple Notes, sin toolbar superior. Título editable inline.
- Editor con Markdown ligero: `#` encabezados, `-`/`*` bullets, `[]` checkbox, `**negrita**`, `*cursiva*`, ⌘B/⌘I/⌘U, pegar o arrastrar imágenes JPEG/PNG.
- `/` en la nota abre el menú contextual de **templates** (se aplica al generar).
- **Píldora de grabación** abajo-centro, encima del composer del chat: barras verdes animadas mientras captura; botón **Stop/Resume**; icono de forma de onda a la izquierda del "Ask anything" que abre/cierra el **panel de transcript** (drawer de ~360-420 px): burbujas grises a la izquierda = audio del sistema ("Them"), verdes a la derecha = micrófono ("Me"); **Copy all** arriba a la derecha; clic en un chunk permite copiar o borrar; búsqueda; selector de micrófono al pie (Auto sigue al mic de la app de reunión). Aviso naranja si la transcripción falla.
- **Indicador flotante de reunión**: cuando otra app tiene el foco, la píldora se "despega" y flota en el borde derecho de la pantalla con un asa de arrastre abajo; clic vuelve a la nota. Se desactiva en Settings > Preferences > General.
- **Imágenes / snapshots** (solo macOS 14+): stack "Images" al pie de la nota, muestra "Capturing", clic expande, hover + × borra; "Turn off for this meeting"; "Paused screen capture" cuando la ventana de la reunión no es visible.
- **Chat en vivo** (`⌘J` / `Ctrl+J`): preguntas sobre lo hablado hasta el momento ("What did I miss?"), prompts rápidos. Historial efímero de la reunión.
- **Brief** (Business+): arriba del panel de chat, 2-3 bullets, clic para expandir, pulgar abajo para feedback, banner "Connect Gmail".
- Cambio de idioma de transcripción desde los controles de la reunión.

### 1.4 Vista de nota (después)

- Auto-stop al detectar fin de llamada (longitud del transcript, app de llamada en uso, hora de fin del evento, 15 min sin audio, suspensión). Botón Stop manual. Resume desde el panel de transcript.
- **Enhance notes**: genera automáticamente al parar. Pestañas **My notes** / **Enhanced** (icono de tres líneas horizontales vuelve a las notas crudas). Estados "Transcribing", "Enhancing notes", "Generate notes".
- Texto del usuario en negro, texto IA en gris; ediciones del usuario vuelven a negro.
- Cada bullet IA lleva una **lupa** al hover que abre un popover con el fragmento del transcript de origen.
- Botón **✨** arriba a la derecha: selector de template (defaults por tipo de usuario: 1:1, Stand-up, Weekly team meeting, Customer interview…; "Auto" por defecto; "All templates…"); **🔁** regenera con el mismo template.
- **Shared**: muestra quién tiene acceso; **Copy link** con dropdown de acceso (Private / Only my company / Anyone with the link); roles Viewer (lee notas, hover de contexto, chat; no transcript) y Collaborator (edita y aplica templates); **Request access** / **Grant access**. Menú **⋯**: Copy enhanced notes (Markdown), Send notes via email, enviar a Slack / Notion / HubSpot / Attio / Affinity / Salesforce / Zapier.
- Acciones rápidas post-reunión: List actions, Write follow-up email, Draft project plan.
- Edición de asistentes. "View draft" del follow-up email arriba de la nota.

### 1.5 Granola Chat

- Siempre abajo ("Ask anything"). Alcance implícito según dónde estés: home = todas las reuniones; nota individual; carpeta/space; selección múltiple; persona o empresa.
- Botón de adjuntar archivos; seleccionar texto de la nota antes de preguntar lo enfoca; "rewrite…" reescribe secciones de la nota.
- Historial de chats con papelera al hover. Privado aunque la nota esté compartida.
- Selector de modelo bajo el cuadro ("Models": Auto en Basic; lista en Business; indicador "Thought for Xs" expandible).
- **Workflows / Chat actions** (Settings > Connectors): redactar y enviar email (Gmail), mensaje Slack, crear evento (Google Calendar) con borrador editable y confirmación explícita.
- Citas con enlaces a la fuente.

### 1.6 Recipes

- Menú `/` en el chat: recipes por defecto, mías y del equipo. Pestaña **Recipes** en el sidebar con secciones **Discover**, **My recipes**, **Workspace recipes** y buscador (título y texto).
- Formulario: Name (es el atajo), Prompt, Availability (Single meetings / Multiple meetings), Advanced settings (modelo). Compartir: Private / Anyone in your workspace / Anyone with link. Remix crea copia editable. Sin variables ni placeholders.

### 1.7 Templates

- Prompt en tres partes: propósito y contexto; longitud y estilo; estructura de secciones. Privados por defecto, "private to me" → compartir con la organización (requiere dominio corporativo). "Manage templates" desde el menú del workspace. No hay template por defecto distinto de Auto.

### 1.8 Spaces, Folders, People & Companies, Customer calls

Ver 1.2. Además: auto-add de reuniones recurrentes a una carpeta ("Auto-add future meetings?"), una nota puede estar en varias carpetas, miembros por email, auto-post de carpetas a Slack/HubSpot/Attio/Zapier ("Integrations" en la cabecera de carpeta). People & Companies se construyen solo desde asistentes de eventos de calendario, con foto, cargo y empresa enriquecidos en segundo plano; no se pueden añadir a mano. Customer calls: vista agregada del equipo con "Sharing is on / Stop adding my calls", × por nota, recipes "Gather product feedback", "Weekly revenue review".

### 1.9 Settings (menú del workspace > Settings)

- **Profile**: puesto y contexto para la IA; **Generate CSV** (export de todas las notas, título + resumen + transcript, por email en horas).
- **Preferences**: General (Reposition Granola for meetings, indicador flotante), Features (Suggested follow-up emails, Take snapshots of shared screens), Language (Transcription language: English / Multi-language; Summary language: English / Auto; Personal jargon, 30 términos), Data & sharing (opt-out de entrenamiento, Auto deletion period for transcripts: 1 día … 1 año / Off, con cooldown de una semana), Labs (canal beta), Speaker tags (on por defecto).
- **Calendar**: toggles por calendario; cómo aparecen en Coming up.
- **Notifications**: global y por app.
- **Voice typing**: Enable, Shortcut, Language, Microphone, Sound > Mute other audio, Screen context, Dictation styles, historial.
- **Templates**, **Recipes**, **Spaces**, **Connectors** (Gmail, Slack, Google Calendar), **Integrations** (Zapier, Notion, HubSpot, Attio, Affinity, Slack), **Workspace** (General: nombre, logo, auto-join por dominio, Data security, Workspace jargon 50 términos, Danger zone; Members: roles Admin/Member, invitaciones), **Billing**, **Troubleshooting** (Run check de red).

### 1.10 Notificaciones y detección de reuniones

- Ventanas propias de la app en la esquina superior derecha (no notificaciones nativas).
- **Calendario**: 1 minuto antes, solo eventos con 2+ asistentes; barra sólida de color a la izquierda; clic abre la URL de la videollamada y la nota, y empieza a transcribir.
- **Ad hoc**: detecta uso del micrófono por otra app. Títulos "Huddle detected" (Slack), "Call detected" (FaceTime, WhatsApp), "Meeting detected" (resto); cuerpo con el nombre de la app y botón **Take Notes**; línea discontinua a la izquierda. Si ocurre en los 15 min posteriores a un evento, lo asocia a ese evento.
- Nunca graba sin una nota abierta. Si abres una nota antes de la hora, empieza a la hora programada.

### 1.11 Speaker tags

Por defecto "Me" / "Them". Con permiso de Accesibilidad lee el nombre del participante y el indicador de "hablando" de las apps de escritorio de Zoom Workplace y Microsoft Teams, y de Google Meet en Chrome (o con su extensión). Solo durante la transcripción. No funciona para Zoom en Windows ni en Zoom web (según sus propios docs). Móvil hasta 10 hablantes presenciales por diarización.

### 1.12 Voice Typing (beta, Mac y Windows, gratis)

Mantener `fn` (Mac) o `Ctrl izq + Win` (Windows) y hablar; soltar pega el texto en el cursor. Doble toque = modo manos libres hasta 20 min; Esc cancela. Píldora con forma de onda abajo en pantalla. Limpia muletillas, puntúa, formatea listas, adapta el estilo a la app destino (lee contexto por Accesibilidad; opcional). "Mute other audio" por defecto. Hasta tres idiomas o auto-detect. No sirve para dictar dentro de una reunión.

### 1.13 Transparencia y consentimiento

Mensaje automático en el chat de Zoom/Teams/Meet al empezar, marca de agua en el vídeo (macOS 14+, Windows 11), "Heads Up" por add-on de calendario, recordatorio de consentimiento en presenciales y al empezar snapshots.

### 1.14 Integraciones, MCP y API

- Nativas: Slack, Notion, HubSpot, Attio, Affinity, Gmail, Google Calendar, Outlook. Vía Zapier: Salesforce, Asana, Jira, Linear, GitHub, Google Docs/Drive, OneNote…
- MCP (Streamable HTTP + OAuth 2.0 con registro dinámico): `query_granola_meetings`, `list_meetings`, `list_meeting_folders`, `get_meetings`, `get_meeting_attachment`, `get_meeting_transcript`, `get_account_info`, `switch_workspace`, `sign_out`.
- API REST: List/Search/Get Notes, Get Transcript, List Folders, List Spaces, Webhooks, Audit, Legal Holds.

### 1.15 Lo que Granola NO hace (oportunidades)

No guarda audio ni permite reproducirlo, no permite editar el transcript, no importa archivos de audio, no soporta Apple Calendar, no tiene template por defecto distinto de Auto, no funciona offline, no exporta en Markdown/JSON desde la app (solo CSV por email). Todo esto es trivial o natural en una app local y se marca como "extra" en el roadmap.

---

## 2. Qué cambia al pasar a modelos open source y local

| Capacidad | Granola | Este proyecto |
|---|---|---|
| ASR en streaming | Proveedor cloud (AssemblyAI y otros) | Modelo local en streaming sobre dos canales (mic y sistema) con VAD |
| Re-transcripción de calidad | No existe | Pase batch opcional al terminar con un modelo mayor (Meetily lo llama "Enhance transcript") |
| Diarización | Nombres por Accesibilidad; "Me/Them" como fallback | Igual, más diarización acústica local opcional dentro del canal "Them" |
| Enhance / Chat / Recipes / Briefs | Modelos de OpenAI y Anthropic | LLM local (GGUF) con salida estructurada; opcional endpoint OpenAI-compatible externo para quien quiera |
| Búsqueda | Chat + buscador | FTS local (tantivy) + embeddings locales para RAG en Chat |
| Briefs con web + email | Búsqueda web y Gmail en servidor | Fuentes locales (notas, calendario) por defecto; web y Gmail opcionales y bajo clave propia |
| Sync, compartir, web view, móvil | Backend de Granola | Local-first. Un servidor opcional self-hosted (fase 6) para compartir enlaces y sync E2E |
| Audio | Se borra tras transcribir | Se guarda por defecto (configurable), permite reproducir y re-transcribir |

El resultado debe sentirse idéntico en la UI, pero el "motor" tiene además un gestor de modelos (descarga, verificación, selección de backend GPU) que Granola no necesita. En la UI eso vive en Settings > Models, una sección nueva que en Granola no existe.

---

## 3. Decisión de stack

### 3.1 Opción recomendada: Tauri v2 + Rust core + frontend web (React + TipTap)

**Por qué.**
- Granola es Electron: su UI está hecha con tecnologías web. Replicarla píxel a píxel es directo con el mismo medio.
- El editor de notas (Markdown ligero, imágenes, slash menu, dos tonos de texto, enlaces bullet→transcript) es el componente más caro de construir. TipTap/ProseMirror lo resuelve con extensiones; en Rust nativo no existe nada equivalente maduro (ver 3.3).
- Es el stack de anarlog (ex-Hyprnote, ~9.5k estrellas, MIT) y Meetily (~31k estrellas, MIT): hay código de referencia para captura de audio, plugins de Tauri, descarga de modelos y empaquetado.
- Tauri v2 tiene plugins oficiales para todo lo periférico que Granola usa: global-shortcut, notification, tray, autostart, updater, single-instance, window-state, deep-link, store, clipboard, dialog, sql, stronghold.
- Todo el valor diferencial (audio, ASR, diarización, LLM, datos, calendario, detección, MCP) está en Rust puro, reutilizable desde cualquier UI futura.

**Coste.** El frontend es TypeScript. Si "hacerlo en Rust" es un requisito duro del 100 % del código, ver 3.2.

### 3.2 Alternativa: Dioxus 0.7/0.8 (UI escrita en Rust, renderizada en webview) — descartada (decisión cerrada)

Permite escribir la UI en Rust con sintaxis tipo React y seguir usando CSS idéntico al de Granola. Problema: el editor de texto rico. Habría que embeber ProseMirror vía JS interop (lo que vuelve a meter JS) o escribir un editor propio. Dioxus Native (renderer Blitz sin webview) sigue en alpha y con soporte CSS incompleto a fecha de hoy; no es opción para una UI "exactamente igual". Viable solo si se acepta el editor vía interop.

### 3.3 Alternativa: 100 % nativo (GPUI + gpui-component, o Iced) — descartada (decisión cerrada)

GPUI es el framework de Zed y rinde muy bien, pero en 2026 el propio GPUI no trae widget de entrada de texto rico; `gpui-component` y `twrite` aportan un editor de texto/Markdown, no un editor WYSIWYG por bloques con imágenes, menús `/` y decoraciones por tramo. Habría que construir el editor desde cero. Es la opción más cara (estimación: el editor por sí solo duplica el esfuerzo total) y la que menos garantiza paridad visual. Descartada salvo que el rendimiento nativo sea prioritario sobre la paridad.

### 3.4 Stack concreto (opción recomendada)

> Lista de partida, con alternativas por capa. Lo que se usa de verdad está en [HANDOFF.md](HANDOFF.md), «Código, crates y runtimes» (por ejemplo, el STT en Mac va con `mlxrs` y Qwen3-ASR, no con sherpa-onnx ni whisper).

**Rust (núcleo, workspace de crates)**
- Runtime: `tokio`. Errores: `thiserror`/`anyhow`. Logs: `tracing`.
- Audio: `cpal` (mic en los tres SO; WASAPI loopback en Windows; PipeWire/Pulse `.monitor` en Linux), `screencapturekit` (system audio en macOS 13+), Core Audio process taps vía `objc2`/`coreaudio-sys` (macOS 14.2+, sin permiso de grabación de pantalla), `rubato` (resampling a 16 kHz mono), `hound`/`symphonia` (WAV/MP3/FLAC), `webrtc-audio-processing` o `aec` propio para cancelación de eco entre canales.
- ASR y voz: `sherpa-onnx` (streaming y offline, VAD Silero/TEN, diarización, embeddings de hablante, puntuación; wrapper Rust oficial), `whisper-rs` (whisper.cpp con Metal/CoreML/CUDA/Vulkan) o `transcribe-rs` (multi-motor: Parakeet, Moonshine, Whisper…). Diarización: `speakrs` (pipeline pyannote community-1 en Rust con CoreML/ONNX).
- LLM: `llama-cpp-2` (GGUF, Metal/CUDA/Vulkan; gramáticas GBNF y JSON schema) como backend por defecto; `mistral.rs` como backend alternativo puro Rust; cliente OpenAI-compatible (`reqwest`) para Ollama/LM Studio/servidores externos.
- Embeddings: `fastembed` (ONNX, CPU) o el propio llama.cpp con un modelo de embeddings GGUF; índice vectorial en `sqlite-vec` o `usearch`.
- Datos: `sqlx` (SQLite, migraciones), `tantivy` (FTS), `keyring` (secretos en el llavero del SO), `serde`/`serde_json`.
- Calendario: Google Calendar REST con `oauth2` + loopback PKCE; Microsoft Graph igual; `eventkit-rs` para Apple Calendar (extra). 
- Detección de reuniones: `side-huddle` (detecta Teams/Zoom/Meet por bundle id, proceso y título de ventana en los tres SO) y monitorización de uso del micrófono (Core Audio `kAudioDevicePropertyDeviceIsRunningSomewhere` en macOS; `IAudioSessionManager2` en Windows; PipeWire en Linux).
- Accesibilidad (speaker tags, contexto para Voice Typing): `accessibility-sys` / `objc2-application-services` en macOS; UI Automation vía `windows` crate en Windows.
- MCP: `rmcp` (SDK oficial, Streamable HTTP). API: `axum`.
- Tauri: `tauri` 2 + `tauri-specta` (tipos TS generados desde Rust) + plugins oficiales listados arriba.

**Frontend**
- React + TypeScript + Vite + Tailwind. Editor: TipTap (extensiones: StarterKit, TaskList, Image, Placeholder, slash-menu, marca personalizada `ai-text` para el gris, nodo `transcript-ref` para la lupa). Estado: Zustand o TanStack Query sobre los comandos de Tauri. Animaciones: Framer Motion para la píldora.

---

## 4. Arquitectura

```
juanola/
├─ apps/
│  └─ desktop/            # Tauri v2 (src-tauri + src)
├─ crates/
│  ├─ core-types/         # Modelos de dominio (Note, Transcript, Segment, Meeting, Person, Company, Folder, Space, Recipe, Template)
│  ├─ db/                 # SQLite (sqlx), migraciones, repos, eventos reactivos
│  ├─ search/             # tantivy FTS + índice vectorial
│  ├─ audio-capture/      # Captura mic + system por SO, mezcla, AEC, resample, VU meters
│  ├─ audio-store/        # Grabación a disco (opcional), chunking, snapshots
│  ├─ stt/                # Trait `Transcriber` (streaming/batch) + backends sherpa-onnx, whisper.cpp
│  ├─ diarize/            # Trait `Diarizer` + backend speakrs/sherpa; fusión con speaker tags
│  ├─ llm/                # Trait `ChatModel` (stream, JSON schema) + backends llama.cpp, mistral.rs, openai-compatible
│  ├─ embed/              # Trait `Embedder` + fastembed/llama
│  ├─ enhance/            # Pipelines: enhance notes, templates, recipes, chat RAG, briefs, follow-up, rewrite
│  ├─ prompts/            # Plantillas de prompts (askama/minijinja) versionadas
│  ├─ calendar/           # Google, Microsoft, Apple; cache local, "Coming up", recurrencia
│  ├─ detect/             # Detección de reunión (apps, mic en uso), auto-stop
│  ├─ speaker-tags/       # Accesibilidad: Zoom/Teams/Meet → (nombre, hablando) con timestamps
│  ├─ dictation/          # Voice Typing: hotkey, captura, STT, limpieza LLM, pegado, contexto de app
│  ├─ models/             # Registro de modelos, descarga (HF), verificación SHA, selección de backend GPU
│  ├─ mcp-server/         # rmcp: mismas 9 tools que Granola
│  ├─ api-server/         # axum: REST local equivalente a la API de Granola
│  └─ integrations/       # Slack, Notion, HubSpot, Attio, Affinity, Zapier webhooks, Gmail
├─ plugins/               # Tauri plugins finos que exponen cada crate al frontend (patrón anarlog)
└─ models.toml            # Catálogo de modelos por slot (lo rellenas tú)
```

**Principios.**
- Local-first: SQLite + ficheros en `~/Library/Application Support/<app>` (o equivalente). Sin cuenta obligatoria.
- Traits por capacidad (`Transcriber`, `Diarizer`, `ChatModel`, `Embedder`) para que cambiar de modelo o runtime sea configuración, no código.
- Actores por reunión (`ractor` o canales tokio): un supervisor por sesión de grabación coordina captura → VAD → STT → diarización → persistencia → eventos al frontend. anarlog usa exactamente `ractor` + un supervisor.
- Todo lo que cruza a la UI va tipado con `tauri-specta`.

---

## 5. Captura de audio (el punto más delicado por plataforma)

Granola captura dos fuentes y las mantiene separadas en el transcript (Me/Them). Replicar exactamente eso:

| SO | Micrófono | Audio del sistema | Permisos | Notas |
|---|---|---|---|---|
| macOS 14.2+ | `cpal` (CoreAudio) | **Core Audio process tap** sobre el dispositivo de salida por defecto (`AudioHardwareCreateProcessTap`) | Micrófono + prompt de "tap" (sin Screen Recording, sin punto naranja) | Recomendado. Granola pide "Screen & System Audio Recording" porque usa ScreenCaptureKit; el tap es más limpio. |
| macOS 13 – 14.1 | `cpal` | `screencapturekit` (SCStream solo audio) | Micrófono + Screen & System Audio Recording | Fallback. |
| Windows 10/11 | `cpal` (WASAPI) | WASAPI loopback del render device por defecto (`wasapi` crate o cpal con loopback) | Automáticos | Opcional: process loopback (`ActivateAudioInterfaceAsync` con `PROCESS_LOOPBACK`) para capturar solo Zoom/Teams y excluir música, algo que Granola no puede. |
| Linux | `cpal` (PipeWire/Pulse) | fuente `.monitor` del sink por defecto, o `pw-record` como sidecar | Ninguno | Extra respecto a Granola (que no tiene Linux). |

Pipeline común: dos streams → resample a 16 kHz mono (`rubato`) → AEC para quitar del canal mic lo que suena por los altavoces (si no hay auriculares) → VU meters a 20 Hz para las barras animadas → VAD por canal → ventanas a STT. Seguimiento de cambios de dispositivo (Bluetooth, docks) con reconexión automática, y el selector de micrófono "Auto" que sigue al dispositivo que usa la app de reunión (detectable por `kAudioDevicePropertyDeviceIsRunningSomewhere` en macOS).

---

## 6. ASR, VAD y diarización (slots de modelo)

> Modelos elegidos para cada slot: [modelos-mvp.md](modelos-mvp.md). Resultados medidos: [experimentos.md](experimentos.md), «STT».

| Slot | Función | Requisitos | Formato / runtime |
|---|---|---|---|
| **VAD** | Cortar habla por canal, alimentar auto-stop (15 min sin audio) | <10 ms por frame en CPU | ONNX en sherpa-onnx (Silero, TEN) |
| **STT streaming** | Transcript en vivo, 2 canales en paralelo, latencia < 1-2 s | Multilingüe (Granola: 31 idiomas, modo Multi-language), decode parcial + final | ONNX en sherpa-onnx (transducer streaming, Moonshine streaming…) o whisper.cpp con LocalAgreement (`yamabiko-whisper`) |
| **STT batch** (opcional) | Re-transcripción de calidad al terminar o bajo demanda | Sin límite estricto de latencia; GPU si existe | GGML (whisper.cpp) u ONNX (Parakeet, Canary, Whisper…) |
| **Puntuación / capitalización** | Si el modelo streaming no la da | CPU | ONNX en sherpa-onnx (CT-Transformer) |
| **Diarización** | Separar hablantes dentro del canal "Them" cuando no hay speaker tags, y en presencial | Offline al terminar (pyannote-like) y, opcionalmente, online por embeddings | `speakrs` (community-1, CoreML/ONNX) o sherpa-onnx (segmentación pyannote 3.0 + embeddings 3D-Speaker) |
| **Embeddings de hablante** | Re-identificar "Me" entre reuniones, nombrar hablantes recurrentes | CPU | ONNX en sherpa-onnx |
| **Jargon / hotwords** | Equivalente a Personal/Workspace jargon | Hotwords en transducer (sherpa-onnx) o prompt inicial (whisper) | — |

Fusión de hablantes: prioridad a speaker tags por Accesibilidad (nombre + intervalo "hablando"); si no hay, diarización acústica; si no, "Me/Them". La UI pinta igual que Granola: nombre encima de la burbuja.

Gestión de modelos: catálogo `models.toml` por slot con URL de Hugging Face, SHA256, tamaño, backends soportados; descarga en segundo plano con progreso en Settings > Models; selección automática del backend (Metal/CoreML en Apple Silicon, CUDA/Vulkan en Windows/Linux, CPU como fallback) con un benchmark de arranque de 5 s para elegir el modelo por defecto que el hardware aguanta en tiempo real.

---

## 7. LLM: enhance, chat, recipes, briefs, follow-ups (slots de modelo)

> El enhance tal como funciona hoy (Markdown con citas en vez de JSON, pasadas de detalle, post-proceso en código) está en [HANDOFF.md](HANDOFF.md), «El enhance hoy»; el punto 1 de abajo es el diseño de partida.

| Slot | Función | Requisitos |
|---|---|---|
| **LLM principal** | Enhance notes, templates, chat RAG, recipes, rewrite, follow-up emails, briefs | Contexto ≥ 32k (una hora de reunión son ~10-15k tokens de transcript), salida estructurada (JSON schema / GBNF), streaming |
| **LLM pequeño** | Limpieza de dictado (Voice Typing), títulos, extracción de action items en vivo | Latencia < 1 s para 100 tokens |
| **Embeddings de texto** | Búsqueda semántica y RAG en Chat sobre carpetas/historial | CPU, multilingüe |
| **Reranker** (opcional) | Mejorar citas en Chat | CPU |

Runtime por defecto `llama-cpp-2` (GGUF). Ofrecer además "Custom OpenAI-compatible endpoint" en Settings para Ollama/LM Studio/servidor propio, como hacen anarlog y Meetily.

**Pipelines (crate `enhance`).**
1. **Enhance notes**: entrada = notas crudas (ProseMirror JSON → Markdown con ids de bloque), transcript segmentado con timestamps y hablante, metadatos del evento (título, asistentes, descripción), template elegido, perfil del usuario, idioma de resumen. Salida JSON validada: lista de secciones → bullets, cada bullet con `source: {kind: "user_note"|"transcript", block_id | [t0,t1]}` y `is_user_text`. El frontend pinta negro/gris y la lupa a partir de `source`. Para transcripts largos, map-reduce por ventanas con solapamiento.
2. **Chat**: scope → recuperación (FTS + vectorial) → prompt con citas numeradas → streaming. Soporta multiselección, adjuntos (PDF/texto extraídos localmente), "rewrite" que devuelve un patch de la nota.
3. **Recipes**: prompt estático del usuario + scope. Igual que chat.
4. **Templates**: se inyectan en el prompt de enhance como "propósito / estilo / estructura".
5. **Briefs**: job nocturno (o al abrir la nota) para eventos con asistentes externos: notas previas con esas personas/empresa + descripción del evento (+ Gmail/web si el usuario conecta claves). 2-3 bullets.
6. **Follow-up email**: tras enhance, si hay invitado externo y ≤ 10 personas; borrador editable; envío vía Gmail API (conector opcional) o `mailto:`.
7. **Live chat** ("What did I miss?"): sobre el transcript parcial, sin persistir.

---

## 8. Datos

**SQLite** (sqlx, WAL): `workspaces`, `spaces`, `folders`, `folder_notes`, `notes` (título, prosemirror_json, enhanced_json, template_id, status, created_at, calendar_event_id, trashed_at), `transcripts`, `segments` (note_id, channel mic/system, speaker_id, t0, t1, text, confidence), `speakers`, `people`, `companies`, `note_people`, `calendar_events`, `templates`, `recipes`, `chats`, `chat_messages`, `snapshots`, `settings`, `models`, `jobs`.

**Búsqueda**: tantivy sobre título, notas, enhanced y segments; vectores en `sqlite-vec` por chunk de ~300 tokens. Granola no tiene buscador de transcript; aquí sí (es trivial).

**Ficheros**: `audio/<note_id>/{mic,system}.flac` (opcional, borrable por política de retención igual que la de transcripts: 1 día … 1 año / Off), `snapshots/<note_id>/*.png`, `models/<slot>/<name>/`.

**Retención**: job diario que aplica "Auto deletion period for transcripts" y la papelera de 30 días.

---

## 9. Calendario, detección y notificaciones

- **Google Calendar**: OAuth PKCE con loopback, `events.list` con `syncToken`, polling cada 2-5 min más push opcional. **Microsoft Graph**: idem (`/me/calendarView`, delta). **Apple Calendar** vía EventKit como extra. Filtrar rechazados y OOO/Focus/Working Location. Vincular recurrentes por `recurringEventId` o título exacto (igual que Granola). Multi-cuenta.
- **Notificaciones**: ventanas Tauri propias, sin decoración, siempre encima, esquina superior derecha, con los dos estilos (barra sólida / línea discontinua) y los textos exactos ("Meeting detected", "Take Notes"). Lógica: 1 min antes y 2+ asistentes; detección de mic en uso → título según app; asociación a evento en ventana de 15 min.
- **Auto-stop**: evento terminado + app de reunión cerrada + sin voz 15 min + suspensión.
- **Reposición de ventana** al empezar una reunión (toggle) e indicador flotante arrastrable (ventana Tauri transparente sin foco).

---

## 10. Especificación de UI (paridad)

El objetivo es paridad 1:1. Las fuentes públicas describen la estructura pero no dan medidas ni tokens de diseño, así que la **fase 0** incluye una auditoría con la app real: instalar Granola, capturar cada pantalla y estado en claro y oscuro, medir tipografía, espaciados, radios, colores (acento verde, grises del texto IA) y animaciones (barras de la píldora, transiciones del drawer), y guardar todo en `design/` como referencia. Con eso se construye un sistema de tokens CSS y un Storybook con cada componente al lado de su captura.

Componentes a implementar (todos ya descritos en la sección 1):

- Shell: sidebar colapsable, header mínimo sin toolbar, área principal, barra Ask anything fija abajo, menú de workspace abajo-izquierda, soporte de modo oscuro, atajos (`⌘K`, `⌘J`, `⌘N`, Esc).
- Home: Coming up (paginado de 5), Recent meetings (filas, hover, checkbox, +N), banners.
- Nota: editor TipTap con dos tonos, lupa con popover de transcript, tabs My notes/Enhanced, botón ✨ con picker y 🔁, Shared/Copy link/⋯, Images stack, Brief, View draft.
- Píldora de grabación + drawer de transcript (burbujas, Copy all, búsqueda, selector de mic, Stop/Resume) + indicador flotante.
- Chat: panel, historial, selector de modelo, citas, adjuntos, menú `/` de recipes, acciones con borrador y confirmación.
- Recipes (Discover / My / Workspace, formulario), Templates (manage), Spaces y Folders (crear, icono, descripción, compartir, Integrations), People y Companies, Customer calls, Trash.
- Settings con todas las secciones de 1.9 más **Models** (nueva).
- Onboarding: sign-in opcional, permisos (micrófono, system audio, Accesibilidad), calendario, descarga de modelos, nota demo.
- Voice Typing: píldora inferior independiente de la ventana principal.
- Tray/menu bar: entrada discreta.

---

## 11. MCP, API, integraciones, compartir

- **MCP local** (`rmcp`, Streamable HTTP en `localhost`, token local): las 9 tools de Granola con los mismos nombres y formas, para que Claude, ChatGPT, Cursor o Claude Code se conecten igual que a Granola.
- **API REST local** (`axum`) con los mismos endpoints (List/Search/Get Notes, Get Transcript, List Folders/Spaces, Webhooks).
- **Integraciones**: Slack (chat.postMessage), Notion (pages), HubSpot/Attio/Affinity (notas en registros), Zapier (webhooks), Gmail (drafts/send), Google Calendar (events.insert). Todas con OAuth propio del usuario y claves en `keyring`.
- **Compartir**: fase 6, servidor opcional self-hosted (axum + Postgres o SQLite + S3) con enlaces de solo-notas (sin transcript, igual que Granola web), roles Viewer/Collaborator, sync cifrada E2E entre dispositivos. Hasta entonces: export Markdown/HTML/PDF y "Copy enhanced notes".

---

## 12. Roadmap por fases

Estado a 10 nov: ✅ hecho · 🟡 en parte · ⬜ sin empezar. Todo lo hecho es en Mac; Windows y Linux están sin empezar.

**Fase 0 — Auditoría y cimientos (2-3 semanas)** 🟡 workspace, esquema SQLite, gestor de modelos con descarga desde HF y diseño a partir de capturas de Granola hechos; CI, firma y notarización, y `tauri-specta`, pendientes.
- Instalar Granola, capturar todas las pantallas y estados, extraer tokens de diseño.
- Workspace Rust + Tauri v2 + tauri-specta; CI para macOS (arm64, x64), Windows, Linux; firma y notarización.
- Esquema SQLite inicial, gestor de modelos con descarga desde HF y `models.toml`.
- Decisión y prueba de los modelos por slot (tú eliges; aquí se mide latencia en CPU y GPU en los equipos objetivo).

**Fase 1 — MVP grabable (4-6 semanas)** ✅ en Mac (captura dual, STT en dos canales, editor, enhance con lupa, Home, papelera, ajustes y modelos), salvo el auto-stop por silencio y el fallback ScreenCaptureKit.
- Captura dual en macOS (process tap + SCK fallback) y Windows (WASAPI loopback); VU meters.
- STT streaming en dos canales con VAD; transcript con burbujas Me/Them; Stop/Resume; auto-stop básico.
- Editor TipTap con Markdown ligero e imágenes; nota con título inline.
- Enhance notes con LLM local: dos tonos + lupa a transcript; template Auto.
- Home con Recent meetings; New Note; papelera.
- Settings: Models, Language, Preferences básicas.

**Fase 2 — Calendario y detección (3-4 semanas)** 🟡 detección de llamadas, calendario de macOS (EventKit), Coming up, avisos y Take Notes hechos; Google y Microsoft directos, reposición de ventana, indicador flotante y tray, pendientes.
- Google y Microsoft Calendar, Coming up, notificaciones propias (ambos estilos), asociación evento↔nota, recurrentes.
- Detección de mic en uso y de apps de reunión; títulos exactos; Take Notes.
- Reposición de ventana, indicador flotante arrastrable, tray.

**Fase 3 — Templates, Chat, Recipes (4-5 semanas)** 🟡 núcleo de plantillas y cinco integradas hechos, sin conectar a la app ([plantillas.md](plantillas.md)); búsqueda ⌘K por texto hecha; chat, recipes y embeddings sin empezar.
- Picker ✨ y 🔁, Manage templates, `/` en nota.
- FTS + embeddings; Chat con scopes (home, nota, carpeta, selección, persona/empresa), citas, historial, selector de modelo, adjuntos, rewrite, live chat ⌘J.
- Recipes: menú `/`, pestaña con Discover/My/Workspace, formulario, Remix.

**Fase 4 — Organización (3 semanas)** 🟡 carpetas de My notes y papelera hechas; lo demás sin empezar.
- Spaces (My notes / Team space / custom), Folders con subcarpetas, auto-add de recurrentes, multiselección.
- People & Companies desde asistentes; Customer calls.
- Export Markdown/HTML/CSV; retención y auto-deletion; papelera 30 días.

**Fase 5 — Speaker tags, Voice Typing, snapshots, briefs (4-6 semanas)** ⬜
- Accesibilidad para Zoom/Teams/Meet → nombres en transcript; diarización acústica como fallback.
- Voice Typing: hotkey global, píldora, modo manos libres, Esc, limpieza LLM, contexto de app, Mute other audio.
- Snapshots de pantalla compartida (macOS, ScreenCaptureKit) con stack Images.
- Briefs y follow-up emails (fuentes locales; Gmail opcional).
- Mensaje automático de consentimiento y marca de agua (transparencia).

**Fase 6 — Ecosistema (abierto)** ⬜
- MCP y API locales; integraciones Slack/Notion/CRM/Zapier.
- Servidor opcional self-hosted para compartir enlaces y sync E2E; vista web de notas.
- Linux como plataforma soportada; importación de audio/SRT/VTT; edición de transcript (extras sobre Granola).
- Móvil (fuera de alcance de Rust puro; evaluar Tauri Mobile o nativo más adelante).

---

## 13. Riesgos y mitigaciones

| Riesgo | Mitigación |
|---|---|
| Latencia del STT local en CPU sin GPU | Slot streaming con modelos pequeños cuantizados; batch de calidad al terminar; benchmark de arranque que degrada automáticamente |
| Captura de audio del sistema en macOS < 14.2 y permisos TCC | Doble ruta (tap / SCK) y onboarding que explica el permiso exacto, como Granola |
| cpal y loopback WASAPI inestables | Usar `wasapi` crate directo en Windows (anarlog y Vexa lo hacen) |
| Enhance de 1 h de reunión en modelos de 8-32k de contexto | Map-reduce por ventanas; segmentos con ids para mantener la trazabilidad bullet→transcript |
| Accesibilidad para speaker tags cambia con cada versión de Zoom/Teams | Selectores por versión y fallback a diarización; mismo problema que tiene Granola |
| Paridad visual sin acceso al código de Granola | Fase 0 con capturas reales, Storybook comparativo, revisión píxel a píxel por pantalla |
| Tamaño del bundle (modelos + runtimes GPU) | Modelos se descargan tras instalar; runtimes por feature flags de Cargo por plataforma |
| Licencias de modelos y de código de referencia | Clones MIT/Apache-2 (anarlog, Meetily, Open Granola) se pueden leer y adaptar; verificar licencia de cada modelo elegido |

---

## 14. Fuentes

Granola (oficial): [granola.ai](https://www.granola.ai/), [pricing](https://www.granola.ai/pricing), [enterprise](https://www.granola.ai/enterprise), [integrations](https://www.granola.ai/integrations), [Granola 101](https://docs.granola.ai/help-center/getting-started/granola-101), [setup](https://docs.granola.ai/help-center/getting-started/setting-up-granola-for-the-first-time), [calendar sync](https://docs.granola.ai/help-center/getting-started/syncing-your-calendars), [how transcription works](https://docs.granola.ai/help-center/taking-notes/transcription), [transcription issues](https://docs.granola.ai/help-center/troubleshooting/transcription-issues), [writing notes](https://docs.granola.ai/help-center/taking-notes/taking-notes-in-granola), [AI-enhanced notes](https://docs.granola.ai/help-center/taking-notes/ai-enhanced-notes), [templates](https://docs.granola.ai/help-center/taking-notes/customise-notes-with-templates), [templates vs recipes](https://www.granola.ai/blog/meeting-recipes-repeatable-formats), [chat](https://docs.granola.ai/help-center/getting-more-from-your-notes/chatting-with-your-meetings), [chat workflows](https://docs.granola.ai/help-center/getting-more-from-your-notes/workflows-in-chat), [model selection](https://docs.granola.ai/help-center/getting-more-from-your-notes/understanding-model-selection-in-granola-chat), [recipes](https://docs.granola.ai/help-center/getting-more-from-your-notes/recipes), [spaces & folders](https://docs.granola.ai/help-center/sharing/folders/spaces-and-folders), [people & companies](https://docs.granola.ai/help-center/people-and-companies), [customer calls](https://docs.granola.ai/help-center/sharing/customer-calls), [sharing](https://docs.granola.ai/help-center/sharing/sharing-notes), [exporting](https://docs.granola.ai/help-center/sharing/exporting-notes), [workspaces](https://docs.granola.ai/help-center/workspaces), [preferences](https://docs.granola.ai/help-center/customising-granola/profile-and-preferences), [multi-language](https://docs.granola.ai/help-center/customising-granola/multi-language), [jargon](https://docs.granola.ai/help-center/customising-granola/customising-transcription), [notifications](https://docs.granola.ai/help-center/taking-notes/notifications), [speaker tags](https://docs.granola.ai/help-center/taking-notes/speaker-attribution), [snapshots](https://docs.granola.ai/help-center/taking-notes/capture-shared-screens), [briefs](https://docs.granola.ai/help-center/taking-notes/pre-meeting-briefs), [follow-up emails](https://docs.granola.ai/help-center/taking-notes/follow-up-emails), [deleting notes](https://docs.granola.ai/help-center/taking-notes/deleting-notes), [auto-deletion](https://docs.granola.ai/help-center/consent-security-privacy/transcript-auto-deletion), [voice typing](https://docs.granola.ai/help-center/voice-typing/intro), [MCP](https://docs.granola.ai/help-center/sharing/integrations/mcp), [feature requests](https://docs.granola.ai/help-center/feature-requests), [Windows](https://www.granola.ai/blog/granola-for-windows-setup-teams-and-zoom-capture-and-what-it-controls), [Android](https://www.granola.ai/blog/granola-for-android-ai-meeting-notes-for-mixed-device-teams), [llms.txt índice](https://docs.granola.ai/llms.txt).

Granola (terceros): [TechCrunch Serie C](https://techcrunch.com/2026/03/25/granola-raises-125m-hits-1-5b-valuation-as-it-expands-from-meeting-notetaker-to-enterprise-ai-app/), [Desktop Insights (Electron)](https://desktopinsights.com/apps/granola), [oferta Electron](https://jobs.firstminute.capital/companies/granola/jobs/49915456-electron-engineer-windows), [Granola UX reference](https://cdn.jsdelivr.net/npm/@agent-native/core@0.124.6/corpus/templates/clips/desktop/design-refs/granola-ux.md), [Zapier](https://zapier.com/blog/granola-ai/), [Wonder Tools](https://wondertools.substack.com/p/granolaguide), [TheSweetBits](https://thesweetbits.com/tools/granola-ai-review/), [zackproser](https://zackproser.com/blog/granola-ai-review), [Jamie](https://www.meetjamie.ai/blog/granola-review), [HappyScribe](https://www.happyscribe.com/blog/granola-ai-review), [Bluedot](https://www.bluedothq.com/blog/granola-review), [tl;dv](https://tldv.io/blog/granola-review/).

Clones open source: [anarlog / Hyprnote](https://github.com/fastrepl/hyprnote) y [docs](https://docs.anarlog.so), [Meetily](https://github.com/Zackriya-Solutions/meetily), [Open Granola](https://github.com/anshuman-pandey/open-granola), [Vexa desktop](https://github.com/Vexa-ai/vexa-desktop), [hex](https://github.com/anomalyco/hex).

Rust, audio: [screencapturekit](https://crates.io/crates/screencapturekit), [scrybe-meeting-capture-mac (tap vs SCK)](https://docs.rs/scrybe-meeting-capture-mac/latest/scrybe_capture_mac/), [recall.ai sobre system audio](https://www.recall.ai/blog/how-to-get-access-to-system-audio), [cpal](https://crates.io/crates/cpal), [cpal loopback issue](https://github.com/RustAudio/cpal/issues/476), [wasapi](https://docs.rs/wasapi), [dual-track WASAPI](https://dev.to/madnessbrains_i/building-a-dual-track-audio-pipeline-on-windows-mic-system-audio-without-joining-the-call-1d7a), [side-huddle (detección)](https://docs.rs/crate/side-huddle/0.2.1/source/src/lib.rs).

Rust, voz e IA: [sherpa-onnx docs](https://k2-fsa.github.io/sherpa/onnx/index.html), [sherpa-onnx crate](https://crates.io/crates/sherpa-onnx), [transcribe-rs](https://github.com/cjpais/transcribe-rs), [whisper-rs](https://openapps.pro/packages/whisper-rs), [yamabiko-whisper](https://docs.rs/yamabiko-whisper), [wavo](https://github.com/Dtatarchuk/wavo), [speakrs](https://docs.rs/speakrs), [pyannote](https://github.com/pyannote/pyannote-audio), [mistral.rs review](https://hysenlabs.com/projects/ericlbuehler-mistral-rs), [Rust LLM ecosystem](https://hackmd.io/@Hamze/Hy5LiRV1gg), [fastembed-rs](https://hnshowcase.com/post/48538943/fastembed-rs-rust-library-for-generating-vector-embeddings-r), [rmcp](https://docs.rs/crate/rmcp/3.1.0/source/README.md).

Rust, UI y plataforma: [Rust GUI 2026 (gpui-starter)](https://gpui-starter.freeoxide.com/blog/rust-gui-frameworks-comparison-2026/), [survey 2026](https://blog.wybxc.cc/blog/rust-gui-survey-2026/), [Dioxus 0.7](https://dioxuslabs.com/blog/release-070), [twrite-gpui](https://docs.rs/crate/twrite-gpui/0.9.2), [Tauri sidecar](https://stuffbucket.github.io/skills/catalog/tauri-sidecar/), [Tauri local LLM](https://codenote.pages.dev/en/posts/tauri-cross-platform-app-with-local-llm), [eventkit-rs](https://docs.rs/eventkit-rs), [oauth2 msgraph example](https://rust-digger.code-maven.com/src/oauth2-5.0.0-rc.1/examples/msgraph.rs), [oauth-toolkit](https://docs.rs/oauth-toolkit).
