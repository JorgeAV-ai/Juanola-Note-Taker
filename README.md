<h1 align="center">
  <img src="docs/assets/logo.svg" alt="" height="44" align="absmiddle" />
  Juanola Note Taker
</h1>

<p align="center"><b>The lightest, fastest &amp; safest AI note taker.</b></p>

Native Rust core. 100% on-device. Open-source models. Your meetings never leave your machine.

Juanola is an open-source, local-first take on [Granola](https://www.granola.ai): it listens to your meetings (mic + system audio), transcribes them live, and turns your rough notes into polished, structured summaries — all without sending a single byte to the cloud.

## Why Juanola?

- 🪶 **Lightest** — Rust core + Tauri v2 instead of a heavy Electron shell. Tiny footprint, instant startup.
- 🚀 **Fastest** — Streaming speech-to-text and local LLM inference with GPU backends (Metal, MLX, llama.cpp).
- 🔒 **Safest** — Everything runs on your device. No accounts, no uploads, no telemetry. You choose the models.
- 🧠 **Smart** — Enhanced notes, meeting recipes, follow-ups, briefs, and "Ask anything" chat over all your notes.
- 🔌 **Open** — Local MCP server and REST API so your own tools and agents can use your notes.

## Project layout

```
apps/desktop/          Tauri v2 (src-tauri) + React/TipTap frontend (src)
crates/core-types      Domain models (Note, Segment, Folder, Recipe, …)
crates/db              SQLite (sqlx) + migrations
crates/search          Full-text (tantivy) + vector search
crates/audio-capture   Mic + system audio per OS, resampling, VU meters
crates/audio-store     Optional recording, retention, snapshots
crates/stt             Streaming/batch STT traits + VAD (sherpa-onnx, whisper.cpp)
crates/diarize         Diarization and merging with speaker tags
crates/llm             ChatModel: llama.cpp, mistral.rs, OpenAI-compatible
crates/embed           Embeddings
crates/prompts         Prompt templates
crates/enhance         Pipelines: enhance, chat, recipes, briefs, follow-ups
crates/calendar        Google, Microsoft, (Apple)
crates/detect          Meeting detection and mic-in-use detection
crates/speaker-tags    Speaker names via Accessibility APIs
crates/dictation       Voice typing
crates/models          models.toml catalog, downloads, GPU backends
crates/mcp-server      Local MCP server
crates/api-server      Local REST API
crates/integrations    Slack, Notion, CRMs, Zapier, Gmail
models.toml            Model slots (you pick them)
```

## Requirements

- Rust stable (1.85+), Node 20+, pnpm.
- macOS: Xcode Command Line Tools. Linux: Tauri dependencies (webkit2gtk, etc.).

## Development

```bash
cargo check --workspace
```

```bash
cd apps/desktop && pnpm install && pnpm tauri dev
```

## Status

Early stage. The workspace compiles and the app boots with the full UI shell (sidebar, Home, note view with TipTap editor, recording pill, "Ask anything" bar). Real audio, STT and LLM backends are feature-gated and activate once you pick your models.

The full roadmap lives in [docs/plan.md](docs/plan.md).
