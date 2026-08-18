# System Overview

## Thesis

WinZSH ships as one management binary (`winzsh.exe`) with no required runtime dependencies
beyond PowerShell 7. On install it:

1. Detects the environment (pwsh, Windows Terminal, Git, existing profile).
2. Backs up the user profile and inserts a **marked** managed hook (no manual editing).
3. Materializes a **versioned PowerShell module** under `~/.winzsh/cache/runtime/`.
4. Leaves interactive prompt, suggestions, highlighting, and completions to that
   **in-process** module (no per-prompt process spawn).

Rust owns install, config, plugins, themes, updates, doctor, and runtime generation.
PowerShell executes the hot path.

## Topology

```text
Windows Terminal → PowerShell 7 → Managed Profile Hook → WinZSH.psm1 (cache)
                                              ↑
                                         read-only
                                              ↑
~/.winzsh/  ←  winzsh.exe (CLI: install/update/plugin/theme/doctor)
                                              ↓
                              GitHub Releases / winget / plugin registry
```

Optional background maintenance runs as `winzsh agent` (same user-facing binary).
It can compact history, refresh the plugin registry cache, and optionally check for updates.

## Runtime split

| Path | Latency budget | Implementation |
|------|----------------|----------------|
| Prompt render | < 20ms typical, < 50ms p99 | In-process PS module + cached segment data |
| Autosuggest / syntax | interactive keystroke | PSReadLine handlers in runtime module |
| Completions | interactive Tab | Lazy-loaded completion scripts |
| `winzsh` CLI | human-scale | Rust binary |
| Background maintenance | async | `winzsh agent` (same binary) |

**Rule:** The Rust binary must never be required on the prompt hot path.
Rust compiles/materializes the runtime; PowerShell executes it.

## On-disk layout (`~/.winzsh/`)

```text
~/.winzsh/
  config.toml
  config.toml.bak
  state.json                 # install id, last update check, schema versions
  logs/winzsh.log
  backups/profile/           # timestamped profile backups
  plugins/                   # installed plugin packages
  themes/                    # installed themes
  cache/
    runtime/                 # generated WinZSH module + hash
    detect.json              # tool detection cache with TTL
    registry/                # index cache
  history/
    history.db               # or spool/ + compacted db
  locks/
    shell.active
    agent.pid
    agent.heartbeat.json
    agent.stop
```
## Managed profile hook

Installer owns create/update/remove of a single marked block:

```powershell
# >>> winzsh >>>
# ... load cached module ...
# <<< winzsh <<<
```

If the cache is missing or corrupt, the hook loads a minimal safe-mode stub and tells
the user to run `winzsh doctor`.

## Inter-component contracts

| Mechanism | Used for |
|-----------|----------|
| Owned Rust types / traits | Config, manifests, theme resolve, `ShellHost` |
| `Diagnostic` / `Report` values | Doctor + installer verify |
| `runtime.lock.json` | Hash of inputs; skip rebuild when unchanged |
| Spool files / SQLite | History bridge between PS and Rust |
| Exit codes + `--json` | Automation, winget, CI |

No in-process plugin ABI in V1 (no `dlopen` of random DLLs).
No gRPC daemon in V1.
Hooks are declared in manifests and **materialized** into the PS module by
`winzsh-runtime-gen`.
