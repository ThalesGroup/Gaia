use std::io::{self, IsTerminal};

use anyhow::{Context, Result, bail};
use clap::Args;
use gaia_core::backend::backend_from_name;
use gaia_core::config::{AppConfig, expand_tilde};
use gaia_core::machine::MachineSpecs;
use gaia_core::model_catalog::ModelCatalog;
use gaia_tui::app::SelectorInput;
use gaia_tui::{SelectorOutcome, run_selector};

use crate::chatbot_support::{
    ChatbotDevServer, has_npm, install_dependencies, start_dev_server, write_chatbot_template,
};
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
        default_chatbot_enabled: config.chatbot.enabled,
        default_chatbot_port: config.chatbot.port,
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
            config.chatbot.enabled = selection.chatbot_enabled;
            config.chatbot.port = selection.chatbot_port;
            if config.chatbot.path.trim().is_empty() {
                config.chatbot.path = "~/.local/share/gaia/chatbot".to_owned();
            }

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

            let mut notes = Vec::new();
            let mut chatbot_url: Option<String> = None;
            let mut chatbot_server = None;

            if selection.chatbot_enabled {
                let chatbot_dir = expand_tilde(&config.chatbot.path);
                write_chatbot_template(
                    &chatbot_dir,
                    &api_base_url,
                    &selection.api_key,
                    &selection.model_id,
                )?;
                notes.push(format!(
                    "Chatbot template generated at {}",
                    chatbot_dir.display()
                ));
                chatbot_url = Some(format!("http://localhost:{}", selection.chatbot_port));

                if has_npm() {
                    match install_dependencies(&chatbot_dir) {
                        Ok(()) => match start_dev_server(&chatbot_dir, selection.chatbot_port) {
                            Ok(server) => {
                                chatbot_server = Some(server);
                            }
                            Err(error) => {
                                notes.push(format!(
                                    "Unable to start chatbot server automatically: {error}"
                                ));
                                notes.push(format!(
                                    "Start manually: cd {} && npm run dev -- --host 0.0.0.0 --port {}",
                                    chatbot_dir.display(),
                                    selection.chatbot_port
                                ));
                            }
                        },
                        Err(error) => {
                            notes.push(format!("npm ci failed for chatbot: {error}"));
                            notes.push(format!(
                                "Run manually: cd {} && npm ci && npm run dev -- --host 0.0.0.0 --port {}",
                                chatbot_dir.display(),
                                selection.chatbot_port
                            ));
                        }
                    }
                } else {
                    notes.push(
                        "Node.js/npm not detected; chatbot generated but not started.".to_owned(),
                    );
                    notes.push(format!(
                        "Run manually: cd {} && npm ci && npm run dev -- --host 0.0.0.0 --port {}",
                        chatbot_dir.display(),
                        selection.chatbot_port
                    ));
                }
            }

            let chatbot_summary_url = if selection.chatbot_enabled {
                chatbot_url.clone()
            } else {
                None
            };
            print_final_summary(
                &backend_label,
                &selection.model_id,
                &api_base_url,
                &selection.api_key,
                chatbot_summary_url.as_deref(),
            );
            print_connection_examples(&api_base_url, &selection.api_key, &selection.model_id);

            if !container_id.is_empty() {
                println!("Container ID: {container_id}");
            }

            if let Some(ChatbotDevServer { pid, log_path }) = chatbot_server {
                println!("Chatbot dev server PID: {pid}");
                println!("Chatbot logs: {}", log_path.display());
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

            for note in notes {
                println!("{note}");
            }

            Ok(())
        }
    }
}
