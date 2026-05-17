# Deployment Patterns

This guide explains practical deployment topologies for `gaia`.

## Pattern 1: Single Node, Single Model

Use when:

- you need the fastest setup
- one model is enough
- workload is low to moderate

Flow:

1. `gaia doctor`
2. `gaia serve --backend ... --model ... --detach`
3. `gaia status` and `gaia logs`

## Pattern 2: Single Node, Multiple Models

Use when:

- you want local model diversity on one machine
- you can manage GPU memory contention carefully

How:

- run multiple `gaia serve --detach` commands on different ports
- give each service a unique model/backend pair
- route clients by endpoint or behind a gateway

Important caveat:

- GPU backends currently use `--gpus all`, so running several heavy containers on one host can cause contention and unstable latency.

## Pattern 3: Multi-Worker Inference + LiteLLM Gateway

This is the recommended high-level architecture for production-like systems.

- `gaia` runs on each worker node to manage backend/model runtime.
- LiteLLM runs as a lightweight gateway (often CPU-only) on a separate node.
- clients call LiteLLM only.
- LiteLLM routes to worker endpoints.

Why this works well:

- clear separation of responsibilities
- independent scaling of gateway and inference workers
- easier failover/fallback logic
- keeps `gaia` focused on runtime orchestration

Typical control flow:

1. Provision N worker machines.
2. Deploy one or more `gaia` services per worker.
3. Register worker endpoints in LiteLLM config.
4. Expose one gateway endpoint to applications.

## Pattern 4: Kubernetes Manifests

Use when:

- your platform is already Kubernetes-first
- you want declarative deployment manifests

Generate manifests:

```bash
gaia generate-k8s --backend vllm --model Qwen/Qwen2.5-7B-Instruct --replicas 1
```

Then apply with your regular cluster workflow (`kubectl apply`, Helm overlays, GitOps, etc.).

## Pattern 5: systemd Services

Use when:

- you operate bare-metal or VM workers
- you need boot-time restarts and service management

Generate unit:

```bash
gaia generate-systemd --backend sglang --model Qwen/Qwen2.5-7B-Instruct
```

Install and enable with `systemctl`.

## Compose For Local Bundles

`generate-compose` is ideal for local or small-stack setups where backend + chatbot are deployed together:

```bash
gaia generate-compose --backend tgi --model mistralai/Mistral-7B-Instruct-v0.3 --with-chatbot
```

## Security And Operations Checklist

- Use non-empty API keys in production.
- Set `HF_TOKEN` through secure environment injection for gated models.
- Prefer short-lived, least-privilege HF tokens and rotate them regularly.
- Pin container images by digest (`image@sha256:...`) for deterministic rollouts.
- Run user-facing services as non-root where possible.
- Restrict network exposure of worker endpoints (private subnet/internal LB).
- Put TLS/auth at gateway or ingress level.
- Monitor:
  - request latency
  - error rates
  - GPU memory and utilization
  - container restarts

## What Belongs In `gaia` vs Gateway

Keep in `gaia`:

- backend runtime start/stop/status
- model-specific process/container orchestration
- deployment artifact generation (compose/k8s/systemd)

Keep in LiteLLM or higher control plane:

- multi-endpoint routing policies
- fallback and weighted balancing
- tenant/rate-limit governance
- cross-worker traffic decisions
