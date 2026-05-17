use crate::backend::common::{DockerRunProfile, base_docker_run_args};
use crate::backend::{BackendAvailability, ServingBackend};
use crate::command_spec::CommandSpec;
use crate::config::ServeConfig;
use crate::machine::MachineSpecs;

const LLAMACPP_IMAGE: &str = "ghcr.io/ggml-org/llama.cpp@sha256:785bda5afb7430425e6b26006959b4d986ffcd08a006cc386af1f929016b74e2";
const LLAMACPP_USER: &str = "1000:1000";

#[derive(Debug, Default)]
pub struct LlamaCppBackend;

impl ServingBackend for LlamaCppBackend {
    fn name(&self) -> &'static str {
        "llamacpp"
    }

    fn display_name(&self) -> &'static str {
        "llama.cpp"
    }

    fn is_available(&self, machine: &MachineSpecs) -> BackendAvailability {
        if !machine.docker.installed {
            return BackendAvailability::unavailable("Docker is not installed.");
        }

        if !machine.docker.daemon_accessible {
            return BackendAvailability::unavailable("Docker daemon is not accessible.");
        }

        let reason = if machine.gpu.is_some() {
            "Docker available (GPU detected, can use CUDA builds)."
        } else {
            "Docker available (CPU mode possible; performance will be lower)."
        };

        BackendAvailability::available(reason)
    }

    fn build_docker_command(&self, config: &ServeConfig) -> CommandSpec {
        let mut args = base_docker_run_args(DockerRunProfile {
            container_name: &config.container_name,
            user: LLAMACPP_USER,
            host_port: config.port,
            container_port: 8080,
            detach: config.detach,
            gpu_all: false,
            runtime: None,
        });
        args.extend([
            "-v".to_owned(),
            format!("{}:/models:ro", config.huggingface_cache_dir.display()),
        ]);

        if config.quantization != "none" {
            args.extend([
                "-e".to_owned(),
                format!("LLAMACPP_QUANT={}", config.quantization),
            ]);
        }

        args.extend([
            LLAMACPP_IMAGE.to_owned(),
            "-m".to_owned(),
            config.model_id.clone(),
            "--host".to_owned(),
            "0.0.0.0".to_owned(),
            "--port".to_owned(),
            "8080".to_owned(),
            "--api-key".to_owned(),
            config.api_key.clone(),
        ]);

        CommandSpec::new("docker").args(args)
    }

    fn build_compose_service(&self, config: &ServeConfig) -> String {
        format!(
            "  gaia-llamacpp:\n    image: {LLAMACPP_IMAGE}\n    container_name: {}\n    restart: unless-stopped\n    user: \"{LLAMACPP_USER}\"\n    security_opt:\n      - no-new-privileges:true\n    cap_drop:\n      - ALL\n    read_only: true\n    tmpfs:\n      - /tmp\n    ports:\n      - \"{}:8080\"\n    volumes:\n      - {}:/models:ro\n    environment:\n      - GAIA_API_KEY=${{GAIA_API_KEY:-local-key}}\n    command:\n      - -m\n      - {}\n      - --host\n      - 0.0.0.0\n      - --port\n      - \"8080\"\n      - --api-key\n      - ${{GAIA_API_KEY:-local-key}}\n",
            config.container_name,
            config.port,
            config.huggingface_cache_dir.display(),
            config.model_id
        )
    }

    fn api_base_url(&self, config: &ServeConfig) -> String {
        format!("http://{}:{}/v1", config.host, config.port)
    }
}
