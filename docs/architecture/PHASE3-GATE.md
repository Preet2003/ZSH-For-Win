# Phase 3 review gate

## Complete

- [x] PSReadLine history autosuggestions (`PredictionSource History`, InlineView)
- [x] RightArrow / Ctrl+F accept suggestion; Ctrl+Right accepts next word
- [x] Theme-aware syntax highlighting (`Set-PSReadLineOption -Colors`)
- [x] Optional `fzf` Ctrl+R fuzzy history (when installed)
- [x] Optional `zoxide` init (when installed)
- [x] Config: `features.autosuggestions`, `features.syntax`, `smart.fzf`, `smart.zoxide`

## Try it

```powershell
cargo run -p winzsh -- reload
# new PowerShell tab:
Get-WinZshInfo   # Phase field tracks the latest shipped phase (currently phase-5)
# type part of a previous command — ghost text appears
# press RightArrow to accept
```

Optional installs:

```powershell
winget install junegunn.fzf
winget install ajeetdsouza.zoxide
cargo run -p winzsh -- reload
```

## Next

Phase 4 — see [`PHASE4-GATE.md`](PHASE4-GATE.md).
