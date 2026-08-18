# Phase 6 review gate

## Complete

- [x] Opt-in AI (`features.ai`, `winzsh ai enable|disable|status`)
- [x] **Local-only** offline provider: `explain`, `ask`, `alias`, `check`
- [x] No cloud / OpenAI / API keys (free forever)
- [x] Safety heuristics (`ai check`) work even when AI is disabled
- [x] Config `[ai] provider = "local"`; doctor reports AI status
- [x] CLI wired through `winzsh-ai` (no network in this crate)

## Try it

```powershell
cargo run -p winzsh -- ai enable
cargo run -p winzsh -- ai status
cargo run -p winzsh -- ai explain git status
cargo run -p winzsh -- ai ask delete node_modules recursively
cargo run -p winzsh -- ai check "git push --force"
cargo run -p winzsh -- ai alias "git status short"
```

## Related

Multi-shell / agent — see [`MULTI-SHELL-GATE.md`](MULTI-SHELL-GATE.md).
Plugin registry — see [`REGISTRY-GATE.md`](REGISTRY-GATE.md).
Settings sync — see [`SYNC-GATE.md`](SYNC-GATE.md).
