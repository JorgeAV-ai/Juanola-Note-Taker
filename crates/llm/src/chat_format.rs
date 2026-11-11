//! Formato de prompt de LFM3 (Liquid AI), equivalente a su plantilla Jinja
//! (`tokenizer.chat_template` del GGUF) para mensajes system/user/assistant sin
//! tools ni imágenes, con `enable_thinking = false`.
//!
//! La plantilla usa `{% generation %}`, que solo entiende el Jinja de Hugging
//! Face, así que no se puede renderizar con `minijinja`. Los tests comparan con
//! la salida de la plantilla original renderizada en Python.

use crate::Message;

pub const LFM3_BOS: &str = "<|startoftext|>";

/// Prompt listo para tokenizar (con `parse_special = true`), terminado en el
/// turno del asistente con el razonamiento desactivado (`<think></think>`).
pub fn lfm3_prompt(messages: &[Message]) -> String {
    let mut out = String::from(LFM3_BOS);
    let mut rest = messages;
    if let Some(first) = messages.first() {
        if first.role == "system" {
            if !first.content.is_empty() {
                out.push_str("<|im_start|>system\n");
                out.push_str(&first.content);
                out.push_str("<|im_end|>\n");
            }
            rest = &messages[1..];
        }
    }
    for m in rest {
        out.push_str("<|im_start|>");
        out.push_str(&m.role);
        out.push('\n');
        if m.role == "assistant" {
            // Turnos previos al último del usuario: el razonamiento no se conserva.
            let content = m.content.rsplit("</think>").next().unwrap_or("").trim();
            out.push_str("<think></think>");
            out.push_str(content);
        } else {
            out.push_str(&m.content);
        }
        out.push_str("<|im_end|>\n");
    }
    out.push_str("<|im_start|>assistant\n<think></think>");
    out
}

/// Prompt ChatML de Qwen3/Qwen3.5 (su plantilla Jinja con
/// `enable_thinking = false`): sin BOS y con el bloque de razonamiento vacío.
pub fn qwen_prompt(messages: &[Message]) -> String {
    let mut out = String::new();
    for m in messages {
        out.push_str("<|im_start|>");
        out.push_str(&m.role);
        out.push('\n');
        // El razonamiento solo se quita en turnos del asistente (como la
        // plantilla Jinja); en user/system es contenido legítimo.
        if m.role == "assistant" {
            out.push_str(m.content.rsplit("</think>").next().unwrap_or("").trim_start());
        } else {
            out.push_str(&m.content);
        }
        out.push_str("<|im_end|>\n");
    }
    out.push_str("<|im_start|>assistant\n<think>\n\n</think>\n\n");
    out
}

/// Quita cualquier bloque `<think>…</think>` que el modelo emita pese a todo.
pub fn strip_thinking(text: &str) -> &str {
    match text.rfind("</think>") {
        Some(i) => text[i + "</think>".len()..].trim_start(),
        None => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qwen_without_thinking() {
        let m = |r: &str, c: &str| Message { role: r.into(), content: c.into() };
        assert_eq!(qwen_prompt(&[m("user", "Hola")]), "<|im_start|>user\nHola<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n");
    }

    fn m(role: &str, content: &str) -> Message {
        Message { role: role.into(), content: content.into() }
    }

    // Esperados: plantilla Jinja del GGUF LiquidAI/LFM3-1.7B renderizada con
    // jinja2 (add_generation_prompt=True, enable_thinking=False).
    #[test]
    fn system_and_user() {
        assert_eq!(
            lfm3_prompt(&[m("system", "Eres útil."), m("user", "Hola\nqué tal")]),
            "<|startoftext|><|im_start|>system\nEres útil.<|im_end|>\n<|im_start|>user\nHola\nqué tal<|im_end|>\n<|im_start|>assistant\n<think></think>"
        );
    }

    #[test]
    fn user_only() {
        assert_eq!(
            lfm3_prompt(&[m("user", "Hola")]),
            "<|startoftext|><|im_start|>user\nHola<|im_end|>\n<|im_start|>assistant\n<think></think>"
        );
    }

    #[test]
    fn multi_turn() {
        assert_eq!(
            lfm3_prompt(&[m("system", "S"), m("user", "a"), m("assistant", "b"), m("user", "c")]),
            "<|startoftext|><|im_start|>system\nS<|im_end|>\n<|im_start|>user\na<|im_end|>\n<|im_start|>assistant\n<think></think>b<|im_end|>\n<|im_start|>user\nc<|im_end|>\n<|im_start|>assistant\n<think></think>"
        );
    }

    #[test]
    fn strips_think() {
        assert_eq!(strip_thinking("<think>hmm</think>\n{\"a\":1}"), "{\"a\":1}");
        assert_eq!(strip_thinking("{\"a\":1}"), "{\"a\":1}");
    }
}
