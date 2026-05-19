# Troubleshooting

This guide covers common issues and fast recovery steps.

## Quick Diagnostic Baseline

Run these first:

```bash
gaia doctor
gaia status
gaia logs --lines 200
```

If using mock mode:

```bash
gaia logs --mock --lines 200
```

## `gaia: command not found`

Cause:

- binary is not installed in your `PATH`

Fix:

```bash
cargo build --release -p gaia-cli
./target/release/gaia --help
./install.sh
```

Then ensure `~/.local/bin` is in `PATH`.

## Docker Daemon Not Reachable

Symptoms:

- `doctor` reports daemon not reachable
- `serve` fails when trying to run Docker

Fix:

- start Docker service
- run with a user that can access Docker socket
- on Linux, ensure docker group permissions are configured

## No NVIDIA GPU Detected

Symptoms:

- `doctor` shows no GPU
- GPU backends marked unavailable/limited

Fix:

- check `nvidia-smi`
- verify NVIDIA driver installation
- configure NVIDIA Container Toolkit for Docker

## Gated Model Download Fails

Cause:

- `HF_TOKEN` missing or invalid

Fix:

```bash
export HF_TOKEN=hf_xxx
gaia doctor
```

Security note: prefer short-lived, least-privilege HF tokens.

## `prod` Security Profile Rejects Launch

Symptoms:

- `Security profile 'prod' requires an explicit API key`
- gated model launch fails with missing `HF_TOKEN`

Cause:

- `prod` mode disables weak/local API key fallback
- gated models require a valid Hugging Face token

Fix:

```bash
export GAIA_API_KEY=your-strong-random-key
export HF_TOKEN=hf_xxx   # only for gated models
gaia serve --security-profile prod ...
```

## `gaia select` Fails With TTY Error

Cause:

- command launched in non-interactive terminal/session

Fix:

- run `gaia select` in an interactive terminal
- use `gaia serve ...` directly in automated scripts

## Mock API Keeps Running

Stop cleanly:

```bash
gaia stop --mock
```

If needed, inspect status and logs:

```bash
gaia status
gaia logs --mock --lines 200
```

## Port Already In Use

Symptoms:

- backend fails to bind host port

Fix:

- change `--port` (API)
- stop old processes/containers before relaunch
- verify with system tools (`ss`, `lsof`, or Docker status)

## Benchmark Fails On First Requests

Checklist:

- ensure API endpoint is reachable (`/v1/chat/completions`)
- check API key value
- confirm model id exists on target backend
- if you pinned `--model-revision`, verify the 40-char commit SHA exists upstream
- test one manual curl request before running benchmark loops
