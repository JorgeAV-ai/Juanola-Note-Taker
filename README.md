# Juanola

Réplica de [Granola](https://www.granola.ai) con modelos open source ejecutados en local. Núcleo en Rust, UI en Tauri v2 + React (misma estructura y features que Granola). El plan completo está en [docs/plan.md](docs/plan.md).

## Estructura

```
apps/desktop/          Tauri v2 (src-tauri) + frontend React/TipTap (src)
crates/core-types      Modelos de dominio (Note, Segment, Folder, Recipe, …)
crates/db              SQLite (sqlx) + migraciones
crates/search          FTS (tantivy) + vectorial
crates/audio-capture   Mic + audio del sistema por SO, resample, VU meters
crates/audio-store     Grabación opcional, retención, snapshots
crates/stt             Traits de STT streaming/batch + VAD (sherpa-onnx, whisper.cpp)
crates/diarize         Diarización y fusión con speaker tags
crates/llm             ChatModel: llama.cpp, mistral.rs, OpenAI-compatible
crates/embed           Embeddings
crates/prompts         Plantillas de prompts
crates/enhance         Pipelines: enhance, chat, recipes, briefs, follow-up
crates/calendar        Google, Microsoft, (Apple)
crates/detect          Detección de reuniones y mic en uso
crates/speaker-tags    Nombres de hablantes vía Accesibilidad
crates/dictation       Voice Typing
crates/models          Catálogo models.toml, descarga, backends GPU
crates/mcp-server      MCP local (9 tools de Granola)
crates/api-server      API REST local
crates/integrations    Slack, Notion, CRMs, Zapier, Gmail
models.toml            Slots de modelos (los eliges tú)
```

## Requisitos

- Rust estable (1.85+), Node 20+, pnpm.
- macOS: Xcode Command Line Tools. Linux: dependencias de Tauri (webkit2gtk, etc.).

## Desarrollo

```bash
cargo check --workspace
```

```bash
cd apps/desktop && pnpm install && pnpm tauri dev
```

## Estado

Esqueleto (fase 0 del plan): workspace compila, app arranca con shell de UI (sidebar, Home, nota con editor TipTap, píldora de grabación, barra Ask anything). Los backends reales de audio, STT y LLM están gated por features y se activan al elegir modelos.
