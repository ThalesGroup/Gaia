use serde_json::{Value, json};
use tiny_http::StatusCode;

use super::routes_chat::extract_text_from_value;
use super::sse::{split_for_streaming, to_sse_body};
use super::{HttpResponse, json_response, next_id, sse_response, unix_timestamp};

pub(crate) fn build_responses_api_response(payload: &Value, default_model: &str) -> HttpResponse {
    let model = payload
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or(default_model);
    let stream = payload
        .get("stream")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let input_text = extract_responses_input(payload)
        .unwrap_or_else(|| "Hello from Gaia mock responses mode.".to_owned());
    let output_text =
        format!("(mock response) I received: {input_text}. This is a simulated response.");

    if stream {
        return sse_response(StatusCode(200), build_responses_stream(model, &output_text));
    }

    let response_id = next_id("resp");
    let message_id = next_id("msg");
    json_response(
        StatusCode(200),
        json!({
            "id": response_id,
            "object": "response",
            "created_at": unix_timestamp(),
            "model": model,
            "status": "completed",
            "output_text": output_text,
            "output": [{
                "id": message_id,
                "type": "message",
                "role": "assistant",
                "status": "completed",
                "content": [{
                    "type": "output_text",
                    "text": output_text,
                    "annotations": []
                }]
            }]
        }),
    )
}

fn build_responses_stream(model: &str, output_text: &str) -> String {
    let response_id = next_id("resp");
    let message_id = next_id("msg");
    let created_at = unix_timestamp();
    let mut events = Vec::new();
    events.push(json!({
        "type": "response.created",
        "sequence_number": 0,
        "response": {
            "id": response_id,
            "object": "response",
            "created_at": created_at,
            "model": model,
            "status": "in_progress",
            "output": []
        }
    }));

    for (index, chunk) in split_for_streaming(output_text).iter().enumerate() {
        events.push(json!({
            "type": "response.output_text.delta",
            "sequence_number": index + 1,
            "item_id": message_id,
            "output_index": 0,
            "content_index": 0,
            "delta": chunk
        }));
    }

    let completed_seq = events.len();
    events.push(json!({
        "type": "response.completed",
        "sequence_number": completed_seq,
        "response": {
            "id": response_id,
            "object": "response",
            "created_at": created_at,
            "model": model,
            "status": "completed",
            "output": [{
                "id": message_id,
                "type": "message",
                "role": "assistant",
                "status": "completed",
                "content": [{
                    "type": "output_text",
                    "text": output_text,
                    "annotations": []
                }]
            }]
        }
    }));

    to_sse_body(&events)
}

fn extract_responses_input(payload: &Value) -> Option<String> {
    match payload.get("input") {
        Some(Value::String(text)) => Some(text.clone()),
        Some(Value::Array(items)) => {
            let collected = items
                .iter()
                .filter_map(|item| extract_text_from_value(Some(item)))
                .collect::<Vec<_>>();
            if collected.is_empty() {
                None
            } else {
                Some(collected.join(" "))
            }
        }
        Some(value) => extract_text_from_value(Some(value)),
        None => None,
    }
}
