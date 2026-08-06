# Phase 5 review gate

## Complete

- [x] Plugin manifest (`plugin.toml`) parse + validation
- [x] Install lifecycle: `plugin add|remove|enable|disable|list`
- [x] First-party packs: `docker`, `git`, `node`, `rust` (embedded + `plugins/`)
- [x] Runtime-gen merges plugin aliases (user wins) + failure-isolated hooks
- [x] Command gating via `commands = [...]` + detect
- [x] Config `[plugins].enabled`; doctor reports missing/inactive plugins

## Try it

```powershell
cargo run -p winzsh -- plugin list
cargo run -p winzsh -- plugin add docker
cargo run -p winzsh -- plugin add git
cargo run -p winzsh -- plugin add rust
# new PowerShell tab:
Get-WinZshInfo   # Phase = phase-5; Plugins includes enabled packs
dps              # docker ps (docker plugin)
gst              # git status -sb (git plugin)
cb               # cargo build (rust plugin)
```

Local path install (warning printed):

```powershell
cargo run -p winzsh -- plugin add .\plugins\node
```

## Next

Phase 6 — see [`PHASE6-GATE.md`](PHASE6-GATE.md).
