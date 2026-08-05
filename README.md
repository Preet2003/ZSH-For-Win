# WinZSH

**Oh My Zsh–style developer experience for Windows shells.**

WinZSH is not another shell or terminal. It is a Windows Developer Experience Layer that
enhances PowerShell 7 with zero manual profile editing:

- Beautiful prompt, autosuggestions, syntax highlighting
- Smart history, aliases, themes
- Completions for git, docker, kubernetes, npm, cloud CLIs, and more
- First-class plugin ecosystem
- One command install (`winget install winzsh` / `winzsh install`)

> Status: **Phase 3 smart shell**. Prompt, themes, aliases, history, autosuggest accept,
> syntax colors, optional fzf/zoxide. Completions/plugins come later.
> See [docs/architecture/](docs/architecture/).

## Quick start

Requirements: Rust stable (1.85+), PowerShell 7 (recommended) or Windows PowerShell.

```powershell
cargo build -p winzsh
cargo run -p winzsh -- install
cargo run -p winzsh -- theme set modern
# Restart PowerShell / open a new tab, then:
Get-WinZshInfo
gs   # git status alias
```

Day-to-day commands and workflows: **[docs/essential-commands.md](docs/essential-commands.md)** (keep this updated as the CLI grows).

Developer checks:

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
./scripts/ci/check-crate-deps.ps1
```

## Workspace

Rust workspace under `crates/`. Normative design docs under `docs/architecture/`.
PowerShell runtime templates under `runtime/powershell/`.

## License

MIT OR Apache-2.0
