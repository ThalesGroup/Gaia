use serde_json::{Value, json};
use tiny_http::StatusCode;

use super::sse::{split_for_streaming, to_sse_body};
use super::{HttpResponse, json_response, next_id, sse_response, unix_timestamp};

pub(crate) fn build_chat_completions_response(
    payload: &Value,
    default_model: &str,
) -> HttpResponse {
    let model = payload
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or(default_model);
    let stream = payload
        .get("stream")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let user_text = extract_last_user_message(payload)
        .unwrap_or_else(|| "Hello from Gaia mock mode.".to_owned());
    let completion =
        format!("(mock) I received: {user_text}. This is a simulated answer from Gaia.");

    if stream {
        return sse_response(
            StatusCode(200),
            build_chat_completion_stream(model, &completion),
        );
    }

    let prompt_tokens = approximate_tokens(&user_text);
    let completion_tokens = approximate_tokens(&completion);
    json_response(
        StatusCode(200),
        json!({
            "id": next_id("chatcmpl"),
            "object": "chat.completion",
            "created": unix_timestamp(),
            "model": model,
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": completion
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": prompt_tokens,
                "completion_tokens": completion_tokens,
                "total_tokens": prompt_tokens + completion_tokens
            }
        }),
    )
}

pub(crate) fn build_completions_response(payload: &Value, default_model: &str) -> HttpResponse {
    let model = payload
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or(default_model);
    let stream = payload
        .get("stream")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let prompt = extract_prompt(payload).unwrap_or_else(|| "Hello from Gaia mock mode.".to_owned());
    let completion =
        format!("(mock completion) Prompt received: {prompt}. This is a simulated completion.");

    if stream {
        return sse_response(
            StatusCode(200),
            build_completions_stream(model, &completion),
        );
    }

    let prompt_tokens = approximate_tokens(&prompt);
    let completion_tokens = approximate_tokens(&completion);
    json_response(
        StatusCode(200),
        json!({
            "id": next_id("cmpl"),
            "object": "text_completion",
            "created": unix_timestamp(),
            "model": model,
            "choices": [{
                "text": completion,
                "index": 0,
                "logprobs": Value::Null,
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": prompt_tokens,
                "completion_tokens": completion_tokens,
                "total_tokens": prompt_tokens + completion_tokens
            }
        }),
    )
}

fn build_chat_completion_stream(model: &str, completion: &str) -> String {
    let chat_id = next_id("chatcmpl");
    let created = unix_timestamp();
    let mut events = Vec::new();
    events.push(json!({
        "id": chat_id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model,
        "choices": [{
            "index": 0,
            "delta": { "role": "assistant" },
            "finish_reason": Value::Null
        }]
    }));

    for chunk in split_for_streaming(completion) {
        events.push(json!({
            "id": chat_id,
            "object": "chat.completion.chunk",
            "created": created,
            "model": model,
            "choices": [{
                "index": 0,
                "delta": { "content": chunk },
                "finish_reason": Value::Null
            }]
        }));
    }

    events.push(json!({
        "id": chat_id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model,
        "choices": [{
            "index": 0,
            "delta": {},
            "finish_reason": "stop"
        }]
    }));

    to_sse_body(&events)
}

fn build_completions_stream(model: &str, completion: &str) -> String {
    let completion_id = next_id("cmpl");
    let created = unix_timestamp();
    let mut events = Vec::new();
    for chunk in split_for_streaming(completion) {
        events.push(json!({
            "id": completion_id,
            "object": "text_completion",
            "created": created,
            "model": model,
            "choices": [{
                "text": chunk,
                "index": 0,
                "logprobs": Value::Null,
                "finish_reason": Value::Null
            }]
        }));
    }
    events.push(json!({
        "id": completion_id,
        "object": "text_completion",
        "created": created,
        "model": model,
        "choices": [{
            "text": "",
            "index": 0,
            "logprobs": Value::Null,
            "finish_reason": "stop"
        }]
    }));
    to_sse_body(&events)
}

fn extract_last_user_message(payload: &Value) -> Option<String> {
    let messages = payload.get("messages")?.as_array()?;
    messages.iter().rev().find_map(|message| {
        let role = message.get("role").and_then(Value::as_str)?;
        if role != "user" {
            return None;
        }
        extract_text_from_value(message.get("content"))
    })
}

fn extract_prompt(payload: &Value) -> Option<String> {
    match payload.get("prompt") {
        Some(Value::String(prompt)) => Some(prompt.clone()),
        Some(Value::Array(prompts)) => prompts.iter().find_map(|entry| match entry {
            Value::String(prompt) => Some(prompt.clone()),
            _ => None,
        }),
        _ => None,
    }
}

pub(crate) fn extract_text_from_value(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(text) => Some(text.clone()),
        Value::Array(items) => {
            let parts = items
                .iter()
                .filter_map(|item| match item {
                    Value::String(text) => Some(text.clone()),
                    Value::Object(_) => item
                        .get("text")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                        .or_else(|| {
                            item.get("input_text")
                                .and_then(Value::as_str)
                                .map(str::to_owned)
                        }),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if parts.is_empty() {
                None
            } else {
                Some(parts.join(" "))
            }
        }
        Value::Object(object) => object
            .get("content")
            .and_then(|content| extract_text_from_value(Some(content)))
            .or_else(|| {
                object
                    .get("text")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
            .or_else(|| {
                object
                    .get("input_text")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            }),
        _ => None,
    }
}

pub(crate) fn approximate_tokens(text: &str) -> u64 {
    text.split_whitespace().count().max(1) as u64
}
