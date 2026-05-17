use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;

use gaia_core::config::{AppConfig, normalize_model_revision};

#[derive(Debug, Args)]
pub struct GenerateSystemdArgs {
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
    pub user: Option<String>,
    #[arg(long)]
    pub output: Option<PathBuf>,
    #[arg(long)]
    pub mock: bool,
}

pub fn run(args: GenerateSystemdArgs) -> Result<()> {
    let mut config = AppConfig::load_or_transient_default()?;
    if let Some(backend) = args.backend {
        config.backend.name = backend;
    }
    if let Some(model) = args.model {
        config.model.id = model;
    }
    config.model.revision = normalize_model_revision(
        args.model_revision
            .as_deref()
            .or(config.model.revision.as_deref()),
    )?;

    let serve_config = config.to_serve_config();
    let user = args.user.unwrap_or_else(|| "gaia".to_owned());
    let service_name = format!("gaia-{}.service", serve_config.backend.replace('.', "-"));
    let output = args.output.unwrap_or_else(|| PathBuf::from(&service_name));

    let revision_flag = serve_config
        .model_revision
        .as_ref()
        .map(|revision| format!(" --model-revision {}", shell_escape(revision)))
        .unwrap_or_default();
    let mock_flag = if args.mock { " --mock" } else { "" };
    let service = format!(
        "[Unit]\nDescription=gaia {} {}\nAfter=network-online.target docker.service\nWants=network-online.target\n\n[Service]\nType=simple\nUser={}\nRestart=always\nRestartSec=3\nNoNewPrivileges=true\nPrivateTmp=true\nProtectControlGroups=true\nProtectKernelModules=true\nProtectKernelTunables=true\nEnvironmentFile=-/etc/gaia/gaia.env\nEnvironment=HF_TOKEN=${{HF_TOKEN}}\nExecStart=/usr/local/bin/gaia serve --backend {} --model {}{} --host {} --port {}{}\n\n[Install]\nWantedBy=multi-user.target\n",
        serve_config.backend,
        serve_config.model_id,
        user,
        serve_config.backend,
        serve_config.model_id,
        revision_flag,
        serve_config.host,
        serve_config.port,
        mock_flag
    );

    fs::write(&output, service).with_context(|| format!("Unable to write {}", output.display()))?;
    println!("Generated {}", output.display());
    println!("Install with:");
    println!("  sudo cp {} /etc/systemd/system/", output.display());
    println!("  sudo install -d -m 0750 /etc/gaia");
    println!("  sudo sh -c 'echo \"GAIA_API_KEY=your-strong-api-key\" > /etc/gaia/gaia.env'");
    println!("  sudo chmod 600 /etc/gaia/gaia.env");
    println!("  sudo systemctl daemon-reload");
    println!(
        "  sudo systemctl enable --now {}",
        output.file_name().unwrap_or_default().to_string_lossy()
    );
    Ok(())
}

fn shell_escape(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "-_./:@".contains(ch))
    {
        value.to_owned()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}
