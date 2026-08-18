# Registry gate

## Complete

- [x] `winzsh-registry` index fetch/cache/embedded fallback
- [x] SHA-256 verify + zip extract (zip-slip safe)
- [x] CLI: `plugin search|info|update`; `plugin add` resolves registry
- [x] Config `[registry] url` / `require_signature`
- [x] Sample community plugin `demo-aliases` (embedded package)
- [x] Origin metadata `.winzsh-origin.toml`

## Try it

```powershell
cargo run -p winzsh -- plugin search
cargo run -p winzsh -- plugin info demo-aliases
cargo run -p winzsh -- plugin add demo-aliases
# new tab:
la
up
```

Local index override:

```powershell
$env:WINZSH_REGISTRY_URL = "file:///C:/Codes/Personal/ZshForWin/zsh-for-win/registry/index.json"
winzsh plugin search
```

## Next

- Minisign / ed25519 signature verify when `require_signature=true`
- Community submission guide + CI package checks
- Theme registry (same index schema, `kind` field)
