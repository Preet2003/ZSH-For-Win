# WinZSH

**Oh My Zsh-style developer experience for Windows - without replacing your shell.**

WinZSH is a **Windows Developer Experience Layer**. It is not another shell, not another terminal emulator, and not just a prompt theme. It enhances **PowerShell 7** (with Windows PowerShell as a fallback) so you get a modern interactive experience with **zero manual profile editing**.

One Setup.exe. One management binary (`winzsh`). Nested sessions via `zsh-for-win`. Stock PowerShell stays stock until you opt in.

> **AI is local-only** - offline heuristics, no API keys, no cloud billing.

**Repo:** [github.com/Preet2003/ZSH-For-Win](https://github.com/Preet2003/ZSH-For-Win)

---

## Why WinZSH exists

macOS and Linux developers often get a polished shell story for free: Oh My Zsh, Starship, fish, rich completions, fuzzy history. On Windows, the same bar usually means hand-editing `$PROFILE`, stitching PSReadLine/themes/completions yourself, and losing that setup on the next machine.

WinZSH's mission:

> Install once -> open a new tab -> type `zsh-for-win` -> feel at home.

Design principles:

1. **Never ask users to edit profile files** - the installer owns a marked hook block.
2. **Single user-facing binary** - `winzsh` (plus nested launcher `zsh-for-win`).
3. **Hot path stays in PowerShell** - Rust builds/manages; a generated module runs interactively.
4. **Opt-in activation** - everyday PowerShell stays clean until you enter WinZSH.
5. **Free forever for AI** - local helpers only.

---

## Features

| Area | What you get |
|------|----------------|
| Prompt & themes | Path + git-aware prompt; `modern`, `powerline`, `catppuccin`, `tokyo-night`, … |
| Autosuggest / syntax | Ghost text (RightArrow / Ctrl+F); PSReadLine colors |
| History | Spool + compact store; optional **fzf** Ctrl+R; optional **zoxide** |
| Completions | Lazy packs: git, docker, kubectl, npm/pnpm/yarn, terraform, ssh, aws, az, … |
| Aliases | Builtins + plugins + `winzsh alias set` |
| Plugins | First-party + community registry (`plugin search` / `add` / `update`) |
| Local AI | Opt-in `explain` / `ask` / `check` / `alias` (offline only) |
| Sync | Export/import JSON bundles (OneDrive/USB); HTTPS pull |
| Update | GitHub Releases or `--from-source` |
| Multi-shell | CMD launcher; experimental Nu/Bash bridges |
| Agent | `winzsh agent` background maintenance (same binary) |

---

## How activation works

| Action | Effect |
|--------|--------|
| `zsh-for-win` | Enter nested WinZSH session; sets global `shell.active` lock |
| Other stock terminals | Can auto-join on next prompt while lock exists |
| `exit` in any WinZSH session | Clears lock -> back to stock |

```powershell
zsh-for-win
Get-WinZshInfo
exit
```

---

## Install

### A) Download Setup.exe (recommended)

1. Publish a GitHub Release (see below) **or** open [Releases](https://github.com/Preet2003/ZSH-For-Win/releases).
2. Download **`WinZSH-Setup-x86_64.exe`**.
3. Run it:

```powershell
.\WinZSH-Setup-x86_64.exe setup -y
```

4. Open a **new** PowerShell tab, then `zsh-for-win`.

Direct URL (works only after a release asset exists):

`https://github.com/Preet2003/ZSH-For-Win/releases/latest/download/WinZSH-Setup-x86_64.exe`

#### Create the first release (fixes website 404)

```powershell
cd zsh-for-win   # workspace root
./scripts/release/package.ps1
```

Then on GitHub:

1. Open [Create a new release](https://github.com/Preet2003/ZSH-For-Win/releases/new)
2. Tag: `v0.1.0` (or newer)
3. Upload from `dist/`:
   - **`WinZSH-Setup-x86_64.exe`** (required for the website Download button)
   - `winzsh-x86_64-pc-windows-msvc.exe` (for `winzsh update`)
   - optional zip + `SHA256SUMS.txt`
4. Publish the release

### B) From source

```powershell
git clone https://github.com/Preet2003/ZSH-For-Win.git
cd ZSH-For-Win
Set-ExecutionPolicy -Scope Process Bypass
./scripts/install-from-source.ps1
```

Needs Git + Rust 1.85+ + PowerShell 7 recommended.

### C) winget (when published)

```powershell
winget install --id WinZSH.WinZSH -e
```

---

## Quick start

```powershell
zsh-for-win
winzsh doctor
winzsh theme set modern
winzsh plugin add git
winzsh plugin add docker
winzsh ai enable
winzsh ai check "git push --force"
exit
```

Uninstall:

```powershell
winzsh uninstall           # remove profile hook; keep data
winzsh uninstall --purge   # delete ~/.winzsh too
```

---

## Configuration

`%USERPROFILE%\.winzsh\config.toml`

```powershell
winzsh config path
winzsh config show
winzsh config validate
```

See [`docs/architecture/config-schema.md`](docs/architecture/config-schema.md). AI: `provider = "local"` only.

---

## Website / SEO

Static landing page: [`website/`](website/). Host with GitHub Pages from `/website`.

Download buttons use your real repo release asset URL (not a placeholder org).

---

## Developing

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
./scripts/ci/check-crate-deps.ps1
```

Docs: [`docs/essential-commands.md`](docs/essential-commands.md), [`docs/architecture/`](docs/architecture/).

---

## License

MIT OR Apache-2.0

## Links

- Repo: https://github.com/Preet2003/ZSH-For-Win
- Releases: https://github.com/Preet2003/ZSH-For-Win/releases
- Issues: https://github.com/Preet2003/ZSH-For-Win/issues
