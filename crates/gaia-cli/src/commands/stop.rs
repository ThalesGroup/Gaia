use anyhow::{Context, Result, bail};
use clap::Args;
use gaia_core::config::AppConfig;
use std::process::Command;

use crate::mock_support::stop_mock_process;

#[derive(Debug, Args)]
pub struct StopArgs {
    #[arg(long)]
    pub container: Option<String>,
    #[arg(long)]
    pub mock: bool,
}

pub fn run(args: StopArgs) -> Result<()> {
    if args.mock {
        match stop_mock_process()? {
            Some(pid) => println!("Stopped mock API process `{pid}`."),
            None => println!("No mock API process to stop."),
        }
        return Ok(());
    }

    let config = AppConfig::load_or_transient_default()?;
    let default_container = config.to_serve_config().container_name;
    let container = args.container.unwrap_or(default_container);

    let output = Command::new("docker")
        .args(["stop", &container])
        .output()
        .context("Failed to execute docker stop")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Unable to stop container `{container}`: {stderr}");
    }

    println!("Stopped container `{container}`");
    Ok(())
}
