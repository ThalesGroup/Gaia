use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use gaia_core::backend::backend_from_name;
use gaia_core::config::{
    AppConfig, expand_tilde, is_prod_security_profile, normalize_model_revision,
    normalize_security_profile,
};

#[derive(Debug, Args)]
pub struct GenerateComposeArgs {
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
    pub output: Option<PathBuf>,
    #[arg(long)]
    pub with_chatbot: bool,
    #[arg(
        long,
        value_name = "dev|prod",
        help = "Security profile (`dev` keeps local fallbacks, `prod` requires explicit secrets)"
    )]
    pub security_profile: Option<String>,
}

pub fn run(args: GenerateComposeArgs) -> Result<()> {
    let mut config = AppConfig::load_or_transient_default()?;

    if let Some(backend) = args.backend {
        config.backend.name = backend;
    }
    if let Some(model) = args.model {
        config.model.id = model;
    }
    if let Some(profile) = args.security_profile {
        config.security.profile = normalize_security_profile(Some(profile.as_str()))?;
    }
    if let Ok(profile) = std::env::var("GAIA_SECURITY_PROFILE")
        && !profile.trim().is_empty()
    {
        config.security.profile = profile;
    }
    config.security.profile = normalize_security_profile(Some(config.security.profile.as_str()))?;
    let prod_profile = is_prod_security_profile(&config.security.profile);
    config.model.revision = normalize_model_revision(
        args.model_revision
            .as_deref()
            .or(config.model.revision.as_deref()),
    )?;

    let serve_config = config.to_serve_config();
    let backend = backend_from_name(&serve_config.backend).with_context(|| {
        format!(
            "Unsupported backend `{}` for compose generation",
            serve_config.backend
        )
    })?;

    let backend_service =
        maybe_enforce_prod_compose(backend.build_compose_service(&serve_config), prod_profile);
    let mut compose = String::from("services:\n");
    compose.push_str(&backend_service);

    if args.with_chatbot || config.chatbot.enabled {
        compose.push_str(&chatbot_service(&config, backend.name()));
    }

    let output_path = args
        .output
        .unwrap_or_else(|| PathBuf::from("docker-compose.yml"));
    fs::write(&output_path, compose)
        .with_context(|| format!("Unable to write {}", output_path.display()))?;

    println!("Generated {}", output_path.display());
    if prod_profile {
        println!("Security profile: prod");
        println!("Export GAIA_API_KEY explicitly before running docker compose.");
    } else {
        println!("Security profile: dev");
        println!("Export GAIA_API_KEY (and optional HF_TOKEN) before running docker compose.");
    }
    Ok(())
}

fn chatbot_service(config: &AppConfig, backend_name: &str) -> String {
    let chatbot_path = expand_tilde(&config.chatbot.path);
    format!(
        "  gaia-chatbot:\n    build:\n      context: \"{}\"\n      dockerfile: Dockerfile\n    image: gaia-chatbot:local\n    container_name: gaia-chatbot\n    user: \"1000:1000\"\n    read_only: true\n    tmpfs:\n      - /tmp\n    security_opt:\n      - no-new-privileges:true\n    cap_drop:\n      - ALL\n    command:\n      - npm\n      - run\n      - preview\n      - --\n      - --host\n      - 0.0.0.0\n      - --port\n      - \"{}\"\n    ports:\n      - \"{}:{}\"\n    depends_on:\n      - gaia-{}\n",
        chatbot_path.display(),
        config.chatbot.port,
        config.chatbot.port,
        config.chatbot.port,
        backend_name
    )
}

fn maybe_enforce_prod_compose(service: String, prod_profile: bool) -> String {
    if !prod_profile {
        return service;
    }

    service.replace("${GAIA_API_KEY:-local-key}", "${GAIA_API_KEY}")
}
