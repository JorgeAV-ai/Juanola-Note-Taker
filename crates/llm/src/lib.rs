//! Modelos de lenguaje. Un trait con streaming y salida estructurada, y tres
//! backends: llama.cpp (por defecto, GGUF), mistral.rs y cualquier endpoint
//! OpenAI-compatible (Ollama, LM Studio, servidor propio).

use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String, // system | user | assistant
    pub content: String,
}

#[derive(Debug, Clone, Default)]
pub struct GenOptions {
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    /// JSON Schema que la salida debe cumplir (GBNF en llama.cpp, response_format en OpenAI).
    pub json_schema: Option<serde_json::Value>,
    /// Gramática GBNF (raíz `root`) que la salida debe cumplir; solo llama.cpp.
    /// Si también hay `json_schema`, gana este.
    pub grammar: Option<String>,
    /// Semilla del muestreo; `None` = fija (resultados reproducibles).
    pub seed: Option<u32>,
}

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("contexto excedido: {0} tokens")]
    ContextExceeded(usize),
    #[error("{0}")]
    Backend(String),
}

#[async_trait::async_trait]
pub trait ChatModel: Send + Sync {
    fn context_window(&self) -> usize;
    async fn complete(&self, msgs: &[Message], opts: &GenOptions) -> Result<String, LlmError>;
    /// Varias generaciones independientes. Por defecto, una tras otra y en
    /// orden; un backend local puede generarlas a la vez (llama.cpp las
    /// decodifica en un mismo lote, que cuesta casi lo mismo que una sola).
    async fn complete_many(&self, reqs: &[(Vec<Message>, GenOptions)]) -> Result<Vec<String>, LlmError> {
        let mut out = Vec::with_capacity(reqs.len());
        for (msgs, opts) in reqs {
            out.push(self.complete(msgs, opts).await?);
        }
        Ok(out)
    }
    fn stream<'a>(
        &'a self,
        msgs: &'a [Message],
        opts: &'a GenOptions,
    ) -> BoxStream<'a, Result<String, LlmError>>;
}

pub mod chat_format;

pub mod backends {
    #[cfg(feature = "openai-compatible")]
    pub mod openai_compatible {
        //! Cliente para /v1/chat/completions (Ollama, LM Studio, vLLM...).
        pub struct OpenAiCompatible {
            pub base_url: String,
            pub model: String,
            pub api_key: Option<String>,
        }
    }
    #[cfg(feature = "llamacpp")]
    pub mod llamacpp;
}
