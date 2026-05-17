use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const DEFAULT_CATALOG: &str = include_str!("../../../catalog/models.yaml");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCatalog {
    pub models: Vec<ModelEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    pub id: String,
    pub display_name: String,
    pub family: String,
    pub params_b: f32,
    pub categories: Vec<String>,
    pub recommended_use: String,
    pub min_vram_gb_fp16: f32,
    pub min_vram_gb_int8: f32,
    pub min_vram_gb_int4: f32,
    pub supports_vllm: bool,
    pub supports_tgi: bool,
    pub gated: bool,
}

impl ModelCatalog {
    pub fn load_default() -> Result<Self> {
        let parsed = serde_yaml::from_str::<Self>(DEFAULT_CATALOG)
            .context("Unable to parse embedded model catalog YAML")?;
        Ok(parsed)
    }

    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let content = fs::read_to_string(path)
            .with_context(|| format!("Unable to read catalog file: {}", path.display()))?;
        let parsed = serde_yaml::from_str::<Self>(&content)
            .with_context(|| format!("Unable to parse catalog file: {}", path.display()))?;
        Ok(parsed)
    }

    pub fn filtered(
        &self,
        category: Option<&str>,
        max_params: Option<f32>,
        backend: Option<&str>,
    ) -> Vec<ModelEntry> {
        self.models
            .iter()
            .filter(|model| {
                if let Some(category) = category {
                    let category = category.to_lowercase();
                    if !model.categories.iter().any(|item| item == &category) {
                        return false;
                    }
                }
                true
            })
            .filter(|model| {
                if let Some(max_params) = max_params {
                    model.params_b <= max_params
                } else {
                    true
                }
            })
            .filter(|model| {
                if let Some(backend) = backend {
                    model.supports_backend(backend)
                } else {
                    true
                }
            })
            .cloned()
            .collect()
    }
}

impl ModelEntry {
    pub fn supports_backend(&self, backend: &str) -> bool {
        match backend.to_ascii_lowercase().as_str() {
            "vllm" => self.supports_vllm,
            "tgi" => self.supports_tgi,
            "sglang" => self.supports_vllm,
            "llamacpp" | "llama.cpp" => true,
            "ollama" => true,
            _ => false,
        }
    }

    pub fn categories_label(&self) -> String {
        self.categories.join(",")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_model(
        id: &str,
        params_b: f32,
        categories: &[&str],
        supports_vllm: bool,
        supports_tgi: bool,
    ) -> ModelEntry {
        ModelEntry {
            id: id.to_owned(),
            display_name: id.to_owned(),
            family: "test".to_owned(),
            params_b,
            categories: categories.iter().map(|item| (*item).to_owned()).collect(),
            recommended_use: "tests".to_owned(),
            min_vram_gb_fp16: 8.0,
            min_vram_gb_int8: 6.0,
            min_vram_gb_int4: 4.0,
            supports_vllm,
            supports_tgi,
            gated: false,
        }
    }

    #[test]
    fn supports_backend_maps_aliases_and_defaults() {
        let model = make_model("test/model", 7.0, &["instruct"], true, false);

        assert!(model.supports_backend("vllm"));
        assert!(!model.supports_backend("tgi"));
        assert!(model.supports_backend("sglang"));
        assert!(model.supports_backend("llamacpp"));
        assert!(model.supports_backend("llama.cpp"));
        assert!(model.supports_backend("ollama"));
        assert!(!model.supports_backend("unknown"));
    }

    #[test]
    fn filtered_applies_category_size_and_backend_constraints() {
        let catalog = ModelCatalog {
            models: vec![
                make_model("provider/model-a", 7.0, &["instruct"], true, false),
                make_model("provider/model-b", 20.0, &["chat"], false, true),
            ],
        };

        let filtered = catalog.filtered(Some("InStruct"), Some(10.0), Some("vllm"));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "provider/model-a");
    }

    #[test]
    fn categories_label_joins_with_commas() {
        let model = make_model(
            "provider/model",
            7.0,
            &["instruct", "chat", "code"],
            true,
            true,
        );
        assert_eq!(model.categories_label(), "instruct,chat,code");
    }
}
