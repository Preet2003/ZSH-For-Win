# Configuration Schema

## Location

User config: `~/.winzsh/config.toml`

Format: **TOML only** for human-edited config.
Schema version is required.

## Layering (low → high precedence)

1. Compiled defaults
2. System config (future)
3. User `config.toml`
4. Environment `WINZSH_*`
5. CLI flags

## Unknown keys

- Runtime: warn and ignore (forward compatible).
- `winzsh config validate` (strict): reject unknown keys.
- CI fixtures: strict.

## Migrations

`winzsh-config` runs ordered migrations on load.
User keys are never silently dropped without writing a backup (`config.toml.bak`).

## Illustrative schema (`schema_version = 1`)

```toml
schema_version = 1

theme = "modern"

[features]
autosuggestions = true
syntax = true
history = true
ai = false

[prompt]
git = true
budget_ms = 20

[history]
enabled = true
max_entries = 10000

[smart]
fzf = true
zoxide = true

[plugins]
enabled = ["git", "docker"]

[aliases]
# user aliases only; plugin aliases come from manifests
# gs = "git status -sb"

[update]
channel = "stable"
check_on_start = true

[telemetry]
# default off; explicit opt-in only if a future ADR allows it
enabled = false
```

## Normative types

Rust types in `winzsh-config` are the source of truth.
A JSON Schema export may be generated later for docs/CI; until then, this document and the
Rust structs define the contract.

## Derived artifacts

Runtime PowerShell modules under `~/.winzsh/cache/runtime/` are **derived**.
Users must not hand-edit them; changes go through config/plugins/themes + `winzsh-runtime-gen`.
