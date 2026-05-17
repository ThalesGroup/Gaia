use std::fs;
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use reqwest::blocking::Client;
use serde_json::json;

static SANDBOX_COUNTER: AtomicUsize = AtomicUsize::new(1);

struct MockServer {
    child: Child,
    base_url: String,
    api_key: String,
}

impl MockServer {
    fn start() -> Self {
        let sandbox = create_sandbox();
        let config_home = sandbox.join("config");
        let data_home = sandbox.join("data");
        let state_home = sandbox.join("state");
        fs::create_dir_all(&config_home).expect("must create XDG config home");
        fs::create_dir_all(&data_home).expect("must create XDG data home");
        fs::create_dir_all(&state_home).expect("must create XDG state home");

        let port = free_port();
        let api_key = "core-contract-key".to_owned();
        let child = Command::new(gaia_bin_path())
            .args([
                "__mock-api",
                "--host",
                "127.0.0.1",
                "--port",
                &port.to_string(),
                "--api-key",
                &api_key,
                "--model",
                "Qwen/Qwen2.5-7B-Instruct",
            ])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .env("HOME", &sandbox)
            .env("XDG_CONFIG_HOME", &config_home)
            .env("XDG_DATA_HOME", &data_home)
            .env("XDG_STATE_HOME", &state_home)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("mock server should start");

        let server = Self {
            child,
            base_url: format!("http://127.0.0.1:{port}/v1"),
            api_key,
        };
        server.wait_until_ready();
        server
    }

    fn wait_until_ready(&self) {
        let client = Client::builder()
            .timeout(Duration::from_millis(400))
            .build()
            .expect("must build reqwest client");
        let url = format!("{}/models", self.base_url);
        for _ in 0..40 {
            if let Ok(response) = client
                .get(&url)
                .bearer_auth(&self.api_key)
                .send()
                .and_then(|response| response.error_for_status())
            {
                let _ = response.text();
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
        panic!("mock server did not become ready in time");
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn openai_core_contract_holds_for_declared_backends() {
    let server = MockServer::start();
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("must build reqwest client");
    let backends = ["vllm", "tgi", "sglang", "llamacpp", "ollama"];

    for backend in backends {
        let model_name = format!("gaia-core-contract-{backend}");
        let endpoint = format!("{}/chat/completions", server.base_url);

        let non_stream = client
            .post(&endpoint)
            .bearer_auth(&server.api_key)
            .json(&json!({
                "model": model_name,
                "messages": [{"role": "user", "content": format!("healthcheck {backend}")}],
                "stream": false
            }))
            .send()
            .expect("non-stream request should complete");
        assert!(
            non_stream.status().is_success(),
            "non-stream chat/completions must succeed for backend `{backend}`"
        );
        let non_stream_json = non_stream
            .json::<serde_json::Value>()
            .expect("non-stream response should be JSON");
        assert_eq!(
            non_stream_json
                .get("object")
                .and_then(|value| value.as_str()),
            Some("chat.completion"),
            "non-stream response should be chat.completion for backend `{backend}`"
        );

        let stream = client
            .post(&endpoint)
            .bearer_auth(&server.api_key)
            .json(&json!({
                "model": model_name,
                "messages": [{"role": "user", "content": format!("stream {backend}")}],
                "stream": true
            }))
            .send()
            .expect("stream request should complete");
        assert!(
            stream.status().is_success(),
            "stream chat/completions must succeed for backend `{backend}`"
        );
        let stream_body = stream.text().expect("stream response should be readable");
        assert!(
            stream_body.contains("[DONE]"),
            "stream response should terminate with [DONE] for backend `{backend}`"
        );

        let unauthorized = client
            .post(&endpoint)
            .bearer_auth("wrong-key")
            .json(&json!({
                "model": model_name,
                "messages": [{"role": "user", "content": "auth check"}],
                "stream": false
            }))
            .send()
            .expect("unauthorized request should complete");
        assert_eq!(
            unauthorized.status().as_u16(),
            401,
            "auth failure must return 401 for backend `{backend}`"
        );
    }
}

fn gaia_bin_path() -> String {
    if let Some(path) = option_env!("CARGO_BIN_EXE_gaia") {
        return path.to_owned();
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root should exist for gaia-cli tests");

    let fallback_binary = workspace_root
        .join("target")
        .join("debug")
        .join(if cfg!(windows) { "gaia.exe" } else { "gaia" });

    if fallback_binary.exists() {
        return fallback_binary.to_string_lossy().into_owned();
    }

    panic!(
        "Unable to resolve gaia binary path. CARGO_BIN_EXE_gaia is not set and fallback `{}` does not exist.",
        fallback_binary.display()
    );
}

fn create_sandbox() -> PathBuf {
    let id = SANDBOX_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path =
        std::env::temp_dir().join(format!("gaia-openai-core-it-{}-{id}", std::process::id()));
    if path.exists() {
        let _ = fs::remove_dir_all(&path);
    }
    fs::create_dir_all(&path).expect("must create sandbox directory");
    path
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("must bind random local port")
        .local_addr()
        .expect("local addr should exist")
        .port()
}
