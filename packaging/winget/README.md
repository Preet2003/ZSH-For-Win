# Winget packaging (WinZSH.WinZSH)

## Primary UX (no Rust required)

Share the GitHub Release file **`WinZSH-Setup-x86_64.exe`**:

1. User downloads the `.exe`
2. Double-clicks / runs it (same binary as `winzsh`, auto-runs `setup` when the name contains `Setup`)
3. Opens a new PowerShell tab → `zsh-for-win`

Silent / automation:

```powershell
.\WinZSH-Setup-x86_64.exe setup -y
```

## winget (after community publish)

```powershell
winget install --id WinZSH.WinZSH -e
```

Winget runs the same Setup.exe with `setup -y`.

## Layout

| Path | Role |
|------|------|
| `WinZSH.WinZSH/*.yaml` | Multi-file winget manifest (schema 1.6, `InstallerType: exe`) |
| `../../scripts/release/package.ps1` | Build Setup.exe + update `.exe` + zip + SHA256SUMS |
| `../../scripts/release/fill-winget-manifest.ps1` | Stamp version, URL, SHA256 into manifests |

## Release checklist

1. Bump `[workspace.package].version` in `Cargo.toml` (and CHANGELOG).
2. Tag and push: `git tag v0.1.0 && git push origin v0.1.0`
3. GitHub Actions **Release** workflow publishes **WinZSH-Setup-x86_64.exe**.
4. Fill manifests and PR to `winget-pkgs` under `manifests/w/WinZSH/WinZSH/<version>/`.

Local dry-run:

```powershell
./scripts/release/package.ps1
./scripts/release/fill-winget-manifest.ps1
winget validate --manifest packaging\winget\WinZSH.WinZSH
```

## Artifact contract

| Asset | Consumers |
|-------|-----------|
| `WinZSH-Setup-x86_64.exe` | **Primary share link** — download and run |
| `winzsh-x86_64-pc-windows-msvc.exe` | `winzsh update` (not named Setup) |
| `winzsh-vVERSION-…zip` | Optional archive |
| `SHA256SUMS.txt` | checksums |
