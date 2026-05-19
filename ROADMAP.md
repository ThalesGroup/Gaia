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

## Platform Orchestration Expansion

### 4) Fleet Management And Environments (Planned)

Goal: manage multiple deployment targets through explicit environments (`dev`, `stage`, `prod`) across VMs and Kubernetes clusters.

Scope:

- inventory of targets/environments (`dev`, `stage`, `prod`, VM, cluster)
- environment-scoped credentials and defaults
- target selection and safety confirmations by environment

### 5) Desired State And Reconciliation (Exploring)

Goal: define a desired runtime state and continuously converge actual runtime toward it.

Scope:

- declarative desired state representation (model/backend/version/replicas/policy)
- reconciliation loop to detect drift and re-apply desired configuration
- explicit convergence status in CLI output

### 6) Rollout Strategies (Exploring)

Goal: support safer production releases with progressive traffic movement and conditional rollback.

Scope:

- canary rollout
- blue-green rollout
- progressive rollout with policy checks
- conditional rollback based on health/SLO thresholds

### 7) Auto-Remediation (Exploring)

Goal: trigger recovery actions automatically when service quality degrades.

Scope:

- restart/redeploy rules based on SLO signals (latency, errors, availability)
- cooldown windows and retry budgets
- operator-visible remediation history

### 8) Drift Detection (Planned)

Goal: detect and correct divergence between declared config and live runtime.

Scope:

- config/runtime drift detection
- explainable drift reports
- optional auto-correction or approval-gated correction

### 9) Release History And Audit Trail (Planned)

Goal: provide reliable deployment traceability.

Scope:

- who deployed what, when, and where
- deployment result/status with failure reason
- searchable release timeline per environment

## Product Features (Visible UX)

### 10) `gaia env` (Planned)

Manage target environments and deployment contexts.

### 11) `gaia rollout` (Exploring)

Run canary/blue-green/progressive rollout flows in one command.

### 12) `gaia watch` (Exploring)

Live deployment/runtime watch mode with simple alerts and health signals.

### 13) `gaia diff` / `gaia plan` (Planned)

Preview changes before apply (`desired` vs `current`) with risk hints.

### 14) `gaia promote` (Planned)

Promote a validated model/version from one environment to another.

