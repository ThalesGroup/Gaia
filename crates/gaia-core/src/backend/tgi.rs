use crate::backend::common::{
    DockerRunProfile, add_optional_hf_token_arg, apply_optional_hf_token_env, base_docker_run_args,
};
use crate::backend::{BackendAvailability, ServingBackend};
use crate::command_spec::CommandSpec;
use crate::config::ServeConfig;
use crate::machine::MachineSpecs;

const TGI_IMAGE: &str = "ghcr.io/huggingface/text-generation-inference@sha256:e6b0af6e0bf65337b84a19f15d74660c7892192f555fb0b68d3f3d62bf0c1e9a";
const TGI_USER: &str = "1000:1000";

#[derive(Debug, Default)]
pub struct TgiBackend;

impl ServingBackend for TgiBackend {
    fn name(&self) -> &'static str {
        "tgi"
    }

    fn display_name(&self) -> &'static str {
        "Text Generation Inference"
    }

    fn is_available(&self, machine: &MachineSpecs) -> BackendAvailability {
        if !machine.docker.installed {
            return BackendAvailability::unavailable("Docker is not installed.");
        }

        if !machine.docker.daemon_accessible {
            return BackendAvailability::unavailable("Docker daemon is not accessible.");
        }

        if machine.gpu.is_none() {
            return BackendAvailability::unavailable("NVIDIA GPU is recommended for TGI.");
        }

        BackendAvailability::available("Docker + NVIDIA GPU detected.")
    }

    fn build_docker_command(&self, config: &ServeConfig) -> CommandSpec {
        let mut args = base_docker_run_args(DockerRunProfile {
            container_name: &config.container_name,
            user: TGI_USER,
            host_port: config.port,
            container_port: 80,
            detach: config.detach,
            gpu_all: true,
            runtime: None,
        });
        args.extend([
            "-v".to_owned(),
            format!("{}:/data:rw", config.huggingface_cache_dir.display()),
        ]);

        add_optional_hf_token_arg(&mut args, config.hf_token.as_deref());

        args.extend([
            TGI_IMAGE.to_owned(),
            "--model-id".to_owned(),
            config.model_id.clone(),
        ]);

        if let Some(revision) = &config.model_revision {
            args.extend(["--revision".to_owned(), revision.clone()]);
        }

        let command = CommandSpec::new("docker").args(args);
        apply_optional_hf_token_env(command, config.hf_token.as_deref())
    }

    fn build_compose_service(&self, config: &ServeConfig) -> String {
        let mut service = format!(
            "  gaia-tgi:\n    image: {TGI_IMAGE}\n    container_name: {}\n    restart: unless-stopped\n    user: \"{TGI_USER}\"\n    security_opt:\n      - no-new-privileges:true\n    cap_drop:\n      - ALL\n    read_only: true\n    tmpfs:\n      - /tmp\n    ports:\n      - \"{}:80\"\n    volumes:\n      - {}:/data:rw\n    environment:\n      - HF_TOKEN=${{HF_TOKEN}}\n    deploy:\n      resources:\n        reservations:\n          devices:\n            - driver: nvidia\n              count: all\n              capabilities: [gpu]\n    command:\n      - --model-id\n      - {}\n",
            config.container_name,
            config.port,
            config.huggingface_cache_dir.display(),
            config.model_id
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
