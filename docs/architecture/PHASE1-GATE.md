# Phase 1 review gate

## Scaffold (complete)

- [x] Normative docs under `docs/architecture/`
- [x] Rust workspace + crate stubs with dependency edges
- [x] CI skeleton (`fmt`, `clippy`, `test`, `cargo-deny`, edge checks)

## Phase 1 foundation (complete)

- [x] CLI: `install`, `uninstall`, `doctor`, `config`, `status`
- [x] Config IO (`config.toml` load/save/validate/migrate)
- [x] Logging to stderr + `~/.winzsh/logs/winzsh.log`
- [x] Installer + PowerShell managed profile hook
- [x] Minimal runtime-gen (`cache/runtime/WinZSH.psm1`)
- [x] Profile backup under `~/.winzsh/backups/profile/`
- [x] Doctor diagnostics / verify path

## Try it

```powershell
cargo run -p winzsh -- install
cargo run -p winzsh -- doctor
cargo run -p winzsh -- status
# open a new PowerShell tab, then:
Get-WinZshInfo
```

## Next

Phase 2 (UX): prompt engine, git segment, themes, aliases, history.
