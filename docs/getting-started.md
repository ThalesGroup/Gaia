# Getting Started

This guide gets you from source code to a running OpenAI-compatible endpoint.

## Prerequisites

- Linux or macOS (Linux recommended for GPU workloads)
- Rust toolchain (`cargo`, `rustc`)
- Docker engine
- NVIDIA drivers + NVIDIA Container Toolkit (for GPU backends such as vLLM, TGI, SGLang)
- Optional:
  - `HF_TOKEN` for gated Hugging Face models

Security profiles:

- `dev` (default): easiest local workflow, API key can be auto-generated.
- `prod`: strict mode, explicit API key required and gated models require `HF_TOKEN`.

## Build And Install

From the repository root:

```bash
cargo build --release -p gaia-cli
```

Binary path:

```bash
./target/release/gaia
```

Optional install helper:

```bash
./install.sh
```

By default, this installs `gaia` into `~/.local/bin`.

## First Validation

Run:

```bash
gaia doctor
```

`doctor` prints:

- machine information (OS, CPU, RAM, GPU)
- Docker availability and daemon reachability
- `HF_TOKEN` presence
- backend availability table
- recommended models for your machine

## Quick Interactive Workflow

Use the TUI wizard:

```bash
gaia select
```

Typical flow:

1. Browse and filter models.
2. Switch backend (`b`) if needed.
3. Set API port and key.
4. Confirm launch.

When the launch succeeds, `gaia` prints:

- backend/model summary
- API base URL and chat endpoint
- Python, JavaScript, and curl snippets

## Mock Mode End-To-End (No GPU Required)

Use mock mode to validate API integration quickly.

The mock server exposes OpenAI-style endpoints for:

- `/v1/models`
- `/v1/chat/completions`
- `/v1/completions`
- `/v1/responses`
- `/v1/embeddings`
- `/v1/audio/transcriptions`
- `/v1/audio/speech`

Start API:

```bash
gaia serve --mock --detach --host 0.0.0.0 --port 8000
```

Open:

- API health: `http://localhost:8000/health`

Stop mock API:

```bash
gaia stop --mock
```

## Real Backend Workflow

Example with vLLM:

```bash
gaia serve \
  --security-profile prod \
  --backend vllm \
  --model Qwen/Qwen2.5-7B-Instruct \
  --model-revision 0123456789abcdef0123456789abcdef01234567 \
  --host 0.0.0.0 \
  --port 8000 \
  --api-key "$GAIA_API_KEY" \
  --detach
```

If the selected model is gated, export `HF_TOKEN` first:

```bash
export HF_TOKEN=hf_xxx
```

Check runtime state:

```bash
gaia status
gaia logs --lines 200
```

Stop container:

```bash
gaia stop
```

## Configuration File

`gaia` stores defaults in:

```text
~/.config/gaia/config.toml
```

The file keeps:

- server host/port/api key defaults
- security profile (`dev` or `prod`)
- selected backend
- selected model, revision, and quantization settings

Most commands can override these values with CLI flags.
