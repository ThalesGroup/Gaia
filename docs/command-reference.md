# Command Reference

This document covers all public `gaia` commands.

## Global

```bash
gaia --help
gaia --version
```

## `doctor`

Inspect machine readiness and backend availability.

```bash
gaia doctor
```

Use this first on any new machine.

## `models`

List models from `catalog/models.yaml` with filters.

```bash
gaia models [--category <CATEGORY>] [--max-params <N>] [--backend <BACKEND>] [--recommended-only]
```

Examples:

```bash
gaia models
gaia models --category instruct
gaia models --max-params 14
gaia models --backend vllm --recommended-only
```

## `recommend`

Show ranked model recommendations based on detected hardware.

```bash
gaia recommend [--backend <BACKEND>] [--top <N>]
```

Examples:

```bash
gaia recommend
gaia recommend --backend tgi --top 12
```

## `select`

Launch the interactive TUI wizard.

```bash
gaia select [--mock]
```

Notes:

- Requires an interactive TTY terminal.
- `--mock` launches the mock API instead of a real backend after confirmation.
- Mock API endpoints include `/v1/models`, `/v1/chat/completions`, `/v1/completions`, `/v1/responses`, `/v1/embeddings`, `/v1/audio/transcriptions`, and `/v1/audio/speech`.

## `serve`

Launch backend serving directly from CLI flags + config defaults.

```bash
gaia serve [OPTIONS]
```

Main options:

- `--backend <BACKEND>`
- `--model <MODEL>`
- `--model-revision <COMMIT_SHA>` (must be a 40-char immutable commit SHA)
- `--security-profile <dev|prod>`
- `--host <HOST>`
- `--port <PORT>`
- `--api-key <API_KEY>`
- `--dtype <DTYPE>`
- `--quantization <QUANTIZATION>`
- `--quantization-profile <none|quality|balanced|memory|speed>`
- `--max-model-len <TOKENS>`
- `--detach`
- `--dry-run`
- `--force`
- `--no-save`
- `--mock`

Examples:

```bash
gaia serve --backend vllm --model Qwen/Qwen2.5-7B-Instruct --port 8000 --detach
gaia serve --security-profile prod --backend vllm --model Qwen/Qwen2.5-7B-Instruct --api-key "$GAIA_API_KEY" --detach
gaia serve --backend vllm --model Qwen/Qwen2.5-7B-Instruct --model-revision 0123456789abcdef0123456789abcdef01234567 --detach
gaia serve --backend sglang --model Qwen/Qwen2.5-7B-Instruct --quantization-profile speed --detach
gaia serve --mock --port 8000 --detach
gaia serve --dry-run --backend tgi --model mistralai/Mistral-7B-Instruct-v0.3
```

Security profile behavior:

- `dev` (default): user-friendly local defaults; API key may be auto-generated and saved.
- `prod`: strict mode; explicit API key is required (`--api-key` or `GAIA_API_KEY`), and gated models require `HF_TOKEN`.

OpenAI compatibility scope:

- `gaia` provides an OpenAI-style base URL/auth flow and targets `chat/completions` for real backends.
- strict endpoint-by-endpoint parity is backend-dependent outside the Gaia mock server.
- see `docs/openai-compatibility.md` for the compatibility matrix.

## `stop`

Stop the active backend container or mock API process.

```bash
gaia stop [--container <NAME>] [--mock]
```

Examples:

```bash
gaia stop
gaia stop --container gaia-vllm-qwen-qwen2-5-7b-instruct
gaia stop --mock
```

## `status`

Show running `gaia` Docker containers and mock API status.

```bash
gaia status
```

## `logs`

Show container logs or mock API logs.

```bash
gaia logs [--container <NAME>] [-f|--follow] [--lines <N>] [--mock]
```

Examples:

```bash
gaia logs --lines 100
gaia logs --follow
gaia logs --mock --lines 200
```

## `generate-compose`

Generate a Docker Compose file for one backend/model.

Notes:

- Backend images are pinned by digest.
- Chatbot service is built once from a Dockerfile and runs without `npm install` at runtime.
- Generated backend services run with non-root user, read-only root filesystem, and dropped Linux capabilities.
- Export `GAIA_API_KEY` (and optional `HF_TOKEN`) before `docker compose up`.

```bash
gaia generate-compose [--backend <BACKEND>] [--model <MODEL>] [--model-revision <COMMIT_SHA>] [--security-profile <dev|prod>] [--output <PATH>] [--with-chatbot]
```

Examples:

```bash
gaia generate-compose --backend tgi --model mistralai/Mistral-7B-Instruct-v0.3
gaia generate-compose --security-profile prod --backend vllm --model Qwen/Qwen2.5-7B-Instruct
gaia generate-compose --with-chatbot --output docker-compose.local.yml
```

## `generate-k8s`

Generate Kubernetes Deployment + Service manifests.

```bash
gaia generate-k8s [--backend <BACKEND>] [--model <MODEL>] [--model-revision <COMMIT_SHA>] [--namespace <NS>] [--replicas <N>] [--output <PATH>]
```

Examples:

```bash
gaia generate-k8s --backend vllm --model Qwen/Qwen2.5-7B-Instruct
gaia generate-k8s --backend sglang --replicas 2 --namespace llm
```

The generated manifests include pinned image digests and baseline container security context hardening.
They also expect a Kubernetes secret named `gaia-secrets` containing `api-key` (required) and `hf-token` (optional).

## `generate-systemd`

Generate a `systemd` unit that runs `gaia serve`.

```bash
gaia generate-systemd [--backend <BACKEND>] [--model <MODEL>] [--model-revision <COMMIT_SHA>] [--user <USER>] [--output <PATH>] [--mock]
```

Examples:

```bash
gaia generate-systemd --backend ollama --model mistralai/Mistral-7B-Instruct-v0.3
gaia generate-systemd --mock --output gaia-mock.service
```

The generated unit reads `GAIA_API_KEY` and optional `HF_TOKEN` from `/etc/gaia/gaia.env` to avoid putting secrets in process args.

## `benchmark`

Run latency/throughput benchmark against an OpenAI-compatible endpoint.

```bash
gaia benchmark [--base-url <URL>] [--api-key <KEY>] [--model <MODEL>] [--requests <N>] [--prompt <TEXT>] [--mock]
```

Examples:

```bash
gaia benchmark
gaia benchmark --requests 30 --prompt "Summarize Rust ownership in one sentence."
gaia benchmark --base-url http://10.0.0.20:8000/v1 --api-key local-key --model Qwen/Qwen2.5-7B-Instruct
gaia benchmark --mock --requests 100
```

## `init-chatbot`

Scaffold the React/Vite chatbot template.

```bash
gaia init-chatbot [--path <PATH>] [--force] [--skip-install]
```

Examples:

```bash
gaia init-chatbot
gaia init-chatbot --path ./gaia-chatbot --force
gaia init-chatbot --skip-install
```

## `open-chatbot`

Print chatbot URL (helpful for remote terminals).

```bash
gaia open-chatbot [--url <URL>]
```

Example:

```bash
gaia open-chatbot
```

## Backends Supported

- `vllm`
- `tgi`
- `sglang`
- `llamacpp` (or `llama.cpp`)
- `ollama`

## Exit Paths And Safety Tips

- Use `--dry-run` when validating command generation.
- Use `--detach` for long-running workloads.
- Use `gaia status` and `gaia logs` before restarting.
- Use `gaia stop --mock` to cleanly stop detached mock processes.
