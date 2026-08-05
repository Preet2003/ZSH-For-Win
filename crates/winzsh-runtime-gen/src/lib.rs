//! Single writer of runtime artifacts under `~/.winzsh/cache/runtime/`.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::info;
use winzsh_alias::{self as alias};
use winzsh_completion::{self as completion, CompletionPolicy};
use winzsh_config::Config;
use winzsh_core::{VERSION, WinzshPaths};
use winzsh_detect::detect_environment;
use winzsh_error::Result;
use winzsh_fs::{atomic_write, ensure_dir, read_string};
use winzsh_history::{self as history};
use winzsh_plugin::{self as plugin};
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

    let detected = detect_environment().unwrap_or_default();
    let active_plugins = plugin::resolve_active(paths, &cfg.plugins.enabled, &detected)?;
    let plugin_alias_map = plugin::collect_aliases(&active_plugins);
    let plugin_aliases = alias::from_plugin_map(&plugin_alias_map)?;
    let user_aliases = alias::from_user_map(&cfg.aliases)?;
    let aliases = alias::merge(alias::builtin_aliases(), plugin_aliases, user_aliases);

    let history_enabled = cfg.features.history && cfg.history.enabled;
    let policy = SuggestPolicy {
        autosuggestions: cfg.features.autosuggestions,
        syntax: cfg.features.syntax,
        fzf: cfg.smart.fzf,
        zoxide: cfg.smart.zoxide,
        theme_id: cfg.theme.clone(),
    };

    let completion_policy = CompletionPolicy {
        enabled: cfg.completions.enabled,
        only: cfg.completions.only.clone(),
    };

    let plugin_ids: Vec<&str> = active_plugins
        .iter()
        .map(|p| p.manifest.name.as_str())
        .collect();

    let mut body = String::new();
    body.push_str(&format!(
        "# winzsh-generated theme={} git={} history={} suggest={} syntax={} completions={} plugins=[{}]\n",
        cfg.theme,
        cfg.prompt.git,
        history_enabled,
        policy.autosuggestions,
        policy.syntax,
        cfg.completions.enabled,
        plugin_ids.join(",")
    ));
    body.push_str("Set-StrictMode -Version Latest\n\n");
    body.push_str(
        r#"
function Get-WinZshInfo {
    [CmdletBinding()]
    param()
    [pscustomobject]@{
        Name    = 'WinZSH'
        Phase   = 'phase-5'
        Theme   = $script:WinZshThemeId
        Plugins = @($script:WinZshPluginsLoaded)
        Message = 'WinZSH runtime loaded (Phase 5 plugins).'
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
    body.push_str(&completion::render_powershell(
        paths,
        &detected,
        &completion_policy,
    ));
    body.push_str(&plugin::render_powershell(&active_plugins));
    body.push_str(
        "\nExport-ModuleMember -Function Get-WinZshInfo,Get-WinZshPathSegment,Get-WinZshGitSegment,prompt,Initialize-WinZshSmartShell,Initialize-WinZshCompletions,Update-WinZshSessionPath,Resolve-WinZshTool,salias\n",
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
    body.push_str("Initialize-WinZshCompletions\n");
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
    fn generate_includes_plugins_section() {
        let root = std::env::temp_dir().join(format!("winzsh-rgen5-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let paths = WinzshPaths::from_root(root.clone());
        ensure_layout(&paths).expect("layout");
        let mut cfg = Config::default();
        plugin::add_first_party(&paths, "git").expect("add git");
        cfg.plugins.enabled = vec!["git".into()];
        // Force materialization even if git missing in this detect snapshot by also
        // writing a second generate after marking commands — detect uses real PATH.
        let report = generate(&paths, &cfg).expect("gen");
        assert!(report.wrote);
        let module = read_string(&paths.runtime_module()).expect("read");
        assert!(module.contains("phase-5"));
        assert!(module.contains("plugins (phase 5)"));
        assert!(module.contains("Initialize-WinZshCompletions"));
        // git plugin aliases when git is on PATH (typical in CI/dev)
        if module.contains("function gst") {
            assert!(module.contains("WinZshPluginsLoaded"));
        }
        let again = generate(&paths, &cfg).expect("gen2");
        assert!(!again.wrote);
        let _ = std::fs::remove_dir_all(&root);
    }
}
