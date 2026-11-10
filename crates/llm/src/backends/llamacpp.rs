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

use crate::chat_format::{lfm3_prompt, qwen_prompt, strip_thinking};
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
    /// Formato de chat, según la arquitectura del GGUF.
    qwen: bool,
}

impl LlamaCpp {
    /// Carga un GGUF con todas las capas en GPU. `n_ctx` es el contexto por
    /// generación (el modelo admite más, pero la caché KV crece con él).
    pub fn load(path: &Path, n_ctx: u32) -> Result<Self, LlmError> {
        let params = LlamaModelParams::default().with_n_gpu_layers(u32::MAX);
        let model = LlamaModel::load_from_file(backend()?, path, &params).map_err(err)?;
        // LFM3 es el modelo de la app; Qwen se admite para comparar (ver
        // `docs/experimentos.md`). Cualquier otra arquitectura usa el de LFM3.
        let qwen = model.meta_val_str("general.architecture").is_ok_and(|a| a.starts_with("qwen"));
        Ok(Self { model, n_ctx, n_ubatch: 512, flash_attention: None, qwen })
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
        let mut out = self.complete_many_blocking(&[(msgs, opts)])?;
        Ok(out.remove(0))
    }

    /// Varias generaciones independientes a la vez, en el orden de `reqs`.
    /// Se agrupan mientras quepan en `n_ctx`; cada grupo comparte un contexto
    /// con una secuencia por petición. El prefill va secuencia a secuencia y la
    /// decodificación, en un mismo lote con un token de cada secuencia por paso:
    /// leer los pesos domina el coste, así que un paso con N secuencias cuesta
    /// casi lo mismo que con una.
    pub fn complete_many_blocking(&self, reqs: &[(&[Message], &GenOptions)]) -> Result<Vec<(String, GenStats)>, LlmError> {
        let vocab = self.model.vocab();
        let mut prepared = Vec::with_capacity(reqs.len());
        for (msgs, opts) in reqs {
            let prompt = if self.qwen { qwen_prompt(msgs) } else { lfm3_prompt(msgs) };
            // El de LFM3 ya trae `<|startoftext|>`, y Qwen no usa BOS: no añadir otro.
            let tokens = vocab.tokenize(prompt.as_bytes(), false, true);
            let max_new = opts.max_tokens.unwrap_or(2048) as usize;
            if tokens.len() + max_new > self.n_ctx as usize {
                return Err(LlmError::ContextExceeded(tokens.len() + max_new));
            }
            prepared.push((tokens, max_new, *opts));
        }
        let mut out = Vec::with_capacity(reqs.len());
        let mut group: Vec<(Vec<llama_cpp_2::token::LlamaToken>, usize, &GenOptions)> = Vec::new();
        let mut used = 0;
        for p in prepared {
            let need = p.0.len() + p.1;
            if !group.is_empty() && used + need > self.n_ctx as usize {
                out.extend(self.generate_group(std::mem::take(&mut group))?);
                used = 0;
            }
            used += need;
            group.push(p);
        }
        if !group.is_empty() {
            out.extend(self.generate_group(group)?);
        }
        Ok(out)
    }

    fn sampler(&self, opts: &GenOptions) -> Result<LlamaSampler, LlmError> {
        let vocab = self.model.vocab();
        let mut samplers = Vec::new();
        // El razonamiento va desactivado (`<think></think>` en el prompt), pero
        // con entradas cortas LFM3 abre otro `<think>` y gasta todo el
        // presupuesto pensando: se prohíbe ese token.
        let think = vocab.tokenize(b"<think>", false, true);
        if let [tok] = think[..] {
            samplers.push(LlamaSampler::logit_bias(
                self.model.n_vocab(),
                &[llama_cpp_2::token::logit_bias::LlamaLogitBias::new(tok, f32::NEG_INFINITY)],
            ));
        }
        if let Some(g) = &opts.grammar {
            samplers.push(LlamaSampler::grammar(&self.model, g, "root").map_err(err)?);
        } else if let Some(schema) = &opts.json_schema {
            let gbnf = llama_cpp_2::json_schema_to_grammar(&schema.to_string()).map_err(err)?;
            samplers.push(LlamaSampler::grammar(&self.model, &gbnf, "root").map_err(err)?);
        }
        samplers.push(LlamaSampler::penalties(self.model.n_vocab(), REPEAT_LAST_N, REPEAT_PENALTY, 0.0, 0.0));
        samplers.push(LlamaSampler::top_k(TOP_K));
        samplers.push(LlamaSampler::temp(opts.temperature.unwrap_or(TEMPERATURE)));
        samplers.push(LlamaSampler::dist(opts.seed.unwrap_or(1234)));
        Ok(LlamaSampler::chain_simple(samplers))
    }

    fn generate_group(
        &self,
        group: Vec<(Vec<llama_cpp_2::token::LlamaToken>, usize, &GenOptions)>,
    ) -> Result<Vec<(String, GenStats)>, LlmError> {
        let vocab = self.model.vocab();
        let n_seq = group.len();
        // La caché KV se dimensiona a lo que hace falta (redondeado a 1024), no
        // al máximo configurado: reservar 32K para un prompt de 2K cuesta tiempo.
        // Unificada: las secuencias tienen longitudes muy distintas (el resumen
        // lleva toda la transcripción; los detalles, 4 minutos).
        let total: usize = group.iter().map(|(t, m, _)| t.len() + m).sum();
        let needed = (total.div_ceil(1024) * 1024) as u32;
        let mut ctx_params = LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(needed.min(self.n_ctx)))
            .with_n_batch(N_BATCH)
            .with_n_ubatch(self.n_ubatch)
            .with_n_seq_max(n_seq as u32)
            .with_kv_unified(true);
        if let Some(on) = self.flash_attention {
            // LLAMA_FLASH_ATTN_TYPE_ENABLED = 1, _DISABLED = 0 (llama.h).
            ctx_params = ctx_params.with_flash_attention_policy(if on { 1 } else { 0 });
        }
        let mut ctx = self.model.new_context(backend()?, ctx_params).map_err(err)?;

        struct Seq {
            sampler: LlamaSampler,
            next: llama_cpp_2::token::LlamaToken,
            pos: i32,
            max_new: usize,
            out: Vec<u8>,
            stats: GenStats,
            done: bool,
            max_refs: Option<usize>,
            /// Tokens que se meten en lugar de los muestreados (el cierre de una cita).
            forced: std::collections::VecDeque<llama_cpp_2::token::LlamaToken>,
        }
        let close_cite = vocab.tokenize(b"]\n", false, false);
        let mut seqs = Vec::with_capacity(n_seq);
        let mut batch = LlamaBatch::new(N_BATCH as usize, n_seq as i32);
        // Prefill por bloques de N_BATCH, secuencia a secuencia; el primer token
        // se muestrea en cuanto acaba su prompt.
        for (s, (tokens, max_new, opts)) in group.iter().enumerate() {
            let mut sampler = self.sampler(opts)?;
            let t = std::time::Instant::now();
            let last = tokens.len() - 1;
            for (chunk_i, chunk) in tokens.chunks(N_BATCH as usize).enumerate() {
                batch.clear();
                for (j, tok) in chunk.iter().enumerate() {
                    let pos = chunk_i * N_BATCH as usize + j;
                    batch.add(*tok, pos as i32, &[s as i32], pos == last).map_err(err)?;
                }
                ctx.decode(&mut batch).map_err(err)?;
            }
            let next = sampler.sample(&ctx, batch.n_tokens() - 1);
            let stats = GenStats { prompt_tokens: tokens.len(), prefill_ms: t.elapsed().as_millis() as u64, ..Default::default() };
            seqs.push(Seq {
                sampler,
                next,
                pos: tokens.len() as i32,
                max_new: *max_new,
                out: Vec::new(),
                stats,
                done: false,
                max_refs: opts.max_refs,
                forced: Default::default(),
            });
        }

        // Decodificación: en cada paso, un token de cada secuencia viva.
        let t = std::time::Instant::now();
        let mut in_batch: Vec<usize> = Vec::with_capacity(n_seq);
        loop {
            batch.clear();
            in_batch.clear();
            for (s, q) in seqs.iter_mut().enumerate() {
                if q.done {
                    continue;
                }
                if let Some(tok) = q.forced.pop_front() {
                    q.next = tok;
                } else if let Some(max) = q.max_refs {
                    let piece = vocab.token_to_piece(q.next, false, None);
                    if piece.contains(&b',') && open_cite_refs(&q.out) >= max {
                        q.forced.extend(close_cite.iter().copied());
                        q.next = q.forced.pop_front().expect("«]\\n» tiene tokens");
                    }
                }
                if vocab.is_eog(q.next) || q.stats.generated_tokens >= q.max_new {
                    q.done = true;
                    q.stats.decode_ms = t.elapsed().as_millis() as u64;
                    continue;
                }
                q.out.extend(vocab.token_to_piece(q.next, false, None));
                q.stats.generated_tokens += 1;
                batch.add(q.next, q.pos, &[s as i32], true).map_err(err)?;
                q.pos += 1;
                in_batch.push(s);
            }
            if in_batch.is_empty() {
                break;
            }
            ctx.decode(&mut batch).map_err(err)?;
            for (i, &s) in in_batch.iter().enumerate() {
                seqs[s].next = seqs[s].sampler.sample(&ctx, i as i32);
            }
        }

        Ok(seqs
            .into_iter()
            .map(|q| {
                let text = String::from_utf8_lossy(&q.out);
                (strip_thinking(&text).trim().to_owned(), q.stats)
            })
            .collect())
    }
}

/// Referencias de la cita que está abierta al final de `out` (`[12, 14, 1` → 3);
/// 0 si no hay ninguna abierta o lo abierto no es una cita (`[nota`).
fn open_cite_refs(out: &[u8]) -> usize {
    let Some(open) = out.iter().rposition(|&b| b == b'[') else { return 0 };
    let inner = &out[open + 1..];
    // Bytes no ASCII: los guiones Unicode de los rangos (`162‑166`, `12–14`).
    if inner.contains(&b']') || !inner.iter().all(|b| b.is_ascii_digit() || b" ,-;".contains(b) || *b >= 0x80) {
        return 0;
    }
    inner.split(|&b| b == b',' || b == b';').filter(|p| p.iter().any(u8::is_ascii_digit)).count()
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

    async fn complete_many(&self, reqs: &[(Vec<Message>, GenOptions)]) -> Result<Vec<String>, LlmError> {
        let refs: Vec<(&[Message], &GenOptions)> = reqs.iter().map(|(m, o)| (m.as_slice(), o)).collect();
        let t = std::time::Instant::now();
        // JUANOLA_LLM_NO_BATCH=1: una tras otra, para comparar (diagnóstico).
        let out = if std::env::var_os("JUANOLA_LLM_NO_BATCH").is_some() {
            refs.iter().map(|r| tokio::task::block_in_place(|| self.complete_blocking(r.0, r.1))).collect::<Result<Vec<_>, _>>()?
        } else {
            tokio::task::block_in_place(|| self.complete_many_blocking(&refs))?
        };
        let trace = std::env::var_os("JUANOLA_LLM_TRACE").is_some();
        for (_, stats) in &out {
            tracing::debug!(?stats, "llamacpp: generación en lote");
            if trace {
                eprintln!("[llm] lote de {}: {stats:?}", out.len());
            }
        }
        if trace {
            eprintln!("[llm] lote de {} en {} ms", out.len(), t.elapsed().as_millis());
        }
        Ok(out.into_iter().map(|(text, _)| text).collect())
    }

    fn stream<'a>(&'a self, msgs: &'a [Message], opts: &'a GenOptions) -> futures::stream::BoxStream<'a, Result<String, LlmError>> {
        // Sin streaming token a token todavía: un único trozo con todo el texto.
        Box::pin(futures::stream::once(async move { crate::ChatModel::complete(self, msgs, opts).await }))
    }
}

#[cfg(test)]
mod tests {
    use super::open_cite_refs;

    #[test]
    fn counts_refs_of_the_open_citation() {
        assert_eq!(open_cite_refs(b"- Algo [12, 14, 1"), 3);
        assert_eq!(open_cite_refs(b"- Algo [12-14; 20"), 2);
        assert_eq!(open_cite_refs("- Algo [162\u{2011}166, 167, 1".as_bytes()), 3);
        assert_eq!(open_cite_refs(b"- Algo [12, 14]. Otra"), 0);
        assert_eq!(open_cite_refs(b"- Algo [nota, 3"), 0);
        assert_eq!(open_cite_refs(b"- Sin citas"), 0);
        assert_eq!(open_cite_refs(b"- Algo ["), 0);
    }
}
