# WinZSH

**Oh My Zsh–style developer experience for Windows shells.**

WinZSH is not another shell or terminal. It is a Windows Developer Experience Layer that
enhances PowerShell 7 with zero manual profile editing:

- Beautiful prompt, autosuggestions, syntax highlighting
- Smart history, aliases, themes
- Completions for git, docker, kubernetes, npm, cloud CLIs, and more
- First-class plugin ecosystem
- One command install (`winget install winzsh` / `winzsh install`)

> Status: **architecture scaffold**. Phase 1 feature work (installer, real prompt, etc.)
> has not started. See [docs/architecture/](docs/architecture/).

## Quick start (developers)

Requirements: Rust stable (1.85+), PowerShell 7 for e2e later.

```powershell
cargo build -p winzsh
cargo run -p winzsh -- status
cargo test --workspace
./scripts/ci/check-crate-deps.ps1
```

## Workspace

Rust workspace under `crates/`. Normative design docs under `docs/architecture/`.
PowerShell runtime templates under `runtime/powershell/`.

## License

MIT OR Apache-2.0
