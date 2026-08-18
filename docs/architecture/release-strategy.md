# Release Strategy

## Versioning

- CLI binary: semver.
- Config schema and plugin manifest schema: independent version numbers.
- Runtime module declares `min_cli_version`; mismatch → safe mode + update nudge.

## Channels

- `stable`
- `beta`

Configured under `[update].channel` in `config.toml`.

## Artifacts

- `WinZSH-Setup-x86_64.exe` — **primary** download-and-run installer (same binary as the CLI)
- `winzsh-x86_64-pc-windows-msvc.exe` — self-update asset (not named Setup)
- Optional zip archive + `SHA256SUMS.txt`
- Future: `aarch64-pc-windows-msvc`, signatures

## Distribution

1. **GitHub Releases — `WinZSH-Setup-x86_64.exe`** (primary share / download-and-run).
2. winget (`winget install --id WinZSH.WinZSH -e`) wraps the same Setup.exe.
3. `winzsh update` for in-place upgrades of non-winget installs.

### Setup.exe (why Rust)

One native binary is the installer **and** the CLI:

- Filename containing `Setup` → running with no args performs full setup
- Explicit: `winzsh setup -y` / `WinZSH-Setup-x86_64.exe setup -y`
- Copies itself to `~/.winzsh/bin/winzsh.exe`, adds user PATH, writes launcher, profile hook, theme

### Winget

- Package id: `WinZSH.WinZSH`
- Installer: `exe` with `Silent: setup -y`
- Manifests: `packaging/winget/WinZSH.WinZSH/`

See [packaging/winget/README.md](../../packaging/winget/README.md).

### Self-update (`winzsh update`)

| Mode | When |
|------|------|
| `winzsh update --from-source [--pull]` | Clone installs |
| `winzsh update --check` | Report only |
| `winzsh update` | GitHub Release `.exe` (not Setup-named) |
| `winzsh update --rollback` | Restore `.bak` |

## Release train

```text
tag vX.Y.Z → Release workflow → WinZSH-Setup-x86_64.exe (+ update .exe) → winget-pkgs PR
```

## Rollback

- Previous binary kept as `~/.winzsh/bin/winzsh.exe.bak`.
- `winzsh update --rollback`.
- Or re-run an older Setup.exe.

## Security

- `SECURITY.md` with private reporting channel.
- 90-day disclosure norm.
- Registry signatures mandatory before community plugins leave experimental.
- Telemetry remains off by default (see ADR-0001).
- GitHub downloads use HTTPS; checksum verification lands with signed release assets.
