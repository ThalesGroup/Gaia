use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

use crate::machine::MachineSpecs;
use crate::model_catalog::{ModelCatalog, ModelEntry};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FitStatus {
    Easy,
    Fits,
    Tight,
    RequiresQuantization,
    RequiresMultiGpu,
    NotRecommended,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct ModelRecommendation {
    pub model: ModelEntry,
    pub status: FitStatus,
    pub explanation: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreferredBackend {
    Vllm,
    Tgi,
    None,
}

impl PreferredBackend {
    pub fn as_str(self) -> Option<&'static str> {
        match self {
            Self::Vllm => Some("vllm"),
            Self::Tgi => Some("tgi"),
            Self::None => None,
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Vllm => "vLLM",
            Self::Tgi => "TGI",
            Self::None => "None",
        }
    }
}

impl FitStatus {
    pub fn rank(self) -> u8 {
        match self {
            Self::Easy => 6,
            Self::Fits => 5,
            Self::Tight => 4,
            Self::RequiresQuantization => 3,
            Self::RequiresMultiGpu => 2,
            Self::Unknown => 1,
            Self::NotRecommended => 0,
        }
    }

    pub fn as_badge(self) -> &'static str {
        match self {
            Self::Easy => "easy",
            Self::Fits => "fits",
            Self::Tight => "tight",
            Self::RequiresQuantization => "requires quantization",
            Self::RequiresMultiGpu => "requires multi-gpu",
            Self::NotRecommended => "not recommended",
            Self::Unknown => "unknown",
        }
    }

    pub fn is_positive(self) -> bool {
        matches!(
            self,
            Self::Easy | Self::Fits | Self::Tight | Self::RequiresQuantization
        )
    }
}

pub struct RecommendationEngine;

impl RecommendationEngine {
    pub fn preferred_backend(machine: &MachineSpecs) -> PreferredBackend {
        if !machine.docker.installed || !machine.docker.daemon_accessible {
            return PreferredBackend::None;
        }

        if machine.gpu.is_some() {
            PreferredBackend::Vllm
        } else {
            PreferredBackend::None
        }
    }

    pub fn recommend_models(
        machine: &MachineSpecs,
        catalog: &ModelCatalog,
        backend: Option<&str>,
    ) -> Vec<ModelRecommendation> {
        let mut recommendations = catalog
            .models
            .iter()
            .filter(|model| {
                if let Some(backend_name) = backend {
                    model.supports_backend(backend_name)
                } else {
                    true
                }
            })
            .map(|model| {
                let (status, explanation) = Self::evaluate_fit(machine, model);
                ModelRecommendation {
                    model: model.clone(),
                    status,
                    explanation,
                }
            })
            .collect::<Vec<_>>();

        recommendations.sort_by(compare_recommendations);
        recommendations
    }

    pub fn evaluate_fit(machine: &MachineSpecs, model: &ModelEntry) -> (FitStatus, String) {
        let Some(vram_gb) = machine.gpu_vram_gb() else {
            return (
                FitStatus::NotRecommended,
                "No NVIDIA GPU detected; GPU backends are not recommended.".to_owned(),
            );
        };

        let mut status = if vram_gb >= model.min_vram_gb_fp16 + 2.0 {
            FitStatus::Easy
        } else if vram_gb >= model.min_vram_gb_fp16 {
            FitStatus::Fits
        } else if vram_gb >= model.min_vram_gb_int8 {
            FitStatus::Tight
        } else if vram_gb >= model.min_vram_gb_int4 {
            FitStatus::RequiresQuantization
        } else if model.params_b >= 30.0 {
            FitStatus::RequiresMultiGpu
        } else {
            FitStatus::NotRecommended
        };

        let min_ram_gb = (model.params_b * 2.0).ceil();
        if machine.ram_total_gb + 0.1 < min_ram_gb {
            status = downgrade(status);
        }

        let explanation = match status {
            FitStatus::Easy => format!(
                "VRAM {:.1} GB comfortably exceeds FP16 target ({:.1} GB).",
                vram_gb, model.min_vram_gb_fp16
            ),
            FitStatus::Fits => format!(
                "VRAM {:.1} GB fits FP16 minimum ({:.1} GB).",
                vram_gb, model.min_vram_gb_fp16
            ),
            FitStatus::Tight => format!(
                "VRAM {:.1} GB is below FP16 but above INT8 threshold ({:.1} GB).",
                vram_gb, model.min_vram_gb_int8
            ),
            FitStatus::RequiresQuantization => format!(
                "VRAM {:.1} GB suggests INT4/INT8 quantization is required.",
                vram_gb
            ),
            FitStatus::RequiresMultiGpu => {
                "Model is too large for single GPU at this memory level.".to_owned()
            }
            FitStatus::NotRecommended => {
                "Current VRAM is too low for a reliable deployment.".to_owned()
            }
            FitStatus::Unknown => "Hardware constraints are unclear for this model.".to_owned(),
        };

        (status, explanation)
    }
}

fn downgrade(status: FitStatus) -> FitStatus {
    match status {
        FitStatus::Easy => FitStatus::Fits,
        FitStatus::Fits => FitStatus::Tight,
        FitStatus::Tight => FitStatus::RequiresQuantization,
        FitStatus::RequiresQuantization => FitStatus::NotRecommended,
        other => other,
    }
}

fn compare_recommendations(left: &ModelRecommendation, right: &ModelRecommendation) -> Ordering {
    right.status.rank().cmp(&left.status.rank()).then_with(|| {
        left.model
            .params_b
            .partial_cmp(&right.model.params_b)
            .unwrap_or(Ordering::Equal)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::{DockerStatus, GpuInfo};

    fn machine_specs(vram_gb: Option<f32>, ram_total_gb: f32) -> MachineSpecs {
        MachineSpecs {
            os_name: "Linux".to_owned(),
            kernel_version: Some("test-kernel".to_owned()),
            cpu_cores: 16,
            ram_total_gb,
            docker: DockerStatus {
                installed: true,
                daemon_accessible: true,
                version: Some("Docker test".to_owned()),
                details: None,
            },
            gpu: vram_gb.map(|vram| GpuInfo {
                name: "Test GPU".to_owned(),
                vram_gb: vram,
                driver_version: None,
                cuda_version: None,
            }),
            hf_token_present: false,
            warnings: Vec::new(),
        }
    }

    fn model_entry(id: &str, params_b: f32, fp16: f32, int8: f32, int4: f32) -> ModelEntry {
        ModelEntry {
            id: id.to_owned(),
            display_name: id.to_owned(),
            family: "test".to_owned(),
            params_b,
            categories: vec!["instruct".to_owned()],
            recommended_use: "tests".to_owned(),
            min_vram_gb_fp16: fp16,
            min_vram_gb_int8: int8,
            min_vram_gb_int4: int4,
            supports_vllm: true,
            supports_tgi: true,
            gated: false,
        }
    }

    #[test]
    fn preferred_backend_requires_docker_and_gpu() {
        let machine_with_gpu = machine_specs(Some(24.0), 64.0);
        assert_eq!(
            RecommendationEngine::preferred_backend(&machine_with_gpu),
            PreferredBackend::Vllm
        );

        let machine_without_gpu = machine_specs(None, 64.0);
        assert_eq!(
            RecommendationEngine::preferred_backend(&machine_without_gpu),
            PreferredBackend::None
        );
    }

    #[test]
    fn evaluate_fit_without_gpu_is_not_recommended() {
        let machine = machine_specs(None, 64.0);
        let model = model_entry("provider/model", 7.0, 8.0, 6.0, 4.0);
        let (status, explanation) = RecommendationEngine::evaluate_fit(&machine, &model);

        assert_eq!(status, FitStatus::NotRecommended);
        assert!(explanation.contains("No NVIDIA GPU"));
    }

    #[test]
    fn evaluate_fit_downgrades_when_ram_is_insufficient() {
        let machine = machine_specs(Some(16.0), 10.0);
        let model = model_entry("provider/model", 8.0, 12.0, 8.0, 6.0);
        let (status, explanation) = RecommendationEngine::evaluate_fit(&machine, &model);

        assert_eq!(status, FitStatus::Fits);
        assert!(explanation.contains("fits FP16 minimum"));
    }

    #[test]
    fn evaluate_fit_marks_large_models_as_multi_gpu_when_needed() {
        let machine = machine_specs(Some(6.0), 256.0);
        let model = model_entry("provider/large-model", 70.0, 24.0, 16.0, 8.0);
        let (status, _) = RecommendationEngine::evaluate_fit(&machine, &model);
        assert_eq!(status, FitStatus::RequiresMultiGpu);
    }

    #[test]
    fn recommend_models_sorts_by_rank_then_model_size() {
        let machine = machine_specs(Some(18.0), 256.0);
        let small_easy = model_entry("provider/small", 7.0, 8.0, 6.0, 4.0);
        let large_easy = model_entry("provider/large", 20.0, 8.0, 6.0, 4.0);
        let tight = model_entry("provider/tight", 12.0, 20.0, 17.0, 8.0);

        let catalog = ModelCatalog {
            models: vec![large_easy.clone(), tight.clone(), small_easy.clone()],
        };

        let recommendations = RecommendationEngine::recommend_models(&machine, &catalog, None);
        let ordered_ids = recommendations
            .iter()
            .map(|recommendation| recommendation.model.id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            ordered_ids,
            vec!["provider/small", "provider/large", "provider/tight"]
        );
    }
}
