# Phase 1 review gate

Architecture scaffold deliverables are complete:

- [x] Normative docs under `docs/architecture/`
- [x] Rust workspace + crate stubs with dependency edges
- [x] CI skeleton (`fmt`, `clippy`, `test`, `cargo-deny`, edge checks)
- [x] Minimal `winzsh status` stub only (no installer/prompt/plugin product logic)

**Do not start Phase 1 feature implementation until this gate is explicitly approved.**

Phase 1 scope (after approval): CLI surface for install/uninstall/doctor, config IO,
logging to `~/.winzsh/logs`, installer + PowerShell hook, minimal runtime-gen,
profile backup, verify path.
