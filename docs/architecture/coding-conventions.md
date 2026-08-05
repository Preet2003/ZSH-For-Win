# Coding Conventions

## Rust

- **Edition:** 2024 (pinned via `rust-toolchain.toml`).
- **Crate naming:** `winzsh-*`; modules `snake_case`.
- **Docs:** public APIs documented; libs use `#![warn(missing_docs)]`.
- **Unsafe:** `#![forbid(unsafe_code)]` by default; exceptions need an ADR.
- **Panics:** never for user input. `unwrap`/`expect` forbidden in non-test code (clippy/CI).
- **Errors:** `thiserror` enums per crate; CLI maps to `miette::Report` with stable error codes
  (`winzsh::config::invalid_theme`) and exit codes.
- **Logging:** libraries use `tracing` only (no `println!`). Subscriber initialized once in `winzsh`.
- **Formatting:** `rustfmt` + `clippy -D warnings` in CI.
- **Features:** crate features for optional surfaces (`history-sqlite`, `network`, `ai`).
  Default binary enables only what V1 needs.
- **API stability:** crates are internal until a public SDK release. User-facing semver surfaces
  are CLI UX, config schema, and plugin manifest schema.

### Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Usage / clap |
| 3 | Not installed / state conflict |
| 4 | Network |
| 5 | Integrity / signature |

### Logging

- Default level: `info` for CLI ops.
- `--verbose` / `WINZSH_LOG` for debug/trace.
- Sinks: human stderr + rotating file under `~/.winzsh/logs/`.
- `--json` command output is **not** logs (stdout vs tracing sinks).
- Redact secrets and optionally absolute home paths in doctor export bundles.

## PowerShell

- Templates live under `runtime/powershell/`.
- Generated module uses `Set-StrictMode`.
- PSScriptAnalyzer runs in CI for templates and first-party plugins.
- User-facing English strings in V1; CLI strings centralized in `winzsh-cli` for future i18n.

## Commits / PRs

- Small vertical slices.
- Features that can break install land with doctor coverage.
- Do not implement Phase N feature logic before the matching phase gate.
