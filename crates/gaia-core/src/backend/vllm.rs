use crate::backend::common::{
    DockerRunProfile, add_optional_hf_token_arg, apply_optional_hf_token_env, base_docker_run_args,
};
use crate::backend::{BackendAvailability, ServingBackend};
use crate::command_spec::CommandSpec;
use crate::config::ServeConfig;
use crate::machine::MachineSpecs;

const VLLM_IMAGE: &str =
    "vllm/vllm-openai@sha256:70a098d90dbab428a001d9e852fc0fc8d67da5beb03e7851a22247653bf35923";
const VLLM_USER: &str = "1000:1000";
const HF_CACHE_DIR: &str = "/data/hf-cache";

#[derive(Debug, Default)]
pub struct VllmBackend;

impl ServingBackend for VllmBackend {
    fn name(&self) -> &'static str {
        "vllm"
    }

    fn display_name(&self) -> &'static str {
        "vLLM"
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
                "NVIDIA GPU is required for vLLM docker mode.",
            );
        }

        BackendAvailability::available("Docker + NVIDIA GPU detected.")
    }

    fn build_docker_command(&self, config: &ServeConfig) -> CommandSpec {
        let mut args = base_docker_run_args(DockerRunProfile {
            container_name: &config.container_name,
            user: VLLM_USER,
            host_port: config.port,
            container_port: 8000,
            detach: config.detach,
            gpu_all: true,
            runtime: Some("nvidia"),
        });
        args.extend([
            "-v".to_owned(),
            format!(
                "{}:{HF_CACHE_DIR}:rw",
                config.huggingface_cache_dir.display()
            ),
            "--ipc=host".to_owned(),
            "-e".to_owned(),
            "HF_HOME".to_owned(),
            "-e".to_owned(),
            "HUGGINGFACE_HUB_CACHE".to_owned(),
            "-e".to_owned(),
            "TRANSFORMERS_CACHE".to_owned(),
        ]);

        add_optional_hf_token_arg(&mut args, config.hf_token.as_deref());

        args.extend([
            VLLM_IMAGE.to_owned(),
            "--model".to_owned(),
            config.model_id.clone(),
            "--host".to_owned(),
            "0.0.0.0".to_owned(),
            "--port".to_owned(),
            "8000".to_owned(),
            "--dtype".to_owned(),
            config.dtype.clone(),
            "--api-key".to_owned(),
            config.api_key.clone(),
        ]);

        if let Some(revision) = &config.model_revision {
            args.extend(["--revision".to_owned(), revision.clone()]);
        }

        if config.max_model_len > 0 {
            args.extend([
                "--max-model-len".to_owned(),
                config.max_model_len.to_string(),
            ]);
        }

        if config.quantization != "none" {
            args.extend(["--quantization".to_owned(), config.quantization.clone()]);
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
            "  gaia-vllm:\n    image: {VLLM_IMAGE}\n    container_name: {}\n    restart: unless-stopped\n    runtime: nvidia\n    user: \"{VLLM_USER}\"\n    security_opt:\n      - no-new-privileges:true\n    cap_drop:\n      - ALL\n    read_only: true\n    tmpfs:\n      - /tmp\n    ports:\n      - \"{}:8000\"\n    volumes:\n      - {}:{HF_CACHE_DIR}:rw\n    ipc: host\n    environment:\n      - HF_HOME={HF_CACHE_DIR}\n      - HUGGINGFACE_HUB_CACHE={HF_CACHE_DIR}\n      - TRANSFORMERS_CACHE={HF_CACHE_DIR}\n      - GAIA_API_KEY=${{GAIA_API_KEY:-local-key}}\n      - HF_TOKEN=${{HF_TOKEN}}\n    command:\n      - --model\n      - {}\n      - --host\n      - 0.0.0.0\n      - --port\n      - \"8000\"\n      - --dtype\n      - {}\n      - --api-key\n      - ${{GAIA_API_KEY:-local-key}}\n",
            config.container_name,
            config.port,
            config.huggingface_cache_dir.display(),
            config.model_id,
            config.dtype,
        );

        if let Some(revision) = &config.model_revision {
            service.push_str(&format!("      - --revision\n      - \"{}\"\n", revision));
        }

        if config.max_model_len > 0 {
            service.push_str(&format!(
                "      - --max-model-len\n      - \"{}\"\n",
                config.max_model_len
            ));
        }

        service
    }

    fn api_base_url(&self, config: &ServeConfig) -> String {
        format!("http://{}:{}/v1", config.host, config.port)
    }
}
