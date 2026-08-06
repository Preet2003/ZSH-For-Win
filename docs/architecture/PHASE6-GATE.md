# Phase 6 review gate

## Complete

- [x] Opt-in AI (`features.ai`, `winzsh ai enable|disable|status`)
- [x] Local offline provider: `explain`, `ask`, `alias`, `check`
- [x] Optional OpenAI-compatible cloud (`ai.provider=openai` + `WINZSH_AI_API_KEY`)
- [x] Safety heuristics (`ai check`) work even when AI is disabled
- [x] Config `[ai]` provider/model/api_base; doctor reports AI status
- [x] CLI wired through `winzsh-ai` (network allowed in this crate only)

## Try it

```powershell
cargo run -p winzsh -- ai enable
cargo run -p winzsh -- ai status
cargo run -p winzsh -- ai explain git status
cargo run -p winzsh -- ai ask delete node_modules recursively
cargo run -p winzsh -- ai check "git push --force"
cargo run -p winzsh -- ai alias "git status short"
```

Optional cloud:

```toml
# ~/.winzsh/config.toml
[features]
ai = true

[ai]
provider = "openai"
model = "gpt-4o-mini"
api_base = "https://api.openai.com/v1"
```

```powershell
$env:WINZSH_AI_API_KEY = "sk-..."
cargo run -p winzsh -- ai ask list files including hidden
```

## Next

Plugin registry (signed community packs) and/or sync — separate tracks from AI.
