use serde_json::{Value, json};
use tiny_http::StatusCode;

use super::routes_chat::{approximate_tokens, extract_text_from_value};
use super::{HttpResponse, binary_response, json_response};

pub(crate) fn build_embeddings_response(payload: &Value, default_model: &str) -> HttpResponse {
    let model = payload
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or(default_model);
    let inputs = extract_embedding_inputs(payload)
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>();
    let final_inputs = if inputs.is_empty() {
        vec!["".to_owned()]
    } else {
        inputs
    };

    let data = final_inputs
        .iter()
        .enumerate()
        .map(|(index, input)| {
            json!({
                "object": "embedding",
                "index": index,
                "embedding": generate_embedding(input, 24)
            })
        })
        .collect::<Vec<_>>();

    let token_count = final_inputs
        .iter()
        .map(|input| approximate_tokens(input))
        .sum::<u64>();

    json_response(
        StatusCode(200),
        json!({
            "object": "list",
            "data": data,
            "model": model,
            "usage": {
                "prompt_tokens": token_count,
                "total_tokens": token_count
            }
        }),
    )
}

pub(crate) fn build_audio_transcription_response(audio_body: &[u8]) -> HttpResponse {
    let transcript = if audio_body.is_empty() {
        "(mock transcription) empty audio payload.".to_owned()
    } else {
        format!(
            "(mock transcription) received {} bytes of audio data.",
            audio_body.len()
        )
    };
    json_response(
        StatusCode(200),
        json!({
            "text": transcript
        }),
    )
}

pub(crate) fn build_audio_speech_response(payload: &Value) -> HttpResponse {
    let input_text = extract_text_from_value(payload.get("input"))
        .unwrap_or_else(|| "Hello from Gaia mock speech mode.".to_owned());
    let audio_bytes = format!("MOCK_MP3:{input_text}").into_bytes();
    binary_response(StatusCode(200), audio_bytes, "audio/mpeg")
}

fn extract_embedding_inputs(payload: &Value) -> Vec<String> {
    match payload.get("input") {
        Some(Value::String(text)) => vec![text.clone()],
        Some(Value::Array(values)) => values
            .iter()
            .filter_map(|value| match value {
                Value::String(text) => Some(text.clone()),
                Value::Array(tokens) => Some(
                    tokens
                        .iter()
                        .filter_map(Value::as_i64)
                        .map(|value| value.to_string())
                        .collect::<Vec<_>>()
                        .join(" "),
                ),
                _ => extract_text_from_value(Some(value)),
            })
            .collect(),
        Some(value) => extract_text_from_value(Some(value)).into_iter().collect(),
        None => Vec::new(),
    }
}

fn generate_embedding(input: &str, dimensions: usize) -> Vec<f64> {
    let mut state = fnv1a_hash(input.as_bytes());
    (0..dimensions)
        .map(|_| {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let value = ((state >> 33) as f64) / (u32::MAX as f64);
            (value * 2.0) - 1.0
        })
        .collect()
}

fn fnv1a_hash(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
