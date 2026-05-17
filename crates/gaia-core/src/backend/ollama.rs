use crate::backend::common::{
    DockerRunProfile, add_optional_hf_token_arg, apply_optional_hf_token_env, base_docker_run_args,
};
use crate::backend::{BackendAvailability, ServingBackend};
use crate::command_spec::CommandSpec;
use crate::config::ServeConfig;
use crate::machine::MachineSpecs;

const OLLAMA_IMAGE: &str =
    "ollama/ollama@sha256:7ffd891da3e9e278d042c856c0fbe1b10fa03ed1791471377dde331eae4ea834";
const OLLAMA_USER: &str = "1000:1000";
const OLLAMA_MODELS_DIR: &str = "/data/ollama";

#[derive(Debug, Default)]
pub struct OllamaBackend;

impl ServingBackend for OllamaBackend {
    fn name(&self) -> &'static str {
        "ollama"
    }

    fn display_name(&self) -> &'static str {
        "Ollama"
    }

    fn is_available(&self, machine: &MachineSpecs) -> BackendAvailability {
        if !machine.docker.installed {
            return BackendAvailability::unavailable("Docker is not installed.");
        }

        if !machine.docker.daemon_accessible {
            return BackendAvailability::unavailable("Docker daemon is not accessible.");
        }

        let reason = if machine.gpu.is_some() {
            "Docker + NVIDIA GPU detected."
        } else {
            "Docker available (CPU mode possible; performance can be limited)."
        };

        BackendAvailability::available(reason)
    }

    fn build_docker_command(&self, config: &ServeConfig) -> CommandSpec {
        let mut args = base_docker_run_args(DockerRunProfile {
            container_name: &config.container_name,
            user: OLLAMA_USER,
            host_port: config.port,
            container_port: 11434,
            detach: config.detach,
            gpu_all: false,
            runtime: None,
        });
        args.extend([
            "-v".to_owned(),
            format!(
                "{}:{OLLAMA_MODELS_DIR}:rw",
                config.huggingface_cache_dir.display()
            ),
            "-e".to_owned(),
            "OLLAMA_MODELS".to_owned(),
        ]);

        add_optional_hf_token_arg(&mut args, config.hf_token.as_deref());

        args.push(OLLAMA_IMAGE.to_owned());

        let mut command = CommandSpec::new("docker")
            .args(args)
            .env("OLLAMA_MODELS", OLLAMA_MODELS_DIR);
        command = apply_optional_hf_token_env(command, config.hf_token.as_deref());
        command
    }

    fn build_compose_service(&self, config: &ServeConfig) -> String {
        let mut service = format!(
            "  gaia-ollama:\n    image: {OLLAMA_IMAGE}\n    container_name: {}\n    restart: unless-stopped\n    user: \"{OLLAMA_USER}\"\n    security_opt:\n      - no-new-privileges:true\n    cap_drop:\n      - ALL\n    read_only: true\n    tmpfs:\n      - /tmp\n    ports:\n      - \"{}:11434\"\n    volumes:\n      - {}:{OLLAMA_MODELS_DIR}:rw\n    environment:\n      - OLLAMA_MODELS={OLLAMA_MODELS_DIR}\n",
            config.container_name,
            config.port,
            config.huggingface_cache_dir.display(),
        );
        if config.hf_token.is_some() {
            service.push_str("      - HF_TOKEN=${HF_TOKEN}\n");
        }
        service
    }

    fn api_base_url(&self, config: &ServeConfig) -> String {
        format!("http://{}:{}/v1", config.host, config.port)
    }
}
