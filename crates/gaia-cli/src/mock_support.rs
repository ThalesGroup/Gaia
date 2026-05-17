use std::fs::{self, OpenOptions};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use directories::BaseDirs;

#[derive(Debug, Clone)]
pub struct MockProcessInfo {
    pub pid: u32,
    pub pid_file: PathBuf,
    pub log_file: PathBuf,
}

pub fn spawn_mock_api_process(
    host: &str,
    port: u16,
    model_id: &str,
    api_key: &str,
) -> Result<MockProcessInfo> {
    let paths = mock_paths()?;
    fs::create_dir_all(&paths.state_dir)
        .with_context(|| format!("Unable to create {}", paths.state_dir.display()))?;

    let stdout_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.log_file)
        .with_context(|| format!("Unable to open {}", paths.log_file.display()))?;
    let stderr_file = stdout_file
        .try_clone()
        .with_context(|| format!("Unable to clone {}", paths.log_file.display()))?;

    let executable = std::env::current_exe().context("Unable to resolve current executable")?;
    let child = Command::new(executable)
        .arg("__mock-api")
        .arg("--host")
        .arg(host)
        .arg("--port")
        .arg(port.to_string())
        .arg("--model")
        .arg(model_id)
        .arg("--api-key")
        .arg(api_key)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file))
        .spawn()
        .context("Unable to spawn Gaia mock API process")?;

    let pid = child.id();
    fs::write(&paths.pid_file, pid.to_string())
        .with_context(|| format!("Unable to write {}", paths.pid_file.display()))?;

    Ok(MockProcessInfo {
        pid,
        pid_file: paths.pid_file,
        log_file: paths.log_file,
    })
}

pub fn read_mock_pid() -> Result<Option<u32>> {
    let paths = mock_paths()?;
    if !paths.pid_file.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&paths.pid_file)
        .with_context(|| format!("Unable to read {}", paths.pid_file.display()))?;
    let pid = content.trim().parse::<u32>().ok();
    Ok(pid)
}

pub fn stop_mock_process() -> Result<Option<u32>> {
    let Some(pid) = read_mock_pid()? else {
        return Ok(None);
    };

    if !is_process_running(pid) {
        let _ = remove_mock_pid_file();
        return Ok(Some(pid));
    }

    let status = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .context("Unable to stop mock API process")?;

    if !status.success() {
        bail!("Unable to terminate mock API process `{pid}`.");
    }

    remove_mock_pid_file()?;
    Ok(Some(pid))
}

pub fn is_process_running(pid: u32) -> bool {
    PathBuf::from(format!("/proc/{pid}")).exists()
}

pub fn mock_log_file() -> Result<PathBuf> {
    Ok(mock_paths()?.log_file)
}

fn remove_mock_pid_file() -> Result<()> {
    let paths = mock_paths()?;
    if paths.pid_file.exists() {
        fs::remove_file(&paths.pid_file)
            .with_context(|| format!("Unable to remove {}", paths.pid_file.display()))?;
    }
    Ok(())
}

struct MockPaths {
    state_dir: PathBuf,
    pid_file: PathBuf,
    log_file: PathBuf,
}

fn mock_paths() -> Result<MockPaths> {
    let base = BaseDirs::new().context("Unable to resolve home directory")?;
    let state_dir = base.config_dir().join("gaia").join("state");
    Ok(MockPaths {
        pid_file: state_dir.join("mock-api.pid"),
        log_file: state_dir.join("mock-api.log"),
        state_dir,
    })
}
