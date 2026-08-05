# Phase 2 review gate

## Complete

- [x] Theme engine with built-ins (`minimal`, `classic`, `powerline`, `modern`, `catppuccin`, `tokyo-night`)
- [x] Prompt with path + git segments (in-process PowerShell)
- [x] Alias merge (builtins + user) and runtime codegen
- [x] *(Phase 5)* Plugin aliases also merge into the same pipeline (user wins)
- [x] History spool + compacted JSONL store + CLI query
- [x] CLI: `theme`, `alias`, `history`, `reload`

## Try it

```powershell
cargo run -p winzsh -- install --force
cargo run -p winzsh -- theme set modern
# new PowerShell tab:
Get-WinZshInfo
gs
winzsh history list
```

## Next

Phase 3 (Smart shell): autosuggestions, syntax highlighting, fuzzy/zoxide/fzf integrations.
