pub mod common;
pub mod llamacpp;
pub mod ollama;
pub mod sglang;
pub mod tgi;
pub mod vllm;

use crate::command_spec::CommandSpec;
use crate::config::ServeConfig;
use crate::machine::MachineSpecs;

#[derive(Debug, Clone)]
pub struct BackendAvailability {
    pub available: bool,
    pub reason: String,
    pub warnings: Vec<String>,
}

impl BackendAvailability {
    pub fn available(reason: impl Into<String>) -> Self {
        Self {
            available: true,
            reason: reason.into(),
            warnings: Vec::new(),
        }
    }

    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            available: false,
            reason: reason.into(),
            warnings: Vec::new(),
        }
    }
}

pub trait ServingBackend {
    fn name(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn is_available(&self, machine: &MachineSpecs) -> BackendAvailability;
    fn build_docker_command(&self, config: &ServeConfig) -> CommandSpec;
    fn build_compose_service(&self, config: &ServeConfig) -> String;
    fn api_base_url(&self, config: &ServeConfig) -> String;
}

pub fn backend_from_name(name: &str) -> Option<Box<dyn ServingBackend>> {
    match name.to_ascii_lowercase().as_str() {
        "vllm" => Some(Box::new(vllm::VllmBackend)),
        "tgi" => Some(Box::new(tgi::TgiBackend)),
        "sglang" => Some(Box::new(sglang::SglangBackend)),
        "llamacpp" | "llama.cpp" => Some(Box::new(llamacpp::LlamaCppBackend)),
        "ollama" => Some(Box::new(ollama::OllamaBackend)),
        _ => None,
    }
}

pub fn all_backends() -> Vec<Box<dyn ServingBackend>> {
    vec![
        Box::new(vllm::VllmBackend),
        Box::new(tgi::TgiBackend),
        Box::new(sglang::SglangBackend),
        Box::new(llamacpp::LlamaCppBackend),
        Box::new(ollama::OllamaBackend),
    ]
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn backend_from_name_supports_case_and_aliases() {
        assert_eq!(
            backend_from_name("vLLM").expect("vLLM must resolve").name(),
            "vllm"
        );
        assert_eq!(
            backend_from_name("llama.cpp")
                .expect("llama.cpp alias must resolve")
                .name(),
            "llamacpp"
        );
        assert_eq!(
            backend_from_name("llamacpp")
                .expect("llamacpp canonical name must resolve")
                .name(),
            "llamacpp"
        );
    }

    #[test]
    fn backend_from_name_rejects_unknown_backend() {
        assert!(backend_from_name("unknown-backend").is_none());
    }

    #[test]
    fn all_backends_contains_expected_unique_entries() {
        let names = all_backends()
            .into_iter()
            .map(|backend| backend.name().to_owned())
            .collect::<BTreeSet<_>>();

        let expected = ["vllm", "tgi", "sglang", "llamacpp", "ollama"]
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();

        assert_eq!(names, expected);
    }
}
