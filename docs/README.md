# Gaia Documentation

This directory contains the complete project documentation.

## Guides

- `docs/getting-started.md`
  - Installation from source
  - First launch with `doctor` and `select`
  - Mock mode end-to-end workflow (API + chatbot)
  - Real backend launch workflow
- `docs/command-reference.md`
  - Full CLI reference for all public commands
  - Options and practical examples per command
- `docs/architecture.md`
  - Workspace structure and runtime flow
  - Core abstractions, backend boundaries, and extension points
- `docs/chatbot-agent-from-scratch.md`
  - End-to-end tutorial for building a chatbot and basic agents
  - Includes local, production-like, and multi-VM flow
- `docs/openai-compatibility.md`
  - OpenAI compatibility contract (`core` vs backend-dependent)
  - Current backend matrix and mock coverage
- `docs/deployment-patterns.md`
  - Single-node and multi-service deployment patterns
  - Multi-worker architecture with LiteLLM as gateway
  - Kubernetes and systemd usage guidance
- `docs/model-catalog.md`
  - `catalog/models.yaml` schema
  - Catalog filtering and recommendation behavior
  - Hugging Face prefill script usage
- `docs/troubleshooting.md`
  - Most common setup/runtime issues
  - Quick diagnostics and recovery commands

## Recommended Reading Order

1. `docs/getting-started.md`
2. `docs/command-reference.md`
3. `docs/architecture.md`
4. `docs/chatbot-agent-from-scratch.md`
5. `docs/openai-compatibility.md`
6. `docs/deployment-patterns.md`
7. `docs/model-catalog.md`
8. `docs/troubleshooting.md`
