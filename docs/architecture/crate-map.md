# Crate Map

Workspace members are library crates plus one binary (`winzsh`).
No other crate defines `[[bin]]` except a future optional `winzsh-agent`.

## Foundation

| Crate | Responsibility |
|-------|----------------|
| `winzsh-error` | Domain errors (`thiserror`) and CLI-boundary reporting helpers. No logging, no IO policy. |
| `winzsh-core` | Version constants, channels (`stable`/`beta`), `WinzshPaths`, IDs (`PluginId`, `ThemeId`). No clap, no network. |
| `winzsh-fs` | Atomic writes, backup rotation, lock files, safe path join under `~/.winzsh`. |
| `winzsh-log` | `tracing` setup, redaction, log file under `~/.winzsh/logs/`. |
| `winzsh-config` | Schema-versioned `config.toml`, defaults, validation, migrations. |
| `winzsh-test-support` | Temp homes, golden helpers, fake PATH, sample manifests (dev-dep only). |

## Orchestration

| Crate | Responsibility |
|-------|----------------|
| `winzsh-cli` | clap command tree, human/JSON output, exit codes. Thin dispatch only. |
| `winzsh-installer` | Detect, backup, install, uninstall, verify. Idempotent. |
| `winzsh-doctor` | Structured diagnostics and remediation hints. |
| `winzsh-update` | Channels, checksum verify, staged replace, rollback metadata. |
| `winzsh-detect` | Tool/binary detection and capability bits for lazy enablement. |
| `winzsh-runtime-gen` | **Single writer** of runtime artifacts (merged module + lockfile hash). |

## Shell integration

| Crate | Responsibility |
|-------|----------------|
| `winzsh-shell-host` | `ShellHost` trait and capability model (multi-shell future). |
| `winzsh-powershell` | PS7 profile markers, module path, PSReadLine detection. Not aesthetics. |

## Experience engines

Rust owns contracts and codegen inputs; PowerShell executes at runtime.

| Crate | Responsibility |
|-------|----------------|
| `winzsh-prompt` | Segment model, timing budgets, theme binding IDs. |
| `winzsh-completion` | Completion pack catalog and lazy-load rules. |
| `winzsh-suggest` | Suggestion sources, history policy, syntax token rules. |
| `winzsh-theme` | Theme package format, install/remove, palette validation. |
| `winzsh-alias` | Global/workspace/plugin alias merge and conflict policy. |
| `winzsh-history` | History store schema and query API. |

## Ecosystem

| Crate | Responsibility |
|-------|----------------|
| `winzsh-plugin` | Manifest parse, lifecycle, trust, dependency DAG. No network. |
| `winzsh-registry` | HTTPS index client, signature hooks, caching. |
| `winzsh-ai` | AI explain/ask/alias/safety (local + optional OpenAI-compatible HTTP). |
| `winzsh-sync` | Stub until sync phase. |

## Binary

| Crate | Responsibility |
|-------|----------------|
| `winzsh` | `main`, panic hook, init logging, call `winzsh_cli::run()`. Depends only on `winzsh-cli` directly. |

## Non-crate trees

| Path | Role |
|------|------|
| `runtime/powershell/` | Checked-in PS templates consumed by runtime-gen |
| `plugins/` | First-party plugin packs |
| `themes/` | First-party themes |
| `tests/` | Integration and e2e fixtures |
| `scripts/ci/`, `scripts/release/` | Automation |
| `website/` | Deferred placeholder |
