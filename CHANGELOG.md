# Changelog

All notable changes to WinZSH will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

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
