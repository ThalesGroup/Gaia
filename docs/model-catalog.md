# Model Catalog

`gaia` uses a local YAML catalog at:

```text
catalog/models.yaml
```

This is the **active curated catalog** used by `gaia models`, `gaia recommend`, and the TUI.

The repository also includes `catalog/models.generated.yaml` as an **example output** from
`gaia catalog refresh`. It is not the active catalog: review it and run `gaia catalog promote`
to replace `models.yaml`, or regenerate it locally when refreshing metadata from Hugging Face.

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

## Refresh From Hugging Face API (`gaia catalog`)

Use:

```bash
gaia catalog refresh
```

Useful modes:

```bash
# Add top N discovered text-generation models to existing ids
gaia catalog refresh --discover-limit 20

# Only discovered models, no existing catalog seed
gaia catalog refresh --no-existing --discover-limit 50

# Force downloads ranking instead of trending
gaia catalog refresh --sort downloads --discover-limit 30

# Explicit list
gaia catalog refresh --no-existing \
  --ids Qwen/Qwen2.5-7B-Instruct mistralai/Mistral-7B-Instruct-v0.3

# Preview without writing a file
gaia catalog refresh --dry-run
```

Then review the generated file and promote it:

```bash
gaia catalog promote
```

`promote` validates the generated file before replacing `catalog/models.yaml`.

## Refresh Defaults

`gaia catalog refresh` defaults to:

- text-generation discovery pipeline
- existing catalog seed from `catalog/models.yaml` (unless `--no-existing`)
- output file `catalog/models.generated.yaml`
- token from `HF_TOKEN` if available

Parameter sizes are estimated in this order: explicit values from metadata and
model names (largest plausible value wins, so MoE names like `30B-A3B` resolve
to total parameters), then a transformer estimate derived from `config.json`,
then a conservative fallback.

## Refresh From The TUI

In `gaia select`, press `r` to refresh the catalog in the background (current
catalog ids + top trending models). The refreshed catalog applies to the
current session only; run `gaia catalog refresh` + `gaia catalog promote` to
persist changes.

## Curation Tips

- Keep categories stable and lowercase for predictable filtering.
- Verify backend support booleans before large updates.
- Re-check `gated` models and ensure deployment docs include `HF_TOKEN`.
- For production, pin catalog snapshots in version control and review changes before rollout.
