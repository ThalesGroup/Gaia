use std::process::Stdio;

use anyhow::{Context, Result, bail};
use clap::Args;
use gaia_core::backend::backend_from_name;
use gaia_core::config::{
    AppConfig, ServeConfig, is_prod_security_profile, normalize_model_revision,
    normalize_security_profile,
};
use gaia_core::machine::MachineSpecs;
use gaia_core::model_catalog::ModelCatalog;
use rand::Rng;
use rand::distr::Alphanumeric;

use crate::commands::mock_api;
use crate::final_output::{print_connection_examples, print_final_summary};
use crate::mock_support::{MockProcessInfo, spawn_mock_api_process};

#[derive(Debug, Args)]
pub struct ServeArgs {
    #[arg(long)]
    pub backend: Option<String>,
    #[arg(long)]
    pub model: Option<String>,
    #[arg(
        long,
        value_name = "COMMIT_SHA",
        help = "Immutable 40-character Hugging Face commit SHA"
    )]
    pub model_revision: Option<String>,
    #[arg(long)]
    pub host: Option<String>,
    #[arg(long)]
    pub port: Option<u16>,
    #[arg(long)]
    pub api_key: Option<String>,
    #[arg(
        long,
        value_name = "dev|prod",
        help = "Security profile (`dev` keeps local defaults, `prod` enforces explicit secrets)"
    )]
    pub security_profile: Option<String>,
    #[arg(long)]
    pub dtype: Option<String>,
    #[arg(long)]
    pub quantization: Option<String>,
    #[arg(long)]
    pub max_model_len: Option<u32>,
    #[arg(long)]
    pub detach: bool,
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub force: bool,
    #[arg(long)]
    pub no_save: bool,
    #[arg(long)]
    pub quantization_profile: Option<String>,
    #[arg(long)]
    pub mock: bool,
}

pub fn run(args: ServeArgs) -> Result<()> {
    let mut config = AppConfig::init_default_if_missing()?;
    apply_cli_overrides(&mut config, &args)?;
    apply_quantization_profile(&mut config, args.quantization_profile.as_deref())?;
    if args.security_profile.is_none()
        && let Ok(profile) = std::env::var("GAIA_SECURITY_PROFILE")
        && !profile.trim().is_empty()
    {
        config.security.profile = profile;
    }
    config.security.profile = normalize_security_profile(Some(config.security.profile.as_str()))?;
    let prod_profile = is_prod_security_profile(&config.security.profile);

    if args.api_key.is_none()
        && let Ok(api_key) = std::env::var("GAIA_API_KEY")
        && !api_key.trim().is_empty()
    {
        config.server.api_key = api_key;
    }

    let mut generated_local_api_key = false;
    let has_placeholder_local_key = config.server.api_key.trim() == "local-key";
    if prod_profile && (config.server.api_key.trim().is_empty() || has_placeholder_local_key) {
        bail!(
            "Security profile `prod` requires an explicit API key. Set `--api-key` or `GAIA_API_KEY`."
        );
    }
    if !prod_profile && (config.server.api_key.trim().is_empty() || has_placeholder_local_key) {
        config.server.api_key = generate_local_key();
        generated_local_api_key = true;
        if let Ok(path) = AppConfig::config_path() {
            if args.no_save || args.dry_run {
                println!(
                    "Generated local API key for this run (not saved due --no-save/--dry-run). Persist in {}",
                    path.display()
                );
            } else {
                println!(
                    "Generated local API key automatically. It will be saved in {}",
                    path.display()
                );
            }
        } else {
            println!("Generated local API key automatically.");
        }
    }

    let mut serve_config = config.to_serve_config();
    serve_config.detach = args.detach;
    serve_config.model_revision = normalize_model_revision(serve_config.model_revision.as_deref())?;

    if args.mock {
        return run_mock_mode(args, config, serve_config);
    }

    let backend = backend_from_name(&serve_config.backend)
        .with_context(|| format!("Unsupported backend `{}`", serve_config.backend))?;

    let machine = MachineSpecs::detect();
    let availability = backend.is_available(&machine);
    if !availability.available && !args.force && !args.dry_run {
        bail!(
            "Backend {} is not available: {} (use --force to bypass)",
            backend.display_name(),
            availability.reason
        );
    }

    let command_spec = backend.build_docker_command(&serve_config);

    println!("Launching backend: {}", backend.display_name());
    println!("Security profile: {}", serve_config.security_profile);
    println!("Model: {}", serve_config.model_id);
    if let Some(revision) = &serve_config.model_revision {
        println!("Model revision: {revision}");
    }
    let gated_model = model_requires_hf_token(&serve_config.model_id);
    if gated_model && serve_config.hf_token.is_none() {
        if prod_profile {
            bail!(
                "Model `{}` is gated on Hugging Face and requires HF_TOKEN when security profile is `prod`.",
                serve_config.model_id
            );
        }
        println!(
            "Warning: model `{}` is gated on Hugging Face and requires HF_TOKEN for downloads.",
            serve_config.model_id
        );
    }
    if serve_config.hf_token.is_some() {
        println!("Security note: prefer short-lived HF_TOKEN values with minimum required scope.");
    }
    if !availability.available {
        println!("Warning: {}", availability.reason);
    }
    println!("Docker command:");
    println!(
        "  {}",
        redact_command(
            &command_spec.to_shell_string(),
            &[
                serve_config.api_key.as_str(),
                serve_config.hf_token.as_deref().unwrap_or_default(),
            ],
        )
    );

    if args.dry_run {
        println!();
        println!("Dry run enabled, command was not executed.");
        return Ok(());
    }

    if !args.no_save {
        config.save()?;
        if generated_local_api_key {
            println!("Saved generated local API key to the Gaia config file.");
        }
    }

    if serve_config.detach {
        let output = command_spec
            .to_command()
            .output()
            .context("Failed to execute detached docker command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("Docker command failed: {stderr}");
        }

        let container_id = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        println!();
        println!("Server started in detached mode.");
        if !container_id.is_empty() {
            println!("Container ID: {container_id}");
        }
    } else {
        let mut process = command_spec.to_command();
        process
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        let status = process
            .status()
            .context("Failed to launch docker command")?;
        if !status.success() {
            bail!("Docker command exited with status {status}");
        }
    }

    let api_base = backend.api_base_url(&serve_config);
    print_final_summary(
        backend.display_name(),
        &serve_config.model_id,
        &api_base,
        &serve_config.api_key,
    );
    print_connection_examples(&api_base, &serve_config.api_key, &serve_config.model_id);

    Ok(())
}

fn apply_cli_overrides(config: &mut AppConfig, args: &ServeArgs) -> Result<()> {
    if let Some(backend) = &args.backend {
        config.backend.name = backend.to_ascii_lowercase();
    }
    if let Some(model) = &args.model {
        config.model.id = model.clone();
    }
    if let Some(model_revision) = &args.model_revision {
        config.model.revision = normalize_model_revision(Some(model_revision.as_str()))?;
    }
    if let Some(host) = &args.host {
        config.server.host = host.clone();
    }
    if let Some(port) = args.port {
        config.server.port = port;
    }
    if let Some(api_key) = &args.api_key {
        config.server.api_key = api_key.clone();
    }
    if let Some(profile) = &args.security_profile {
        config.security.profile = normalize_security_profile(Some(profile.as_str()))?;
    }
    if let Some(dtype) = &args.dtype {
        config.model.dtype = dtype.clone();
    }
    if let Some(quantization) = &args.quantization {
        config.model.quantization = quantization.clone();
    }
    if let Some(max_model_len) = args.max_model_len {
        config.model.max_model_len = max_model_len;
    }
    if let Some(profile) = &args.quantization_profile {
        config.model.quantization_profile = profile.to_ascii_lowercase();
    }
    Ok(())
}

fn generate_local_key() -> String {
    let random = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(20)
        .map(char::from)
        .collect::<String>();
    format!("local-{random}")
}

fn apply_quantization_profile(config: &mut AppConfig, profile: Option<&str>) -> Result<()> {
    let selected = profile
        .map(str::to_ascii_lowercase)
        .or_else(|| Some(config.model.quantization_profile.to_ascii_lowercase()))
        .unwrap_or_else(|| "balanced".to_owned());

    config.model.quantization_profile = selected.clone();
    match selected.as_str() {
        "none" => {}
        "quality" => {
            config.model.quantization = "none".to_owned();
            config.model.dtype = "float16".to_owned();
        }
        "balanced" => {
            config.model.quantization = "none".to_owned();
            if config.model.dtype.trim().is_empty() {
                config.model.dtype = "auto".to_owned();
            }
        }
        "memory" => {
            config.model.quantization = "int4".to_owned();
            config.model.dtype = "auto".to_owned();
        }
        "speed" => {
            config.model.quantization = "int8".to_owned();
            config.model.dtype = "auto".to_owned();
        }
        unsupported => bail!(
            "Unsupported quantization profile `{unsupported}`. Use one of: none, quality, balanced, memory, speed."
        ),
    }

    Ok(())
}

fn run_mock_mode(args: ServeArgs, config: AppConfig, serve_config: ServeConfig) -> Result<()> {
    if args.dry_run {
        println!("Mock mode dry run.");
        println!(
            "Mock API command: gaia __mock-api --host {} --port {} --model {} --api-key [REDACTED]",
            serve_config.host, serve_config.port, serve_config.model_id
        );
        return Ok(());
    }

    if !args.no_save {
        config.save()?;
    }

    let mut mock_process: Option<MockProcessInfo> = None;
    if serve_config.detach {
        mock_process = Some(spawn_mock_api_process(
            &serve_config.host,
            serve_config.port,
            &serve_config.model_id,
            &serve_config.api_key,
        )?);
    } else {
        println!("Running mock API in foreground. Press Ctrl+C to stop.");
        println!("This mode simulates an OpenAI-compatible backend for frontend demos.");
    }

    let api_base = serve_config.openai_base_url();
    print_final_summary(
        "Mock OpenAI API",
        &serve_config.model_id,
        &api_base,
        &serve_config.api_key,
    );
    print_connection_examples(&api_base, &serve_config.api_key, &serve_config.model_id);

    if let Some(process) = mock_process {
        println!("Mock API started in detached mode.");
        println!("Mock PID: {}", process.pid);
        println!("Mock PID file: {}", process.pid_file.display());
        println!("Mock logs: {}", process.log_file.display());
        return Ok(());
    }

    mock_api::run_server_loop(
        &serve_config.host,
        serve_config.port,
        &serve_config.model_id,
        &serve_config.api_key,
    )
}

fn model_requires_hf_token(model_id: &str) -> bool {
    let Ok(catalog) = ModelCatalog::load_default() else {
        return false;
    };
    catalog
        .models
        .iter()
        .find(|model| model.id == model_id)
        .map(|model| model.gated)
        .unwrap_or(false)
}

fn redact_command(command: &str, secrets: &[&str]) -> String {
    let mut output = command.to_owned();
    for secret in secrets {
        if secret.is_empty() {
            continue;
        }
        let quoted = shell_quote(secret);
        output = output.replace(secret, "[REDACTED]");
        output = output.replace(&quoted, "'[REDACTED]'");
    }
    output
}

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_owned();
    }
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "-_./=:".contains(ch))
    {
        return value.to_owned();
    }
    format!("'{}'", value.replace('\'', r"'\''"))
}
