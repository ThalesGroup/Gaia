use std::fs;
use std::path::PathBuf;

use crate::chatbot_support::{has_npm, install_dependencies, write_chatbot_template};
use anyhow::{Context, Result, bail};
use clap::Args;
use gaia_core::config::{AppConfig, expand_tilde};

#[derive(Debug, Args)]
pub struct InitChatbotArgs {
    #[arg(long)]
    pub path: Option<PathBuf>,
    #[arg(long)]
    pub force: bool,
    #[arg(long)]
    pub skip_install: bool,
}

#[derive(Debug, Args, Default)]
pub struct OpenChatbotArgs {
    #[arg(long)]
    pub url: Option<String>,
}

pub fn run_init(args: InitChatbotArgs) -> Result<()> {
    let mut config = AppConfig::init_default_if_missing()?;
    let target_dir = args
        .path
        .clone()
        .unwrap_or_else(|| expand_tilde(&config.chatbot.path));

    if target_dir.exists() && !args.force {
        bail!(
            "Chatbot directory already exists at {} (use --force to overwrite)",
            target_dir.display()
        );
    }

    if target_dir.exists() {
        fs::remove_dir_all(&target_dir).with_context(|| {
            format!(
                "Unable to clean existing directory {}",
                target_dir.display()
            )
        })?;
    }

    write_chatbot_template(
        &target_dir,
        &config.to_serve_config().openai_base_url(),
        &config.server.api_key,
        &config.model.id,
    )?;

    config.chatbot.enabled = true;
    config.chatbot.path = target_dir.to_string_lossy().to_string();
    config.save()?;

    println!("Chatbot template generated at {}", target_dir.display());
    println!(
        "URL (when started): http://localhost:{}",
        config.chatbot.port
    );

    if !args.skip_install && has_npm() {
        if let Err(error) = install_dependencies(&target_dir) {
            println!(
                "npm ci failed ({error}), continue manually in {}",
                target_dir.display(),
            );
        } else {
            println!("npm ci completed.");
        }
    } else {
        println!("Run manually:");
        println!("  cd {}", target_dir.display());
        println!("  npm ci");
        println!(
            "  npm run dev -- --host 0.0.0.0 --port {}",
            config.chatbot.port
        );
    }

    Ok(())
}

pub fn run_open(args: OpenChatbotArgs) -> Result<()> {
    let config = AppConfig::load_or_transient_default()?;
    let url = args
        .url
        .unwrap_or_else(|| format!("http://localhost:{}", config.chatbot.port));

    println!("Chatbot URL: {url}");
    println!("If terminal supports links, open: {url}");
    Ok(())
}
