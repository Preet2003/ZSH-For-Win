//! Completion pack catalog and lazy-load PowerShell codegen.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use winzsh_core::WinzshPaths;
use winzsh_detect::DetectionReport;

/// How a completion pack is materialized at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionStrategy {
    /// Curated subcommand / argument list.
    Builtin,
    /// Lazy-load via `<cmd> completion powershell` (or equivalent) into cache.
    NativeGenerate,
    /// SSH hosts from `~/.ssh/config`.
    SshHosts,
}

/// Descriptor for a completion pack.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionPack {
    /// Stable pack id.
    pub id: String,
    /// Primary command name (Tab target).
    pub command: String,
    /// Extra binaries that enable this pack (any match).
    pub detect: Vec<String>,
    /// Materialization strategy.
    pub strategy: CompletionStrategy,
    /// Builtin words (used for Builtin and as NativeGenerate first-Tab fallback).
    pub words: Vec<String>,
    /// Optional native generator argv after the command (e.g. `completion`, `powershell`).
    pub native_args: Vec<String>,
}

/// Completion feature policy from config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionPolicy {
    /// Master enable.
    pub enabled: bool,
    /// If non-empty, only these pack ids are registered.
    pub only: Vec<String>,
}

impl Default for CompletionPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            only: Vec::new(),
        }
    }
}

/// First-party Phase 4 completion packs.
pub fn builtin_packs() -> Vec<CompletionPack> {
    vec![
        pack(
            "git",
            "git",
            &["git"],
            CompletionStrategy::Builtin,
            &[
                "status", "add", "commit", "push", "pull", "fetch", "clone", "checkout", "switch",
                "branch", "merge", "rebase", "log", "diff", "stash", "remote", "reset", "tag",
                "cherry-pick", "restore", "clean", "init", "submodule",
            ],
            &[],
        ),
        pack(
            "docker",
            "docker",
            &["docker"],
            CompletionStrategy::NativeGenerate,
            &[
                "build", "run", "ps", "images", "pull", "push", "exec", "logs", "compose", "volume",
                "network", "stop", "start", "rm", "rmi", "inspect", "login", "logout", "system",
            ],
            &["completion", "powershell"],
        ),
        pack(
            "kubectl",
            "kubectl",
            &["kubectl"],
            CompletionStrategy::NativeGenerate,
            &[
                "get", "describe", "apply", "delete", "logs", "exec", "port-forward", "config",
                "rollout", "scale", "create", "edit", "run", "top", "cordon", "drain",
            ],
            &["completion", "powershell"],
        ),
        pack(
            "npm",
            "npm",
            &["npm"],
            CompletionStrategy::Builtin,
            &[
                "install", "ci", "run", "test", "publish", "init", "update", "outdated", "audit",
                "exec", "pack", "link", "unlink", "cache", "config", "version",
            ],
            &[],
        ),
        pack(
            "pnpm",
            "pnpm",
            &["pnpm"],
            CompletionStrategy::Builtin,
            &[
                "install", "add", "remove", "run", "test", "exec", "dlx", "update", "outdated",
                "store", "env", "publish", "init",
            ],
            &[],
        ),
        pack(
            "yarn",
            "yarn",
            &["yarn"],
            CompletionStrategy::Builtin,
            &[
                "install", "add", "remove", "run", "test", "build", "workspace", "workspaces",
                "upgrade", "init", "dlx",
            ],
            &[],
        ),
        pack(
            "terraform",
            "terraform",
            &["terraform"],
            CompletionStrategy::Builtin,
            &[
                "init", "plan", "apply", "destroy", "validate", "fmt", "state", "output", "workspace",
                "import", "taint", "untaint", "force-unlock", "providers",
            ],
            &[],
        ),
        pack(
            "ssh",
            "ssh",
            &["ssh"],
            CompletionStrategy::SshHosts,
            &[],
            &[],
        ),
        pack(
            "aws",
            "aws",
            &["aws"],
            CompletionStrategy::Builtin,
            &[
                "s3", "ec2", "iam", "lambda", "sts", "cloudformation", "ecs", "eks", "rds", "logs",
                "configure", "help",
            ],
            &[],
        ),
        pack(
            "az",
            "az",
            &["az"],
            CompletionStrategy::NativeGenerate,
            &[
                "login", "account", "group", "vm", "aks", "acr", "storage", "network", "webapp",
                "keyvault", "configure", "version",
            ],
            // `az` ships completer via `az.completion.sh` style; on Windows try:
            &["completion", "-s", "powershell"],
        ),
    ]
}

fn pack(
    id: &str,
    command: &str,
    detect: &[&str],
    strategy: CompletionStrategy,
    words: &[&str],
    native_args: &[&str],
) -> CompletionPack {
    CompletionPack {
        id: id.into(),
        command: command.into(),
        detect: detect.iter().map(|s| (*s).to_string()).collect(),
        strategy,
        words: words.iter().map(|s| (*s).to_string()).collect(),
        native_args: native_args.iter().map(|s| (*s).to_string()).collect(),
    }
}

/// Whether a pack should be active given detection + policy.
pub fn pack_enabled(pack: &CompletionPack, detected: &DetectionReport, policy: &CompletionPolicy) -> bool {
    if !policy.enabled {
        return false;
    }
    if !policy.only.is_empty() && !policy.only.iter().any(|id| id == &pack.id) {
        return false;
    }
    pack.detect.iter().any(|name| command_present(detected, name))
}

fn command_present(detected: &DetectionReport, name: &str) -> bool {
    // Trust DetectionReport.commands (populated by detect_environment with PATH + WinGet).
    // Avoid re-probing PATH here so codegen/tests stay deterministic.
    detected
        .commands
        .iter()
        .any(|c| c.eq_ignore_ascii_case(name))
}

/// Render PowerShell that registers lazy completers for enabled packs.
pub fn render_powershell(
    paths: &WinzshPaths,
    detected: &DetectionReport,
    policy: &CompletionPolicy,
) -> String {
    if !policy.enabled {
        return "\n# --- completions (phase 4): disabled ---\n".into();
    }

    let packs = builtin_packs();
    let active: Vec<&CompletionPack> = packs
        .iter()
        .filter(|p| pack_enabled(p, detected, policy))
        .collect();

    let cache_dir = paths
        .root
        .join("cache")
        .join("completions")
        .display()
        .to_string()
        .replace('\'', "''");

    let mut out = String::from("\n# --- completions (phase 4) ---\n");
    out.push_str(&format!(
        "$script:WinZshCompletionCache = '{cache_dir}'\n$script:WinZshCompletionLoaded = @{{}}\n"
    ));

    // ArgumentCompleter scriptblocks run outside module scope — never call module-private
    // functions from them. Capture locals via GetNewClosure; emit CompletionResult inline.
    out.push_str(
        r#"
function Register-WinZshBuiltinCompleter {
    param(
        [Parameter(Mandatory)][string]$CommandName,
        [Parameter(Mandatory)][string[]]$Words
    )
    $wordList = @($Words)
    Register-ArgumentCompleter -Native -CommandName $CommandName -ScriptBlock {
        param($wordToComplete, $commandAst, $cursorPosition)
        $prefix = [string]$wordToComplete + '*'
        foreach ($w in $wordList) {
            if ($w -like $prefix) {
                [System.Management.Automation.CompletionResult]::new($w, $w, 'ParameterValue', $w)
            }
        }
    }.GetNewClosure()
}

function Register-WinZshLazyNativeCompleter {
    param(
        [Parameter(Mandatory)][string]$CommandName,
        [Parameter(Mandatory)][string[]]$NativeArgs,
        [Parameter(Mandatory)][string[]]$FallbackWords
    )
    $cacheDir = $script:WinZshCompletionCache
    $loadedMap = $script:WinZshCompletionLoaded
    $cacheFile = Join-Path $cacheDir ($CommandName + '.ps1')
    $fallback = @($FallbackWords)
    $argsCopy = @($NativeArgs)
    Register-ArgumentCompleter -Native -CommandName $CommandName -ScriptBlock {
        param($wordToComplete, $commandAst, $cursorPosition)
        if (-not $loadedMap.ContainsKey($CommandName)) {
            try {
                if (-not (Test-Path -LiteralPath $cacheDir)) {
                    New-Item -ItemType Directory -Path $cacheDir -Force | Out-Null
                }
                if (-not (Test-Path -LiteralPath $cacheFile)) {
                    $generated = & $CommandName @argsCopy 2>$null | Out-String
                    if (-not [string]::IsNullOrWhiteSpace($generated)) {
                        Set-Content -LiteralPath $cacheFile -Value $generated -Encoding utf8
                    }
                }
                if (Test-Path -LiteralPath $cacheFile) {
                    . $cacheFile
                }
            } catch {
                Write-Verbose "WinZSH: native completion for $CommandName failed: $_"
            } finally {
                $loadedMap[$CommandName] = $true
            }
        }
        $prefix = [string]$wordToComplete + '*'
        foreach ($w in $fallback) {
            if ($w -like $prefix) {
                [System.Management.Automation.CompletionResult]::new($w, $w, 'ParameterValue', $w)
            }
        }
    }.GetNewClosure()
}

function Initialize-WinZshCompletions {
    [CmdletBinding()]
    param()
"#,
    );

    if active.is_empty() {
        out.push_str("    Write-Verbose 'WinZSH: no completion packs matched detected tools'\n}\n");
        return out;
    }

    for pack in &active {
        match pack.strategy {
            CompletionStrategy::Builtin => {
                let words = ps_string_array(&pack.words);
                out.push_str(&format!(
                    "    Register-WinZshBuiltinCompleter -CommandName '{cmd}' -Words @({words})\n",
                    cmd = pack.command,
                ));
            }
            CompletionStrategy::NativeGenerate => {
                let words = ps_string_array(&pack.words);
                let native = ps_string_array(&pack.native_args);
                out.push_str(&format!(
                    "    Register-WinZshLazyNativeCompleter -CommandName '{cmd}' -NativeArgs @({native}) -FallbackWords @({words})\n",
                    cmd = pack.command,
                ));
            }
            CompletionStrategy::SshHosts => {
                out.push_str(
                    r#"
    Register-ArgumentCompleter -Native -CommandName ssh -ScriptBlock {
        param($wordToComplete, $commandAst, $cursorPosition)
        $prefix = [string]$wordToComplete + '*'
        $config = Join-Path $HOME '.ssh\config'
        if (-not (Test-Path -LiteralPath $config)) { return }
        $hosts = Get-Content -LiteralPath $config -ErrorAction SilentlyContinue |
            ForEach-Object {
                if ($_ -match '^\s*Host\s+(.+)$') {
                    $Matches[1].Split(' ', [System.StringSplitOptions]::RemoveEmptyEntries) |
                        Where-Object { $_ -ne '*' -and $_ -notlike '*[*]*' }
                }
            } | Select-Object -Unique
        foreach ($h in $hosts) {
            if ($h -like $prefix) {
                [System.Management.Automation.CompletionResult]::new($h, $h, 'ParameterValue', $h)
            }
        }
    }
"#,
                );
            }
        }
        out.push_str(&format!(
            "    Write-Verbose 'WinZSH: registered completion pack {}'\n",
            pack.id
        ));
    }

    out.push_str("}\n");
    out
}

fn ps_string_array(items: &[String]) -> String {
    items
        .iter()
        .map(|s| format!("'{}'", s.replace('\'', "''")))
        .collect::<Vec<_>>()
        .join(",")
}

/// Pack ids that would be active for this environment.
pub fn active_pack_ids(detected: &DetectionReport, policy: &CompletionPolicy) -> Vec<String> {
    builtin_packs()
        .into_iter()
        .filter(|p| pack_enabled(p, detected, policy))
        .map(|p| p.id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docker_pack_when_detected() {
        let detected = DetectionReport {
            commands: vec!["docker".into()],
            ..DetectionReport::default()
        };
        let ids = active_pack_ids(&detected, &CompletionPolicy::default());
        assert!(ids.contains(&"docker".into()));
        assert!(!ids.contains(&"kubectl".into()));
    }

    #[test]
    fn render_registers_git() {
        let paths = WinzshPaths::from_root(std::env::temp_dir().join("wz-comp"));
        let detected = DetectionReport {
            commands: vec!["git".into(), "ssh".into()],
            ..DetectionReport::default()
        };
        let ps = render_powershell(&paths, &detected, &CompletionPolicy::default());
        assert!(ps.contains("Register-WinZshBuiltinCompleter"));
        assert!(ps.contains("CommandName ssh"));
        assert!(ps.contains(".ssh\\config") || ps.contains(".ssh/config") || ps.contains("'.ssh"));
        assert!(ps.contains("Initialize-WinZshCompletions"));
        assert!(ps.contains("CompletionResult]::new"));
        assert!(!ps.contains("New-WinZshCompletionResult"));
    }
}
