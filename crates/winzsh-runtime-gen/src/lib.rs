//! Single writer of runtime artifacts under `~/.winzsh/cache/runtime/`.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::info;
use winzsh_alias::{self as alias};
use winzsh_config::Config;
use winzsh_core::{VERSION, WinzshPaths};
use winzsh_error::Result;
use winzsh_fs::{atomic_write, ensure_dir, read_string};
use winzsh_history::{self as history};
use winzsh_prompt::{self as prompt};
use winzsh_suggest::{self as suggest, SuggestPolicy};
use winzsh_theme::{self as theme};

const PSD1_TEMPLATE: &str = include_str!("../../../runtime/powershell/WinZSH.psd1.template");

/// Hash/lockfile summary of inputs used to decide whether regen is needed.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeLock {
    /// Opaque content hash of generation inputs.
    pub input_hash: String,
    /// CLI version that generated the runtime.
    pub cli_version: String,
    /// Minimum CLI version required to load the module.
    pub min_cli_version: String,
}

/// Outcome of a generate pass.
#[derive(Debug, Clone)]
pub struct GenerateReport {
    /// Whether files were rewritten.
    pub wrote: bool,
    /// Lock describing the current runtime.
    pub lock: RuntimeLock,
}

/// Generate (or skip) the cached PowerShell runtime module from config.
pub fn generate(paths: &WinzshPaths, cfg: &Config) -> Result<GenerateReport> {
    ensure_dir(&paths.runtime_cache())?;
    let _ = history::ensure(paths);
    let psm1 = render_psm1(paths, cfg)?;
    let psd1 = render_psd1();
    let input_hash = hash_inputs(cfg, &psm1, &psd1);
    let lock = RuntimeLock {
        input_hash: input_hash.clone(),
        cli_version: VERSION.to_string(),
        min_cli_version: VERSION.to_string(),
    };

    if let Ok(existing) = read_string(&paths.runtime_lock())
        && let Ok(prev) = serde_json::from_str::<RuntimeLock>(&existing)
        && prev == lock
        && paths.runtime_module().is_file()
        && paths.runtime_manifest().is_file()
    {
        return Ok(GenerateReport { wrote: false, lock });
    }

    atomic_write(&paths.runtime_module(), psm1)?;
    atomic_write(&paths.runtime_manifest(), psd1)?;
    let lock_json = serde_json::to_string_pretty(&lock)
        .map_err(|e| winzsh_error::runtime(format!("serialize runtime.lock.json: {e}")))?;
    atomic_write(&paths.runtime_lock(), lock_json)?;
    info!(hash = %input_hash, "generated WinZSH runtime module");
    Ok(GenerateReport { wrote: true, lock })
}

/// Load config and regenerate runtime artifacts.
pub fn regenerate_from_disk(paths: &WinzshPaths) -> Result<GenerateReport> {
    let cfg = winzsh_config::load(paths)?;
    generate(paths, &cfg)
}

fn hash_inputs(cfg: &Config, psm1: &str, psd1: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(VERSION.as_bytes());
    hasher.update(b"\0");
    if let Ok(raw) = toml::to_string(cfg) {
        hasher.update(raw.as_bytes());
    }
    hasher.update(b"\0");
    hasher.update(psm1.as_bytes());
    hasher.update(b"\0");
    hasher.update(psd1.as_bytes());
    hex::encode(hasher.finalize())
}

fn render_psm1(paths: &WinzshPaths, cfg: &Config) -> Result<String> {
    let resolved = theme::resolve(&cfg.theme)?;
    let mut plan = prompt::plan_from_flags(cfg.prompt.git);
    plan.budget_ms = cfg.prompt.budget_ms;

    let user_aliases = alias::from_user_map(&cfg.aliases)?;
    let aliases = alias::merge(alias::builtin_aliases(), [], user_aliases);

    let history_enabled = cfg.features.history && cfg.history.enabled;
    let policy = SuggestPolicy {
        autosuggestions: cfg.features.autosuggestions,
        syntax: cfg.features.syntax,
        fzf: cfg.smart.fzf,
        zoxide: cfg.smart.zoxide,
        theme_id: cfg.theme.clone(),
    };

    let mut body = String::new();
    body.push_str(&format!(
        "# winzsh-generated theme={} git={} history={} suggest={} syntax={}\n",
        cfg.theme, cfg.prompt.git, history_enabled, policy.autosuggestions, policy.syntax
    ));
    body.push_str("Set-StrictMode -Version Latest\n\n");
    body.push_str(
        r#"
function Get-WinZshInfo {
    [CmdletBinding()]
    param()
    [pscustomobject]@{
        Name    = 'WinZSH'
        Phase   = 'phase-3'
        Theme   = $script:WinZshThemeId
        Message = 'WinZSH runtime loaded (Phase 3 smart shell).'
    }
}
"#,
    );
    body.push_str(&format!(
        "\n$script:WinZshThemeId = '{}'\n",
        prompt::ps_single_quote(&resolved.theme.id)
    ));
    body.push_str(&history::render_powershell(paths, history_enabled));
    body.push_str(&prompt::render_powershell(&resolved.theme, &plan));
    body.push_str(&alias::render_powershell(&aliases));
    body.push_str(&suggest::render_powershell(&policy));
    body.push_str(
        "\nExport-ModuleMember -Function Get-WinZshInfo,Get-WinZshPathSegment,Get-WinZshGitSegment,prompt,Initialize-WinZshSmartShell,Update-WinZshSessionPath,Resolve-WinZshTool\n",
    );
    for name in aliases.aliases.keys() {
        body.push_str(&format!("Export-ModuleMember -Function {name}\n"));
    }
    if history_enabled {
        body.push_str("Export-ModuleMember -Function Write-WinZshHistoryFromPrompt\n");
    }
    // MUST run after exporting `prompt`. zoxide wraps the session prompt to learn `cd` targets;
    // exporting afterward would replace that wrapper and leave the DB empty forever.
    body.push_str("\nInitialize-WinZshSmartShell\n");
    Ok(body)
}

fn render_psd1() -> String {
    PSD1_TEMPLATE.replace("0.1.0", VERSION)
}

#[cfg(test)]
mod tests {
    use super::*;
    use winzsh_fs::ensure_layout;

    #[test]
    fn generate_includes_smart_shell() {
        let root = std::env::temp_dir().join(format!("winzsh-rgen3-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let paths = WinzshPaths::from_root(root.clone());
        ensure_layout(&paths).expect("layout");
        let cfg = Config::default();
        let report = generate(&paths, &cfg).expect("gen");
        assert!(report.wrote);
        let module = read_string(&paths.runtime_module()).expect("read");
        assert!(module.contains("function prompt"));
        assert!(module.contains("AcceptSuggestion"));
        assert!(module.contains("phase-3"));
        assert!(module.contains("Initialize-WinZshSmartShell"));
        let export_at = module
            .rfind("Export-ModuleMember")
            .expect("export");
        let init_at = module.rfind("Initialize-WinZshSmartShell").expect("init");
        assert!(
            init_at > export_at,
            "zoxide/smart init must run after Export-ModuleMember"
        );
        let again = generate(&paths, &cfg).expect("gen2");
        assert!(!again.wrote);
        let _ = std::fs::remove_dir_all(&root);
    }
}
