use std::io::{self, IsTerminal};

use anyhow::{Context, Result, bail};
use clap::Args;
use gaia_core::backend::backend_from_name;
use gaia_core::config::AppConfig;
use gaia_core::machine::MachineSpecs;
use gaia_core::model_catalog::ModelCatalog;
use gaia_tui::app::SelectorInput;
use gaia_tui::{SelectorOutcome, run_selector};

use crate::final_output::{print_connection_examples, print_final_summary};
use crate::mock_support::{MockProcessInfo, spawn_mock_api_process};

#[derive(Debug, Args, Default)]
pub struct SelectArgs {
    #[arg(long)]
    pub mock: bool,
}

pub fn run(args: SelectArgs) -> Result<()> {
    if !io::stdout().is_terminal() || !io::stdin().is_terminal() {
        bail!("`gaia select` requires an interactive TTY terminal.");
    }

    let mut config = AppConfig::init_default_if_missing()?;
    let machine = MachineSpecs::detect();
    let catalog = ModelCatalog::load_default()?;

    let input = SelectorInput {
        machine: machine.clone(),
        catalog,
        default_backend: config.backend.name.clone(),
        default_model: Some(config.model.id.clone()),
        default_port: config.server.port,
        default_api_key: config.server.api_key.clone(),
    };

    match run_selector(input)? {
        SelectorOutcome::Cancelled => {
            println!("Selection cancelled.");
            Ok(())
        }
        SelectorOutcome::Launch(selection) => {
            config.backend.name = selection.backend.clone();
            config.backend.mode = "docker".to_owned();
            config.model.id = selection.model_id.clone();
            config.model.revision = None;
            config.server.port = selection.port;
            config.server.api_key = selection.api_key.clone();

            config.save()?;

            let mut serve_config = config.to_serve_config();
            serve_config.detach = true;
            let (backend_label, api_base_url, container_id, mock_process) = if args.mock {
                let process = spawn_mock_api_process(
                    &serve_config.host,
                    serve_config.port,
                    &selection.model_id,
                    &selection.api_key,
                )?;
                (
                    format!("{} (mock)", selection.backend),
                    serve_config.openai_base_url(),
                    String::new(),
                    Some(process),
                )
            } else {
                let backend = backend_from_name(&selection.backend)
                    .with_context(|| format!("Unsupported backend `{}`", selection.backend))?;

                let availability = backend.is_available(&machine);
                if !availability.available {
                    println!("Warning: {}", availability.reason);
                }

                let launch_output = backend
                    .build_docker_command(&serve_config)
                    .to_command()
                    .output()
                    .context("Failed to launch docker backend")?;
                if !launch_output.status.success() {
                    let stderr = String::from_utf8_lossy(&launch_output.stderr);
                    bail!("Unable to start docker container: {stderr}");
                }

                (
                    backend.display_name().to_owned(),
                    backend.api_base_url(&serve_config),
                    String::from_utf8_lossy(&launch_output.stdout)
                        .trim()
                        .to_owned(),
                    None,
                )
            };

            print_final_summary(
                &backend_label,
                &selection.model_id,
                &api_base_url,
                &selection.api_key,
            );
            print_connection_examples(&api_base_url, &selection.api_key, &selection.model_id);

            if !container_id.is_empty() {
                println!("Container ID: {container_id}");
            }

            if let Some(MockProcessInfo {
                pid,
                pid_file,
                log_file,
            }) = mock_process
            {
                println!("Mock API mode enabled.");
                println!("Mock API PID: {pid}");
                println!("Mock PID file: {}", pid_file.display());
                println!("Mock API logs: {}", log_file.display());
            }

            Ok(())
        }
    }
}
