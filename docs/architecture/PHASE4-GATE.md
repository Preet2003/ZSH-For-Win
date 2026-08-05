# Phase 4 review gate

## Complete

- [x] Completion pack catalog (git, docker, kubectl, npm, pnpm, yarn, terraform, ssh, aws, az)
- [x] Auto-detect binaries; only register packs for tools present
- [x] Lazy native completion (`docker|kubectl|az completion …`) cached under `~/.winzsh/cache/completions/`
- [x] Builtin word lists for common CLIs; SSH hosts from `~/.ssh/config`
- [x] Config: `[completions] enabled`, `only = []`
- [x] CLI: `completion list`; doctor reports active packs

## Try it

```powershell
cargo run -p winzsh -- reload
cargo run -p winzsh -- completion list
# new PowerShell tab:
Get-WinZshInfo   # Phase = phase-4
git <Tab>        # subcommands
# if docker installed:
docker <Tab>
```

Optional config filter:

```toml
[completions]
enabled = true
only = ["git", "docker"]
```

## Next

Phase 5 — see [`PHASE5-GATE.md`](PHASE5-GATE.md).
