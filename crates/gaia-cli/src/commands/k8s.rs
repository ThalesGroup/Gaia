use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::Args;

use gaia_core::config::{AppConfig, normalize_model_revision};

#[derive(Debug, Args)]
pub struct GenerateK8sArgs {
    #[arg(long)]
    pub backend: Option<String>,
    #[arg(long)]
    pub model: Option<String>,
    #[arg(
        long,
        value_name = "COMMIT_SHA",
        help = "Immutable 40-character Hugging Face commit SHA"
    )]
    pub model_revision: Option<String>,
    #[arg(long)]
    pub namespace: Option<String>,
    #[arg(long, default_value_t = 1)]
    pub replicas: u16,
    #[arg(long)]
    pub output: Option<PathBuf>,
}

pub fn run(args: GenerateK8sArgs) -> Result<()> {
    let mut config = AppConfig::load_or_transient_default()?;
    if let Some(backend) = args.backend {
        config.backend.name = backend;
    }
    if let Some(model) = args.model {
        config.model.id = model;
    }
    config.model.revision = normalize_model_revision(
        args.model_revision
            .as_deref()
            .or(config.model.revision.as_deref()),
    )?;

    let serve_config = config.to_serve_config();
    let backend = serve_config.backend.to_ascii_lowercase();
    let namespace = args.namespace.unwrap_or_else(|| "default".to_owned());
    let deployment_name = format!("gaia-{}", backend.replace('.', "-"));
    let (image, container_port, args_list) = deployment_spec(
        &backend,
        &serve_config.model_id,
        serve_config.model_revision.as_deref(),
    )?;

    let yaml = format!(
        "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: {deployment_name}\n  namespace: {namespace}\nspec:\n  replicas: {}\n  selector:\n    matchLabels:\n      app: {deployment_name}\n  template:\n    metadata:\n      labels:\n        app: {deployment_name}\n    spec:\n      containers:\n        - name: {deployment_name}\n          image: {image}\n          securityContext:\n            runAsNonRoot: true\n            runAsUser: 1000\n            runAsGroup: 1000\n            readOnlyRootFilesystem: true\n            allowPrivilegeEscalation: false\n            capabilities:\n              drop: [\"ALL\"]\n            seccompProfile:\n              type: RuntimeDefault\n          env:\n            - name: GAIA_API_KEY\n              valueFrom:\n                secretKeyRef:\n                  name: gaia-secrets\n                  key: api-key\n            - name: HF_TOKEN\n              valueFrom:\n                secretKeyRef:\n                  name: gaia-secrets\n                  key: hf-token\n                  optional: true\n          ports:\n            - containerPort: {container_port}\n          args:\n{args_list}\n---\napiVersion: v1\nkind: Service\nmetadata:\n  name: {deployment_name}\n  namespace: {namespace}\nspec:\n  selector:\n    app: {deployment_name}\n  ports:\n    - name: http\n      protocol: TCP\n      port: {}\n      targetPort: {container_port}\n  type: ClusterIP\n",
        args.replicas, serve_config.port
    );

    let output = args
        .output
        .unwrap_or_else(|| PathBuf::from(format!("k8s-{}.yaml", backend.replace('.', "-"))));
    fs::write(&output, yaml).with_context(|| format!("Unable to write {}", output.display()))?;
    println!("Generated {}", output.display());
    println!("Create/update Kubernetes secret `gaia-secrets` before deploy:");
    println!(
        "  kubectl -n {} create secret generic gaia-secrets --from-literal=api-key='your-api-key' --from-literal=hf-token='your-hf-token' --dry-run=client -o yaml | kubectl apply -f -",
        namespace
    );
    Ok(())
}

fn deployment_spec(
    backend: &str,
    model_id: &str,
    model_revision: Option<&str>,
) -> Result<(&'static str, u16, String)> {
    let (image, port, mut args) = match backend {
        "vllm" => (
            "vllm/vllm-openai@sha256:70a098d90dbab428a001d9e852fc0fc8d67da5beb03e7851a22247653bf35923",
            8000,
            vec![
                "--model".to_owned(),
                model_id.to_owned(),
                "--host".to_owned(),
                "0.0.0.0".to_owned(),
                "--port".to_owned(),
                "8000".to_owned(),
                "--dtype".to_owned(),
                "auto".to_owned(),
                "--api-key".to_owned(),
                "$(GAIA_API_KEY)".to_owned(),
            ],
        ),
        "tgi" => (
            "ghcr.io/huggingface/text-generation-inference@sha256:e6b0af6e0bf65337b84a19f15d74660c7892192f555fb0b68d3f3d62bf0c1e9a",
            80,
            vec!["--model-id".to_owned(), model_id.to_owned()],
        ),
        "sglang" => (
            "lmsysorg/sglang@sha256:061fb71f838e82000a1768c159654d526c2f17ebe751c21e7fc48ca53c8ef975",
            30000,
            vec![
                "python3".to_owned(),
                "-m".to_owned(),
                "sglang.launch_server".to_owned(),
                "--model-path".to_owned(),
                model_id.to_owned(),
                "--host".to_owned(),
                "0.0.0.0".to_owned(),
                "--port".to_owned(),
                "30000".to_owned(),
                "--api-key".to_owned(),
                "$(GAIA_API_KEY)".to_owned(),
            ],
        ),
        "llamacpp" | "llama.cpp" => (
            "ghcr.io/ggml-org/llama.cpp@sha256:785bda5afb7430425e6b26006959b4d986ffcd08a006cc386af1f929016b74e2",
            8080,
            vec![
                "-m".to_owned(),
                model_id.to_owned(),
                "--host".to_owned(),
                "0.0.0.0".to_owned(),
                "--port".to_owned(),
                "8080".to_owned(),
                "--api-key".to_owned(),
                "$(GAIA_API_KEY)".to_owned(),
            ],
        ),
        "ollama" => (
            "ollama/ollama@sha256:7ffd891da3e9e278d042c856c0fbe1b10fa03ed1791471377dde331eae4ea834",
            11434,
            vec![],
        ),
        "mock" => (
            "curlimages/curl@sha256:4026b29997dc7c823b51c164b71e2b51e0fd95cce4601f78202c513d97da2922",
            8000,
            vec![],
        ),
        _ => bail!("Unsupported backend `{backend}` for Kubernetes generation"),
    };

    if let Some(revision) = model_revision
        && !revision.trim().is_empty()
        && matches!(backend, "vllm" | "tgi" | "sglang")
    {
        args.push("--revision".to_owned());
        args.push(revision.trim().to_owned());
    }

    let mut yaml_args = String::new();
    for item in args {
        yaml_args.push_str(&format!(
            "            - \"{}\"\n",
            item.replace('"', "\\\"")
        ));
    }
    if yaml_args.is_empty() {
        yaml_args.push_str("            []\n");
    }

    Ok((image, port, yaml_args))
}
