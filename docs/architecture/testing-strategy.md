# Testing Strategy

| Layer | Where | What |
|-------|-------|------|
| Unit | each crate | parse / migrate / merge / conflict logic |
| Contract | `winzsh-shell-host` mocks | host trait behavior |
| Golden | `winzsh-runtime-gen` | generated PS snippets vs fixtures |
| Integration | `tests/` + temp home | install → doctor → plugin add → regen |
| E2E | CI Windows runners | real `pwsh`, profile load, prompt non-empty |
| Perf | benches + CI budgets | runtime-gen time; prompt script targets |
| Fuzz | config/manifest parsers | `cargo fuzz` on TOML inputs |
| Supply chain | `cargo deny`, audit, SBOM | license + advisory gate |

## Rules

- Tests use `winzsh-test-support` for frozen clocks/PATH and temp `~/.winzsh`.
- No network in unit/integration unless explicitly marked.
- High coverage on config/plugin/alias merge and installer state machine.
- Pragmatic coverage on CLI formatting.
- Deterministic goldens: normalize line endings and absolute paths in assertions.
