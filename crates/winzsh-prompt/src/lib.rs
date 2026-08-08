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
            budget_ms: 50,
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
        budget_ms: 50,
        git_enabled: show_git,
    }
}

/// Escape a string for inclusion inside single-quoted PowerShell literals.
pub fn ps_single_quote(value: &str) -> String {
    value.replace('\'', "''")
}

/// Map a `$PSStyle.Foreground.X` palette expression to a `Write-Host -ForegroundColor` name.
fn console_color_from_palette(expr: &str) -> &'static str {
    let upper = expr.to_ascii_uppercase();
    for (name, color) in [
        ("BRIGHTBLUE", "Blue"),
        ("BRIGHTGREEN", "Green"),
        ("BRIGHTYELLOW", "Yellow"),
        ("BRIGHTMAGENTA", "Magenta"),
        ("BRIGHTCYAN", "Cyan"),
        ("BRIGHTRED", "Red"),
        ("BRIGHTWHITE", "White"),
        ("CYAN", "Cyan"),
        ("GREEN", "Green"),
        ("YELLOW", "Yellow"),
        ("MAGENTA", "Magenta"),
        ("BLUE", "Blue"),
        ("RED", "Red"),
        ("WHITE", "White"),
        ("GRAY", "Gray"),
        ("DARKGRAY", "DarkGray"),
    ] {
        if upper.contains(name) {
            return color;
        }
    }
    "Cyan"
}

/// Render PowerShell prompt helpers for a theme + plan.
pub fn render_powershell(theme: &Theme, plan: &PromptPlan) -> String {
    let path_color = console_color_from_palette(&theme.palette.path);
    let git_clean = console_color_from_palette(&theme.palette.git_clean);
    let git_dirty = console_color_from_palette(&theme.palette.git_dirty);
    let prompt_color = console_color_from_palette(&theme.palette.prompt);
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
$script:WinZshPathColor = '{path_color}'
$script:WinZshGitCleanColor = '{git_clean}'
$script:WinZshGitDirtyColor = '{git_dirty}'
$script:WinZshPromptColor = '{prompt_color}'
$script:WinZshPromptSymbol = '{prompt_sym}'

function Get-WinZshPathSegment {{
    [CmdletBinding()]
    param()
    $cwd = (Get-Location).Path
    $homePath = $HOME
    if ($homePath -and ($cwd -eq $homePath -or $cwd.StartsWith($homePath + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase) -or $cwd.StartsWith($homePath + [IO.Path]::AltDirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase))) {{
        $cwd = '~' + $cwd.Substring($homePath.Length)
    }}
    return $cwd
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
            return "$branch{dirty_sym}"
        }}
        if ('{clean_sym}') {{
            return "$branch{clean_sym}"
        }}
        return "$branch"
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
    # Global activation: another terminal's `exit` clears locks/shell.active.
    if ($global:__WinZshActiveLock -and -not (Test-Path -LiteralPath $global:__WinZshActiveLock)) {{
        Write-Host 'WinZSH deactivated in another terminal — returning to stock PowerShell.' -ForegroundColor Yellow
        exit 0
    }}
    $pathSeg = Get-WinZshPathSegment
    $gitSeg = Get-WinZshGitSegment
    if (Get-Command Write-WinZshHistoryFromPrompt -ErrorAction SilentlyContinue) {{
        Write-WinZshHistoryFromPrompt
    }}
    # Teach zoxide about the current directory (zoxide cannot wrap our module-exported prompt).
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

    # Paint with Write-Host so Cursor/Windows Terminal always show colors (PSStyle-in-return is flaky).
    Write-Host -NoNewline -ForegroundColor $script:WinZshPathColor $pathSeg
    if ($gitSeg) {{
        Write-Host -NoNewline -ForegroundColor DarkGray ' on '
        $gitColor = if ($gitSeg.EndsWith('*') -or $gitSeg.EndsWith('{dirty_sym}')) {{ $script:WinZshGitDirtyColor }} else {{ $script:WinZshGitCleanColor }}
        Write-Host -NoNewline -ForegroundColor $gitColor $gitSeg
    }}
    Write-Host ''
    Write-Host -NoNewline -ForegroundColor $script:WinZshPromptColor $script:WinZshPromptSymbol
    return ' '
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
        assert!(ps.contains("__WinZshActiveLock"));
        assert!(ps.contains("Get-WinZshPathSegment"));
        assert!(ps.contains("Get-WinZshGitSegment"));
        assert!(ps.contains("Write-Host -NoNewline"));
        assert!(ps.contains(" on "));
        assert!(ps.contains("add --"));
        assert!(ps.contains("WinZshZoxidePath"));
        assert!(ps.contains("Get-Variable"));
    }
}
