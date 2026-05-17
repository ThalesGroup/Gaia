use std::fs;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

const PACKAGE_JSON: &str = include_str!("../../../templates/chatbot-react/package.json");
const PACKAGE_LOCK: &str = include_str!("../../../templates/chatbot-react/package-lock.json");
const INDEX_HTML: &str = include_str!("../../../templates/chatbot-react/index.html");
const TS_CONFIG: &str = include_str!("../../../templates/chatbot-react/tsconfig.json");
const TS_CONFIG_NODE: &str = include_str!("../../../templates/chatbot-react/tsconfig.node.json");
const VITE_CONFIG: &str = include_str!("../../../templates/chatbot-react/vite.config.ts");
const DOCKERFILE: &str = include_str!("../../../templates/chatbot-react/Dockerfile");
const DOCKERIGNORE: &str = include_str!("../../../templates/chatbot-react/.dockerignore");
const MAIN_TSX: &str = include_str!("../../../templates/chatbot-react/src/main.tsx");
const APP_TSX: &str = include_str!("../../../templates/chatbot-react/src/App.tsx");
const STYLES_CSS: &str = include_str!("../../../templates/chatbot-react/src/styles.css");
const VITE_ENV_D_TS: &str = include_str!("../../../templates/chatbot-react/src/vite-env.d.ts");
const ENV_EXAMPLE: &str = include_str!("../../../templates/chatbot-react/.env.example");

#[derive(Debug, Clone)]
pub struct ChatbotDevServer {
    pub pid: u32,
    pub log_path: PathBuf,
}

pub fn write_chatbot_template(
    target: &Path,
    openai_base_url: &str,
    api_key: &str,
    model_id: &str,
) -> Result<()> {
    fs::create_dir_all(target.join("src"))
        .with_context(|| format!("Unable to create {}", target.join("src").display()))?;

    write_file(target.join("package.json"), PACKAGE_JSON)?;
    write_file(target.join("package-lock.json"), PACKAGE_LOCK)?;
    write_file(target.join("index.html"), INDEX_HTML)?;
    write_file(target.join("tsconfig.json"), TS_CONFIG)?;
    write_file(target.join("tsconfig.node.json"), TS_CONFIG_NODE)?;
    write_file(target.join("vite.config.ts"), VITE_CONFIG)?;
    write_file(target.join("Dockerfile"), DOCKERFILE)?;
    write_file(target.join(".dockerignore"), DOCKERIGNORE)?;
    write_file(target.join("src/main.tsx"), MAIN_TSX)?;
    write_file(target.join("src/App.tsx"), APP_TSX)?;
    write_file(target.join("src/styles.css"), STYLES_CSS)?;
    write_file(target.join("src/vite-env.d.ts"), VITE_ENV_D_TS)?;
    write_file(target.join(".env.example"), ENV_EXAMPLE)?;

    let env_file = format!(
        "VITE_LLM_BASE_URL={openai_base_url}\nVITE_LLM_API_KEY={api_key}\nVITE_LLM_MODEL={model_id}\n"
    );
    write_file(target.join(".env"), &env_file)?;

    Ok(())
}

pub fn has_npm() -> bool {
    Command::new("npm")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

pub fn install_dependencies(target: &Path) -> Result<()> {
    let status = Command::new("npm")
        .arg("ci")
        .current_dir(target)
        .status()
        .context("Unable to run npm ci")?;

    if !status.success() {
        bail!("npm ci failed with status {status}");
    }

    Ok(())
}

pub fn start_dev_server(target: &Path, port: u16) -> Result<ChatbotDevServer> {
    let log_path = target.join("gaia-chatbot.log");
    let stdout_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .with_context(|| format!("Unable to open {}", log_path.display()))?;
    let stderr_file = stdout_file
        .try_clone()
        .with_context(|| format!("Unable to clone {}", log_path.display()))?;

    let child = Command::new("npm")
        .args(["run", "dev", "--", "--host", "0.0.0.0", "--port"])
        .arg(port.to_string())
        .current_dir(target)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file))
        .spawn()
        .context("Unable to start chatbot dev server")?;

    Ok(ChatbotDevServer {
        pid: child.id(),
        log_path,
    })
}

fn write_file(path: PathBuf, content: &str) -> Result<()> {
    fs::write(&path, content).with_context(|| format!("Unable to write {}", path.display()))
}
