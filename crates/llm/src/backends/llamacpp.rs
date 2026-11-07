//! Backend llama.cpp (`llama-cpp-2`) para GGUF local, con Metal en Mac.
//!
//! Pensado para LFM3-1.7B: prompt con `chat_format::lfm3_prompt` (razonamiento
//! desactivado), muestreo recomendado por Liquid AI (temperatura 0,1, top-k 50,
//! penalización de repetición 1,1; son también los `general.sampling.*` del
//! GGUF) y, si se pide JSON Schema, una gramática GBNF generada por llama.cpp
//! que obliga a que la salida lo cumpla.

use std::num::NonZeroU32;
use std::path::Path;
use std::sync::OnceLock;

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::sampling::LlamaSampler;

use crate::chat_format::{lfm3_prompt, strip_thinking};
use crate::{GenOptions, LlmError, Message};

const TEMPERATURE: f32 = 0.1;
const TOP_K: i32 = 50;
const REPEAT_PENALTY: f32 = 1.1;
const REPEAT_LAST_N: i32 = 64;
const N_BATCH: u32 = 2048;

fn err(e: impl std::fmt::Display) -> LlmError {
    LlmError::Backend(e.to_string())
}

/// El backend de llama.cpp es global al proceso y solo se inicializa una vez.
fn backend() -> Result<&'static LlamaBackend, LlmError> {
    static BACKEND: OnceLock<LlamaBackend> = OnceLock::new();
    if let Some(b) = BACKEND.get() {
        return Ok(b);
    }
    let mut b = LlamaBackend::init().map_err(err)?;
    // JUANOLA_LLM_LOGS=1 deja ver los logs de llama.cpp (reparto CPU/GPU…).
    if std::env::var_os("JUANOLA_LLM_LOGS").is_none() {
        b.void_logs();
    }
    Ok(BACKEND.get_or_init(|| b))
}

/// Estadísticas de una generación.
#[derive(Debug, Clone, Default)]
pub struct GenStats {
    pub prompt_tokens: usize,
    pub generated_tokens: usize,
    pub prefill_ms: u64,
    pub decode_ms: u64,
}

pub struct LlamaCpp {
    model: LlamaModel,
    n_ctx: u32,
    n_ubatch: u32,
    /// `None` = automático (lo decide llama.cpp).
    flash_attention: Option<bool>,
}

impl LlamaCpp {
    /// Carga un GGUF con todas las capas en GPU. `n_ctx` es el contexto por
    /// generación (el modelo admite más, pero la caché KV crece con él).
    pub fn load(path: &Path, n_ctx: u32) -> Result<Self, LlmError> {
        let params = LlamaModelParams::default().with_n_gpu_layers(u32::MAX);
        let model = LlamaModel::load_from_file(backend()?, path, &params).map_err(err)?;
        Ok(Self { model, n_ctx, n_ubatch: 512, flash_attention: None })
    }

    /// Tamaño de micro-lote del prefill.
    pub fn with_ubatch(mut self, n: u32) -> Self {
        self.n_ubatch = n.clamp(32, N_BATCH);
        self
    }

    pub fn with_flash_attention(mut self, on: Option<bool>) -> Self {
        self.flash_attention = on;
        self
    }

    pub fn context_window(&self) -> usize {
        self.n_ctx as usize
    }

    pub fn count_tokens(&self, text: &str) -> usize {
        self.model.vocab().tokenize(text.as_bytes(), false, true).len()
    }

    /// Generación completa, bloqueante.
    pub fn complete_blocking(&self, msgs: &[Message], opts: &GenOptions) -> Result<(String, GenStats), LlmError> {
        let vocab = self.model.vocab();
        let prompt = lfm3_prompt(msgs);
        // El prompt ya trae `<|startoftext|>`: no añadir otro BOS.
        let tokens = vocab.tokenize(prompt.as_bytes(), false, true);
        let max_new = opts.max_tokens.unwrap_or(2048) as usize;
        if tokens.len() + max_new > self.n_ctx as usize {
            return Err(LlmError::ContextExceeded(tokens.len() + max_new));
        }

        // La caché KV se dimensiona al prompt + salida (redondeado a 1024), no al
        // máximo configurado: reservar 32K para un prompt de 2K cuesta tiempo.
        let needed = ((tokens.len() + max_new).div_ceil(1024) * 1024) as u32;
        let mut ctx_params = LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(needed.min(self.n_ctx)))
            .with_n_batch(N_BATCH)
            .with_n_ubatch(self.n_ubatch);
        if let Some(on) = self.flash_attention {
            // LLAMA_FLASH_ATTN_TYPE_ENABLED = 1, _DISABLED = 0 (llama.h).
            ctx_params = ctx_params.with_flash_attention_policy(if on { 1 } else { 0 });
        }
        let mut ctx = self.model.new_context(backend()?, ctx_params).map_err(err)?;

        let mut samplers = Vec::new();
        if let Some(schema) = &opts.json_schema {
            let gbnf = llama_cpp_2::json_schema_to_grammar(&schema.to_string()).map_err(err)?;
            samplers.push(LlamaSampler::grammar(&self.model, &gbnf, "root").map_err(err)?);
        }
        samplers.push(LlamaSampler::penalties(self.model.n_vocab(), REPEAT_LAST_N, REPEAT_PENALTY, 0.0, 0.0));
        samplers.push(LlamaSampler::top_k(TOP_K));
        samplers.push(LlamaSampler::temp(opts.temperature.unwrap_or(TEMPERATURE)));
        samplers.push(LlamaSampler::dist(opts.seed.unwrap_or(1234)));
        let mut sampler = LlamaSampler::chain_simple(samplers);

        // Prefill por bloques de N_BATCH.
        let t = std::time::Instant::now();
        let mut batch = LlamaBatch::new(N_BATCH as usize, 1);
        let last = tokens.len() - 1;
        for (chunk_i, chunk) in tokens.chunks(N_BATCH as usize).enumerate() {
            batch.clear();
            for (j, tok) in chunk.iter().enumerate() {
                let pos = chunk_i * N_BATCH as usize + j;
                batch.add(*tok, pos as i32, &[0], pos == last).map_err(err)?;
            }
            ctx.decode(&mut batch).map_err(err)?;
        }
        let prefill_ms = t.elapsed().as_millis() as u64;

        let t = std::time::Instant::now();
        let mut out: Vec<u8> = Vec::new();
        let mut pos = tokens.len() as i32;
        let mut generated = 0usize;
        loop {
            let tok = sampler.sample(&ctx, batch.n_tokens() - 1);
            if vocab.is_eog(tok) || generated >= max_new {
                break;
            }
            out.extend(vocab.token_to_piece(tok, false, None));
            generated += 1;
            batch.clear();
            batch.add(tok, pos, &[0], true).map_err(err)?;
            pos += 1;
            ctx.decode(&mut batch).map_err(err)?;
        }
        let decode_ms = t.elapsed().as_millis() as u64;

        let text = String::from_utf8_lossy(&out);
        let text = strip_thinking(&text).trim().to_owned();
        Ok((text, GenStats { prompt_tokens: tokens.len(), generated_tokens: generated, prefill_ms, decode_ms }))
    }
}

#[async_trait::async_trait]
impl crate::ChatModel for LlamaCpp {
    fn context_window(&self) -> usize {
        self.n_ctx as usize
    }

    /// Bloquea un hilo del runtime mientras genera (`block_in_place`): requiere
    /// el runtime multihilo de tokio.
    async fn complete(&self, msgs: &[Message], opts: &GenOptions) -> Result<String, LlmError> {
        let (text, stats) = tokio::task::block_in_place(|| self.complete_blocking(msgs, opts))?;
        tracing::debug!(?stats, "llamacpp: generación");
        if std::env::var_os("JUANOLA_LLM_TRACE").is_some() {
            eprintln!("[llm] {stats:?}");
        }
        Ok(text)
    }

    fn stream<'a>(
        &'a self,
        msgs: &'a [Message],
        opts: &'a GenOptions,
    ) -> futures::stream::BoxStream<'a, Result<String, LlmError>> {
        // Sin streaming token a token todavía: un único trozo con todo el texto.
        Box::pin(futures::stream::once(async move { crate::ChatModel::complete(self, msgs, opts).await }))
    }
}
