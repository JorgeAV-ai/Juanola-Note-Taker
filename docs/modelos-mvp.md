# Modelos y runtimes para el MVP

Fecha: 6 de noviembre de 2026; actualizado el 10 nov con lo que se cambió después de medir (Q8_0 del LLM, Qwen3-ASR 1.7B). Las medidas están en [experimentos.md](experimentos.md). Criterio: relación calidad/peso, no licencia. Formatos nativos por plataforma: MLX y CoreML en Mac con Apple Silicon; GGUF y ONNX en Windows. Tamaños verificados contra la API de Hugging Face hoy. Decisiones del propietario del proyecto: LLM = LFM3-1.7B de Liquid AI; embeddings = EmbeddingGemma 2.

## 1. Decisión en una frase

**LFM3-1.7B** para texto (MLX 4-bit en Mac cuando el runtime Rust lo soporte; GGUF Q8_0 mientras tanto y en Windows, desde el 9 nov, antes Q4_K_M), **Qwen3-ASR** para voz en streaming (1.7B MLX de 4 bits en Mac desde el 9 nov, 0.6B de 4 bits en Mac con menos de 16 GB; 0.6B ONNX int8 en Windows), **EmbeddingGemma 2** solo texto para búsqueda (ONNX q8 en todas; CoreML opcional en Mac), Silero VAD y `speakrs` para diarización. Todo junto pesa unos 3,9 GB de disco en Mac con el STT de 1.7B (unos 3 GB con el 0.6B, que es lo que se instala con menos de 16 GB de RAM).

## 2. Catálogo de Liquid AI y qué sirve aquí

Liquid publicó LFM3 el 27 de octubre de 2026: seis modelos de clase 2B sobre un mismo backbone "tri-híbrido" (convolución corta con puerta + atención de ventana + atención global). Revisado su catálogo completo, esto es lo aprovechable para un notepad de reuniones:

| Modelo | Qué hace | Encaje en el proyecto | Peso 4-bit | Formatos oficiales |
|---|---|---|---|---|
| **LFM3-1.7B** | Texto, 256K de contexto, 16 idiomas (incluye español), razonamiento activable, tool calling, salida estructurada | **LLM principal** (enhance, chat, recipes) y LLM pequeño (dictado, títulos) | 0,97 GB MLX / 1,10 GB Q4_K_M / 1,05 GB QAD-Q4_0 | GGUF, MLX 4/8-bit, ONNX, FP8, NVFP4 |
| **LFM3-Audio-2B** | ASR + TTS + chat de voz; 18 locales en ASR, streaming (encoder causal, TTFT 50 ms, chunks de 0,96 s) **pero el modo live solo en inglés** | Alternativa de STT "todo Liquid"; TTS si algún día hace falta leer notas | 1,10 GB Q4 + 0,16 GB mmproj / 1,75 GB MLX | GGUF (llama.cpp mtmd), MLX 4-bit (MLX-VLM), ONNX, CoreML vía FluidAudio |
| **LFM2-2.6B-Transcript** | Resumen de transcripciones de reuniones de 30-60 min: key points, decisiones, action items; prompt con cabecera de metadatos y líneas por hablante | Es justo el resumen de reuniones que hace falta, pero **solo inglés** y base LFM2 antigua. Útil como referencia de prompt y como tier opcional para usuarios en inglés | 1,56 GB Q4_K_M | GGUF, ONNX, MLX comunidad |
| **LFM2.5-Embedding-350M** | Embeddings de texto, 32K | Alternativa a EmbeddingGemma 2 si se quiere pila 100 % Liquid | 0,23 GB Q4 | GGUF, MLX, ONNX comunidad |
| **LFM2.5-ColBERT-350M** | Retrieval de interacción tardía | Reranker para las citas del chat (fase 3, fuera del MVP) | 0,23 GB Q4 | GGUF, MLX |
| LFM3-Omni-2B | Audio + imagen + vídeo → texto o voz | Entender snapshots de pantalla compartida junto al audio (fase 5) | 1,10 GB + 0,27 GB mmproj | GGUF, MLX, ONNX |
| LFM2.5-VL-1.6B-Extract / 450M-Extract | JSON estructurado a partir de imágenes | Extraer texto y datos de los snapshots de pantalla (fase 5) | 0,4-1 GB | GGUF |
| d1-omni-600M / d1-2B | Clasificación, routing, scoring | Decisiones baratas: "¿esto es una reunión?", "¿merece follow-up email?" (hoy con heurísticas) | 0,3-1 GB | GGUF, MLX |

Lo que no encaja: los modelos JP/ENJP (japonés), Math, PII-Extract-JP y los d1 de API.

## 3. Elección por slot

### Texto: LFM3-1.7B

Benchmarks propios de Liquid (BF16, vLLM): media 49,37 sobre 29 pruebas frente a 47,24 de Qwen3.5-9B, con ventaja clara en seguimiento de instrucciones (IFEval 96,66), tool calling (BFCL V4 71,39) y tareas agénticas, y desventaja en conocimiento y contexto largo (RULER 128K 65,56 frente a 87,11 de Qwen3.5-9B). Para este proyecto importa lo primero: el enhance es "sigue esta plantilla y cita el transcript", no conocimiento del mundo. Y pesa un tercio que Qwen3.5-4B.

Detalles de uso que afectan al código:
- Plantilla ChatML con `<|startoftext|>`; el razonamiento viene activado por defecto y abre un bloque `<think>`. Para enhance y chat se desactiva (`enable_thinking=false`) o se filtra el bloque; Liquid avisa de que la desactivación es inconsistente en tareas de razonamiento pesado.
- Muestreo recomendado: temperature 0,1, top_k 50, repetition_penalty 1,1.
- Tool calling con etiquetas XML `<tool_call>`; sirve para las acciones de chat de la fase 3.
- Contexto 256K: una reunión de una hora (10-15k tokens) entra entera.
- Variante **QAD-Q4_0** (quantization-aware) de 1,05 GB: sin medir. Se midió Q8_0 frente a Q4_K_M y se adoptó Q8_0 (9 nov): mejor en la reunión larga, 1,35-2 × más lento.

Runtimes:
- **Windows, Linux, Mac Intel**: `llama-cpp-2` con una build de llama.cpp que incluya LFM3 (b11960 o posterior). Vulkan/CUDA/Metal.
- **Mac Apple Silicon**: el objetivo es MLX 4-bit (repo oficial `LiquidAI/LFM3-1.7B-MLX-4bit`), pero hoy ningún crate Rust de MLX implementa la arquitectura LFM3: `mlxrs` 0.2 trae qwen3 y lfm2-vl, no lfm3. Hasta que exista, el MVP en Mac ejecuta el GGUF con `llama-cpp-2` + Metal, que a 1 GB ya va muy rápido. La entrada MLX queda en el catálogo marcada `ready = false` y el selector la salta. Opciones para activarla: aportar el bloque LFM3 a `mlxrs` (el tri-híbrido es gated short conv + SWA + atención global, todos ya presentes en MLX), o un sidecar nativo sin Python.

### Voz: Qwen3-ASR (y por qué no LFM3-Audio-2B como principal)

Desde el 9 nov, en Mac, el **1.7B de 4 bits** (1,6 GB; WER 6,3 % frente a 7,9 % del 0.6B en la llamada de ventas sintética) y el 0.6B de 4 bits con menos de 16 GB de RAM. La comparación de abajo, del 6 nov, es la que llevó a Qwen3-ASR frente a LFM3-Audio-2B.

| | Qwen3-ASR 0.6B | LFM3-Audio-2B |
|---|---|---|
| WER inglés (Open ASR) | 6,99 % (leaderboard) | 6,23 % (cifra de Liquid); un benchmark independiente da 5,7 % en un subconjunto y ~13,6 % en las suites completas con acentos |
| Multilingüe | 30 idiomas, streaming en todos | 18 locales en batch; FLEURS 9 idiomas 6,03 %; **streaming solo en inglés** |
| Peso | 0,71 GB MLX / 0,94 GB ONNX | 1,26 GB GGUF / 1,75 GB MLX |
| Rust hoy | `mlxrs` (audio: qwen3-asr), sherpa-onnx | llama.cpp mtmd (soporte en `llama-cpp-2` por verificar), FluidAudio CoreML (sidecar Swift) |

Para transcribir reuniones en español en tiempo real, Qwen3-ASR 0.6B sigue siendo la elección: streaming multilingüe, mitad de peso y ya ejecutable desde Rust en MLX y ONNX. LFM3-Audio-2B entra en el catálogo como alternativa seleccionable, y es especialmente interesante en Mac vía FluidAudio (CoreML en el Neural Engine, con ASR, LiveASR y TTS ya integrados en su PR 1076) para quien hable inglés o quiera TTS.

### Embeddings: EmbeddingGemma 2, solo texto

EmbeddingGemma 2 (Google, 6 de octubre de 2026) es multimodal (740M en total) pero se carga por módulos: la parte de texto son 270M. Salida 768d con Matryoshka a 512/256/128 (256d casi sin pérdida: MTEB multilingüe 60,41 frente a 61,36), contexto 8K, 100+ idiomas. Variantes optimizadas verificadas:

| Formato | Repo | Peso texto | Calidad | Plataforma |
|---|---|---|---|---|
| **ONNX q8** | `onnx-community/embeddinggemma-2-ONNX` (`onnx/model_quantized`) | 314 MB | cos 0,9997 vs fp32 | Todas (ONNX Runtime CPU) |
| ONNX q4 | mismo repo (`onnx/model_q4`) | 174 MB | cos 0,988 (texto) | Todas |
| GGUF Q8_0 | `ggml-org/embeddinggemma-2-GGUF` | 310 MB | — | Todas (llama.cpp embeddings) |
| CoreML | `FluidInference/embeddinggemma-2-coreml` (`EmbeddingGemma2Text`) | 277 MB | — | Mac (ANE, sidecar FluidAudio) |
| MLX 8-bit | `mlx-community/embeddinggemma-2-8bit` | 1,23 GB (incluye encoders) | — | Mac |

Elección MVP: **ONNX q8 de 314 MB en todas las plataformas** cargado con `ort` (o `fastembed` con modelo custom), truncado a 256d para que el índice `sqlite-vec` pese un tercio, con los prefijos obligatorios `task: search result | query: …` y `title: none | text: …`. En Mac no compensa MLX aquí: el MLX de 8 bits pesa cuatro veces más porque incluye visión y audio, y en CPU el q8 tarda milisegundos por chunk. CoreML vía FluidAudio queda como optimización de fase 2 si se adopta ese sidecar para otra cosa. Nota del model card: evitar float16 (da NaN); usar bf16 o fp32, o los int8/q4.

Si más adelante se quiere búsqueda sobre los snapshots de pantalla, el mismo modelo carga el encoder de visión (170M más) y pone imágenes y texto en el mismo espacio.

### VAD y diarización

Sin cambios: Silero VAD (2 MB, ONNX; CoreML vía FluidAudio) y pyannote community-1 con `speakrs` (CoreML en Mac, ONNX en Windows).

## 4. Selección automática de backend

```
macOS + Apple Silicon  -> STT mlx | LLM gguf-metal (mlx cuando mlxrs soporte LFM3) | diar coreml | emb onnx
macOS + Intel          -> STT onnx | LLM gguf-metal | diar onnx | emb onnx
Windows + Vulkan 1.2   -> STT onnx | LLM gguf-vulkan | diar onnx | emb onnx
Windows + CUDA 12      -> STT onnx | LLM gguf-cuda   | diar onnx | emb onnx
Windows sin GPU        -> STT onnx | LLM gguf-cpu    | diar onnx | emb onnx
```

El catálogo marca cada entrada con `ready`; el selector solo elige entradas listas y, dentro de ellas, el formato más nativo de la plataforma.

Tier de RAM:

| RAM | STT | LLM | Comentario |
|---|---|---|---|
| 8 GB | Qwen3-ASR 0.6B | LFM3-1.7B Q4 | Cómodo: STT 0,7 + LLM 1,1 + embeddings 0,3 + KV de 32K de contexto cabe con margen |
| 16 GB | Qwen3-ASR 1.7B de 4 bits (Mac) | igual, contexto completo de la reunión sin recorte | Recomendado y predeterminado desde el 9 nov: STT 1,6 + LLM 1,85 |
| 32 GB+ | igual | LFM2-2.6B-Transcript como tier de inglés, o modelos mayores | Elegible en Settings > Models |

## 5. Requisitos de configuración para el MVP

### Mac
- Apple Silicon (M1+). Intel solo con GGUF/ONNX.
- macOS 14 o posterior; 14.2+ para capturar audio del sistema con Core Audio process taps sin permiso de grabación de pantalla.
- Permisos: micrófono, audio del sistema (tap) o grabación de pantalla (SCK), Accesibilidad (fase 5).
- Disco: ~2,3 GB de modelos (0,7 + 1,1 + 0,3 + 0,06 + 0,002).
- Compilar: Xcode CLT, Rust 1.88+, pnpm, CMake (para `llama-cpp-2`).

### Windows
- Windows 10 1903+ u 11, x64 con AVX2.
- GPU opcional: Vulkan 1.2 o CUDA 12. Sin GPU, LFM3-1.7B en CPU sigue siendo usable (Liquid reporta en edge al menos 1,8× el decode de los modelos comparados; medir en fase 1).
- Permisos: micrófono.
- Disco: ~2,5 GB (0,94 + 1,1 + 0,3 + 0,06).
- Compilar: VS Build Tools C++, CMake, Vulkan SDK opcional, Rust 1.88+, pnpm.

### Ambas
- Descarga de modelos en el primer arranque desde Hugging Face con SHA-256 y reanudación; luego sin red.
- Modelos fuera del bundle, en el directorio de datos de la app.
- Settings > Models muestra tier, backend, progreso y permite cambiar por slot (por ejemplo, pasar a LFM3-Audio-2B o LFM2-2.6B-Transcript).

## 6. Qué entra y qué no en el MVP

Entra: STT streaming en dos canales (Qwen3-ASR), VAD, enhance y chat de una nota con LFM3-1.7B, embeddings EmbeddingGemma 2 a 256d, diarización offline al terminar, selección automática de backend y tier, descarga con progreso.

No entra: MLX para LFM3 desde Rust (bloqueado por `mlxrs`), LFM3-Audio-2B como STT (seleccionable, no por defecto), LFM2-2.6B-Transcript, ColBERT como reranker, Omni/Extract para snapshots, d1 para decisiones, NPU en Windows.

## 7. Riesgos

- `mlxrs` no implementa LFM3: en Mac el LLM corre en llama.cpp Metal hasta que se aporte el bloque o aparezca otro runtime Rust. Riesgo de calendario, no de funcionalidad.
- Soporte de LFM3 en `llama-cpp-2`: el crate sigue a llama.cpp con retraso de días o semanas; hay que fijar una versión con LFM3 (b11960+) y comprobar que el bloque QAD-Q4_0 carga.
- Razonamiento por defecto en LFM3: si `enable_thinking=false` se ignora en algún prompt, el bloque `<think>` debe filtrarse en el parser de salida para que no acabe en las notas.
- Benchmarks de LFM3 y LFM3-Audio son del fabricante; el benchmark independiente de LFM3-Audio muestra degradación fuerte con acentos. Medir ambos STT en español en la fase 1 con FLEURS-es antes de cerrar.
- EmbeddingGemma 2 en `fastembed`: si no está en su lista, se carga el ONNX con `ort` directamente (el tokenizer es el de Gemma, disponible en el repo).

## Fuentes

[LFM3-1.7B](https://huggingface.co/LiquidAI/LFM3-1.7B), [LFM3-Audio-2B](https://huggingface.co/LiquidAI/LFM3-Audio-2B), [LFM2-2.6B-Transcript](https://huggingface.co/LiquidAI/LFM2-2.6B-Transcript), [biblioteca completa de Liquid](https://docs.liquid.ai/lfm/models/complete-library), [Liquid en MLX](https://docs.liquid.ai/deployment/on-device/mlx), [lanzamiento LFM3 (MarkTechPost)](https://www.marktechpost.com/2026/10/27/liquid-ai-lfm3-open-weight-2b-multimodal-models/), [benchmark independiente LFM3-Audio (TypeWhisper)](https://github.com/TypeWhisper/typewhisper-benchmark/pull/11), [FluidAudio PR 1076 LFM3-Audio](https://github.com/FluidInference/FluidAudio/pull/1076), [EmbeddingGemma 2](https://huggingface.co/google/embeddinggemma-2), [EmbeddingGemma 2 ONNX](https://huggingface.co/onnx-community/embeddinggemma-2-ONNX), [EmbeddingGemma 2 GGUF](https://huggingface.co/ggml-org/embeddinggemma-2-GGUF), [EmbeddingGemma 2 CoreML](https://huggingface.co/FluidInference/embeddinggemma-2-coreml), [mlxrs](https://lib.rs/crates/mlxrs), [Open ASR ranking](https://micdrop.dev/blog/top-open-source-speech-to-text-models), [Qwen3-ASR en sherpa-onnx](https://k2-fsa.github.io/sherpa/onnx/qwen3-asr/export.html), [speakrs](https://docs.rs/speakrs).
