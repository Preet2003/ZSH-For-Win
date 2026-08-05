# Plugin Specification (V1)

## Model

V1 plugins are **packages of a manifest + PowerShell assets**, installed into
`~/.winzsh/plugins/` and **materialized** into the runtime module by `winzsh-runtime-gen`.

There is **no** native DLL / `dlopen` plugin ABI in V1.
WASM compute plugins are a Phase 5+ extension point (designed, not built).

## Package layout

```text
my-plugin/
  plugin.toml
  completions/
    docker.ps1
  hooks/
    init.ps1
  aliases.ps1          # optional convenience; aliases usually declared in TOML
```

## Manifest (`plugin.toml`)

```toml
name = "docker"
version = "1.0.0"
description = "Docker aliases and completions"
engines.winzsh = ">=0.1.0"
commands = ["docker"]
aliases = { dcu = "docker compose up" }
completions = ["completions/docker.ps1"]
hooks = []
themes = []
```

### Fields

| Field | Meaning |
|-------|---------|
| `name` | Stable plugin id (`PluginId`) |
| `version` | Semver package version |
| `engines.winzsh` | Compatible CLI/runtime range |
| `commands` | Gate enablement on binary presence (`winzsh-detect`) |
| `aliases` | Alias map contributed by the plugin |
| `completions` | Relative paths to PS completion scripts |
| `hooks` | Constrained init hooks (protected at runtime) |
| `themes` | Optional theme assets shipped with the plugin |

## Lifecycle

```text
winzsh plugin add <name|path>
  → resolve (registry or local)
  → validate manifest
  → install into ~/.winzsh/plugins/
  → update config enabled list
  → runtime-gen rebuild
```

Commands: `add`, `remove`, `enable`, `disable`, `list`, `update` (registry phase).

## Trust model

1. **First-party** (in-repo / signed by WinZSH release key) — default trusted.
2. **Registry** — checksum + signature required; updates follow channel policy.
3. **Local path** — `winzsh plugin add ./path` with an explicit warning.
4. **No arbitrary native code** in V1.

## Failure isolation

Plugin init runs inside protected blocks in the generated module.
A failing plugin must not brick the shell.
`winzsh doctor` can disable all plugins via config write + regen
(`--disable-all-plugins` / equivalent remediation).

## Conflict policy

Alias and completion conflicts resolve deterministically:

1. User config aliases win.
2. Explicitly enabled plugin order in `config.toml` `[plugins].enabled`.
3. First-party before community when order ties (documented in merge code).

Doctor reports conflicts as warnings.
