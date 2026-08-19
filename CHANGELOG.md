# Changelog

All notable changes to WinZSH will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Multi-shell bridges: `shell list|status|enable|disable` (CMD launcher; experimental Nu/Bash)
- Background agent: `agent status|start|stop|run-once|run` (same `winzsh` binary)
- Config `[shells]` and `[agent]`
- AI is **local-only** (OpenAI/cloud path removed; offline heuristics)
- Setup.exe keeps the console open until Enter (clear repair messages when already installed)
- Settings sync: `sync status|export|import|push|pull` (OneDrive/USB/JSON bundle; optional plugins/history)
- Plugin registry: `plugin search|info|update`; `plugin add` resolves community index (SHA-256 zips)
- Sample community plugin `demo-aliases` (embedded package + `registry/`)
- Download-and-run Setup.exe: `winzsh setup` (auto-runs when binary name contains `Setup`)
- Winget packaging: `WinZSH.WinZSH` manifests, release package script, GitHub Release workflow
- Self-update: `winzsh update` / `--check` / `--from-source [--pull]` / `--rollback`
- Config `[update].github_repo` and `[update].source_dir`; install-from-source records checkout path
- Phase 6 AI helpers: `ai enable|disable|status|explain|ask|check|alias` (opt-in; local + optional OpenAI)
- Phase 5 plugin manager: `plugin list|add|remove|enable|disable`
- First-party plugins: docker, git, node, rust (aliases + optional hooks)
- Plugin materialization into runtime (failure-isolated); config `[plugins].enabled`
- Phase 4 completion packs: git, docker, kubectl, npm, pnpm, yarn, terraform, ssh, aws, az
- Lazy native completers cached under `~/.winzsh/cache/completions/`
- CLI: `completion list`; config `[completions]`
- Phase 3 smart shell: PSReadLine autosuggest accept (RightArrow/Ctrl+F), syntax colors
- Optional fzf Ctrl+R history and zoxide init when tools are installed
- Phase 2 UX: themed prompt (path + git), aliases, history
- Built-in themes: minimal, classic, powerline, modern, catppuccin, tokyo-night
- CLI: `theme`, `alias`, `history`, `reload`
- History spool (`history/spool.jsonl`) + compacted store
- Phase 1 foundation: `install|uninstall|doctor|config|status`
- Config system, logging, PowerShell profile hook, runtime-gen
- Architecture docs and Rust workspace scaffold
