# Model Catalog

`gaia` uses a local YAML catalog at:

```text
catalog/models.yaml
```

## Schema

Each model entry includes:

- `id`: Hugging Face model id
- `display_name`: human-readable label
- `family`: model family name
- `params_b`: parameter count (billions)
- `categories`: list of tags used for filtering
- `recommended_use`: short usage hint
- `min_vram_gb_fp16`: VRAM target for FP16
- `min_vram_gb_int8`: VRAM target for INT8
- `min_vram_gb_int4`: VRAM target for INT4
- `supports_vllm`: backend support flag
- `supports_tgi`: backend support flag
- `gated`: whether HF access is gated

## How Filtering Works

The `models` command filters by:

- category (`--category`)
- max parameter count (`--max-params`)
- backend compatibility (`--backend`)

Recommendation ranking uses machine detection + VRAM/RAM heuristics.

## Backend Compatibility Behavior

Current compatibility mapping used by `gaia`:

- `vllm` -> `supports_vllm`
- `tgi` -> `supports_tgi`
- `sglang` -> same compatibility gate as `supports_vllm`
- `llamacpp` / `llama.cpp` -> treated as broadly compatible
- `ollama` -> treated as broadly compatible

Use this when curating or generating catalog entries.

## Auto-Prefill From Hugging Face API

Use:

```bash
python3 hf_catalog_prefill.py --output catalog/models.generated.yaml
```

Useful modes:

```bash
# Add top N discovered text-generation models to existing ids
python3 hf_catalog_prefill.py --discover-limit 20 --output catalog/models.generated.yaml

# Only discovered models, no existing catalog seed
python3 hf_catalog_prefill.py --no-existing --discover-limit 50 --output catalog/models.generated.yaml

# Force downloads ranking instead of trending
python3 hf_catalog_prefill.py --discover-sort downloads --discover-limit 30 --output catalog/models.generated.yaml

# Explicit list
python3 hf_catalog_prefill.py --no-existing \
  --ids Qwen/Qwen2.5-7B-Instruct mistralai/Mistral-7B-Instruct-v0.3 \
  --output catalog/models.generated.yaml
```

Then review and promote:

```bash
cp catalog/models.generated.yaml catalog/models.yaml
```

## Script Defaults

`hf_catalog_prefill.py` defaults to:

- text-generation discovery pipeline
- existing catalog seed enabled (unless `--no-existing`)
- output file `catalog/models.generated.yaml`
- token from `HF_TOKEN` if available

## Curation Tips

- Keep categories stable and lowercase for predictable filtering.
- Verify backend support booleans before large updates.
- Re-check `gated` models and ensure deployment docs include `HF_TOKEN`.
- For production, pin catalog snapshots in version control and review changes before rollout.
