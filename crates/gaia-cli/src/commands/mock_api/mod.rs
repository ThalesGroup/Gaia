use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow};
use clap::Args;
use serde_json::{Value, json};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

mod auth;
mod errors;
mod routes_chat;
mod routes_embeddings;
mod routes_responses;
mod sse;

use auth::is_authorized;
use errors::openai_error_response;
use routes_chat::{build_chat_completions_response, build_completions_response};
use routes_embeddings::{
    build_audio_speech_response, build_audio_transcription_response, build_embeddings_response,
};
use routes_responses::build_responses_api_response;

pub(crate) type HttpResponse = Response<std::io::Cursor<Vec<u8>>>;

static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Args, Clone)]
pub struct MockApiArgs {
    #[arg(long, default_value = "0.0.0.0")]
    pub host: String,
    #[arg(long, default_value_t = 8000)]
    pub port: u16,
    #[arg(long, default_value = "Qwen/Qwen2.5-7B-Instruct")]
    pub model: String,
    #[arg(long, default_value = "local-key")]
    pub api_key: String,
}

pub fn run(args: MockApiArgs) -> Result<()> {
    run_server_loop(&args.host, args.port, &args.model, &args.api_key)
}

pub fn run_server_loop(host: &str, port: u16, model_id: &str, api_key: &str) -> Result<()> {
    let server = Server::http((host, port))
        .map_err(|error| anyhow!("Unable to bind mock server on {host}:{port}: {error}"))?;
    println!("Mock OpenAI API listening on http://{host}:{port}/v1");
    println!("Model: {model_id}");

    for request in server.incoming_requests() {
        if let Err(error) = handle_request(request, model_id, api_key) {
            eprintln!("Mock API request error: {error}");
        }
    }

    Ok(())
}

fn handle_request(mut request: Request, model_id: &str, api_key: &str) -> Result<()> {
    let method = request.method().clone();
    let path = normalize_path(request.url());

    if method == Method::Options {
        request.respond(empty_response(StatusCode(204)))?;
        return Ok(());
    }

    if !is_authorized(&request, api_key) {
        request.respond(openai_error_response(
            StatusCode(401),
            "Invalid API key for mock server.",
            "invalid_request_error",
        ))?;
        return Ok(());
    }

    if method == Method::Get && path.starts_with("/v1/models/") {
        let model_name = path.trim_start_matches("/v1/models/");
        request.respond(json_response(
            StatusCode(200),
            json!({
                "id": model_name,
                "object": "model",
                "created": unix_timestamp(),
                "owned_by": "gaia-mock"
            }),
        ))?;
        return Ok(());
    }

    if method == Method::Get && path.starts_with("/v1/responses/") {
        let response_id = path.trim_start_matches("/v1/responses/");
        let output_text =
            "(mock) Stored response replay is simulated. Use POST /v1/responses for fresh output.";
        request.respond(json_response(
            StatusCode(200),
            json!({
                "id": response_id,
                "object": "response",
                "created_at": unix_timestamp(),
                "model": model_id,
                "status": "completed",
                "output_text": output_text,
                "output": [{
                    "id": format!("msg_{}", next_id("response")),
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
        ))?;
        return Ok(());
    }

    match (method, path.as_str()) {
        (Method::Get, "/health") | (Method::Get, "/v1/health") => {
            request.respond(json_response(StatusCode(200), json!({ "status": "ok" })))?;
        }
        (Method::Get, "/v1/models") => {
            request.respond(json_response(
                StatusCode(200),
                json!({
                    "object": "list",
                    "data": [{
                        "id": model_id,
                        "object": "model",
                        "owned_by": "gaia-mock"
                    }]
                }),
            ))?;
        }
        (Method::Post, "/v1/chat/completions") => {
            let payload = read_json_payload(&mut request).unwrap_or_else(|_| json!({}));
            let response = build_chat_completions_response(&payload, model_id);
            request.respond(response)?;
        }
        (Method::Post, "/v1/completions") => {
            let payload = read_json_payload(&mut request).unwrap_or_else(|_| json!({}));
            let response = build_completions_response(&payload, model_id);
            request.respond(response)?;
        }
        (Method::Post, "/v1/embeddings") => {
            let payload = read_json_payload(&mut request).unwrap_or_else(|_| json!({}));
            let response = build_embeddings_response(&payload, model_id);
            request.respond(response)?;
        }
        (Method::Post, "/v1/responses") => {
            let payload = read_json_payload(&mut request).unwrap_or_else(|_| json!({}));
            let response = build_responses_api_response(&payload, model_id);
            request.respond(response)?;
        }
        (Method::Post, "/v1/audio/transcriptions") => {
            let audio_body = read_request_body(&mut request).unwrap_or_default();
            let response = build_audio_transcription_response(&audio_body);
            request.respond(response)?;
        }
        (Method::Post, "/v1/audio/speech") => {
            let payload = read_json_payload(&mut request).unwrap_or_else(|_| json!({}));
            let response = build_audio_speech_response(&payload);
            request.respond(response)?;
        }
        _ => {
            request.respond(openai_error_response(
                StatusCode(404),
                &format!("Unknown mock endpoint: {}", path),
                "not_found_error",
            ))?;
        }
    }

    Ok(())
}

pub(crate) fn read_request_body(request: &mut Request) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    request
        .as_reader()
        .read_to_end(&mut body)
        .context("Unable to read request body")?;
    Ok(body)
}

pub(crate) fn read_json_payload(request: &mut Request) -> Result<Value> {
    let body = read_request_body(request)?;
    if body.is_empty() {
        return Ok(json!({}));
    }
    let parsed = serde_json::from_slice::<Value>(&body).unwrap_or_else(|_| json!({}));
    Ok(parsed)
}

pub(crate) fn next_id(prefix: &str) -> String {
    let value = REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}-mock-{}-{value}", unix_timestamp())
}

pub(crate) fn normalize_path(url: &str) -> String {
    url.split('?').next().unwrap_or(url).to_owned()
}

pub(crate) fn json_response(status: StatusCode, payload: Value) -> HttpResponse {
    response_with_content_type(status, payload.to_string().into_bytes(), "application/json")
}

pub(crate) fn sse_response(status: StatusCode, body: String) -> HttpResponse {
    response_with_content_type(status, body.into_bytes(), "text/event-stream")
}

pub(crate) fn binary_response(
    status: StatusCode,
    body: Vec<u8>,
    content_type: &str,
) -> HttpResponse {
    response_with_content_type(status, body, content_type)
}

pub(crate) fn empty_response(status: StatusCode) -> HttpResponse {
    response_with_content_type(status, Vec::new(), "application/json")
}

fn response_with_content_type(
    status: StatusCode,
    body: Vec<u8>,
    content_type: &str,
) -> HttpResponse {
    let response = Response::from_data(body).with_status_code(status);
    let response = with_header_if_valid(response, "Content-Type", content_type);
    let response = with_header_if_valid(response, "Access-Control-Allow-Origin", "*");
    let response = with_header_if_valid(
        response,
        "Access-Control-Allow-Headers",
        "Content-Type, Authorization",
    );
    with_header_if_valid(
        response,
        "Access-Control-Allow-Methods",
        "GET, POST, OPTIONS",
    )
}

fn with_header_if_valid(response: HttpResponse, name: &str, value: &str) -> HttpResponse {
    match Header::from_bytes(name.as_bytes(), value.as_bytes()) {
        Ok(header) => response.with_header(header),
        Err(_) => response,
    }
}

pub(crate) fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
