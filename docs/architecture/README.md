# WinZSH Architecture

WinZSH is a **Windows Developer Experience Layer** — not a shell, terminal, or prompt theme.
It enhances PowerShell 7 (V1) with zero manual profile editing, a single management binary,
and a generated in-process PowerShell runtime for the interactive hot path.

## Documents

| Document | Purpose |
|----------|---------|
| [overview.md](overview.md) | System topology, runtime split, on-disk layout |
| [crate-map.md](crate-map.md) | Every crate and its responsibility |
| [dependency-rules.md](dependency-rules.md) | Allowed edges, layering, network boundaries |
| [config-schema.md](config-schema.md) | `config.toml` format, layering, migrations |
| [plugin-spec.md](plugin-spec.md) | Plugin package format, trust, lifecycle |
| [coding-conventions.md](coding-conventions.md) | Rust/PowerShell conventions, errors, logging |
| [testing-strategy.md](testing-strategy.md) | Unit → e2e, fuzz, supply chain |
| [release-strategy.md](release-strategy.md) | Channels, artifacts, winget, rollback |
| [adr/0001-foundation.md](adr/0001-foundation.md) | Foundational architectural decisions |

Product vision (non-normative): [Windows shell experience.md](../Windows%20shell%20experience.md).

## Locked decisions (summary)

1. Enhance PowerShell; do not replace it.
2. Hot path = generated PS module; cold path = Rust CLI.
3. Single binary for users; optional agent only later.
4. Plugins V1 = manifest + PS assets + codegen (no native DLLs).
5. Config = versioned TOML; runtime artifacts are derived.
6. Strict crate layers; no network in core engines.
7. Telemetry off by default unless a future ADR opts in.
