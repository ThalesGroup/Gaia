# Security Policy

## Supported Versions

Security fixes are applied on a best-effort basis to:

- `main` (latest development)
- latest tagged release line

Older releases may not receive patches.

## Reporting a Vulnerability

Please do **not** open a public issue for unpatched vulnerabilities.

Report privately with:

- description of the issue
- impact and attack scenario
- affected versions/commits
- reproduction steps or proof of concept
- proposed mitigation (if available)

If private reporting infrastructure is not configured yet, open a minimal issue saying
"security contact needed" without disclosing exploit details.

## Scope Notes

Gaia orchestrates local/self-hosted inference runtimes. Security posture depends on:

- host hardening
- Docker/runtime configuration
- secret management (`GAIA_API_KEY`, `HF_TOKEN`)
- network exposure and gateway controls

Review `docs/deployment-patterns.md` and `docs/troubleshooting.md` when deploying to production.
