//! `tokenizer.json` para modelos con tokenizador de Qwen2 (BPE a nivel de byte).
//!
//! El repo MLX de Qwen3-ASR trae `vocab.json`, `merges.txt` y
//! `tokenizer_config.json`, pero no el `tokenizer.json` que lee la crate
//! `tokenizers` (lo construye `transformers` al cargar `Qwen2Tokenizer`). Aquí se
//! escribe el mismo JSON que produce `transformers`: el test lo compara con uno
//! generado por él.

use anyhow::{anyhow, Context, Result};
use serde_json::{json, Map, Value};
use std::path::Path;

/// Regex de pre-tokenización de Qwen2 (la misma que `Qwen2TokenizerFast`).
const QWEN2_SPLIT: &str = r"(?i:'s|'t|'re|'ve|'m|'ll|'d)|[^\r\n\p{L}\p{N}]?\p{L}+|\p{N}| ?[^\s\p{L}\p{N}]+[\r\n]*|\s*[\r\n]+|\s+(?!\S)|\s+";

/// Construye el `tokenizer.json` a partir de los tres ficheros de la carpeta.
pub fn qwen2_tokenizer_json(dir: &Path) -> Result<Value> {
    let read = |name: &str| std::fs::read_to_string(dir.join(name)).with_context(|| format!("leyendo {name}"));
    let vocab: Value = serde_json::from_str(&read("vocab.json")?)?;
    let config: Value = serde_json::from_str(&read("tokenizer_config.json")?)?;

    let merges: Vec<Value> = read("merges.txt")?
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with("#version"))
        .map(|l| l.split_once(' ').map(|(a, b)| json!([a, b])).ok_or_else(|| anyhow!("merge sin espacio: {l:?}")))
        .collect::<Result<_>>()?;

    let decoder = config["added_tokens_decoder"].as_object().ok_or_else(|| anyhow!("falta added_tokens_decoder"))?;
    let mut added: Vec<(u64, Value)> = decoder
        .iter()
        .map(|(id, t)| {
            let id: u64 = id.parse().with_context(|| format!("id no numérico: {id}"))?;
            let flag = |k: &str| t[k].as_bool().unwrap_or(false);
            let token = json!({
                "id": id,
                "content": t["content"].as_str().ok_or_else(|| anyhow!("token {id} sin content"))?,
                "single_word": flag("single_word"),
                "lstrip": flag("lstrip"),
                "rstrip": flag("rstrip"),
                "normalized": flag("normalized"),
                "special": flag("special"),
            });
            Ok((id, token))
        })
        .collect::<Result<_>>()?;
    added.sort_by_key(|(id, _)| *id);

    let mut model = Map::new();
    model.insert("type".into(), json!("BPE"));
    model.insert("dropout".into(), Value::Null);
    model.insert("unk_token".into(), Value::Null);
    model.insert("continuing_subword_prefix".into(), json!(""));
    model.insert("end_of_word_suffix".into(), json!(""));
    model.insert("fuse_unk".into(), json!(false));
    model.insert("byte_fallback".into(), json!(false));
    model.insert("ignore_merges".into(), json!(false));
    model.insert("vocab".into(), vocab);
    model.insert("merges".into(), Value::Array(merges));

    Ok(json!({
        "version": "1.0",
        "truncation": null,
        "padding": null,
        "added_tokens": added.into_iter().map(|(_, t)| t).collect::<Vec<_>>(),
        "normalizer": { "type": "NFC" },
        "pre_tokenizer": {
            "type": "Sequence",
            "pretokenizers": [
                { "type": "Split", "pattern": { "Regex": QWEN2_SPLIT }, "behavior": "Isolated", "invert": false },
                { "type": "ByteLevel", "add_prefix_space": false, "trim_offsets": true, "use_regex": false }
            ]
        },
        "post_processor": {
            "type": "TemplateProcessing",
            "single": [{ "Sequence": { "id": "A", "type_id": 0 } }],
            "pair": [{ "Sequence": { "id": "A", "type_id": 0 } }, { "Sequence": { "id": "B", "type_id": 1 } }],
            "special_tokens": {}
        },
        "decoder": { "type": "ByteLevel", "add_prefix_space": true, "trim_offsets": true, "use_regex": true },
        "model": Value::Object(model)
    }))
}

/// Escribe `tokenizer.json` en la carpeta si no existe. Devuelve si lo escribió.
pub fn ensure_qwen2_tokenizer_json(dir: &Path) -> Result<bool> {
    let out = dir.join("tokenizer.json");
    if out.exists() {
        return Ok(false);
    }
    let value = qwen2_tokenizer_json(dir)?;
    let tmp = dir.join("tokenizer.json.part");
    std::fs::write(&tmp, serde_json::to_vec(&value)?)?;
    std::fs::rename(&tmp, &out)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compara con el `tokenizer.json` que generó `transformers` para el modelo
    /// instalado. Se salta si el modelo no está en la máquina.
    #[test]
    fn matches_transformers_output() {
        let Some(dir) = dirs::data_dir().map(|d| d.join("dev.juanola.desktop/models/stt_streaming/qwen3-asr-1.7b-4bit-mlx")) else {
            return;
        };
        let Ok(reference) = std::fs::read_to_string(dir.join("tokenizer.json")) else {
            eprintln!("modelo no instalado, test saltado");
            return;
        };
        let reference: Value = serde_json::from_str(&reference).unwrap();
        let ours = qwen2_tokenizer_json(&dir).unwrap();
        for key in ["version", "truncation", "padding", "added_tokens", "normalizer", "pre_tokenizer", "post_processor", "decoder"] {
            assert_eq!(ours[key], reference[key], "difiere {key}");
        }
        for (key, value) in reference["model"].as_object().unwrap() {
            assert_eq!(&ours["model"][key], value, "difiere model.{key}");
        }
    }
}
