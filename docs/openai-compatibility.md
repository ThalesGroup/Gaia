# OpenAI Compatibility

This guide defines what `gaia` guarantees for OpenAI-style APIs, and what remains backend-dependent.

## Why This Matters

`gaia` orchestrates inference runtimes, but each backend has its own feature surface and release cadence.
Without a clear contract, teams can assume "100% OpenAI parity" and hit production surprises.

## Compatibility Levels

### Level 1: OpenAI Core (Gaia Contract)

`gaia` guarantees these integration primitives:

- API base URL shape: `http://<host>:<port>/v1`
- Bearer API key pattern (`Authorization: Bearer <key>`)
- standard model id forwarding in request payloads
- practical compatibility target for `POST /v1/chat/completions` on real backends

This is the minimum contract for most agent/chat integrations.

### Level 2: Extended OpenAI Surface (Backend-Dependent)

These capabilities are not guaranteed uniformly by `gaia` across all real backends:

- `GET /v1/models`
- `POST /v1/completions`
- `POST /v1/responses`
- `POST /v1/embeddings`
- audio endpoints (`/v1/audio/transcriptions`, `/v1/audio/speech`)
- tool calling/function calling parity details
- strict event schema parity for streaming beyond chat basics

For these features, behavior depends on backend/version/model and should be validated in your environment.

### Level 3: Gaia Mock OpenAI Coverage (`--mock`)

The built-in mock server implements a broad OpenAI-like surface for local validation:

- `GET /v1/models`
- `GET /v1/models/{id}`
- `POST /v1/chat/completions` (including stream mode)
- `POST /v1/completions` (including stream mode)
- `POST /v1/responses` and `GET /v1/responses/{id}`
- `POST /v1/embeddings`
- `POST /v1/audio/transcriptions`
- `POST /v1/audio/speech`

This is ideal for frontend and SDK integration tests before using real inference runtimes.

## Current Matrix

Legend:

- `core`: covered by Gaia Core contract
- `backend`: backend-dependent, no Gaia-wide strict guarantee
- `mock`: implemented in Gaia mock server

| Capability | mock (`--mock`) | vllm | tgi | sglang | llamacpp | ollama |
| --- | --- | --- | --- | --- | --- | --- |
| Base URL `/v1` + Bearer auth flow | core+mock | core | core | core | core | core |
| `POST /v1/chat/completions` | core+mock | backend | backend | backend | backend | backend |
| `GET /v1/models` | mock | backend | backend | backend | backend | backend |
| `POST /v1/completions` | mock | backend | backend | backend | backend | backend |
| `POST /v1/responses` | mock | backend | backend | backend | backend | backend |
| `POST /v1/embeddings` | mock | backend | backend | backend | backend | backend |
| Audio endpoints | mock | backend | backend | backend | backend | backend |

## Production Recommendation

If you need strict, centralized behavior across heterogeneous runtimes, place a gateway/control-plane layer in front of workers (for example LiteLLM) and keep `gaia` focused on runtime orchestration.

## Practical Rollout Strategy

1. Build agents against OpenAI Core (`chat/completions`) first.
2. Add extended endpoints one by one and test per backend.
3. Keep a backend/version compatibility checklist in CI before production rollout.
