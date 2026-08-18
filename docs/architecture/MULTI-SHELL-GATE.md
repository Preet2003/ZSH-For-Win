# Multi-shell / agent gate

## Complete

- [x] Shell catalog: PowerShell (primary), CMD launcher, Nu / Bash experimental bridges
- [x] `winzsh shell list|status|enable|disable`
- [x] Config `[shells].enabled` for opt-in hosts (PowerShell stays install-managed)
- [x] Background agent as **same binary**: `winzsh agent status|start|stop|run-once|run`
- [x] Agent ticks: history compact, registry refresh, optional update check
- [x] Pid + heartbeat under `~/.winzsh/locks/`

## Try it

```powershell
winzsh shell list
winzsh shell enable cmd
# optional experimental bridges:
winzsh shell enable nu
winzsh shell enable bash

winzsh agent run-once
winzsh agent start
winzsh agent status
winzsh agent stop
```

## Design notes

- Full WinZSH runtime remains PowerShell-only. Nu/Bash hooks only expose a `winzsh` / `zsh-for-win` bridge into a nested PowerShell session.
- CMD integration is a `~/.winzsh/bin/zsh-for-win.cmd` launcher (no AutoRun registry edits).
- Agent is **not** a separate user-facing exe (ADR: single binary). Detached `winzsh agent run` is started by `agent start`.

## Out of scope (later)

- Native Nu/Bash prompts, completions, and plugins
- Windows service / Task Scheduler auto-start for the agent
- Authenticated sync from the agent
- CMD AutoRun profile injection
