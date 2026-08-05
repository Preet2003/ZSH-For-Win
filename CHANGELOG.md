# Changelog

All notable changes to WinZSH will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Phase 3 smart shell: PSReadLine autosuggest accept (RightArrow/Ctrl+F), syntax colors
- Optional fzf Ctrl+R history and zoxide init when tools are installed
- Phase 2 UX: themed prompt (path + git), aliases, history
- Built-in themes: minimal, classic, powerline, modern, catppuccin, tokyo-night
- CLI: `theme`, `alias`, `history`, `reload`
- History spool (`history/spool.jsonl`) + compacted store
- Phase 1 foundation: `install|uninstall|doctor|config|status`
- Config system, logging, PowerShell profile hook, runtime-gen
- Architecture docs and Rust workspace scaffold
