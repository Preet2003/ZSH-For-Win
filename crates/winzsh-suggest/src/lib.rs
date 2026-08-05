//! Suggestion sources, syntax-highlight token rules, and PSReadLine codegen.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

/// High-level suggestion / smart-shell policy (executed by the PS runtime).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestPolicy {
    /// History-based inline autosuggestions (PSReadLine Prediction).
    pub autosuggestions: bool,
    /// Token syntax colors via PSReadLine.
    pub syntax: bool,
    /// Wire Ctrl+R fuzzy history when `fzf` is on PATH.
    pub fzf: bool,
    /// Initialize `zoxide` when present.
    pub zoxide: bool,
    /// Active theme id (drives syntax / prediction tint).
    pub theme_id: String,
}

impl Default for SuggestPolicy {
    fn default() -> Self {
        Self {
            autosuggestions: true,
            syntax: true,
            fzf: true,
            zoxide: true,
            theme_id: "modern".into(),
        }
    }
}

/// PSReadLine token colors (ConsoleColor names or ANSI escape strings).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntaxColors {
    /// Commands.
    pub command: String,
    /// Parameters.
    pub parameter: String,
    /// Strings.
    pub string: String,
    /// Operators.
    pub operator: String,
    /// Variables.
    pub variable: String,
    /// Numbers.
    pub number: String,
    /// Types.
    pub type_name: String,
    /// Comments.
    pub comment: String,
    /// Keywords.
    pub keyword: String,
    /// Errors.
    pub error: String,
    /// Inline prediction (ghost text) — critical for visibility + accept UX.
    pub inline_prediction: String,
}

/// Resolve syntax colors for a theme id.
pub fn syntax_colors_for_theme(theme_id: &str) -> SyntaxColors {
    match theme_id {
        "minimal" => SyntaxColors {
            command: "White".into(),
            parameter: "Gray".into(),
            string: "DarkYellow".into(),
            operator: "DarkGray".into(),
            variable: "Gray".into(),
            number: "White".into(),
            type_name: "White".into(),
            comment: "DarkGray".into(),
            keyword: "White".into(),
            error: "Red".into(),
            inline_prediction: "`e[38;5;244m".into(),
        },
        "classic" => SyntaxColors {
            command: "Yellow".into(),
            parameter: "DarkCyan".into(),
            string: "DarkYellow".into(),
            operator: "DarkGray".into(),
            variable: "Green".into(),
            number: "White".into(),
            type_name: "Cyan".into(),
            comment: "DarkGreen".into(),
            keyword: "Blue".into(),
            error: "Red".into(),
            inline_prediction: "`e[90m".into(),
        },
        "catppuccin" => SyntaxColors {
            command: "`e[38;2;137;180;250m".into(),
            parameter: "`e[38;2;148;226;213m".into(),
            string: "`e[38;2;249;226;175m".into(),
            operator: "`e[38;2;108;112;134m".into(),
            variable: "`e[38;2;166;227;161m".into(),
            number: "`e[38;2;250;179;135m".into(),
            type_name: "`e[38;2;137;220;235m".into(),
            comment: "`e[38;2;108;112;134m".into(),
            keyword: "`e[38;2;203;166;247m".into(),
            error: "`e[38;2;243;139;168m".into(),
            inline_prediction: "`e[38;2;88;91;112m".into(),
        },
        "tokyo-night" => SyntaxColors {
            command: "`e[38;2;122;162;247m".into(),
            parameter: "`e[38;2;125;207;255m".into(),
            string: "`e[38;2;224;175;104m".into(),
            operator: "`e[38;2;86;95;137m".into(),
            variable: "`e[38;2;158;206;106m".into(),
            number: "`e[38;2;255;158;100m".into(),
            type_name: "`e[38;2;42;195;222m".into(),
            comment: "`e[38;2;86;95;137m".into(),
            keyword: "`e[38;2;187;154;247m".into(),
            error: "`e[38;2;247;118;142m".into(),
            inline_prediction: "`e[38;2;65;72;104m".into(),
        },
        // modern / powerline / default
        _ => SyntaxColors {
            command: "Cyan".into(),
            parameter: "DarkCyan".into(),
            string: "Yellow".into(),
            operator: "DarkGray".into(),
            variable: "Green".into(),
            number: "White".into(),
            type_name: "Blue".into(),
            comment: "DarkGreen".into(),
            keyword: "Magenta".into(),
            error: "Red".into(),
            inline_prediction: "`e[90m".into(),
        },
    }
}

/// Render PowerShell that configures PSReadLine + optional fzf/zoxide.
pub fn render_powershell(policy: &SuggestPolicy) -> String {
    let colors = syntax_colors_for_theme(&policy.theme_id);
    let mut out = String::from("\n# --- smart shell / PSReadLine (phase 3) ---\n");
    out.push_str(&format!(
        "$script:WinZshAutosuggestions = {}\n$script:WinZshSyntax = {}\n$script:WinZshFzf = {}\n$script:WinZshZoxide = {}\n",
        ps_bool(policy.autosuggestions),
        ps_bool(policy.syntax),
        ps_bool(policy.fzf),
        ps_bool(policy.zoxide),
    ));

    out.push_str(
        r#"
function Initialize-WinZshSmartShell {
    [CmdletBinding()]
    param()
    if (-not (Get-Module -ListAvailable -Name PSReadLine)) {
        Write-Warning 'WinZSH: PSReadLine not found; autosuggestions/syntax unavailable.'
        return
    }
    Import-Module PSReadLine -ErrorAction SilentlyContinue

"#,
    );

    if policy.autosuggestions {
        out.push_str(
            r#"
    try {
        Set-PSReadLineOption -PredictionSource History -ErrorAction Stop
        Set-PSReadLineOption -PredictionViewStyle InlineView -ErrorAction SilentlyContinue
    } catch {
        Write-Verbose "WinZSH: PredictionSource unsupported: $_"
    }

    # RightArrow accepts the full inline suggestion when the cursor is at end-of-line.
    Set-PSReadLineKeyHandler -Key RightArrow -BriefDescription 'WinZSH AcceptSuggestion' -ScriptBlock {
        param($key, $arg)
        $line = $null
        $cursor = $null
        [Microsoft.PowerShell.PSConsoleReadLine]::GetBufferState([ref]$line, [ref]$cursor)
        if ($null -ne $line -and $cursor -eq $line.Length) {
            [Microsoft.PowerShell.PSConsoleReadLine]::AcceptSuggestion($key, $arg)
        } else {
            [Microsoft.PowerShell.PSConsoleReadLine]::ForwardChar($key, $arg)
        }
    }
    Set-PSReadLineKeyHandler -Key Ctrl+f -Function AcceptSuggestion -ErrorAction SilentlyContinue
    Set-PSReadLineKeyHandler -Key Ctrl+RightArrow -Function ForwardWord -ErrorAction SilentlyContinue
"#,
        );
    } else {
        out.push_str(
            r#"
    try { Set-PSReadLineOption -PredictionSource None -ErrorAction SilentlyContinue } catch { }
"#,
        );
    }

    if policy.syntax {
        out.push_str(&format!(
            r#"
    try {{
        Set-PSReadLineOption -Colors @{{
            Command            = {command}
            Parameter          = {parameter}
            String             = {string}
            Operator           = {operator}
            Variable           = {variable}
            Number             = {number}
            Type               = {type_name}
            Comment            = {comment}
            Keyword            = {keyword}
            Error              = {error}
            InlinePrediction   = {inline_prediction}
        }} -ErrorAction Stop
    }} catch {{
        Write-Verbose "WinZSH: syntax colors unsupported: $_"
    }}
"#,
            command = ps_color_literal(&colors.command),
            parameter = ps_color_literal(&colors.parameter),
            string = ps_color_literal(&colors.string),
            operator = ps_color_literal(&colors.operator),
            variable = ps_color_literal(&colors.variable),
            number = ps_color_literal(&colors.number),
            type_name = ps_color_literal(&colors.type_name),
            comment = ps_color_literal(&colors.comment),
            keyword = ps_color_literal(&colors.keyword),
            error = ps_color_literal(&colors.error),
            inline_prediction = ps_color_literal(&colors.inline_prediction),
        ));
    }

    if policy.fzf {
        out.push_str(
            r#"
    if ($script:WinZshFzf -and (Get-Command fzf -ErrorAction SilentlyContinue)) {
        Set-PSReadLineKeyHandler -Key Ctrl+r -BriefDescription 'WinZSH fzf history' -ScriptBlock {
            $histPath = (Get-PSReadLineOption).HistorySavePath
            $lines = @()
            if ($histPath -and (Test-Path -LiteralPath $histPath)) {
                $lines = Get-Content -LiteralPath $histPath -ErrorAction SilentlyContinue
            }
            if (-not $lines -or $lines.Count -eq 0) {
                $lines = @(Get-History | ForEach-Object { $_.CommandLine })
            }
            if (-not $lines -or $lines.Count -eq 0) { return }
            $selection = $lines | fzf --tac --no-sort --height=40% --reverse
            if ($selection) {
                [Microsoft.PowerShell.PSConsoleReadLine]::RevertLine()
                [Microsoft.PowerShell.PSConsoleReadLine]::Insert($selection)
            }
        }
    }
"#,
        );
    }

    if policy.zoxide {
        out.push_str(
            r#"
    if ($script:WinZshZoxide -and (Get-Command zoxide -ErrorAction SilentlyContinue)) {
        try {
            Invoke-Expression (& { (zoxide init powershell | Out-String) })
        } catch {
            Write-Verbose "WinZSH: zoxide init failed: $_"
        }
    }
"#,
        );
    }

    out.push_str(
        r#"
}

Initialize-WinZshSmartShell
"#,
    );
    out
}

fn ps_bool(v: bool) -> &'static str {
    if v { "$true" } else { "$false" }
}

/// Emit a PowerShell string literal for a PSReadLine color value.
///
/// ConsoleColor names use single quotes; VT sequences use double quotes so `` `e `` expands.
fn ps_color_literal(value: &str) -> String {
    if value.contains('[') || value.contains('`') {
        format!("\"{}\"", value.replace('"', "`\""))
    } else {
        format!("'{}'", value.replace('\'', "''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_includes_accept_handler() {
        let ps = render_powershell(&SuggestPolicy::default());
        assert!(ps.contains("AcceptSuggestion"));
        assert!(ps.contains("RightArrow"));
        assert!(ps.contains("InlinePrediction"));
        assert!(ps.contains("Initialize-WinZshSmartShell"));
    }

    #[test]
    fn disabled_autosuggest_sets_none() {
        let policy = SuggestPolicy {
            autosuggestions: false,
            ..SuggestPolicy::default()
        };
        let ps = render_powershell(&policy);
        assert!(ps.contains("PredictionSource None"));
    }
}
