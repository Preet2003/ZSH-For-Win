# Sync gate

## Complete

- [x] Bundle format (`winzsh-sync.json`, schema 1): config + optional plugins + optional history
- [x] `winzsh sync status|export|import|push|pull`
- [x] Config `[sync]` destination / include flags; `WINZSH_SYNC_DEST`
- [x] Config backup on import; sanitize machine-local fields (`update.source_dir`)
- [x] HTTPS **pull** only (push stays local/OneDrive/USB/git folder)

## Try it

```powershell
# Machine A
winzsh sync export -p $env:OneDrive\winzsh-sync.json --plugins
# or configure once:
# [sync]
# destination = "C:/Users/you/OneDrive/winzsh-sync.json"
winzsh sync push --plugins

# Machine B
winzsh sync import -p $env:OneDrive\winzsh-sync.json
# or
winzsh sync pull
```

## Out of scope (later)

- Authenticated cloud push (gist/S3)
- Conflict merge UI
- Encrypted bundles
