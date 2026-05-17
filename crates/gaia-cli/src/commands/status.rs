use anyhow::{Context, Result};
use clap::Args;
use std::process::Command;

use crate::mock_support::{is_process_running, read_mock_pid};

#[derive(Debug, Args, Default)]
pub struct StatusArgs {}

pub fn run(_args: StatusArgs) -> Result<()> {
    let output = Command::new("docker")
        .args([
            "ps",
            "--filter",
            "name=gaia-",
            "--format",
            "table {{.Names}}\t{{.Image}}\t{{.Status}}\t{{.Ports}}",
        ])
        .output()
        .context("Failed to execute docker ps")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("Unable to query docker status: {stderr}");
        return Ok(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        println!("No running Gaia containers.");
    } else {
        println!("{stdout}");
    }

    if let Some(pid) = read_mock_pid()? {
        println!(
            "Mock API: {} (pid {})",
            if is_process_running(pid) {
                "running"
            } else {
                "stopped"
            },
            pid
        );
    } else {
        println!("Mock API: not configured.");
    }

    Ok(())
}
