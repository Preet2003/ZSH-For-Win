# Dependency Rules

Enforced in CI (`scripts/ci/check-crate-deps.ps1` + `cargo deny`).

## Layer graph

```text
winzsh
  └─ winzsh-cli
       ├─ winzsh-installer ──┬─ winzsh-powershell ── winzsh-shell-host
       │                     └─ winzsh-runtime-gen
       ├─ winzsh-doctor
       ├─ winzsh-update
       ├─ winzsh-plugin
       ├─ winzsh-theme
       ├─ winzsh-alias
       ├─ winzsh-ai
       ├─ winzsh-config
       └─ winzsh-runtime-gen ─┬─ winzsh-prompt
                              ├─ winzsh-completion
                              ├─ winzsh-suggest
                              ├─ winzsh-theme
                              ├─ winzsh-alias
                              └─ winzsh-plugin

winzsh-registry → winzsh-plugin
winzsh-ai       → winzsh-core (local only; no HTTP)
winzsh-sync     → winzsh-history, winzsh-config, winzsh-plugin (+ optional HTTPS pull)
winzsh-agent    → winzsh-history, winzsh-registry, winzsh-update, winzsh-config
```

Foundation crates (`winzsh-error`, `winzsh-core`, `winzsh-fs`, `winzsh-log`, `winzsh-config`)
sit at the bottom. Engines sit in the middle. CLI/installer sit at the top.

## Hard rules

1. **Acyclic graph.** Cycles fail CI.
2. **No engine → CLI** dependencies.
3. **Network only** in `winzsh-registry`, `winzsh-update`, `winzsh-sync` (HTTPS pull), and `winzsh-agent` (via those crates).
4. **PowerShell string templates only** in `winzsh-powershell`, `winzsh-runtime-gen`, and
   `runtime/powershell/`.
5. Prefer `std` + small deps. Staples: `serde`, `toml`, `thiserror`, `miette`, `tracing`,
   `clap`, `dirs`, `time`. `tokio` only where async IO is real. HTTP clients only in network crates.
   SQLite only behind history.
6. **`#![forbid(unsafe_code)]`** on all crates by default. Opt-out requires an ADR.
7. Binary crate `winzsh` depends **only** on `winzsh-cli` (plus workspace-indirect graph).

## Allowed foundation edges

Most product crates may depend on:

- `winzsh-error`
- `winzsh-core`
- `winzsh-fs` (when performing IO)
- `winzsh-log` (when emitting structured logs from orchestration crates; engines prefer
  `tracing` macros without owning subscriber setup)

`winzsh-test-support` is a **dev-dependency** (or used from `tests/`), never a release
runtime dependency of the binary graph.
