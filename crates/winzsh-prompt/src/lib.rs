//! Prompt segment contracts and timing budgets (Rust side / codegen inputs).

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use winzsh_theme::Theme;

/// Identifier for a prompt segment contributed to runtime-gen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SegmentKind {
    /// Current working directory (home-shortened).
    Path,
    /// Git branch + dirty marker.
    Git,
    /// Prompt character.
    PromptChar,
}

/// Identifier for a prompt segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentId(pub String);

/// Prompt plan used by runtime-gen.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptPlan {
    /// Ordered segments to render.
    pub segments: Vec<SegmentKind>,
    /// Soft latency budget for the whole prompt (milliseconds).
    pub budget_ms: u64,
    /// Whether git segment is enabled.
    pub git_enabled: bool,
}

impl Default for PromptPlan {
    fn default() -> Self {
        Self {
            segments: vec![SegmentKind::Path, SegmentKind::Git, SegmentKind::PromptChar],
            budget_ms: 20,
            git_enabled: true,
        }
    }
}

/// Build a prompt plan from config-like flags.
pub fn plan_from_flags(show_git: bool) -> PromptPlan {
    let mut segments = vec![SegmentKind::Path];
    if show_git {
        segments.push(SegmentKind::Git);
    }
    segments.push(SegmentKind::PromptChar);
    PromptPlan {
        segments,
        budget_ms: 20,
        git_enabled: show_git,
    }
}

/// Escape a string for inclusion inside single-quoted PowerShell literals.
pub fn ps_single_quote(value: &str) -> String {
    value.replace('\'', "''")
}

/// Render PowerShell prompt helpers for a theme + plan.
pub fn render_powershell(theme: &Theme, plan: &PromptPlan) -> String {
    let path_color = &theme.palette.path;
    let git_clean = &theme.palette.git_clean;
    let git_dirty = &theme.palette.git_dirty;
    let prompt_color = &theme.palette.prompt;
    let reset = &theme.palette.reset;
    let prompt_sym = ps_single_quote(&theme.symbols.prompt);
    let dirty_sym = ps_single_quote(&theme.symbols.git_dirty);
    let clean_sym = ps_single_quote(&theme.symbols.git_clean);
    let git_enabled = if plan.git_enabled { "$true" } else { "$false" };
    let budget = plan.budget_ms;

    format!(
        r#"
# --- prompt (phase 2) ---
$script:WinZshGitEnabled = {git_enabled}
$script:WinZshPromptBudgetMs = {budget}

function Get-WinZshPathSegment {{
    [CmdletBinding()]
    param()
    $cwd = (Get-Location).Path
    $homePath = $HOME
    if ($cwd -like ($homePath + '*')) {{
        $cwd = '~' + $cwd.Substring($homePath.Length)
    }}
    return "{path_color}$cwd{reset}"
}}

function Get-WinZshGitSegment {{
    [CmdletBinding()]
    param()
    if (-not $script:WinZshGitEnabled) {{ return '' }}
    if (-not (Get-Command git -ErrorAction SilentlyContinue)) {{ return '' }}
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    try {{
        $branch = & git rev-parse --abbrev-ref HEAD 2>$null
        if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($branch)) {{ return '' }}
        $porcelain = & git status --porcelain 2>$null
        $isDirty = -not [string]::IsNullOrWhiteSpace(($porcelain | Out-String))
        if ($isDirty) {{
            return " {git_dirty}$branch{dirty_sym}{reset}"
        }}
        if ('{clean_sym}') {{
            return " {git_clean}$branch{clean_sym}{reset}"
        }}
        return " {git_clean}$branch{reset}"
    }} catch {{
        return ''
    }} finally {{
        $sw.Stop()
        if ($sw.ElapsedMilliseconds -gt $script:WinZshPromptBudgetMs) {{
            Write-Verbose "WinZSH git segment took $($sw.ElapsedMilliseconds)ms"
        }}
    }}
}}

function prompt {{
    $pathSeg = Get-WinZshPathSegment
    $gitSeg = Get-WinZshGitSegment
    if (Get-Command Write-WinZshHistoryFromPrompt -ErrorAction SilentlyContinue) {{
        Write-WinZshHistoryFromPrompt
    }}
    # Teach zoxide about the current directory (zoxide cannot wrap our module-exported prompt).
    # Use Get-Variable — StrictMode throws on reading an unset $global:WinZshZoxidePath.
    try {{
        $zoxideExe = Get-Variable -Name WinZshZoxidePath -Scope Global -ValueOnly -ErrorAction SilentlyContinue
        if (-not $zoxideExe) {{
            $zoxideCmd = Get-Command zoxide -ErrorAction SilentlyContinue
            if ($zoxideCmd) {{ $zoxideExe = $zoxideCmd.Source }}
        }}
        if ($zoxideExe) {{
            $cwd = (Get-Location).ProviderPath
            if ($cwd) {{ $null = & $zoxideExe add -- $cwd }}
        }}
    }} catch {{ }}
    return "$pathSeg$gitSeg`n{prompt_color}{prompt_sym}{reset} "
}}
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use winzsh_theme::resolve;

    #[test]
    fn renders_prompt_fn() {
        let theme = resolve("modern").expect("theme").theme;
        let ps = render_powershell(&theme, &PromptPlan::default());
        assert!(ps.contains("function prompt"));
        assert!(ps.contains("Get-WinZshGitSegment"));
        assert!(ps.contains("add --"));
        assert!(ps.contains("WinZshZoxidePath"));
        assert!(ps.contains("Get-Variable"));
    }
}
