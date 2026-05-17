# Contributing

Thanks for your interest in contributing to Gaia.

## Development Setup

1. Install prerequisites:
   - Rust toolchain
   - Docker
   - NVIDIA drivers + NVIDIA Container Toolkit (for GPU backend validation)
2. Build CLI:
   ```bash
   cargo build --release -p gaia-cli
   ```
3. Run health check:
   ```bash
   ./target/release/gaia doctor
   ```

## Contribution Workflow

1. Create a feature branch from `main`.
2. Keep changes focused and reviewable.
3. Update docs when behavior or commands change.
4. Add/adjust tests for behavior changes.

## Quality Checklist

Run before opening a PR:

```bash
cargo fmt
cargo test
```

If your change affects CLI behavior, also validate manually:

```bash
./target/release/gaia --help
./target/release/gaia doctor
```

## Pull Requests

Please include:

- what changed
- why it changed
- how it was tested
- any operational or breaking impact

Small PRs with clear scope are preferred.
