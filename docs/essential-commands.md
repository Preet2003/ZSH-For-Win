# WinZSH — Essential Commands & Steps

Living cheat sheet for day-to-day WinZSH development and usage.
Run all `cargo` commands from the workspace root: `zsh-for-win/`.

Update this file when new CLI commands, flags, or workflows ship.

---

## Prerequisites

| Tool | Notes |
|------|--------|
| Rust stable (1.85+) | `rustc --version` |
| PowerShell 7 (`pwsh`) | **Recommended.** Install: `winget install --id Microsoft.PowerShell` or https://aka.ms/powershell |
| Windows PowerShell 5.1 | Accepted fallback; `winzsh install` will offer to install PowerShell 7 |

---

## First-time setup (from source)

```powershell
cd zsh-for-win
cargo build -p winzsh
cargo run -p winzsh -- install
cargo run -p winzsh -- theme set modern
cargo run -p winzsh -- doctor
```

Then **restart PowerShell** (or open a new tab) so the profile hook loads.

If PowerShell 7 is missing but Windows PowerShell is present, install prompts:

```text
PowerShell 7 is recommended for the best WinZSH experience. Install it now? (Y/n)
```

- `Y` / Enter → tries `winget install Microsoft.PowerShell`, then continues
- `n` → continues with Windows PowerShell

---

## Core CLI commands

Prefix with `cargo run -p winzsh --` while developing, or use `winzsh` once the binary is on PATH.

| Command | Purpose |
|---------|---------|
| `install` | Install or repair (profile hook + runtime + config) |
| `install --force` | Force repair even if already installed |
| `reload` | Rebuild cached PowerShell runtime from config |
| `doctor` | Health checks + remediation hints |
| `status` | Version, home path, install state, theme |
| `config show` | Print active `config.toml` |
| `config path` | Print path to `config.toml` |
| `config validate` | Validate config (+ theme id) |
| `theme list` | List built-in themes |
| `theme show [id]` | Show active or named theme |
| `theme set <id>` | Set theme and regenerate runtime |
| `alias list` | List effective aliases (builtin + user) |
| `alias set <name> <value…>` | Set a user alias |
| `alias remove <name>` | Remove a user alias |
| `history list [-n N] [--contains X]` | List recent history |
| `history compact` | Compact spool into store |
| `uninstall` | Remove managed profile hook (keep `~/.winzsh`) |
| `uninstall --purge` | Remove hook **and** delete `~/.winzsh` |

### Copy-paste (dev)

```powershell
cargo run -p winzsh -- install
cargo run -p winzsh -- reload
cargo run -p winzsh -- theme list
cargo run -p winzsh -- theme set tokyo-night
cargo run -p winzsh -- alias list
cargo run -p winzsh -- alias set gs "git status -sb"
cargo run -p winzsh -- history list -n 20
cargo run -p winzsh -- doctor
cargo run -p winzsh -- status
```

### Global flags

| Flag | Purpose |
|------|---------|
| `--json` | Machine-readable JSON on stdout (skips interactive prompts) |
| `-v` / `--verbose` | Debug logging |

Examples:

```powershell
cargo run -p winzsh -- --json status
cargo run -p winzsh -- --verbose doctor
cargo run -p winzsh -- install --json
```

---

## In-shell (after install + new tab)

| Action | Notes |
|--------|--------|
| Prompt | Path + git branch/dirty + theme colors |
| `Get-WinZshInfo` | Phase / theme metadata |
| `gs`, `gp`, `gl`, `ll`, `dcu`, … | Built-in aliases |
| History | Written to `~/.winzsh/history/spool.jsonl` each prompt |

---

## Typical workflows

### Fresh install → verify

```powershell
cargo run -p winzsh -- install
cargo run -p winzsh -- doctor
cargo run -p winzsh -- status
```

Expect doctor: `All critical checks passed.`

### Change theme

```powershell
cargo run -p winzsh -- theme set catppuccin
# open a new PowerShell tab
```

### Inspect / tweak config

```powershell
cargo run -p winzsh -- config path
cargo run -p winzsh -- config show
# edit ~/.winzsh/config.toml
cargo run -p winzsh -- config validate
cargo run -p winzsh -- reload
```

### Repair after profile edits or broken state

```powershell
cargo run -p winzsh -- install --force
cargo run -p winzsh -- doctor
```

### Clean uninstall

```powershell
cargo run -p winzsh -- uninstall          # hook only
cargo run -p winzsh -- uninstall --purge  # hook + ~/.winzsh
```

---

## Important paths

| Path | What |
|------|------|
| `~/.winzsh/` | WinZSH home |
| `~/.winzsh/config.toml` | User config |
| `~/.winzsh/state.json` | Install state |
| `~/.winzsh/cache/runtime/WinZSH.psm1` | Generated runtime module |
| `~/.winzsh/history/spool.jsonl` | Live history spool |
| `~/.winzsh/history/history.jsonl` | Compacted history store |
| `~/.winzsh/backups/profile/` | Profile backups from install |
| Documents `\PowerShell\` or `\WindowsPowerShell\` | Host profile (hook lives here) |

---

## Developer checks

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
./scripts/ci/check-crate-deps.ps1
```

Architecture / design: [`docs/architecture/`](architecture/).

---

## Future commands (planned)

| Command | Phase / notes |
|---------|----------------|
| `update` | Self-update / channel |
| `plugin` | Plugin install / enable (Phase 5) |
| autosuggest / syntax | Phase 3 |
| `sync` | Settings sync |
| `ai` | AI helpers (Phase 6) |

When a command ships, move it into **Core CLI commands** above and add a short workflow section.

---

## Quick troubleshooting

| Symptom | What to try |
|---------|-------------|
| `pwsh` / PowerShell not found | Install PS7 via winget, or accept Windows PowerShell at the install prompt; reopen terminal |
| Doctor fails on profile hook | `cargo run -p winzsh -- install --force` |
| Theme/alias/prompt not visible | `winzsh reload`, then **new** PowerShell tab |
| Config errors | `config validate`, then fix `~/.winzsh/config.toml` or re-`install` |
