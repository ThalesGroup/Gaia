use anyhow::{Context, Result, bail};
use clap::Args;
use gaia_core::config::AppConfig;
use std::fs;
use std::process::{Command, Stdio};

use crate::mock_support::mock_log_file;

#[derive(Debug, Args)]
pub struct LogsArgs {
    #[arg(long)]
    pub container: Option<String>,
    #[arg(short, long)]
    pub follow: bool,
    #[arg(long, default_value_t = 200)]
    pub lines: usize,
    #[arg(long)]
    pub mock: bool,
}

pub fn run(args: LogsArgs) -> Result<()> {
    if args.mock {
        return run_mock_logs(args.follow, args.lines);
    }

    let config = AppConfig::load_or_transient_default()?;
    let default_container = config.to_serve_config().container_name;
    let container = args.container.unwrap_or(default_container);

    if args.follow {
        let status = Command::new("docker")
            .args(["logs", "--tail", &args.lines.to_string(), "-f", &container])
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .context("Failed to execute docker logs")?;

        if !status.success() {
            bail!("docker logs failed with status {status}");
        }
        return Ok(());
    }

    let output = Command::new("docker")
        .args(["logs", "--tail", &args.lines.to_string(), &container])
        .output()
        .context("Failed to execute docker logs")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Unable to fetch logs for `{container}`: {stderr}");
    }

    println!("{}", String::from_utf8_lossy(&output.stdout));
    Ok(())
}

fn run_mock_logs(follow: bool, lines: usize) -> Result<()> {
    let log_file = mock_log_file()?;
    if !log_file.exists() {
        println!("Mock log file does not exist: {}", log_file.display());
        return Ok(());
    }

    if follow {
        let status = Command::new("tail")
            .args(["-n", &lines.to_string(), "-f"])
            .arg(&log_file)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .context("Unable to follow mock log file")?;
        if !status.success() {
            bail!("tail failed with status {status}");
        }
        return Ok(());
    }

    let content = fs::read_to_string(&log_file)
        .with_context(|| format!("Unable to read {}", log_file.display()))?;
    let mut rows = content.lines().collect::<Vec<_>>();
    if rows.len() > lines {
        rows = rows.split_off(rows.len() - lines);
    }
    for row in rows {
        println!("{row}");
    }
    Ok(())
}
