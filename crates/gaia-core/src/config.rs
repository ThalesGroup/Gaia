use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use directories::BaseDirs;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerSection,
    pub backend: BackendSection,
    pub model: ModelSection,
    pub chatbot: ChatbotSection,
    #[serde(default)]
    pub security: SecuritySection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSection {
    pub host: String,
    pub port: u16,
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendSection {
    pub name: String,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSection {
    pub id: String,
    #[serde(default)]
    pub revision: Option<String>,
    pub dtype: String,
    pub quantization: String,
    #[serde(default = "default_quantization_profile")]
    pub quantization_profile: String,
    pub max_model_len: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatbotSection {
    pub enabled: bool,
    pub port: u16,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SecuritySection {
    #[serde(default = "default_security_profile")]
    pub profile: String,
}

#[derive(Debug, Clone)]
pub struct ServeConfig {
    pub backend: String,
    pub model_id: String,
    pub model_revision: Option<String>,
    pub security_profile: String,
    pub host: String,
    pub port: u16,
    pub api_key: String,
    pub dtype: String,
    pub quantization: String,
    pub quantization_profile: String,
    pub max_model_len: u32,
    pub hf_token: Option<String>,
    pub huggingface_cache_dir: PathBuf,
    pub detach: bool,
    pub container_name: String,
}

impl AppConfig {
    pub fn config_path() -> Result<PathBuf> {
        let base = BaseDirs::new().context("Unable to resolve home directory")?;
        Ok(base.config_dir().join("gaia").join("config.toml"))
    }

    pub fn load() -> Result<Option<Self>> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Unable to read config file: {}", path.display()))?;
        let parsed = toml::from_str::<Self>(&content)
            .with_context(|| format!("Unable to parse config file: {}", path.display()))?;
        Ok(Some(parsed))
    }

    pub fn init_default_if_missing() -> Result<Self> {
        if let Some(config) = Self::load()? {
            return Ok(config);
        }

        let default = Self::default();
        default.save()?;
        Ok(default)
    }

    pub fn load_or_transient_default() -> Result<Self> {
        Ok(Self::load()?.unwrap_or_default())
    }

    pub fn load_or_default() -> Result<Self> {
        Self::init_default_if_missing()
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "Unable to create Gaia config directory: {}",
                    parent.display()
                )
            })?;
        }

        let content = toml::to_string_pretty(self).context("Unable to serialize config as TOML")?;
        fs::write(&path, content)
            .with_context(|| format!("Unable to write config file: {}", path.display()))?;
        enforce_private_file_permissions(&path)?;
        Ok(())
    }

    pub fn to_serve_config(&self) -> ServeConfig {
        let hf_cache = default_hf_cache_dir();
        let container_name = format!(
            "gaia-{}-{}",
            self.backend.name,
            sanitize_container_fragment(&self.model.id)
        );

        ServeConfig {
            backend: self.backend.name.clone(),
            model_id: self.model.id.clone(),
            model_revision: self.model.revision.clone(),
            security_profile: self.security.profile.clone(),
            host: self.server.host.clone(),
            port: self.server.port,
            api_key: self.server.api_key.clone(),
            dtype: self.model.dtype.clone(),
            quantization: self.model.quantization.clone(),
            quantization_profile: self.model.quantization_profile.clone(),
            max_model_len: self.model.max_model_len,
            hf_token: env::var("HF_TOKEN").ok(),
            huggingface_cache_dir: hf_cache,
            detach: false,
            container_name,
        }
    }
}

impl ServeConfig {
    pub fn openai_base_url(&self) -> String {
        format!("http://{}:{}/v1", self.host, self.port)
    }

    pub fn chat_completions_url(&self) -> String {
        format!("{}/chat/completions", self.openai_base_url())
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerSection {
                host: "0.0.0.0".to_owned(),
                port: 8000,
                api_key: "local-key".to_owned(),
            },
            backend: BackendSection {
                name: "vllm".to_owned(),
                mode: "docker".to_owned(),
            },
            model: ModelSection {
                id: "Qwen/Qwen2.5-7B-Instruct".to_owned(),
                revision: None,
                dtype: "auto".to_owned(),
                quantization: "none".to_owned(),
                quantization_profile: default_quantization_profile(),
                max_model_len: 8192,
            },
            chatbot: ChatbotSection {
                enabled: false,
                port: 3000,
                path: "~/.local/share/gaia/chatbot".to_owned(),
            },
            security: SecuritySection {
                profile: default_security_profile(),
            },
        }
    }
}

pub fn expand_tilde(path: &str) -> PathBuf {
    if let Some(stripped) = path.strip_prefix("~/")
        && let Some(base) = BaseDirs::new()
    {
        return base.home_dir().join(stripped);
    }

    Path::new(path).to_path_buf()
}

fn default_hf_cache_dir() -> PathBuf {
    if let Some(base) = BaseDirs::new() {
        return base.home_dir().join(".cache").join("huggingface");
    }

    PathBuf::from(".cache/huggingface")
}

fn sanitize_container_fragment(model_id: &str) -> String {
    model_id
        .to_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .chars()
        .take(32)
        .collect()
}

fn default_quantization_profile() -> String {
    "balanced".to_owned()
}

fn default_security_profile() -> String {
    "dev".to_owned()
}

pub fn normalize_model_revision(input: Option<&str>) -> Result<Option<String>> {
    let Some(raw) = input else {
        return Ok(None);
    };
    let value = raw.trim();
    if value.is_empty() {
        return Ok(None);
    }

    let is_sha = value.len() == 40 && value.chars().all(|ch| ch.is_ascii_hexdigit());
    if !is_sha {
        bail!(
            "Model revision must be an immutable 40-character commit SHA. Received `{}`.",
            value
        );
    }

    Ok(Some(value.to_ascii_lowercase()))
}

pub fn normalize_security_profile(input: Option<&str>) -> Result<String> {
    let value = input.unwrap_or("dev").trim().to_ascii_lowercase();
    match value.as_str() {
        "dev" | "prod" => Ok(value),
        _ => bail!(
            "Unsupported security profile `{}`. Use `dev` or `prod`.",
            input.unwrap_or("dev")
        ),
    }
}

pub fn is_prod_security_profile(profile: &str) -> bool {
    profile.eq_ignore_ascii_case("prod")
}

#[cfg(unix)]
fn enforce_private_file_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("Unable to set secure permissions on {}", path.display()))?;
    Ok(())
}

#[cfg(not(unix))]
fn enforce_private_file_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_model_revision_accepts_valid_sha_and_lowercases() {
        let sha = "0123456789ABCDEF0123456789ABCDEF01234567";
        let normalized = normalize_model_revision(Some(sha)).expect("valid sha must be accepted");
        assert_eq!(
            normalized,
            Some("0123456789abcdef0123456789abcdef01234567".to_owned())
        );
    }

    #[test]
    fn normalize_model_revision_rejects_non_commit_values() {
        assert!(
            normalize_model_revision(Some("main")).is_err(),
            "branch names must be rejected"
        );
        assert!(
            normalize_model_revision(Some("1234")).is_err(),
            "short hashes must be rejected"
        );
    }

    #[test]
    fn normalize_security_profile_behaves_as_expected() {
        assert_eq!(
            normalize_security_profile(None).expect("default profile should resolve"),
            "dev"
        );
        assert_eq!(
            normalize_security_profile(Some(" PROD ")).expect("profile should normalize"),
            "prod"
        );
        assert!(
            normalize_security_profile(Some("staging")).is_err(),
            "unsupported profile must fail"
        );
    }

    #[test]
    fn to_serve_config_sanitizes_container_name() {
        let mut config = AppConfig::default();
        config.model.id = "Org/My-Model:Very-Long-Name-With-Many-Characters-1234567890".to_owned();
        config.backend.name = "vllm".to_owned();

        let serve = config.to_serve_config();
        let suffix = serve
            .container_name
            .strip_prefix("gaia-vllm-")
            .expect("container name must include backend prefix");

        assert!(
            suffix.len() <= 32,
            "sanitized model fragment should be capped to 32 characters"
        );
        assert!(
            suffix
                .chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-'),
            "sanitized fragment should contain only lowercase/digits/dashes"
        );
        assert!(!suffix.starts_with('-'));
        assert!(!suffix.ends_with('-'));
    }

    #[test]
    fn serve_config_builds_openai_urls() {
        let config = AppConfig::default();
        let serve = config.to_serve_config();
        assert_eq!(serve.openai_base_url(), "http://0.0.0.0:8000/v1");
        assert_eq!(
            serve.chat_completions_url(),
            "http://0.0.0.0:8000/v1/chat/completions"
        );
    }

    #[test]
    fn expand_tilde_keeps_non_tilde_paths_unchanged() {
        assert_eq!(
            expand_tilde("/tmp/gaia-config.toml"),
            PathBuf::from("/tmp/gaia-config.toml")
        );
    }

    #[test]
    fn is_prod_security_profile_is_case_insensitive() {
        assert!(is_prod_security_profile("prod"));
        assert!(is_prod_security_profile("PrOd"));
        assert!(!is_prod_security_profile("dev"));
    }
}
