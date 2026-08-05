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

- `x86_64-pc-windows-msvc`
- `aarch64-pc-windows-msvc`
- Zip archives + checksums + signatures (sigstore/minisign).

## Distribution

1. GitHub Releases (canonical artifacts).
2. **winget** as primary install UX (`winget install winzsh`).
3. `winzsh update` self-update for non-winget installs.

## Release train

```text
tag → CI build/test/sign → GitHub Release → winget manifest PR
```

## Rollback

- Keep previous binary + `state.json` `previous_version`.
- `winzsh update --rollback`.

## Security

- `SECURITY.md` with private reporting channel.
- 90-day disclosure norm.
- Registry signatures mandatory before community plugins leave experimental.
- Telemetry remains off by default (see ADR-0001).
