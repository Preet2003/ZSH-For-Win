# ADR-0001: Foundation Architecture

## Status

Accepted

## Context

WinZSH aims to be the Windows equivalent of Oh My Zsh: a zero-config developer experience
layer over existing shells, starting with PowerShell 7. It must scale to millions of users,
remain maintainable for years, and avoid becoming another shell or terminal emulator.

## Decisions

1. **Enhance PowerShell; do not replace it.**
2. **Hot path = generated in-process PowerShell module; cold path = Rust CLI.**
   Never spawn `winzsh.exe` per prompt.
3. **Single user-facing binary** (`winzsh`). Optional background agent only in later phases.
4. **Plugins V1** are signed/trusted packages of manifest + PowerShell assets, materialized by
   codegen — not native DLLs.
5. **Config** is versioned TOML; runtime artifacts under `~/.winzsh/cache/` are derived.
6. **Strict crate layering** with no network access from core experience engines.
7. **Telemetry off by default** unless a future ADR explicitly opts in.

## Consequences

- Crate graph is larger early, but teams can evolve installer, prompt, and registry independently.
- PowerShell templates and Rust contracts must stay versioned together (`runtime.lock.json`).
- Multi-shell support later fits behind `ShellHost` without rewriting engines.
- Community plugins require a signed registry before broad enablement.

## Alternatives considered

| Alternative | Why rejected |
|-------------|--------------|
| Spawn Rust binary every prompt | Latency and process churn at interactive scale |
| Always-on daemon in V1 | Operational complexity before product-market fit |
| Native DLL plugins in V1 | Security and support burden |
| Hand-edited profiles as primary UX | Violates zero-config mission |
