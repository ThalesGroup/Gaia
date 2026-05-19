# Architecture

Gaia is organized as a small Rust workspace with a terminal-first CLI, an interactive TUI, and a reusable core crate. The project keeps runtime orchestration separate from user interaction so that backend support, config handling, and deployment artifact generation can evolve without coupling every command to terminal UI code.

## Workspace Layout

- `crates/gaia-core`
  - backend abstraction and Docker command generation
  - machine detection and backend availability checks
  - configuration loading, normalization, and serve config mapping
  - model catalog loading and recommendation logic
- `crates/gaia-cli`
  - public `gaia` command entrypoint
  - command implementations such as `serve`, `doctor`, `models`, `generate-compose`, `generate-k8s`, and `generate-systemd`
  - mock OpenAI-style API used for local development and integration tests
- `crates/gaia-tui`
  - interactive model/backend selection wizard
  - terminal rendering and input handling
- `catalog`
  - curated model metadata consumed by `models`, `recommend`, and the TUI
- `docs`
  - user guides, compatibility notes, deployment patterns, and this architecture overview

## Runtime Flow

The direct serving path starts with either CLI flags or the TUI wizard. Both paths resolve to a `ServeConfig`, which is passed to a backend implementation.

```text
User
  -> gaia CLI / TUI
  -> AppConfig + CLI overrides
  -> ServeConfig
  -> ServingBackend
  -> CommandSpec
  -> Docker / mock API process
```

The backend layer owns backend-specific command construction. Each backend implements the `ServingBackend` trait and produces:

- a Docker command for `gaia serve`
- a Docker Compose service for `gaia generate-compose`
- an OpenAI-style base URL for client commands

Shared Docker hardening and common run arguments live in `backend/common.rs` so that container security defaults stay consistent across vLLM, TGI, SGLang, llama.cpp, and Ollama.

## Configuration Model

Gaia distinguishes between two configuration access patterns:

- read/write initialization for commands that intentionally persist config, such as `serve` and `select`
- read-only loading with transient defaults for commands that should not create config as a side effect, such as `benchmark`, `logs`, `stop`, and artifact generation commands

This keeps read-only workflows predictable while preserving a convenient first-run experience for commands that actually configure or launch services.

## Backends

Backends are selected by name and resolved through the core backend registry. The public backend names are:

- `vllm`
- `tgi`
- `sglang`
- `llamacpp` with `llama.cpp` as an alias
- `ollama`

Backend implementations are responsible for their own image, ports, runtime arguments, environment variables, and compose service shape. Gaia pins backend images by digest to reduce supply-chain drift.

## Deployment Artifact Generation

Gaia can generate deployment files without launching a local service:

- `generate-compose` creates a single-node Docker Compose file
- `generate-k8s` creates a Kubernetes Deployment and Service
- `generate-systemd` creates a systemd unit that runs `gaia serve`

These commands use transient defaults when no config file exists, so they can be used in CI or fresh workspaces without mutating local state.

## OpenAI Compatibility Boundary

Gaia provides an OpenAI-style base URL and auth flow. Real backend compatibility depends on the selected inference backend, while the mock API provides broader endpoint coverage for local development and tests.

The current contract is documented in `docs/openai-compatibility.md`. The important distinction is:

- mock API: controlled by Gaia and suitable for deterministic tests
- real backends: delegated to the backend image and version selected by Gaia

## Security And Supply Chain

The security posture is layered:

- Rust dependencies are audited in CI and remain blocking
- backend container images are pinned by digest
- generated containers use non-root users, dropped capabilities, read-only root filesystems, and no-new-privileges where supported
- image vulnerability/signature checks run in CI as advisory visibility for third-party images

Gaia does not rebuild or own upstream inference images, so third-party image scan failures should be triaged but should not automatically block project pushes.


