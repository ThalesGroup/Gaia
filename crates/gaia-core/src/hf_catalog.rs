//! Hugging Face catalog refresh: discover models and build catalog entries.
//!
//! This module ports the legacy `hf_catalog_prefill.py` logic to Rust so the
//! catalog can be refreshed from the CLI (`gaia catalog refresh`) and the TUI.

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Map, Value};

use crate::model_catalog::ModelEntry;

const HF_API_BASE: &str = "https://huggingface.co/api";
const HF_MODEL_BASE: &str = "https://huggingface.co";
const USER_AGENT: &str = "gaia-catalog-refresh/0.1";
const UNKNOWN_PARAMS_FALLBACK_B: f64 = 70.0;

const PARAM_KEYS: [&str; 9] = [
    "params",
    "parameter_count",
    "parameters",
    "model_size",
    "size",
    "num_parameters",
    "n_params",
    "total_params",
    "total_parameters",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoverSort {
    Trending,
    Downloads,
}

impl DiscoverSort {
    fn api_field(self) -> &'static str {
        match self {
            Self::Trending => "trendingScore",
            Self::Downloads => "downloads",
        }
    }
}

#[derive(Debug, Clone)]
pub struct RefreshOptions {
    /// Model ids to refresh (typically the current catalog ids).
    pub seed_ids: Vec<String>,
    /// Discover top N text-generation models from the HF API (0 disables discovery).
    pub discover_limit: usize,
    pub discover_sort: DiscoverSort,
    /// Optional HF API search query for discovery.
    pub search: Option<String>,
    /// Optional HF token (gated metadata access and better rate limits).
    pub token: Option<String>,
    pub timeout_secs: u64,
    /// Keep models even when they do not look like text-generation models.
    pub allow_non_text_generation: bool,
}

impl Default for RefreshOptions {
    fn default() -> Self {
        Self {
            seed_ids: Vec::new(),
            discover_limit: 0,
            discover_sort: DiscoverSort::Trending,
            search: None,
            token: std::env::var("HF_TOKEN")
                .ok()
                .filter(|t| !t.trim().is_empty()),
            timeout_secs: 25,
            allow_non_text_generation: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum RefreshProgress {
    Discovering,
    Model {
        index: usize,
        total: usize,
        id: String,
    },
}

#[derive(Debug, Clone, Default)]
pub struct RefreshReport {
    pub entries: Vec<ModelEntry>,
    pub skipped: Vec<String>,
    pub errors: Vec<String>,
}

/// Refresh catalog entries from the Hugging Face API.
///
/// `progress` is invoked before each network step so callers (CLI prints, TUI
/// status line) can surface progress without coupling to this module.
pub fn refresh_entries(
    options: &RefreshOptions,
    mut progress: impl FnMut(RefreshProgress),
) -> Result<RefreshReport> {
    let client = build_client(options.timeout_secs)?;

    let mut model_ids: Vec<String> = options
        .seed_ids
        .iter()
        .map(|id| id.trim().to_owned())
        .filter(|id| !id.is_empty())
        .collect();

    if options.discover_limit > 0 {
        progress(RefreshProgress::Discovering);
        let discovered = discover_model_ids(&client, options)?;
        model_ids.extend(discovered);
    }

    let model_ids = deduplicate_keep_order(model_ids);
    if model_ids.is_empty() {
        bail!("No model ids to refresh. Provide seed ids or enable discovery.");
    }

    let mut report = RefreshReport::default();
    let total = model_ids.len();
    for (index, model_id) in model_ids.iter().enumerate() {
        progress(RefreshProgress::Model {
            index: index + 1,
            total,
            id: model_id.clone(),
        });

        match fetch_and_build_entry(&client, options, model_id) {
            Ok(Some(entry)) => report.entries.push(entry),
            Ok(None) => report.skipped.push(model_id.clone()),
            Err(error) => report.errors.push(format!("{model_id}: {error:#}")),
        }
    }

    if report.entries.is_empty() {
        bail!(
            "No catalog entries generated ({} errors, {} skipped).",
            report.errors.len(),
            report.skipped.len()
        );
    }

    report.entries.sort_by(|a, b| {
        a.params_b
            .total_cmp(&b.params_b)
            .then_with(|| a.id.to_lowercase().cmp(&b.id.to_lowercase()))
    });

    Ok(report)
}

fn fetch_and_build_entry(
    client: &reqwest::blocking::Client,
    options: &RefreshOptions,
    model_id: &str,
) -> Result<Option<ModelEntry>> {
    let meta = fetch_model_metadata(client, options, model_id)?;
    let config_data = fetch_model_config(client, options, model_id);

    if !options.allow_non_text_generation && !is_text_generation_model(&meta, model_id) {
        return Ok(None);
    }

    Ok(Some(build_catalog_entry(&meta, &config_data, model_id)))
}

fn build_client(timeout_secs: u64) -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs.max(1)))
        .user_agent(USER_AGENT)
        .build()
        .context("Unable to build HTTP client for Hugging Face API")
}

fn http_get_json(
    client: &reqwest::blocking::Client,
    options: &RefreshOptions,
    url: &str,
) -> Result<Value> {
    let mut request = client.get(url).header("Accept", "application/json");
    if let Some(token) = &options.token {
        request = request.bearer_auth(token);
    }

    let response = request
        .send()
        .with_context(|| format!("Network error for {url}"))?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().unwrap_or_default();
        let preview: String = body.chars().take(220).collect();
        bail!("HTTP {status} for {url} :: {preview}");
    }

    response
        .json::<Value>()
        .with_context(|| format!("Invalid JSON payload from {url}"))
}

fn discover_model_ids(
    client: &reqwest::blocking::Client,
    options: &RefreshOptions,
) -> Result<Vec<String>> {
    let attempt = |sort_field: &str| -> Result<Value> {
        let mut url = format!(
            "{HF_API_BASE}/models?limit={}&sort={}&direction=-1&pipeline_tag=text-generation&full=true",
            options.discover_limit, sort_field
        );
        if let Some(search) = &options.search
            && !search.trim().is_empty()
        {
            url.push_str("&search=");
            url.push_str(&urlencode(search.trim()));
        }
        http_get_json(client, options, &url)
    };

    let payload = match attempt(options.discover_sort.api_field()) {
        Ok(payload) => payload,
        // Fallback if trending sort is unavailable in the current HF API behavior.
        Err(_) if options.discover_sort == DiscoverSort::Trending => attempt("downloads")?,
        Err(error) => return Err(error),
    };

    let Value::Array(items) = payload else {
        return Ok(Vec::new());
    };
    Ok(items
        .iter()
        .filter_map(|item| item.get("id").and_then(Value::as_str))
        .map(str::to_owned)
        .collect())
}

fn fetch_model_metadata(
    client: &reqwest::blocking::Client,
    options: &RefreshOptions,
    model_id: &str,
) -> Result<Map<String, Value>> {
    let url = format!("{HF_API_BASE}/models/{model_id}?full=true");
    match http_get_json(client, options, &url)? {
        Value::Object(map) => Ok(map),
        _ => Err(anyhow!("Invalid model metadata payload from HF API.")),
    }
}

fn fetch_model_config(
    client: &reqwest::blocking::Client,
    options: &RefreshOptions,
    model_id: &str,
) -> Map<String, Value> {
    let url = format!("{HF_MODEL_BASE}/{model_id}/raw/main/config.json");
    match http_get_json(client, options, &url) {
        Ok(Value::Object(map)) => map,
        _ => Map::new(),
    }
}

fn urlencode(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                output.push(byte as char);
            }
            _ => output.push_str(&format!("%{byte:02X}")),
        }
    }
    output
}

// ---------------------------------------------------------------------------
// Entry construction (pure logic, unit-tested)
// ---------------------------------------------------------------------------

fn build_catalog_entry(
    meta: &Map<String, Value>,
    config_data: &Map<String, Value>,
    fallback_id: &str,
) -> ModelEntry {
    let model_id = meta
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or(fallback_id)
        .to_owned();
    let tags = normalize_tags(meta.get("tags"));
    let pipeline_tag = meta
        .get("pipeline_tag")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    let empty = Map::new();
    let card_data = meta
        .get("cardData")
        .and_then(Value::as_object)
        .unwrap_or(&empty);

    let params_b = estimate_params_b(&model_id, &tags, card_data, config_data);
    let categories = infer_categories(&model_id, &tags, &pipeline_tag, card_data, params_b);
    let family = infer_family(&model_id);
    let display_name = infer_display_name(&model_id, card_data);
    let recommended_use = infer_recommended_use(&categories);
    let (fp16, int8, int4) = estimate_vram_gb(params_b);
    let (supports_vllm, supports_tgi) =
        infer_backend_support(&categories, &pipeline_tag, &tags, &model_id);
    let gated = match meta.get("gated") {
        Some(Value::Bool(value)) => *value,
        Some(Value::String(value)) => !value.is_empty() && value != "false",
        _ => false,
    };

    ModelEntry {
        id: model_id,
        display_name,
        family,
        params_b: params_b as f32,
        categories,
        recommended_use,
        min_vram_gb_fp16: fp16,
        min_vram_gb_int8: int8,
        min_vram_gb_int4: int4,
        supports_vllm,
        supports_tgi,
        gated,
    }
}

fn normalize_tags(value: Option<&Value>) -> Vec<String> {
    let Some(Value::Array(items)) = value else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| item.as_str())
        .map(|tag| tag.trim().to_lowercase())
        .filter(|tag| !tag.is_empty())
        .collect()
}

fn is_text_generation_model(meta: &Map<String, Value>, fallback_id: &str) -> bool {
    let pipeline_tag = meta
        .get("pipeline_tag")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    let tags = normalize_tags(meta.get("tags"));
    let probe = build_probe(fallback_id, &pipeline_tag, &tags);

    if contains_any_token(&probe, &["gguf", "ggml"]) {
        return false;
    }
    if pipeline_tag == "text-generation" {
        return true;
    }
    contains_any_token(
        &probe,
        &[
            "text-generation",
            "text2text-generation",
            "conversational",
            "instruct",
            "chat",
            "causal-lm",
            "causal language model",
        ],
    )
}

fn build_probe(model_id: &str, pipeline_tag: &str, tags: &[String]) -> String {
    let mut probe = model_id.to_lowercase();
    probe.push(' ');
    probe.push_str(pipeline_tag);
    for tag in tags {
        probe.push(' ');
        probe.push_str(tag);
    }
    probe
}

fn infer_family(model_id: &str) -> String {
    let probe = model_id.to_lowercase();
    let mapping = [
        ("qwen", "qwen"),
        ("mistral", "mistral"),
        ("llama", "llama"),
        ("gemma", "gemma"),
        ("deepseek", "deepseek"),
        ("phi", "phi"),
        ("mixtral", "mistral"),
        ("command-r", "cohere"),
        ("yi-", "yi"),
        ("olmo", "olmo"),
        ("gpt", "gpt"),
    ];
    for (token, family) in mapping {
        if probe.contains(token) {
            return family.to_owned();
        }
    }

    let namespace = model_id.split('/').next().unwrap_or("").to_lowercase();
    let sanitized: String = namespace
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect();
    if sanitized.is_empty() {
        "unknown".to_owned()
    } else {
        sanitized
    }
}

fn infer_display_name(model_id: &str, card_data: &Map<String, Value>) -> String {
    for key in ["model_name", "name", "title"] {
        if let Some(value) = card_data.get(key).and_then(Value::as_str) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return trimmed.to_owned();
            }
        }
    }

    let slug = model_id.split('/').next_back().unwrap_or(model_id);
    let pretty: Vec<String> = slug
        .split(['-', '_'])
        .filter(|word| !word.is_empty())
        .map(|word| {
            if word.chars().any(|ch| ch.is_ascii_digit()) {
                word.to_owned()
            } else {
                capitalize(word)
            }
        })
        .collect();
    if pretty.is_empty() {
        slug.to_owned()
    } else {
        pretty.join(" ")
    }
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn estimate_params_b(
    model_id: &str,
    tags: &[String],
    card_data: &Map<String, Value>,
    config_data: &Map<String, Value>,
) -> f64 {
    let mut explicit_candidates: Vec<f64> = Vec::new();
    for source in [card_data, config_data] {
        for key in PARAM_KEYS {
            if let Some(value) = source.get(key)
                && let Some(parsed) = parse_params_value(value)
            {
                explicit_candidates.push(parsed);
            }
        }
    }
    for tag in tags {
        if let Some(parsed) = parse_params_text(tag) {
            explicit_candidates.push(parsed);
        }
    }
    if let Some(parsed) = parse_params_text(model_id) {
        explicit_candidates.push(parsed);
    }

    // For MoE names such as `30B-A3B`, keep total parameters rather than active
    // parameters because deployment memory is closer to total weights.
    let explicit_max = explicit_candidates
        .into_iter()
        .filter(|value| (0.1..=5000.0).contains(value))
        .fold(None, |acc: Option<f64>, value| {
            Some(acc.map_or(value, |current| current.max(value)))
        });
    if let Some(value) = explicit_max {
        return round1(value);
    }

    if let Some(estimated) = estimate_transformer_params_b(config_data)
        && (0.1..=5000.0).contains(&estimated)
    {
        return round1(estimated);
    }

    UNKNOWN_PARAMS_FALLBACK_B
}

fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn parse_params_value(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => {
            let numeric = number.as_f64()?;
            if numeric > 10_000.0 {
                // Probably a raw parameter count.
                Some(numeric / 1_000_000_000.0)
            } else {
                Some(numeric)
            }
        }
        Value::String(text) => parse_params_text(text),
        _ => None,
    }
}

/// Extract parameter sizes such as `7b`, `0.5B`, `1.5 B`, `30b-a3b`, `1t`, `350m`
/// from free-form text, returning the largest candidate in billions.
fn parse_params_text(value: &str) -> Option<f64> {
    let text: Vec<char> = value
        .trim()
        .to_lowercase()
        .replace(',', "")
        .chars()
        .collect();
    let is_alnum = |ch: char| ch.is_ascii_alphanumeric();

    let mut candidates: Vec<f64> = Vec::new();
    let mut index = 0usize;
    while index < text.len() {
        let start = index;
        // Optional `a` prefix (MoE active-params notation such as `a3b`).
        let mut cursor = index;
        if text[cursor] == 'a' {
            cursor += 1;
        }
        if cursor >= text.len() || !text[cursor].is_ascii_digit() {
            index += 1;
            continue;
        }
        // Boundary: the char before the match must not be alphanumeric.
        if start > 0 && is_alnum(text[start - 1]) {
            index += 1;
            continue;
        }

        let digits_start = cursor;
        while cursor < text.len() && text[cursor].is_ascii_digit() {
            cursor += 1;
        }
        // Optional fractional part separated by `.` or `_`.
        let mut fraction = String::new();
        if cursor + 1 < text.len()
            && matches!(text[cursor], '.' | '_')
            && text[cursor + 1].is_ascii_digit()
        {
            cursor += 1;
            while cursor < text.len() && text[cursor].is_ascii_digit() {
                fraction.push(text[cursor]);
                cursor += 1;
            }
        }

        let integer: String = text[digits_start..cursor.min(text.len())]
            .iter()
            .filter(|ch| ch.is_ascii_digit())
            .collect();
        let numeric_text = if fraction.is_empty() {
            integer
        } else {
            let integer_only: String = text[digits_start..]
                .iter()
                .take_while(|ch| ch.is_ascii_digit())
                .collect();
            format!("{integer_only}.{fraction}")
        };
        let Ok(numeric) = numeric_text.parse::<f64>() else {
            index = cursor.max(index + 1);
            continue;
        };

        // Skip whitespace between the number and the unit.
        let mut unit_index = cursor;
        while unit_index < text.len() && text[unit_index].is_whitespace() {
            unit_index += 1;
        }
        if unit_index < text.len() {
            let unit = text[unit_index];
            let after_ok = unit_index + 1 >= text.len() || !is_alnum(text[unit_index + 1]);
            if after_ok {
                match unit {
                    'b' => candidates.push(numeric),
                    'm' => candidates.push(numeric / 1000.0),
                    't' => candidates.push(numeric * 1000.0),
                    _ => {}
                }
            }
        }

        index = cursor.max(index + 1);
    }

    candidates.into_iter().fold(None, |acc, value| {
        Some(acc.map_or(value, |current: f64| current.max(value)))
    })
}

fn estimate_transformer_params_b(config_data: &Map<String, Value>) -> Option<f64> {
    let layers = get_numeric_config(
        config_data,
        &["num_hidden_layers", "n_layer", "n_layers", "num_layers"],
    )?;
    let hidden = get_numeric_config(config_data, &["hidden_size", "n_embd", "d_model", "dim"])?;
    let vocab = get_numeric_config(config_data, &["vocab_size"]);

    let heads = get_numeric_config(config_data, &["num_attention_heads", "n_head", "num_heads"]);
    let kv_heads = get_numeric_config(config_data, &["num_key_value_heads", "n_head_kv"]);
    let attention_params = match (heads, kv_heads) {
        (Some(heads), Some(kv_heads)) if heads > 0.0 => {
            let kv_dim = hidden * (kv_heads / heads);
            2.0 * hidden * hidden + 2.0 * hidden * kv_dim
        }
        _ => 4.0 * hidden * hidden,
    };

    let dense_intermediate = get_numeric_config(
        config_data,
        &["intermediate_size", "ffn_dim", "n_inner", "hidden_dim"],
    );
    let moe_intermediate = get_numeric_config(config_data, &["moe_intermediate_size"]);
    let experts = get_numeric_config(
        config_data,
        &["n_routed_experts", "num_local_experts", "num_experts"],
    );
    let shared_experts =
        get_numeric_config(config_data, &["n_shared_experts", "num_shared_experts"]).unwrap_or(0.0);
    let first_dense_layers =
        get_numeric_config(config_data, &["first_k_dense_replace"]).unwrap_or(0.0);

    let mlp_params = match (experts, moe_intermediate) {
        (Some(experts), Some(moe_intermediate)) => {
            let dense_layers = layers.min(first_dense_layers);
            let moe_layers = (layers - dense_layers).max(0.0);
            let dense_width = dense_intermediate.unwrap_or(moe_intermediate);
            let dense_mlp = dense_layers * 3.0 * hidden * dense_width;
            let moe_mlp = moe_layers * (experts + shared_experts) * 3.0 * hidden * moe_intermediate;
            dense_mlp + moe_mlp
        }
        _ => {
            let intermediate = dense_intermediate.unwrap_or(hidden * 4.0);
            layers * 3.0 * hidden * intermediate
        }
    };

    let embedding_params = vocab.unwrap_or(0.0) * hidden;
    let output_params = match (vocab, config_data.get("tie_word_embeddings")) {
        (Some(vocab), Some(Value::Bool(false))) => vocab * hidden,
        _ => 0.0,
    };

    let total = embedding_params + output_params + layers * attention_params + mlp_params;
    if total <= 0.0 {
        return None;
    }
    Some(total / 1_000_000_000.0)
}

fn get_numeric_config(config_data: &Map<String, Value>, keys: &[&str]) -> Option<f64> {
    for key in keys {
        match config_data.get(*key) {
            Some(Value::Bool(_)) | None => continue,
            Some(Value::Number(number)) => {
                if let Some(value) = number.as_f64() {
                    return Some(value);
                }
            }
            Some(Value::String(text)) => {
                if let Ok(value) = text.trim().parse::<f64>() {
                    return Some(value);
                }
            }
            _ => continue,
        }
    }
    None
}

fn infer_categories(
    model_id: &str,
    tags: &[String],
    pipeline_tag: &str,
    card_data: &Map<String, Value>,
    params_b: f64,
) -> Vec<String> {
    let probe = build_probe(model_id, pipeline_tag, tags);
    let mut categories: Vec<String> = Vec::new();

    if contains_any_token(
        &probe,
        &["instruct", "chat", "assistant", "text-generation"],
    ) {
        categories.push("chat".to_owned());
    }
    if contains_any_token(&probe, &["code", "coder", "programming"]) {
        categories.push("code".to_owned());
    }
    if contains_any_token(&probe, &["reason", "r1", "math", "thinking", "think"]) {
        categories.push("reasoning".to_owned());
    }
    if infer_multilingual(tags, card_data, &probe) {
        categories.push("multilingual".to_owned());
    }
    if (0.1..=8.0).contains(&params_b) {
        categories.push("lightweight".to_owned());
    }
    if contains_any_token(
        &probe,
        &["32k", "64k", "128k", "long-context", "long context"],
    ) {
        categories.push("long-context".to_owned());
    }
    if contains_any_token(
        &probe,
        &[
            "embedding",
            "feature-extraction",
            "sentence-similarity",
            "text-embeddings",
        ],
    ) {
        categories.push("embeddings".to_owned());
    }
    if contains_any_token(
        &probe,
        &["vision", "multimodal", "vl", "image-text-to-text", "llava"],
    ) {
        categories.push("vision/multimodal".to_owned());
    }

    if categories.is_empty() {
        categories.push("general".to_owned());
    }

    let order = [
        "chat",
        "code",
        "reasoning",
        "multilingual",
        "lightweight",
        "long-context",
        "embeddings",
        "vision/multimodal",
        "general",
    ];
    let mut deduped = deduplicate_keep_order(categories);
    deduped.sort_by_key(|item| {
        order
            .iter()
            .position(|known| known == item)
            .unwrap_or(order.len())
    });
    deduped
}

fn infer_multilingual(tags: &[String], card_data: &Map<String, Value>, probe: &str) -> bool {
    if contains_token(probe, "multilingual") {
        return true;
    }
    extract_languages(tags, card_data).len() >= 2
}

fn extract_languages(tags: &[String], card_data: &Map<String, Value>) -> Vec<String> {
    let known_codes = [
        "en", "fr", "de", "es", "it", "pt", "zh", "ja", "ko", "ar", "ru",
    ];
    let mut languages: Vec<String> = Vec::new();
    let mut push = |value: &str| {
        let normalized = value.trim().to_lowercase();
        if !normalized.is_empty() && !languages.contains(&normalized) {
            languages.push(normalized);
        }
    };

    match card_data.get("language") {
        Some(Value::String(value)) => push(value),
        Some(Value::Array(items)) => {
            for item in items {
                if let Some(value) = item.as_str() {
                    push(value);
                }
            }
        }
        _ => {}
    }

    for tag in tags {
        if let Some(code) = tag.strip_prefix("language:") {
            push(code);
        } else if known_codes.contains(&tag.as_str()) {
            push(tag);
        }
    }

    languages
}

fn infer_recommended_use(categories: &[String]) -> String {
    let has = |name: &str| categories.iter().any(|item| item == name);
    if has("embeddings") {
        return "Text embeddings and semantic search".to_owned();
    }
    if has("vision/multimodal") {
        return "Vision and multimodal assistant tasks".to_owned();
    }
    if has("code") && has("chat") {
        return "Coding assistant and developer chat".to_owned();
    }
    if has("reasoning") {
        return "Reasoning-focused assistant".to_owned();
    }
    if has("chat") {
        return "General-purpose assistant".to_owned();
    }
    "General language model inference".to_owned()
}

fn estimate_vram_gb(params_b: f64) -> (f32, f32, f32) {
    let fp16 = (params_b * 2.0 + 2.0).ceil().max(4.0);
    let int8 = (fp16 * 0.62).ceil().max(3.0);
    let int4 = (fp16 * 0.37).ceil().max(2.0);
    (fp16 as f32, int8 as f32, int4 as f32)
}

fn infer_backend_support(
    categories: &[String],
    pipeline_tag: &str,
    tags: &[String],
    model_id: &str,
) -> (bool, bool) {
    let probe = build_probe(model_id, pipeline_tag, tags);
    if contains_token(&probe, "gguf") {
        return (false, false);
    }
    if categories.iter().any(|item| item == "embeddings")
        || categories.iter().any(|item| item == "vision/multimodal")
    {
        return (false, false);
    }
    if contains_any_token(
        &probe,
        &[
            "text-generation",
            "conversational",
            "text2text-generation",
            "instruct",
            "chat",
        ],
    ) {
        return (true, true);
    }
    (false, false)
}

fn contains_any_token(probe: &str, keywords: &[&str]) -> bool {
    keywords
        .iter()
        .any(|keyword| contains_token(probe, keyword))
}

/// `true` when `keyword` appears in `probe`. Purely alphanumeric keywords must
/// be delimited by non-alphanumeric boundaries (so `chat` does not match
/// `chatter`); other keywords use plain substring search.
fn contains_token(probe: &str, keyword: &str) -> bool {
    let key = keyword.to_lowercase();
    if key.is_empty() {
        return false;
    }
    let strict_boundaries = key.chars().all(|ch| ch.is_ascii_alphanumeric());
    if !strict_boundaries {
        return probe.contains(&key);
    }

    let probe_chars: Vec<char> = probe.chars().collect();
    let key_chars: Vec<char> = key.chars().collect();
    let is_alnum = |ch: char| ch.is_ascii_alphanumeric();

    let mut start = 0usize;
    while start + key_chars.len() <= probe_chars.len() {
        if probe_chars[start..start + key_chars.len()] == key_chars[..] {
            let before_ok = start == 0 || !is_alnum(probe_chars[start - 1]);
            let after_index = start + key_chars.len();
            let after_ok = after_index >= probe_chars.len() || !is_alnum(probe_chars[after_index]);
            if before_ok && after_ok {
                return true;
            }
        }
        start += 1;
    }
    false
}

fn deduplicate_keep_order(items: Vec<String>) -> Vec<String> {
    let mut seen: Vec<String> = Vec::with_capacity(items.len());
    for item in items {
        if !seen.contains(&item) {
            seen.push(item);
        }
    }
    seen
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn as_map(value: Value) -> Map<String, Value> {
        match value {
            Value::Object(map) => map,
            _ => panic!("expected object"),
        }
    }

    #[test]
    fn parse_params_text_extracts_sizes_in_billions() {
        assert_eq!(parse_params_text("Qwen/Qwen2.5-7B-Instruct"), Some(7.0));
        assert_eq!(parse_params_text("0.5b"), Some(0.5));
        assert_eq!(parse_params_text("350m"), Some(0.35));
        assert_eq!(parse_params_text("1t"), Some(1000.0));
        assert_eq!(parse_params_text("no size here"), None);
    }

    #[test]
    fn parse_params_text_prefers_total_params_for_moe_names() {
        // `30B-A3B` exposes total (30B) and active (3B) parameters.
        assert_eq!(parse_params_text("Qwen3-30B-A3B"), Some(30.0));
    }

    #[test]
    fn parse_params_text_requires_token_boundaries() {
        // `8bit` must not be read as an 8B parameter size.
        assert_eq!(parse_params_text("model-8bit-quantized"), None);
    }

    #[test]
    fn parse_params_value_converts_raw_counts() {
        assert_eq!(parse_params_value(&json!(7_000_000_000.0_f64)), Some(7.0));
        assert_eq!(parse_params_value(&json!(13.5)), Some(13.5));
    }

    #[test]
    fn estimate_params_b_prefers_largest_explicit_value() {
        let card = Map::new();
        let config = Map::new();
        let params = estimate_params_b("org/Model-30B-A3B", &[], &card, &config);
        assert_eq!(params, 30.0);
    }

    #[test]
    fn estimate_params_b_falls_back_to_config_estimate() {
        let card = Map::new();
        let config = as_map(json!({
            "num_hidden_layers": 32,
            "hidden_size": 4096,
            "vocab_size": 32000,
            "intermediate_size": 11008,
            "num_attention_heads": 32,
            "num_key_value_heads": 32,
            "tie_word_embeddings": false
        }));
        let params = estimate_params_b("org/some-model", &[], &card, &config);
        // Approximation of a Llama-2-7B style architecture.
        assert!((6.0..=8.5).contains(&params), "got {params}");
    }

    #[test]
    fn estimate_params_b_uses_fallback_when_unknown() {
        let params = estimate_params_b("org/mystery-model", &[], &Map::new(), &Map::new());
        assert_eq!(params, UNKNOWN_PARAMS_FALLBACK_B);
    }

    #[test]
    fn infer_categories_orders_and_flags_lightweight() {
        let categories = infer_categories(
            "org/Tiny-Chat-3B-Instruct",
            &["text-generation".to_owned()],
            "text-generation",
            &Map::new(),
            3.0,
        );
        assert_eq!(categories[0], "chat");
        assert!(categories.contains(&"lightweight".to_owned()));
    }

    #[test]
    fn gguf_models_are_not_text_generation_targets() {
        let meta = as_map(json!({
            "pipeline_tag": "text-generation",
            "tags": ["gguf"]
        }));
        assert!(!is_text_generation_model(&meta, "org/model-gguf"));
    }

    #[test]
    fn backend_support_requires_text_generation_signals() {
        let (vllm, tgi) = infer_backend_support(
            &["chat".to_owned()],
            "text-generation",
            &[],
            "org/chat-model",
        );
        assert!(vllm && tgi);

        let (vllm, tgi) = infer_backend_support(
            &["embeddings".to_owned()],
            "feature-extraction",
            &[],
            "org/embedding-model",
        );
        assert!(!vllm && !tgi);
    }

    #[test]
    fn estimate_vram_scales_with_params() {
        let (fp16, int8, int4) = estimate_vram_gb(7.0);
        assert_eq!(fp16, 16.0);
        assert_eq!(int8, 10.0);
        assert_eq!(int4, 6.0);
    }

    #[test]
    fn contains_token_respects_boundaries() {
        assert!(contains_token("nice chat model", "chat"));
        assert!(!contains_token("chatter model", "chat"));
        assert!(contains_token("model-7b-chat", "chat"));
    }
}
