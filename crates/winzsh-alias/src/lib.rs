//! Alias model and deterministic conflict resolution.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use winzsh_error::{Result, message};

/// Where an alias originated (for conflict policy / diagnostics).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AliasSource {
    /// Built-in WinZSH defaults.
    Builtin,
    /// First-party / community plugin.
    Plugin,
    /// User `config.toml` aliases (highest precedence).
    User,
}

/// A single shell alias.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Alias {
    /// Alias name.
    pub name: String,
    /// Expansion body (command + args).
    pub value: String,
    /// Origin of this alias.
    pub source: AliasSource,
}

/// Merged alias set ready for codegen.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AliasSet {
    /// Final aliases keyed by name.
    pub aliases: BTreeMap<String, Alias>,
    /// Conflicts where a higher-precedence source overrode another.
    pub conflicts: Vec<String>,
}

/// Built-in convenience aliases (Oh My Zsh–style starters).
pub fn builtin_aliases() -> Vec<Alias> {
    [
        ("gs", "git status"),
        ("ga", "git add"),
        ("gc", "git commit"),
        ("gp", "git push"),
        ("gl", "git log --oneline --graph -20"),
        ("gd", "git diff"),
        ("gco", "git checkout"),
        ("gb", "git branch"),
        ("dcu", "docker compose up"),
        ("dcd", "docker compose down"),
        ("ll", "Get-ChildItem -Force"),
    ]
    .into_iter()
    .map(|(name, value)| Alias {
        name: name.to_string(),
        value: value.to_string(),
        source: AliasSource::Builtin,
    })
    .collect()
}

/// Merge builtins, plugin aliases, then user aliases (user wins).
pub fn merge(
    builtins: impl IntoIterator<Item = Alias>,
    plugins: impl IntoIterator<Item = Alias>,
    user: impl IntoIterator<Item = Alias>,
) -> AliasSet {
    let mut set = AliasSet::default();
    for alias in builtins {
        insert(&mut set, alias);
    }
    for alias in plugins {
        insert(&mut set, alias);
    }
    for alias in user {
        insert(&mut set, alias);
    }
    set
}

fn insert(set: &mut AliasSet, alias: Alias) {
    if let Some(prev) = set.aliases.get(&alias.name)
        && prev.source != alias.source
    {
        set.conflicts.push(format!(
            "{}: {:?} overrides {:?}",
            alias.name, alias.source, prev.source
        ));
    }
    set.aliases.insert(alias.name.clone(), alias);
}

/// Build user aliases from a name→value map.
pub fn from_user_map(map: &BTreeMap<String, String>) -> Result<Vec<Alias>> {
    aliases_from_map(map, AliasSource::User)
}

/// Build plugin aliases from a name→value map (already validated by plugin manifests).
pub fn from_plugin_map(map: &BTreeMap<String, String>) -> Result<Vec<Alias>> {
    aliases_from_map(map, AliasSource::Plugin)
}

fn aliases_from_map(map: &BTreeMap<String, String>, source: AliasSource) -> Result<Vec<Alias>> {
    let mut out = Vec::new();
    for (name, value) in map {
        let name = name.trim();
        let value = value.trim();
        if name.is_empty() {
            return Err(message("alias name must not be empty"));
        }
        if value.is_empty() {
            return Err(message(format!("alias '{name}' value must not be empty")));
        }
        if !is_valid_alias_name(name) {
            return Err(message(format!(
                "alias '{name}' has an invalid name (use letters, digits, _, -)"
            )));
        }
        out.push(Alias {
            name: name.to_string(),
            value: value.to_string(),
            source,
        });
    }
    Ok(out)
}

fn is_valid_alias_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Render PowerShell alias functions plus session-alias helpers.
pub fn render_powershell(set: &AliasSet) -> String {
    let mut out = String::from("\n# --- aliases (phase 2) ---\n");
    out.push_str(
        r#"
# Temporary alias for this tab only — like Set-Alias, but allows arguments:
#   salias myalias git status
#   salias ll Get-ChildItem -Force
function salias {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory, Position = 0)]
        [ValidatePattern('^[A-Za-z_][A-Za-z0-9_-]*$')]
        [string]$Name,
        [Parameter(Mandatory, Position = 1, ValueFromRemainingArguments = $true)]
        [string[]]$Command
    )
    $expansion = ($Command -join ' ').Trim()
    if ([string]::IsNullOrWhiteSpace($expansion)) {
        throw 'salias: command must not be empty'
    }
    $safe = $expansion.Replace("'", "''")
    Invoke-Expression "function global:$Name { $safe @args }"
    Write-Host "Session alias: $Name -> $expansion  (this tab only)"
}
"#,
    );
    for alias in set.aliases.values() {
        let value = alias.value.replace(['\r', '\n'], " ");
        out.push_str(&format!("function {} {{ {} @args }}\n", alias.name, value));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_overrides_builtin() {
        let user = vec![Alias {
            name: "gs".into(),
            value: "git status -sb".into(),
            source: AliasSource::User,
        }];
        let set = merge(builtin_aliases(), [], user);
        assert_eq!(set.aliases["gs"].value, "git status -sb");
        assert!(!set.conflicts.is_empty());
    }

    #[test]
    fn render_includes_salias() {
        let ps = render_powershell(&AliasSet::default());
        assert!(ps.contains("function salias"));
        assert!(ps.contains("ValueFromRemainingArguments"));
    }
}
