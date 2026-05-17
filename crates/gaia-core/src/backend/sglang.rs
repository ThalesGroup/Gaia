use crate::backend::common::{
    DockerRunProfile, add_optional_hf_token_arg, apply_optional_hf_token_env, base_docker_run_args,
};
use crate::backend::{BackendAvailability, ServingBackend};
use crate::command_spec::CommandSpec;
use crate::config::ServeConfig;
use crate::machine::MachineSpecs;

const SGLANG_IMAGE: &str =
    "lmsysorg/sglang@sha256:061fb71f838e82000a1768c159654d526c2f17ebe751c21e7fc48ca53c8ef975";
const SGLANG_USER: &str = "1000:1000";
const HF_CACHE_DIR: &str = "/data/hf-cache";

#[derive(Debug, Default)]
pub struct SglangBackend;

impl ServingBackend for SglangBackend {
    fn name(&self) -> &'static str {
        "sglang"
    }

    fn display_name(&self) -> &'static str {
        "SGLang"
    }

    fn is_available(&self, machine: &MachineSpecs) -> BackendAvailability {
        if !machine.docker.installed {
            return BackendAvailability::unavailable("Docker is not installed.");
        }

        if !machine.docker.daemon_accessible {
            return BackendAvailability::unavailable("Docker daemon is not accessible.");
        }

        if machine.gpu.is_none() {
            return BackendAvailability::unavailable(
                "NVIDIA GPU is strongly recommended for SGLang.",
            );
        }

        BackendAvailability::available("Docker + NVIDIA GPU detected.")
    }

    fn build_docker_command(&self, config: &ServeConfig) -> CommandSpec {
        let mut args = base_docker_run_args(DockerRunProfile {
            container_name: &config.container_name,
            user: SGLANG_USER,
            host_port: config.port,
            container_port: 30000,
            detach: config.detach,
            gpu_all: true,
            runtime: None,
        });
        args.extend([
            "-v".to_owned(),
            format!(
                "{}:{HF_CACHE_DIR}:rw",
                config.huggingface_cache_dir.display()
            ),
            "-e".to_owned(),
            "HF_HOME".to_owned(),
            "-e".to_owned(),
            "HUGGINGFACE_HUB_CACHE".to_owned(),
            "-e".to_owned(),
            "TRANSFORMERS_CACHE".to_owned(),
        ]);

        add_optional_hf_token_arg(&mut args, config.hf_token.as_deref());

        args.extend([
            SGLANG_IMAGE.to_owned(),
            "python3".to_owned(),
            "-m".to_owned(),
            "sglang.launch_server".to_owned(),
            "--model-path".to_owned(),
            config.model_id.clone(),
            "--host".to_owned(),
            "0.0.0.0".to_owned(),
            "--port".to_owned(),
            "30000".to_owned(),
            "--api-key".to_owned(),
            config.api_key.clone(),
        ]);

        if let Some(revision) = &config.model_revision {
            args.extend(["--revision".to_owned(), revision.clone()]);
        }

        let mut command = CommandSpec::new("docker")
            .args(args)
            .env("HF_HOME", HF_CACHE_DIR)
            .env("HUGGINGFACE_HUB_CACHE", HF_CACHE_DIR)
            .env("TRANSFORMERS_CACHE", HF_CACHE_DIR);
        command = apply_optional_hf_token_env(command, config.hf_token.as_deref());
        command
    }

    fn build_compose_service(&self, config: &ServeConfig) -> String {
        let mut service = format!(
            "  gaia-sglang:\n    image: {SGLANG_IMAGE}\n    container_name: {}\n    restart: unless-stopped\n    user: \"{SGLANG_USER}\"\n    security_opt:\n      - no-new-privileges:true\n    cap_drop:\n      - ALL\n    read_only: true\n    tmpfs:\n      - /tmp\n    ports:\n      - \"{}:30000\"\n    volumes:\n      - {}:{HF_CACHE_DIR}:rw\n    environment:\n      - HF_HOME={HF_CACHE_DIR}\n      - HUGGINGFACE_HUB_CACHE={HF_CACHE_DIR}\n      - TRANSFORMERS_CACHE={HF_CACHE_DIR}\n      - GAIA_API_KEY=${{GAIA_API_KEY:-local-key}}\n      - HF_TOKEN=${{HF_TOKEN}}\n    deploy:\n      resources:\n        reservations:\n          devices:\n            - driver: nvidia\n              count: all\n              capabilities: [gpu]\n    command:\n      - python3\n      - -m\n      - sglang.launch_server\n      - --model-path\n      - {}\n      - --host\n      - 0.0.0.0\n      - --port\n      - \"30000\"\n      - --api-key\n      - ${{GAIA_API_KEY:-local-key}}\n",
            config.container_name,
            config.port,
            config.huggingface_cache_dir.display(),
            config.model_id,
        );

        if let Some(revision) = &config.model_revision {
            service.push_str(&format!("      - --revision\n      - \"{}\"\n", revision));
        }

        service
    }

    fn api_base_url(&self, config: &ServeConfig) -> String {
        format!("http://{}:{}/v1", config.host, config.port)
    }
}
