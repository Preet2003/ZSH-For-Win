# Plugin registry

Community plugins are published through a JSON **index** and checksummed **zip** packages.

## User commands

```powershell
winzsh plugin search
winzsh plugin search kubectl
winzsh plugin info demo-aliases
winzsh plugin add demo-aliases      # registry (after first-party miss)
winzsh plugin update               # refresh registry-origin plugins
winzsh plugin update demo-aliases
```

Resolution order for `plugin add <id>`:

1. Local path (`./foo` or `C:\…`)
2. First-party embedded (`docker`, `git`, `node`, `rust`)
3. Registry index

## Index URL

| Priority | Source |
|----------|--------|
| 1 | `WINZSH_REGISTRY_URL` |
| 2 | `[registry].url` in `config.toml` |
| 3 | `https://raw.githubusercontent.com/winzsh/winzsh/main/registry/index.json` |

Offline: network failure → `~/.winzsh/cache/registry/index.json` → embedded index shipped in the CLI.

## Trust

- **SHA-256** of the zip is always verified.
- **Signature** field is optional until `registry.require_signature = true`.
- Provenance written to `~/.winzsh/plugins/<id>/.winzsh-origin.toml`.

## Publishing a community plugin

1. Add `registry/plugins/<id>/plugin.toml` (+ hooks/completions).
2. Run `./scripts/release/pack-registry.ps1` (builds `registry/packages/<id>-<ver>.zip` + refreshes hashes).
3. Prefer `download_url: "embedded:<id>"` for packs bundled in the CLI, or an `https://…zip` URL for external hosts.
4. Update `crates/winzsh-registry/src/lib.rs` `EMBEDDED_PACKAGES` when bundling a new embedded zip.
5. Commit `registry/index.json`, `registry/packages/`, and plugin sources.

## Layout

```text
registry/
  index.json
  plugins/<id>/          # source
  packages/<id>-ver.zip  # distributable
```
