use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static SANDBOX_COUNTER: AtomicUsize = AtomicUsize::new(1);

struct CliOutput {
    status_ok: bool,
    stdout: String,
    stderr: String,
}

fn run_gaia(args: &[&str]) -> CliOutput {
    let sandbox = create_sandbox();
    let config_home = sandbox.join("config");
    let data_home = sandbox.join("data");
    let state_home = sandbox.join("state");

    fs::create_dir_all(&config_home).expect("must create XDG config home");
    fs::create_dir_all(&data_home).expect("must create XDG data home");
    fs::create_dir_all(&state_home).expect("must create XDG state home");

    let output = Command::new(gaia_bin_path())
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("HOME", &sandbox)
        .env("XDG_CONFIG_HOME", &config_home)
        .env("XDG_DATA_HOME", &data_home)
        .env("XDG_STATE_HOME", &state_home)
        .env_remove("GAIA_API_KEY")
        .env_remove("GAIA_SECURITY_PROFILE")
        .env_remove("HF_TOKEN")
        .output()
        .expect("gaia command should start");

    decode_output(output)
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
    let path = std::env::temp_dir().join(format!("gaia-cli-it-{}-{id}", std::process::id()));
    if path.exists() {
        let _ = fs::remove_dir_all(&path);
    }
    fs::create_dir_all(&path).expect("must create sandbox directory");
    path
}

fn decode_output(output: Output) -> CliOutput {
    CliOutput {
        status_ok: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

#[test]
fn help_output_mentions_main_description() {
    let output = run_gaia(&["--help"]);
    assert!(output.status_ok, "help should exit successfully");
    assert!(
        output
            .stdout
            .contains("LLM serving manager for Hugging Face models")
    );
    assert!(output.stdout.contains("serve"));
}

#[test]
fn version_output_contains_package_version() {
    let output = run_gaia(&["--version"]);
    assert!(output.status_ok, "version should exit successfully");
    assert!(
        output.stdout.contains(env!("CARGO_PKG_VERSION")),
        "version output should contain package version"
    );
}

#[test]
fn doctor_command_prints_expected_sections() {
    let output = run_gaia(&["doctor"]);
    assert!(output.status_ok, "doctor should exit successfully");
    assert!(output.stdout.contains("gaia doctor"));
    assert!(output.stdout.contains("Backend availability"));
    assert!(output.stdout.contains("Recommendations"));
}

#[test]
fn models_command_reports_total_count() {
    let output = run_gaia(&["models", "--backend", "vllm", "--max-params", "8"]);
    assert!(output.status_ok, "models should exit successfully");
    assert!(output.stdout.contains("Total models shown:"));
}

#[test]
fn recommend_command_reports_runtime_context() {
    let output = run_gaia(&["recommend", "--top", "3"]);
    assert!(output.status_ok, "recommend should exit successfully");
    assert!(output.stdout.contains("gaia recommend"));
    assert!(output.stdout.contains("Backend filter:"));
    assert!(output.stdout.contains("GPU:"));
}

#[test]
fn benchmark_mock_runs_without_external_server() {
    let output = run_gaia(&["benchmark", "--mock", "--requests", "3"]);
    assert!(
        output.status_ok,
        "benchmark --mock should exit successfully"
    );
    assert!(output.stdout.contains("Results"));
    assert!(output.stdout.contains("throughput (approx):"));
}

#[test]
fn serve_rejects_non_sha_model_revision() {
    let output = run_gaia(&["serve", "--dry-run", "--model-revision", "main"]);
    assert!(
        !output.status_ok,
        "serve should fail on non-immutable revision"
    );
    assert!(
        output
            .stderr
            .contains("Model revision must be an immutable 40-character commit SHA"),
        "stderr should explain why revision is invalid; got: {}",
        output.stderr
    );
}

#[test]
fn serve_prod_requires_explicit_api_key() {
    let output = run_gaia(&[
        "serve",
        "--dry-run",
        "--security-profile",
        "prod",
        "--backend",
        "vllm",
        "--model",
        "Qwen/Qwen2.5-7B-Instruct",
    ]);
    assert!(
        !output.status_ok,
        "serve in prod should require explicit api key"
    );
    assert!(
        output.stderr.contains("requires an explicit API key"),
        "stderr should mention explicit API key requirement; got: {}",
        output.stderr
    );
}
