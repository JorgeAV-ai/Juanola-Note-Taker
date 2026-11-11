<h1 align="center">
  <img src="docs/assets/logo.svg" alt="" height="44" align="absmiddle" />
  Juanola Note Taker
</h1>

<p align="center"><b>The lightest, fastest &amp; safest AI note taker.</b></p>

<p align="center">
  <a href="https://github.com/JorgeAV-ai/Juanola-Note-Taker/actions/workflows/ci.yml"><img src="https://github.com/JorgeAV-ai/Juanola-Note-Taker/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://jorgeav-ai.github.io/Juanola-Note-Taker/">Website</a> ·
  <a href="https://github.com/JorgeAV-ai/Juanola-Note-Taker/releases">Download (macOS preview)</a>
</p>

Native Rust core. 100% on-device. Open-source models. Your meetings never leave your machine.

Juanola is an open-source, local-first take on [Granola](https://www.granola.ai): it listens to your meetings (mic + system audio), transcribes them live, and turns your rough notes into polished, structured summaries — all without sending a single byte to the cloud.

## Why Juanola?

- 🪶 **Lightest** — Rust core + Tauri v2 instead of a heavy Electron shell. Tiny footprint, instant startup.
- 🚀 **Fastest** — Live speech-to-text on MLX and local LLM inference on llama.cpp + Metal: text appears a fraction of a second after it's spoken, and an hour-long meeting is enhanced in about two minutes on an M4 Pro.
- 🔒 **Safest** — Everything runs on your device. No accounts, no uploads, no telemetry. You choose the models.
- 🧠 **Smart** — Enhanced notes that keep your words in black and the AI's in grey, with every line linked to the moment in the transcript. Chat, recipes and briefs are on the roadmap.
- 🔌 **Open** — Local MCP server and REST API so your own tools and agents can use your notes.

## Project layout

```
apps/desktop/          Tauri v2 (src-tauri) + React/TipTap frontend (src)
crates/core-types      Domain models (Note, Segment, Folder, Recipe, …)
crates/db              SQLite (sqlx) + migrations
crates/search          Full-text (tantivy) + vector search
crates/audio-capture   Mic + system audio per OS, resampling, VU meters
crates/audio-store     Optional recording, retention, snapshots
crates/stt             Live STT (Qwen3-ASR on MLX) + Silero VAD, in a child process
crates/diarize         Diarization and merging with speaker tags
crates/llm             ChatModel: llama.cpp (LFM3-1.7B GGUF), OpenAI-compatible
crates/embed           Embeddings
crates/prompts         Prompt templates
crates/enhance         Pipelines: enhance, chat, recipes, briefs, follow-ups
crates/calendar        Apple Calendar (EventKit); Google and Microsoft planned
crates/detect          Meeting detection and mic-in-use detection
crates/speaker-tags    Speaker names via Accessibility APIs
crates/dictation       Voice typing
crates/models          models.toml catalog, downloads, GPU backends
crates/mcp-server      Local MCP server
crates/api-server      Local REST API
crates/integrations    Slack, Notion, CRMs, Zapier, Gmail
models.toml            Model catalog per slot
site/                  Landing page (GitHub Pages)
```

## Requirements

- macOS 14.6+ on Apple Silicon (16 GB of RAM recommended). Windows and Linux are not supported yet.
- Rust stable (1.95+), Node 22+, pnpm 11, CMake, Xcode Command Line Tools.

## Development

llama.cpp and the MLX kernels are not in git; fetch them once (both scripts check SHA-256):

```bash
scripts/fetch-llama-cpp.sh && scripts/fetch-mlx-metallib.sh
```

```bash
cd apps/desktop && pnpm install && pnpm tauri dev
```

The app downloads its models (about 3.5 GB) from Hugging Face on first launch. System audio only works from a bundled `.app` (macOS asks for permission): `apps/desktop/scripts/bundle-dev.sh --open`.

Checks run by CI: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and `pnpm build`. Releases are built by pushing a `v*` tag (see `.github/workflows/release.yml`).

## Status

Early preview, macOS only. Working today: dual audio capture, live transcription, enhanced notes with citations, call detection and Apple Calendar, folders, search and trash. Next: templates in the app, chat, echo cancellation and Windows.

Documentation (in Spanish) lives in [docs/](docs/README.md): current state in [HANDOFF.md](docs/HANDOFF.md), the roadmap in [plan.md](docs/plan.md) and every measured experiment in [experimentos.md](docs/experimentos.md).

Juanola is an independent project, inspired by Granola and not affiliated with it.
