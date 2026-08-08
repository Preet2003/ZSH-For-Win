# WinZSH

**Oh My Zsh–style developer experience for Windows shells.**

WinZSH is not another shell or terminal. It is a Windows Developer Experience Layer that
enhances PowerShell 7 with zero manual profile editing:

- Beautiful prompt, autosuggestions, syntax highlighting
- Smart history, aliases, themes
- Completions for git, docker, kubectl, npm, cloud CLIs, and more
- First-class plugin ecosystem
- Opt-in AI helpers (`explain` / `ask` / `check` / `alias`)

> Status: **Phase 6**. See [docs/architecture/](docs/architecture/) and
> [PHASE6-GATE.md](docs/architecture/PHASE6-GATE.md).

---

## Install on a new machine (from this clone)

Give someone the repo, then they run **one script** from PowerShell:

### 1. Prerequisites

| Tool | Install |
|------|---------|
| Git | `winget install Git.Git` |
| Rust (stable 1.85+) | https://rustup.rs |
| PowerShell 7 (recommended) | `winget install --id Microsoft.PowerShell` |

Close and reopen the terminal after installing Rust / PowerShell so `cargo` and `pwsh` are on PATH.

### 2. Clone + install

```powershell
git clone <YOUR_REPO_URL> ZshForWin
cd ZshForWin\zsh-for-win
Set-ExecutionPolicy -Scope Process Bypass
./scripts/install-from-source.ps1
```

That script:

1. Builds `winzsh.exe` (release)
2. Installs it to `%USERPROFILE%\.winzsh\bin` and adds that folder to the **user PATH**
3. Adds the `zsh-for-win` launcher
4. Runs `winzsh install` (profile hook + runtime) and sets the `modern` theme

### 3. Use it (same as your machine)

Open a **new** PowerShell tab:

```powershell
zsh-for-win          # enter WinZSH
Get-WinZshInfo
gs                   # example alias
exit                 # back to stock PowerShell
```

Optional:

```powershell
winzsh plugin add docker
winzsh plugin add git
winzsh doctor
```

---

## Manual quick start (developers)

```powershell
cd zsh-for-win
cargo build -p winzsh --release
cargo run -p winzsh --release -- install
cargo run -p winzsh --release -- theme set modern
```

Day-to-day commands: **[docs/essential-commands.md](docs/essential-commands.md)**.

Developer checks:

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
./scripts/ci/check-crate-deps.ps1
```

## Workspace

Rust workspace under `crates/`. Normative design docs under `docs/architecture/`.
PowerShell runtime templates under `runtime/powershell/`. First-party plugins under `plugins/`.

## License

MIT OR Apache-2.0
