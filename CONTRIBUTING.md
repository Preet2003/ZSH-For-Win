# Contributing to WinZSH

Thanks for your interest in contributing.

## Before you write feature code

Read:

1. [docs/architecture/README.md](docs/architecture/README.md)
2. [docs/architecture/dependency-rules.md](docs/architecture/dependency-rules.md)
3. [docs/architecture/coding-conventions.md](docs/architecture/coding-conventions.md)

Phase gates matter: do not land Phase N product behavior before the matching phase.

## Development

```powershell
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
./scripts/ci/check-crate-deps.ps1
```

## Pull requests

- Keep PRs small and vertical.
- New public API items need docs.
- No `unsafe` without an ADR.
- No network code outside allowed crates (`winzsh-registry`, `winzsh-update`, later sync/AI).
- Installer-impacting changes should include doctor coverage when applicable.

## Code of conduct

See [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
