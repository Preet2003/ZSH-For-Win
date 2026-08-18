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

## First-time setup (download Setup.exe)

```powershell
# Download WinZSH-Setup-x86_64.exe from GitHub Releases, then:
.\WinZSH-Setup-x86_64.exe          # or: setup -y
# Open a NEW PowerShell tab:
zsh-for-win
```

---

## First-time setup (winget)

```powershell
winget install --id WinZSH.WinZSH -e   # after community repo publish
```

Upgrade: `winget upgrade WinZSH.WinZSH`

---

## First-time setup (from source)

**Recommended** (build + PATH + profile in one step):

```powershell
cd zsh-for-win
Set-ExecutionPolicy -Scope Process Bypass
./scripts/install-from-source.ps1
```

Manual equivalent:

```powershell
cd zsh-for-win
cargo build -p winzsh --release
# copy target\release\winzsh.exe to ~/.winzsh/bin and put that on PATH, then:
winzsh install
winzsh theme set modern
winzsh doctor
```

Then open a **new PowerShell tab**. Stock PowerShell stays normal. Activate WinZSH with:

```powershell
zsh-for-win
# ... use WinZSH (prompt, aliases, completions, …) ...
exit    # deactivates WinZSH in EVERY PowerShell terminal
```

### Global activation (all terminals)

| Action | Effect |
|--------|--------|
| `zsh-for-win` in any stock terminal | Creates `~/.winzsh/locks/shell.active` and enters a WinZSH nested session |
| Other open stock terminals | On their **next prompt**, auto-join the same WinZSH mode |
| New PowerShell tabs while active | Auto-join on first prompt |
| `exit` in **any** WinZSH session | Clears the lock → every WinZSH session returns to stock |

Lock file: `~/.winzsh/locks/shell.active` (removed on deactivate).

After changing the profile hook, re-run install and open fresh tabs:

```powershell
cargo run -p winzsh -- install
```

### Autosuggestions (Phase 3)

| Key | Action |
|-----|--------|
| RightArrow (at end of line) | Accept full ghost suggestion |
| Ctrl+F | Accept full suggestion |
| Ctrl+RightArrow | Accept next word of suggestion |
| Ctrl+R | Fuzzy history search (**requires `fzf`**) |

Optional tools:

```powershell
winget install junegunn.fzf
winget install ajeetdsouza.zoxide
cargo run -p winzsh -- reload
```

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
| `setup [-y] [--theme id]` | Full download-and-run setup (copy exe, PATH, install, theme) |
| `reload` | Rebuild cached PowerShell runtime from config |
| `doctor` | Health checks + remediation hints |
| `status` | Version, home path, install state, theme |
| `config show` | Print active `config.toml` |
| `config path` | Print path to `config.toml` |
| `config validate` | Validate config (+ theme id) |
| `theme list` | List built-in themes |
| `theme show [id]` | Show active or named theme |
| `theme set <id>` | Set theme and regenerate runtime |
| `alias list` | List effective aliases (builtin + plugin + user) |
| `alias set <name> <value…>` | Set a **permanent** user alias |
| `alias remove <name>` | Remove a user alias |

In the shell (this tab only):

```powershell
salias myalias git status
myalias
```

Permanent (survives new tabs):

```powershell
winzsh alias set myalias "git status"
```
| `history list [-n N] [--contains X]` | List recent history |
| `history compact` | Compact spool into store |
| `completion list` | List completion packs (active vs available) |
| `plugin list` | List first-party / installed plugins |
| `plugin search [query]` | Search the community registry |
| `plugin info <id>` | Registry metadata for a plugin |
| `plugin add <id\|path>` | Install + enable (builtin → registry → path) |
| `plugin update [id]` | Update registry-origin plugins |
| `plugin remove <id>` | Uninstall plugin files + disable |
| `plugin enable <id>` | Enable installed plugin |
| `plugin disable <id>` | Disable (keep files) |
| `ai status` | AI enablement (local offline) |
| `ai enable` / `ai disable` | Toggle `features.ai` |
| `ai explain <cmd…>` | Explain a command (opt-in, local) |
| `ai ask <text…>` | English → PowerShell suggestion (opt-in, local) |
| `ai check <cmd…>` | Safety scan (works even when AI off) |
| `ai alias <text…>` | Suggest an alias (opt-in, local) |
| `sync status` | Sync destination + last export/import |
| `sync export [-p path] [--plugins] [--history]` | Write `winzsh-sync.json` |
| `sync import [-p path\|url]` | Apply bundle (backs up config) |
| `sync push` / `sync pull` | Use `[sync].destination` |
| `shell list` / `shell status` | Multi-shell catalog (detected / hooked / enabled) |
| `shell enable|disable <id>` | Opt-in CMD / Nu / Bash bridges |
| `agent status|start|stop` | Background maintenance agent |
| `agent run-once` / `agent run` | One tick or foreground loop |
| `update` | Apply GitHub Release update (needs `[update].github_repo`) |
| `update --check` | Check only (no download/rebuild) |
| `update --from-source [path] [--pull]` | Rebuild from checkout (source installs) |
| `update --rollback` | Restore previous CLI from `.bak` |
| `uninstall` | Remove managed profile hook (keep `~/.winzsh`) |

### AI helpers (Phase 6)

Off by default. **Local-only** offline heuristics — no API keys, no cloud.

```powershell
cargo run -p winzsh -- ai enable
cargo run -p winzsh -- ai explain "git reset --hard HEAD~1"
cargo run -p winzsh -- ai ask "delete node_modules recursively"
cargo run -p winzsh -- ai check "git push --force"
```

```toml
[features]
ai = true

[ai]
provider = "local"
```
| `uninstall --purge` | Remove hook **and** delete `~/.winzsh` |

### Plugins (Phase 5)

```powershell
cargo run -p winzsh -- plugin list
cargo run -p winzsh -- plugin add docker
cargo run -p winzsh -- plugin add git
cargo run -p winzsh -- reload
# new tab: dps / gst / …
```

First-party ids: `docker`, `git`, `node`, `rust`. Packs live under `plugins/` in the repo and are embedded into the CLI.

### Tab completions (Phase 4)

Packs register only when the tool is detected. Native generators (`docker`, `kubectl`, `az`) lazy-load on first Tab into `~/.winzsh/cache/completions/`.

```powershell
cargo run -p winzsh -- completion list
cargo run -p winzsh -- reload
# new tab: git <Tab> / docker <Tab> / ssh <Tab>
```

Config (`~/.winzsh/config.toml`):

```toml
[completions]
enabled = true
# only = ["git", "docker"]   # optional allow-list
```

### Copy-paste (dev)

```powershell
cargo run -p winzsh -- install
cargo run -p winzsh -- reload
cargo run -p winzsh -- theme list
cargo run -p winzsh -- theme set tokyo-night
cargo run -p winzsh -- alias list
cargo run -p winzsh -- alias set gs "git status -sb"
cargo run -p winzsh -- history list -n 20
cargo run -p winzsh -- completion list
cargo run -p winzsh -- plugin list
cargo run -p winzsh -- plugin add docker
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

### Release / winget dry-run

```powershell
./scripts/release/package.ps1
./scripts/release/fill-winget-manifest.ps1
winget validate --manifest packaging\winget\WinZSH.WinZSH
```

Architecture / design: [`docs/architecture/`](architecture/).

---

## Future commands (planned)

| Command | Phase / notes |
|---------|----------------|
| (none critical) | Native Nu/Bash runtime; Task Scheduler agent auto-start |

When a command ships, move it into **Core CLI commands** above and add a short workflow section.

---

## Multi-shell / agent

```powershell
winzsh shell list
winzsh shell enable cmd
winzsh agent run-once
winzsh agent start
winzsh agent status
winzsh agent stop
```

See [`MULTI-SHELL-GATE.md`](architecture/MULTI-SHELL-GATE.md).

---

## Settings sync

```powershell
winzsh sync export -p $env:OneDrive\winzsh-sync.json --plugins
winzsh sync import -p $env:OneDrive\winzsh-sync.json

# Or configure [sync].destination once, then:
winzsh sync push --plugins
winzsh sync pull
```

See [`SYNC-GATE.md`](architecture/SYNC-GATE.md).

---

## Plugin registry

```powershell
winzsh plugin search
winzsh plugin info demo-aliases
winzsh plugin add demo-aliases
winzsh plugin update
```

Index: `[registry].url` or `WINZSH_REGISTRY_URL` (see `registry/README.md`).

---

## Self-update

Source installs (no GitHub Releases yet):

```powershell
# Uses [update].source_dir, WINZSH_SOURCE, or walks cwd
winzsh update --from-source --pull
winzsh update --check
winzsh update --rollback   # if a previous binary was backed up
```

When Releases exist, set in `~/.winzsh/config.toml`:

```toml
[update]
github_repo = "owner/repo"
channel = "stable"
```

Then `winzsh update` / `winzsh update --check` use GitHub assets.

---

## Quick troubleshooting

| Symptom | What to try |
|---------|-------------|
| `pwsh` / PowerShell not found | Install PS7 via winget, or accept Windows PowerShell at the install prompt; reopen terminal |
| Doctor fails on profile hook | `cargo run -p winzsh -- install --force` |
| Theme/alias/prompt not visible | `winzsh reload`, then **new** PowerShell tab |
| Config errors | `config validate`, then fix `~/.winzsh/config.toml` or re-`install` |
