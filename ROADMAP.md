# Gaia Roadmap

This roadmap tracks high-level product direction for `gaia` as an LLM runtime orchestrator and deployment toolkit.

Status values:

- **Planned**: clear direction, not started
- **In progress**: active implementation
- **Exploring**: discovery/design phase

## Near Term

### 1) SSH Deployment Workflow (Planned)

Goal: deploy and manage Gaia-managed runtimes on remote Linux VMs directly from local `gaia`.

Scope:

- `gaia deploy ssh` command family
- host inventory (single host + named environments)
- remote prerequisites checks (`docker`, GPU runtime, disk, ports)
- remote artifact push + service bootstrap (`systemd` first)
- remote status/log/stop helpers

Success criteria:

- one-command deploy to a fresh VM
- safe idempotent re-run
- clear rollback path

### 2) Kubernetes Deployment Workflow (Planned)

Goal: move from "manifest generation only" to optional direct cluster apply flow.

Scope:

- keep `generate-k8s` as declarative output
- add optional `gaia deploy k8s` apply workflow
- namespace/secret validation before apply
- rollout status and failure diagnostics
- support overlays/values for environment differences

Success criteria:

- predictable deploy/upgrade path on managed clusters
- explicit guardrails around secrets and image pinning

### 3) Model Catalog Management v2 (In progress)

Goal: improve quality, freshness, and operator control of `catalog/models.yaml`.

Scope:

- better HF metadata ingestion and normalization
- explicit confidence/quality annotations for parameter estimates
- catalog linting + validation CLI checks
- controlled merge flow from generated catalog to curated catalog
- clearer categories for deployment sizing decisions

Success criteria:

- fewer wrong-size recommendations
- safer automation with human review checkpoints

## Mid Term

### 4) Deployment State And Drift Visibility (Exploring)

Goal: make local config, generated artifacts, and live runtime state easy to compare.

Potential outcomes:

- `gaia status --wide` with desired vs actual state
- drift warnings for ports/models/backend mismatch
- structured status output for CI pipelines

### 5) Multi-Node Orchestration Helpers (Exploring)

Goal: simplify operating several Gaia worker nodes behind one gateway.

Potential outcomes:

- worker registry commands
- health aggregation
- standard gateway integration patterns (for example LiteLLM)

## Quality Gates (Ongoing)

- preserve OpenAI-compatible API behavior guarantees
- keep security defaults hardened in generated artifacts
- maintain deterministic CI checks for critical paths
- document every new deploy flow with copy-paste runnable examples
